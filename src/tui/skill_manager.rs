use std::collections::{BTreeMap, BTreeSet};
use std::io::IsTerminal;
use std::path::PathBuf;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread;
use std::time::Duration;

use crossterm::cursor::{Hide, Show};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap};
use ratatui::{Frame, Terminal};
use sentra_lib::interfaces::RiskSeverity;
use sentra_lib::risks::{RiskAsset, RiskScanner, ScanReport};
use sentra_lib::{SentraError, SentraResult};

use crate::cli::args::ScanChecker;
use crate::cli::i18n::t;
use crate::core::scan_support::{
    RuleLoadOutput, build_scan_options_with_cache, checker_selection, load_scanner_rules,
};
use crate::core::skill_inventory::{
    AgentSkillInventory, SkillInventoryRow, collect_skill_inventories, delete_skill_from_agent,
    grouped_skill_rows, install_skill_to_agent,
};
use crate::tui::theme;

const DEFAULT_FOOTER: &str =
    "Tab focus  / search  Space one  a group  Ctrl+A all  r invert  s scan";
const HELP_FOOTER_PRIMARY: &str = "Space one  a group  Ctrl+A all  r invert  s scan  Ctrl+S rescan";
const HELP_FOOTER_SECONDARY: &str = "i install  d delete  Tab focus  / search  q/Esc quit";
const MUTATION_SPINNER_TICK: Duration = Duration::from_millis(120);

pub(crate) async fn run() -> SentraResult<()> {
    if !std::io::stdin().is_terminal() || !std::io::stdout().is_terminal() {
        return Err(SentraError::Message(
            t(
                "sentra skill list requires an interactive terminal",
                "sentra skill list 需要交互式终端",
            )
            .to_string(),
        ));
    }
    let home = home::home_dir().ok_or_else(|| {
        SentraError::Message(
            t(
                "could not determine current user home",
                "无法确定当前用户主目录",
            )
            .to_string(),
        )
    })?;
    let inventories = collect_skill_inventories(&home).await?;
    let mut app = SkillManagerApp::new(home, inventories);
    app.run().await
}

struct SkillManagerApp {
    home: PathBuf,
    inventories: Vec<AgentSkillInventory>,
    agent_focus: usize,
    skill_focus: usize,
    selected: BTreeSet<usize>,
    focus: FocusPane,
    agent_search: String,
    skill_search: String,
    search_mode: bool,
    show_help: bool,
    detail_scroll: u16,
    skill_scroll: usize,
    status: String,
    reports: BTreeMap<String, ScanReport>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FocusPane {
    Agents,
    Skills,
    Details,
}

enum AppAction {
    Continue,
    Quit,
}

enum SkillListEntry {
    Header(bool),
    Row { index: usize, display_number: usize },
}

impl SkillManagerApp {
    fn new(home: PathBuf, inventories: Vec<AgentSkillInventory>) -> Self {
        Self {
            home,
            inventories,
            agent_focus: 0,
            skill_focus: 0,
            selected: BTreeSet::new(),
            focus: FocusPane::Skills,
            agent_search: String::new(),
            skill_search: String::new(),
            search_mode: false,
            show_help: false,
            detail_scroll: 0,
            skill_scroll: 0,
            status: t("Ready", "就绪").to_string(),
            reports: BTreeMap::new(),
        }
    }

    async fn run(&mut self) -> SentraResult<()> {
        let mut terminal = TerminalGuard::enter()?;
        loop {
            terminal.draw(|frame| self.render(frame))?;
            let mut redraw = |app: &mut SkillManagerApp| terminal.draw(|frame| app.render(frame));
            if matches!(
                self.handle_key(read_key()?, &mut redraw).await?,
                AppAction::Quit
            ) {
                return Ok(());
            }
        }
    }

    fn render(&mut self, frame: &mut Frame<'_>) {
        let area = frame.area();
        frame.render_widget(Clear, area);
        if area.width < 80 || area.height < 24 {
            frame.render_widget(
                Paragraph::new(vec![
                    Line::from("Skill Manager").style(title_style()),
                    Line::from(""),
                    Line::from(
                        t(
                            "Terminal too small. Resize to at least 80x24.",
                            "终端太小，请调整到至少 80x24。",
                        )
                        .to_string(),
                    )
                    .style(muted_style()),
                ])
                .block(Block::default().borders(Borders::ALL)),
                area,
            );
            return;
        }

        let [header, body, footer] = Layout::vertical([
            Constraint::Length(2),
            Constraint::Min(10),
            Constraint::Length(if self.show_help { 2 } else { 1 }),
        ])
        .areas(area);
        self.render_header(frame, header);

        let [agents, skills, details] = Layout::horizontal([
            Constraint::Percentage(22),
            Constraint::Percentage(38),
            Constraint::Percentage(40),
        ])
        .spacing(1)
        .areas(body);
        self.render_agents(frame, agents);
        self.render_skills(frame, skills);
        self.render_details(frame, details);
        self.render_footer(frame, footer);
    }

    fn render_header(&self, frame: &mut Frame<'_>, area: Rect) {
        let context = format!(
            "{}  {}: {}",
            t("Skill Manager", "技能管理器"),
            t("Agent", "Agent"),
            self.current_agent_name().unwrap_or("-")
        );
        let status = if self.search_mode {
            match self.focus {
                FocusPane::Agents => format!("/ {}: {}", t("agents", "Agent"), self.agent_search),
                FocusPane::Skills => format!("/ {}: {}", t("skills", "技能"), self.skill_search),
                FocusPane::Details => t("/ details are scroll-only", "/ 详情仅可滚动").to_string(),
            }
        } else {
            self.status.clone()
        };
        frame.render_widget(
            Paragraph::new(vec![
                Line::from(context).style(title_style()),
                Line::from(status).style(muted_style()),
            ])
            .style(body_style()),
            area,
        );
    }

    fn render_agents(&self, frame: &mut Frame<'_>, area: Rect) {
        let visible = self.visible_agent_indices();
        let items = visible
            .iter()
            .map(|index| {
                let agent = &self.inventories[*index];
                let pointer = if *index == self.agent_focus {
                    "> "
                } else {
                    "  "
                };
                let line = Line::from(vec![
                    Span::raw(pointer),
                    Span::raw(agent.agent_name.as_str()),
                    Span::raw(format!(" ({})", agent.skills.len())),
                ]);
                if *index == self.agent_focus {
                    ListItem::new(line.style(focus_style()))
                } else {
                    ListItem::new(line.style(body_style()))
                }
            })
            .collect::<Vec<_>>();
        frame.render_widget(
            List::new(if items.is_empty() {
                vec![ListItem::new(t("no agents", "无 Agent"))]
            } else {
                items
            })
            .block(panel_block(
                t("Agents", "Agent"),
                self.focus == FocusPane::Agents,
            )),
            area,
        );
    }

    fn render_skills(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let rows = self.current_rows();
        let visible = self.visible_skill_indices(&rows);
        let max_rows = area.height.saturating_sub(2) as usize;
        let number_width = visible
            .last()
            .map(|number| (number + 1).to_string().len())
            .unwrap_or(1);
        let entries = skill_list_entries(&rows, &visible);
        let focus_pos = entries
            .iter()
            .position(|entry| match entry {
                SkillListEntry::Row { index, .. } => *index == self.skill_focus,
                SkillListEntry::Header(_) => false,
            })
            .unwrap_or(0);
        self.update_skill_scroll(focus_pos, entries.len(), max_rows);
        let start = self.skill_scroll;
        let items = entries
            .iter()
            .skip(start)
            .take(max_rows)
            .map(|entry| match entry {
                SkillListEntry::Header(installed) => ListItem::new(
                    Line::from(if *installed {
                        t("installed", "已安装")
                    } else {
                        t("available", "可用")
                    })
                    .style(muted_style()),
                ),
                SkillListEntry::Row {
                    index,
                    display_number,
                } => self.skill_item(*index, *display_number, number_width, &rows[*index]),
            })
            .collect::<Vec<_>>();
        frame.render_widget(
            List::new(if items.is_empty() {
                vec![ListItem::new(t("no skills", "无技能"))]
            } else {
                items
            })
            .block(panel_block(
                t("Skills", "技能"),
                self.focus == FocusPane::Skills,
            )),
            area,
        );
    }

    fn update_skill_scroll(&mut self, focus_pos: usize, entry_count: usize, max_rows: usize) {
        if max_rows == 0 {
            self.skill_scroll = 0;
            return;
        }
        let max_start = entry_count.saturating_sub(max_rows);
        self.skill_scroll = self.skill_scroll.min(max_start);
        if focus_pos < self.skill_scroll {
            self.skill_scroll = focus_pos;
        } else if focus_pos >= self.skill_scroll + max_rows {
            self.skill_scroll = focus_pos + 1 - max_rows;
        }
    }

    fn skill_item<'a>(
        &self,
        index: usize,
        display_number: usize,
        number_width: usize,
        row: &'a SkillInventoryRow,
    ) -> ListItem<'a> {
        let pointer = if index == self.skill_focus {
            "> "
        } else {
            "  "
        };
        let marker = if self.selected.contains(&index) {
            "[x]"
        } else {
            "[ ]"
        };
        let finding_count = self
            .reports
            .get(&row_key(row))
            .map(|report| report.findings.len())
            .unwrap_or(0);
        let has_findings = finding_count > 0;
        let mut spans = vec![
            Span::styled(pointer, focus_pointer_style(index == self.skill_focus)),
            Span::styled(format!("{display_number:>number_width$}. "), muted_style()),
            Span::styled(
                marker,
                marker_style(self.selected.contains(&index), has_findings),
            ),
            Span::raw(" "),
            Span::styled(row.skill.name.as_str(), skill_name_style(row, has_findings)),
        ];
        if has_findings {
            spans.push(Span::styled(
                format!(" ({})", finding_count_label(finding_count)),
                finding_count_style(),
            ));
        }
        let mut line = Line::from(spans);
        if index == self.skill_focus {
            line = line.patch_style(Style::default().add_modifier(Modifier::BOLD));
        } else if row.installed {
            line = line.patch_style(body_style());
        } else if !row.installed {
            line = line.patch_style(muted_style());
        }
        ListItem::new(line)
    }

    fn render_details(&self, frame: &mut Frame<'_>, area: Rect) {
        let rows = self.current_rows();
        let lines = rows
            .get(self.skill_focus)
            .map(|row| self.detail_lines(row))
            .unwrap_or_else(|| {
                vec![Line::from(t("No skill selected", "未选择技能")).style(muted_style())]
            });
        frame.render_widget(
            Paragraph::new(lines)
                .block(panel_block(
                    t("Details", "详情"),
                    self.focus == FocusPane::Details,
                ))
                .style(body_style())
                .scroll((self.detail_scroll, 0))
                .wrap(Wrap { trim: false }),
            area,
        );
    }

    fn detail_lines(&self, row: &SkillInventoryRow) -> Vec<Line<'static>> {
        let mut lines = vec![
            Line::from(row.skill.name.clone()).style(title_style()),
            Line::from(row.skill.description.clone().unwrap_or_default()).style(muted_style()),
            Line::from(""),
            labeled(
                t("Status", "状态"),
                if row.installed {
                    t("installed", "已安装")
                } else {
                    t("available", "可用")
                },
            ),
            labeled(t("Source", "来源"), &row.source_agent),
            labeled(
                t("Version", "版本"),
                row.skill.version.as_deref().unwrap_or(t("unknown", "未知")),
            ),
            labeled(
                t("Author", "作者"),
                row.skill.author.as_deref().unwrap_or(t("unknown", "未知")),
            ),
            labeled(
                t("Path", "路径"),
                &row.skill
                    .home
                    .as_ref()
                    .map(|path| path.display().to_string())
                    .unwrap_or_else(|| t("unknown", "未知").to_string()),
            ),
        ];
        if !row.skill.tags.is_empty() {
            lines.push(labeled(t("Tags", "标签"), &row.skill.tags.join(", ")));
        }
        lines.push(Line::from(""));
        if let Some(report) = self.reports.get(&row_key(row)) {
            lines.push(Line::from(t("Scan", "扫描")).style(title_style()));
            lines.push(labeled_styled(
                t("Findings", "发现"),
                &report.findings.len().to_string(),
                if report.findings.is_empty() {
                    body_style()
                } else {
                    finding_count_style()
                },
            ));
            lines.push(labeled(
                t("Errors", "错误"),
                &report.errors.len().to_string(),
            ));
            let total = report.findings.len();
            for (index, finding) in report.findings.iter().enumerate() {
                lines.push(Line::from(""));
                lines.push(Line::from(vec![
                    Span::styled(format!("# {}/{}", index + 1, total), theme::focus_style()),
                    Span::raw("  "),
                    Span::styled(
                        finding.title.clone(),
                        severity_style(finding.severity).add_modifier(Modifier::BOLD),
                    ),
                ]));
                lines.push(labeled_styled(
                    t("Severity", "严重性"),
                    severity_label(finding.severity),
                    severity_style(finding.severity),
                ));
                lines.push(labeled(
                    t("Category", "类别"),
                    &format!("{:?}", finding.category),
                ));
                lines.push(labeled(t("Checker", "检查器"), &finding.checker));
                lines.push(labeled(
                    t("Location", "位置"),
                    &format!("{}:{}", finding.file, finding.location.line),
                ));
                if let Some(evidence) = &finding.evidence {
                    lines.push(labeled(t("Evidence", "证据"), evidence));
                }
                if !finding.remediation.trim().is_empty() {
                    lines.push(labeled(t("Fix", "修复"), &finding.remediation));
                }
            }
        } else {
            lines.push(Line::from(t("Scan", "扫描")).style(title_style()));
            lines.push(
                Line::from(t(
                    "No scan result. Press s to scan.",
                    "没有扫描结果，按 s 开始扫描。",
                ))
                .style(muted_style()),
            );
        }
        lines
    }

    fn render_footer(&self, frame: &mut Frame<'_>, area: Rect) {
        let lines = if self.show_help {
            vec![
                Line::from(t(
                    HELP_FOOTER_PRIMARY,
                    "Tab 焦点  Space 单个  a 分组全选/清空  s 扫描  Ctrl+S 重新扫描",
                )),
                Line::from(t(
                    HELP_FOOTER_SECONDARY,
                    "i 安装  d 删除  / 搜索  q/Esc 退出",
                )),
            ]
        } else {
            vec![Line::from(t(
                DEFAULT_FOOTER,
                "Tab 焦点  / 搜索  Space 选择  a 分组  s 扫描  i 安装  d 删除  ? 帮助",
            ))]
        };
        frame.render_widget(Clear, area);
        frame.render_widget(Paragraph::new(lines).style(muted_style()), area);
    }

    async fn handle_key<F>(&mut self, key: KeyEvent, redraw: &mut F) -> SentraResult<AppAction>
    where
        F: FnMut(&mut Self) -> SentraResult<()>,
    {
        if self.search_mode {
            return Ok(self.handle_search_key(key));
        }
        match key {
            KeyEvent {
                code: KeyCode::Esc, ..
            }
            | KeyEvent {
                code: KeyCode::Char('q'),
                modifiers: KeyModifiers::NONE,
                ..
            } => Ok(AppAction::Quit),
            KeyEvent {
                code: KeyCode::Char('?'),
                ..
            } => {
                self.show_help = !self.show_help;
                Ok(AppAction::Continue)
            }
            KeyEvent {
                code: KeyCode::Char('/'),
                ..
            } => {
                if self.focus == FocusPane::Details {
                    self.status =
                        "Details are scroll-only; Tab to Agents or Skills to search.".to_string();
                } else {
                    self.search_mode = true;
                }
                Ok(AppAction::Continue)
            }
            KeyEvent {
                code: KeyCode::Tab, ..
            } => {
                self.focus = match self.focus {
                    FocusPane::Agents => FocusPane::Skills,
                    FocusPane::Skills => FocusPane::Details,
                    FocusPane::Details => FocusPane::Agents,
                };
                Ok(AppAction::Continue)
            }
            KeyEvent {
                code: KeyCode::Up, ..
            }
            | KeyEvent {
                code: KeyCode::Char('k'),
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                self.move_focus(-1);
                Ok(AppAction::Continue)
            }
            KeyEvent {
                code: KeyCode::Down,
                ..
            }
            | KeyEvent {
                code: KeyCode::Char('j'),
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                self.move_focus(1);
                Ok(AppAction::Continue)
            }
            KeyEvent {
                code: KeyCode::Char(' '),
                ..
            } => {
                self.toggle_selection();
                Ok(AppAction::Continue)
            }
            KeyEvent {
                code: KeyCode::Char('a'),
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                self.toggle_current_skill_group_selection();
                Ok(AppAction::Continue)
            }
            KeyEvent {
                code: KeyCode::Char('a'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => {
                self.toggle_visible_skill_selection();
                Ok(AppAction::Continue)
            }
            KeyEvent {
                code: KeyCode::Char('r'),
                modifiers,
                ..
            } if modifiers.is_empty() || modifiers == KeyModifiers::CONTROL => {
                self.invert_visible_skill_selection();
                Ok(AppAction::Continue)
            }
            KeyEvent {
                code: KeyCode::Char('s'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => {
                self.scan_selected(true).await?;
                Ok(AppAction::Continue)
            }
            KeyEvent {
                code: KeyCode::Char('s'),
                ..
            } => {
                self.scan_selected(false).await?;
                Ok(AppAction::Continue)
            }
            KeyEvent {
                code: KeyCode::Char('i'),
                ..
            } => {
                self.install_selected(redraw).await?;
                Ok(AppAction::Continue)
            }
            KeyEvent {
                code: KeyCode::Char('d'),
                ..
            } => {
                self.delete_selected(redraw).await?;
                Ok(AppAction::Continue)
            }
            _ => Ok(AppAction::Continue),
        }
    }

    fn handle_search_key(&mut self, key: KeyEvent) -> AppAction {
        match key {
            KeyEvent {
                code: KeyCode::Esc, ..
            }
            | KeyEvent {
                code: KeyCode::Enter,
                ..
            } => {
                self.search_mode = false;
            }
            KeyEvent {
                code: KeyCode::Backspace,
                ..
            } => match self.focus {
                FocusPane::Agents => {
                    self.agent_search.pop();
                    self.clamp_agent_focus();
                }
                FocusPane::Skills => {
                    self.skill_search.pop();
                    self.clamp_skill_focus();
                }
                FocusPane::Details => {}
            },
            KeyEvent {
                code: KeyCode::Char(ch),
                modifiers,
                ..
            } if !modifiers.contains(KeyModifiers::CONTROL)
                && !modifiers.contains(KeyModifiers::ALT) =>
            {
                match self.focus {
                    FocusPane::Agents => {
                        self.agent_search.push(ch);
                        self.clamp_agent_focus();
                    }
                    FocusPane::Skills => {
                        self.skill_search.push(ch);
                        self.clamp_skill_focus();
                    }
                    FocusPane::Details => {}
                }
            }
            _ => {}
        }
        AppAction::Continue
    }

    fn move_focus(&mut self, delta: isize) {
        match self.focus {
            FocusPane::Agents => {
                let visible = self.visible_agent_indices();
                if let Some(next) = next_index(&visible, self.agent_focus, delta) {
                    self.agent_focus = next;
                    self.skill_focus = 0;
                    self.skill_scroll = 0;
                    self.selected.clear();
                    self.detail_scroll = 0;
                    self.clamp_skill_focus();
                }
            }
            FocusPane::Skills => {
                let rows = self.current_rows();
                let visible = self.visible_skill_indices(&rows);
                if let Some(next) = next_index(&visible, self.skill_focus, delta) {
                    self.skill_focus = next;
                    self.detail_scroll = 0;
                }
            }
            FocusPane::Details => {
                self.detail_scroll = if delta < 0 {
                    self.detail_scroll.saturating_sub(1)
                } else {
                    self.detail_scroll.saturating_add(1)
                };
            }
        }
    }

    fn toggle_selection(&mut self) {
        if self.focus != FocusPane::Skills {
            return;
        }
        if !self.selected.remove(&self.skill_focus) {
            self.selected.insert(self.skill_focus);
        }
    }

    fn toggle_current_skill_group_selection(&mut self) {
        if self.focus != FocusPane::Skills {
            return;
        }
        let rows = self.current_rows();
        let Some(current) = rows.get(self.skill_focus) else {
            self.status = t("No skills available to select.", "没有可选择的技能。").to_string();
            return;
        };
        let installed = current.installed;
        let visible = self.visible_skill_indices(&rows);
        let group = visible
            .into_iter()
            .filter(|index| rows[*index].installed == installed)
            .collect::<Vec<_>>();
        if group.is_empty() {
            self.status = t("No skills available to select.", "没有可选择的技能。").to_string();
            return;
        }
        let all_selected = group.iter().all(|index| self.selected.contains(index));
        if all_selected {
            for index in &group {
                self.selected.remove(index);
            }
        } else {
            for index in &group {
                self.selected.insert(*index);
            }
        }
        let action = if all_selected {
            t("Cleared", "已清空")
        } else {
            t("Selected", "已选择")
        };
        let label = if installed {
            t("installed", "已安装")
        } else {
            t("available", "可用")
        };
        self.status = format!(
            "{action} {} {label} {}",
            group.len(),
            t("skill(s).", "个技能。")
        );
    }

    fn toggle_visible_skill_selection(&mut self) {
        if self.focus != FocusPane::Skills {
            return;
        }
        let rows = self.current_rows();
        let visible = self.visible_skill_indices(&rows);
        if visible.is_empty() {
            self.status = t("No skills available to select.", "没有可选择的技能。").to_string();
            return;
        }
        let all_selected = visible.iter().all(|index| self.selected.contains(index));
        if all_selected {
            for index in &visible {
                self.selected.remove(index);
            }
        } else {
            for index in &visible {
                self.selected.insert(*index);
            }
        }
        let action = if all_selected {
            t("Cleared", "已清空")
        } else {
            t("Selected", "已选择")
        };
        self.status = format!(
            "{action} {} {}",
            visible.len(),
            t("visible skill(s).", "个可见技能。")
        );
    }

    fn invert_visible_skill_selection(&mut self) {
        if self.focus != FocusPane::Skills {
            return;
        }
        let rows = self.current_rows();
        let visible = self.visible_skill_indices(&rows);
        if visible.is_empty() {
            self.status = t("No skills available to select.", "没有可选择的技能。").to_string();
            return;
        }
        let mut selected_count = 0usize;
        for index in &visible {
            if !self.selected.remove(index) {
                self.selected.insert(*index);
                selected_count += 1;
            }
        }
        self.status = format!(
            "{} {} {}",
            t("Inverted", "已反选"),
            visible.len(),
            t("visible skill(s).", "个可见技能。")
        );
        if selected_count == 0 {
            self.status
                .push_str(&format!(" {}", t("None selected.", "当前未选择。")));
        }
    }

    async fn scan_selected(&mut self, no_cache: bool) -> SentraResult<()> {
        let rows = self.target_rows();
        if rows.is_empty() {
            self.status = t("No skills selected to scan.", "未选择要扫描的技能。").to_string();
            return Ok(());
        }
        let enabled = [ScanChecker::Hash, ScanChecker::Yara, ScanChecker::Ti]
            .into_iter()
            .collect::<BTreeSet<_>>();
        let options =
            build_scan_options_with_cache(&self.home, &checker_selection(&enabled), no_cache)?;
        let mut scanner = RiskScanner::new(options)?;
        load_scanner_rules(&mut scanner, skill_manager_rule_load_output())?;
        for row in &rows {
            let report = scanner.scan(RiskAsset::from(&row.skill)).await?;
            self.reports.insert(row_key(row), report);
        }
        self.status = format!(
            "{} {} {}{}.",
            t("Scanned", "已扫描"),
            rows.len(),
            t("skill(s)", "个技能"),
            if no_cache {
                t(" without cache", "，未使用缓存")
            } else {
                ""
            }
        );
        Ok(())
    }

    async fn install_selected<F>(&mut self, redraw: &mut F) -> SentraResult<()>
    where
        F: FnMut(&mut Self) -> SentraResult<()>,
    {
        let agent_name = self.current_agent_name().unwrap_or("").to_string();
        let rows = self
            .target_rows()
            .into_iter()
            .filter(|row| !row.installed)
            .collect::<Vec<_>>();
        if rows.is_empty() {
            self.status = t(
                "Select available skills to install.",
                "请选择可用技能进行安装。",
            )
            .to_string();
            return Ok(());
        }
        let mut changed = 0usize;
        let total = rows.len();
        for (index, row) in rows.iter().enumerate() {
            let (tx, rx) = mpsc::channel();
            let home = self.home.clone();
            let agent_name_for_worker = agent_name.clone();
            let skill = row.skill.clone();
            thread::spawn(move || {
                let result = install_skill_to_agent(&home, &agent_name_for_worker, &skill);
                let _ = tx.send(result);
            });
            let result = self.wait_for_mutation_result(
                rx,
                t("Installing", "正在安装"),
                index,
                total,
                &row.skill.name,
                redraw,
            )?;
            if result.changed {
                changed += 1;
            }
        }
        self.reload().await?;
        self.status = format!(
            "{} {changed} {} {} {agent_name}.",
            t("Installed", "已安装"),
            t("skill(s)", "个技能"),
            t("to", "到")
        );
        Ok(())
    }

    async fn delete_selected<F>(&mut self, redraw: &mut F) -> SentraResult<()>
    where
        F: FnMut(&mut Self) -> SentraResult<()>,
    {
        let agent_name = self.current_agent_name().unwrap_or("").to_string();
        let rows = self
            .target_rows()
            .into_iter()
            .filter(|row| row.installed)
            .collect::<Vec<_>>();
        if rows.is_empty() {
            self.status = t(
                "Select installed skills to delete.",
                "请选择已安装技能进行删除。",
            )
            .to_string();
            return Ok(());
        }
        let mut changed = 0usize;
        let total = rows.len();
        for (index, row) in rows.iter().enumerate() {
            let (tx, rx) = mpsc::channel();
            let home = self.home.clone();
            let agent_name_for_worker = agent_name.clone();
            let skill = row.skill.clone();
            thread::spawn(move || {
                let result = delete_skill_from_agent(&home, &agent_name_for_worker, &skill);
                let _ = tx.send(result);
            });
            let result = self.wait_for_mutation_result(
                rx,
                t("Deleting", "正在删除"),
                index,
                total,
                &row.skill.name,
                redraw,
            )?;
            if result.changed {
                changed += 1;
            }
        }
        self.reload().await?;
        self.status = format!(
            "{} {changed} {} {} {agent_name}.",
            t("Deleted", "已删除"),
            t("skill(s)", "个技能"),
            t("from", "从")
        );
        Ok(())
    }

    fn wait_for_mutation_result<T, F>(
        &mut self,
        rx: mpsc::Receiver<SentraResult<T>>,
        action: &str,
        index: usize,
        total: usize,
        skill_name: &str,
        redraw: &mut F,
    ) -> SentraResult<T>
    where
        F: FnMut(&mut Self) -> SentraResult<()>,
    {
        let mut frame = 0usize;
        loop {
            self.status =
                mutation_progress_status_with_frame(action, index, total, skill_name, frame);
            redraw(self)?;
            match rx.recv_timeout(MUTATION_SPINNER_TICK) {
                Ok(result) => return result,
                Err(RecvTimeoutError::Timeout) => {
                    frame += 1;
                }
                Err(RecvTimeoutError::Disconnected) => {
                    return Err(SentraError::Message(format!(
                        "{action} worker stopped before returning a result"
                    )));
                }
            }
        }
    }

    async fn reload(&mut self) -> SentraResult<()> {
        self.inventories = collect_skill_inventories(&self.home).await?;
        self.clamp_agent_focus();
        self.clamp_skill_focus();
        self.selected.clear();
        Ok(())
    }

    fn target_rows(&self) -> Vec<SkillInventoryRow> {
        let rows = self.current_rows();
        if self.selected.is_empty() {
            return rows.get(self.skill_focus).cloned().into_iter().collect();
        }
        self.selected
            .iter()
            .filter_map(|index| rows.get(*index).cloned())
            .collect()
    }

    fn current_rows(&self) -> Vec<SkillInventoryRow> {
        grouped_skill_rows(&self.inventories, self.agent_focus)
    }

    fn visible_agent_indices(&self) -> Vec<usize> {
        let query = self.agent_search.to_ascii_lowercase();
        self.inventories
            .iter()
            .enumerate()
            .filter(|(_, agent)| {
                query.is_empty()
                    || agent.agent_name.to_ascii_lowercase().contains(&query)
                    || agent.agent_title.to_ascii_lowercase().contains(&query)
            })
            .map(|(index, _)| index)
            .collect()
    }

    fn visible_skill_indices(&self, rows: &[SkillInventoryRow]) -> Vec<usize> {
        let query = self.skill_search.to_ascii_lowercase();
        rows.iter()
            .enumerate()
            .filter(|(_, row)| {
                query.is_empty()
                    || row.skill.name.to_ascii_lowercase().contains(&query)
                    || row
                        .skill
                        .description
                        .as_deref()
                        .unwrap_or_default()
                        .to_ascii_lowercase()
                        .contains(&query)
            })
            .map(|(index, _)| index)
            .collect()
    }

    fn clamp_agent_focus(&mut self) {
        let visible = self.visible_agent_indices();
        if !visible.contains(&self.agent_focus)
            && let Some(first) = visible.first()
        {
            self.agent_focus = *first;
        }
    }

    fn clamp_skill_focus(&mut self) {
        let rows = self.current_rows();
        let visible = self.visible_skill_indices(&rows);
        if !visible.contains(&self.skill_focus) {
            self.skill_focus = visible.first().copied().unwrap_or(0);
            self.skill_scroll = 0;
        }
    }

    fn current_agent_name(&self) -> Option<&str> {
        self.inventories
            .get(self.agent_focus)
            .map(|agent| agent.agent_name.as_str())
    }
}

fn skill_manager_rule_load_output() -> RuleLoadOutput {
    RuleLoadOutput::Silent
}

fn mutation_progress_status_with_frame(
    action: &str,
    index: usize,
    total: usize,
    skill_name: &str,
    frame_index: usize,
) -> String {
    const FRAMES: [&str; 4] = ["|", "/", "-", "\\"];
    let frame = FRAMES[frame_index % FRAMES.len()];
    format!("{frame} {action} {}/{} {skill_name}", index + 1, total)
}

fn skill_list_entries(rows: &[SkillInventoryRow], visible: &[usize]) -> Vec<SkillListEntry> {
    let mut entries = Vec::new();
    let mut last_group: Option<bool> = None;
    for (number, index) in visible.iter().copied().enumerate() {
        let row = &rows[index];
        if last_group != Some(row.installed) {
            last_group = Some(row.installed);
            entries.push(SkillListEntry::Header(row.installed));
        }
        entries.push(SkillListEntry::Row {
            index,
            display_number: number + 1,
        });
    }
    entries
}

fn body_style() -> Style {
    theme::body_style()
}

fn muted_style() -> Style {
    theme::muted_style()
}

fn title_style() -> Style {
    theme::title_style()
}

fn focus_style() -> Style {
    theme::focus_style()
}

fn focus_pointer_style(focused: bool) -> Style {
    if focused {
        theme::focus_style()
    } else {
        muted_style()
    }
}

fn marker_style(selected: bool, has_findings: bool) -> Style {
    if has_findings {
        finding_count_style()
    } else if selected {
        theme::success_style()
    } else {
        muted_style()
    }
}

fn skill_name_style(row: &SkillInventoryRow, has_findings: bool) -> Style {
    if has_findings {
        finding_count_style()
    } else if row.installed {
        body_style()
    } else {
        muted_style()
    }
}

fn finding_count_style() -> Style {
    theme::warning_style().add_modifier(Modifier::BOLD)
}

fn finding_count_label(count: usize) -> String {
    if count == 1 {
        "1 finding".to_string()
    } else {
        format!("{count} findings")
    }
}

fn severity_style(severity: RiskSeverity) -> Style {
    theme::severity_style(severity)
}

fn panel_block(title: &'static str, focused: bool) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .title(Line::from(title).style(title_style()))
        .border_style(theme::border_style(focused))
}

fn labeled(label: &str, value: &str) -> Line<'static> {
    labeled_styled(label, value, body_style())
}

fn labeled_styled(label: &str, value: &str, value_style: Style) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{label:<10}: "), muted_style()),
        Span::styled(value.to_string(), value_style),
    ])
}

fn severity_label(severity: RiskSeverity) -> &'static str {
    match severity {
        RiskSeverity::Critical => "CRITICAL",
        RiskSeverity::High => "HIGH",
        RiskSeverity::Medium => "MEDIUM",
        RiskSeverity::Low => "LOW",
        RiskSeverity::Info => "INFO",
    }
}

fn row_key(row: &SkillInventoryRow) -> String {
    format!(
        "{}:{}:{}",
        row.source_agent,
        row.skill.name,
        row.skill
            .home
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_default()
    )
}

fn next_index(visible: &[usize], current: usize, delta: isize) -> Option<usize> {
    if visible.is_empty() {
        return None;
    }
    let pos = visible
        .iter()
        .position(|index| *index == current)
        .unwrap_or(0);
    let next = if delta < 0 {
        (pos + visible.len() - 1) % visible.len()
    } else {
        (pos + 1) % visible.len()
    };
    visible.get(next).copied()
}

fn read_key() -> SentraResult<KeyEvent> {
    loop {
        if let Event::Key(key) =
            event::read().map_err(|err| SentraError::Message(err.to_string()))?
            && key.kind == KeyEventKind::Press
        {
            return Ok(key);
        }
    }
}

struct TerminalGuard {
    terminal: Terminal<CrosstermBackend<std::io::Stdout>>,
}

impl TerminalGuard {
    fn enter() -> SentraResult<Self> {
        enable_raw_mode().map_err(|err| SentraError::Message(err.to_string()))?;
        let mut stdout = std::io::stdout();
        execute!(stdout, EnterAlternateScreen, Hide)
            .map_err(|err| SentraError::Message(err.to_string()))?;
        let terminal = Terminal::new(CrosstermBackend::new(stdout))?;
        Ok(Self { terminal })
    }

    fn draw<F>(&mut self, render: F) -> SentraResult<()>
    where
        F: FnOnce(&mut Frame<'_>),
    {
        self.terminal.draw(render)?;
        Ok(())
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = execute!(self.terminal.backend_mut(), Show, LeaveAlternateScreen);
        let _ = disable_raw_mode();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use sentra_lib::interfaces::{Finding, RiskCategory};
    use sentra_lib::risks::{ScanMetadata, ScanSummary};

    fn skill(name: &str, home: &str) -> sentra_lib::interfaces::SkillData {
        sentra_lib::interfaces::SkillData {
            name: name.to_string(),
            description: Some(format!("{name} description")),
            home: Some(PathBuf::from(home)),
            ..Default::default()
        }
    }

    fn app_with_report() -> SkillManagerApp {
        let inventories = vec![
            AgentSkillInventory {
                agent_name: "codex".to_string(),
                agent_title: "Codex".to_string(),
                agent_home: PathBuf::from("/home/codex"),
                skills: vec![skill("alpha", "/codex/alpha")],
            },
            AgentSkillInventory {
                agent_name: "sentra".to_string(),
                agent_title: "Sentra".to_string(),
                agent_home: PathBuf::from("/home/sentra"),
                skills: (1..=100)
                    .map(|index| {
                        skill(
                            &format!("skill-{index:03}"),
                            &format!("/sentra/skill-{index:03}"),
                        )
                    })
                    .collect(),
            },
        ];
        let mut app = SkillManagerApp::new(PathBuf::from("/home"), inventories);
        let rows = app.current_rows();
        let mut finding = Finding::new(
            "risk",
            "test-checker",
            RiskSeverity::High,
            RiskCategory::PromptInjection,
            "SKILL.md",
            "Prompt risk",
            "description",
            "remove risky instruction",
        );
        finding.evidence = Some("Ignore previous instructions".to_string());
        app.reports.insert(
            row_key(&rows[0]),
            ScanReport {
                metadata: ScanMetadata {
                    scanner: "skill-scanner".to_string(),
                    scan_time: "2026-07-02T00:00:00Z".to_string(),
                    scan_duration_ms: 1,
                },
                summary: ScanSummary {
                    high: 1,
                    ..Default::default()
                },
                findings: vec![finding],
                errors: Vec::new(),
            },
        );
        app
    }

    fn app_with_many_installed_and_available() -> SkillManagerApp {
        let inventories = vec![
            AgentSkillInventory {
                agent_name: "augment".to_string(),
                agent_title: "Augment".to_string(),
                agent_home: PathBuf::from("/home/augment"),
                skills: (1..=60)
                    .map(|index| {
                        skill(
                            &format!("installed-{index:03}"),
                            &format!("/augment/installed-{index:03}"),
                        )
                    })
                    .collect(),
            },
            AgentSkillInventory {
                agent_name: "cursor".to_string(),
                agent_title: "Cursor".to_string(),
                agent_home: PathBuf::from("/home/cursor"),
                skills: (1..=20)
                    .map(|index| {
                        skill(
                            &format!("available-{index:03}"),
                            &format!("/cursor/available-{index:03}"),
                        )
                    })
                    .collect(),
            },
        ];
        SkillManagerApp::new(PathBuf::from("/home"), inventories)
    }

    #[test]
    fn skill_manager_renders_three_columns_and_risk_details_at_80x24() {
        let mut app = app_with_report();
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal.draw(|frame| app.render(frame)).unwrap();

        let rendered = buffer_text(terminal.backend());
        assert!(rendered.contains("Agents"));
        assert!(rendered.contains("Skills"));
        assert!(rendered.contains("Details"));
        assert!(rendered.contains("installed"));
        assert!(rendered.contains("available"));
        assert!(rendered.contains(" 1. [ ] alpha"));
        assert!(rendered.contains("alpha (1 finding)"), "{rendered}");
        assert!(!rendered.contains("[ ] I "));
        assert!(!rendered.contains("[ ] A "));
        let nine = rendered
            .lines()
            .find(|line| line.contains("skill-008"))
            .unwrap();
        let ten = rendered
            .lines()
            .find(|line| line.contains("skill-009"))
            .unwrap();
        assert_eq!(nine.find("[ ]"), ten.find("[ ]"));
        assert!(rendered.contains("Scan"));
        assert!(rendered.contains("# 1/1  Prompt risk"));
        assert!(rendered.contains("Prompt risk"));
    }

    #[test]
    fn skill_manager_aligns_number_column_to_largest_visible_number() {
        let mut app = app_with_report();
        app.skill_focus = 99;
        let backend = TestBackend::new(80, 120);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal.draw(|frame| app.render(frame)).unwrap();

        let rendered = buffer_text(terminal.backend());
        let ninety_nine = rendered
            .lines()
            .find(|line| line.contains("[ ] skill-098"))
            .unwrap();
        let hundred = rendered
            .lines()
            .find(|line| line.contains("[ ] skill-099"))
            .unwrap();
        assert_eq!(ninety_nine.find("[ ]"), hundred.find("[ ]"));
    }

    #[test]
    fn skill_manager_keeps_focused_skill_visible_after_scroll() {
        let mut app = app_with_report();
        app.skill_focus = 99;
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal.draw(|frame| app.render(frame)).unwrap();

        let rendered = buffer_text(terminal.backend());
        let focused = rendered
            .lines()
            .find(|line| line.contains("> 100. [ ] skill-099"));
        assert!(focused.is_some(), "{rendered}");
    }

    #[test]
    fn skill_manager_cursor_moves_within_view_before_scrolling_up() {
        let mut app = app_with_report();
        app.skill_focus = 80;
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| app.render(frame)).unwrap();

        app.move_focus(-1);
        terminal.draw(|frame| app.render(frame)).unwrap();

        let rendered = buffer_text(terminal.backend());
        let focused_line = rendered
            .lines()
            .position(|line| line.contains(">  80. [ ] skill-079"))
            .unwrap();
        let bottom_border = rendered
            .lines()
            .position(|line| line.contains("└") && line.contains("┘"))
            .unwrap();
        assert!(focused_line + 1 < bottom_border, "{rendered}");
    }

    #[test]
    fn skill_manager_cursor_moves_down_before_scrolling_view() {
        let mut app = app_with_report();
        app.skill_focus = 1;
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| app.render(frame)).unwrap();

        app.move_focus(1);
        terminal.draw(|frame| app.render(frame)).unwrap();

        let rendered = buffer_text(terminal.backend());
        assert!(rendered.contains("installed"));
        assert!(rendered.contains(">   3. [ ] skill-002"), "{rendered}");
    }

    #[test]
    fn skill_manager_keeps_focused_skill_visible_with_status_bar_height() {
        let mut app = app_with_many_installed_and_available();
        app.skill_focus = 72;
        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal.draw(|frame| app.render(frame)).unwrap();

        let rendered = buffer_text(terminal.backend());
        let focused = rendered
            .lines()
            .find(|line| line.contains("> 73. [ ] available-013"));
        assert!(focused.is_some(), "{rendered}");
    }

    #[test]
    fn skill_manager_loads_scan_rules_without_terminal_output() {
        assert_eq!(
            skill_manager_rule_load_output(),
            crate::core::scan_support::RuleLoadOutput::Silent
        );
    }

    #[test]
    fn skill_manager_uses_two_line_top_banner_for_dynamic_status() {
        let mut app = app_with_report();
        app.status = "Loading risk rules 1/3 (33%): loading YARA rules with a very long status that must stay in the banner".to_string();
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal.draw(|frame| app.render(frame)).unwrap();

        let rendered = buffer_text(terminal.backend());
        let lines = rendered.lines().collect::<Vec<_>>();
        assert!(lines[0].contains("Skill Manager"));
        assert!(lines[0].contains("Agent: codex"));
        assert!(lines[1].contains("Loading risk rules 1/3"));
        assert!(lines[2].contains("Agents"));
        assert!(lines[2].contains("Skills"));
        assert!(lines[2].contains("Details"));
    }

    #[test]
    fn skill_manager_shows_default_shortcuts_once() {
        let mut app = app_with_report();
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal.draw(|frame| app.render(frame)).unwrap();

        let rendered = buffer_text(terminal.backend());
        assert_eq!(rendered.matches("Tab focus  / search").count(), 1);
    }

    #[test]
    fn skill_manager_help_footer_uses_two_lines() {
        let mut app = app_with_report();
        app.show_help = true;
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal.draw(|frame| app.render(frame)).unwrap();

        let rendered = buffer_text(terminal.backend());
        let lines = rendered.lines().collect::<Vec<_>>();
        assert!(lines[22].contains(HELP_FOOTER_PRIMARY));
        assert!(lines[23].contains(HELP_FOOTER_SECONDARY));
    }

    #[test]
    fn skill_manager_footer_text_fits_80_columns() {
        assert!(DEFAULT_FOOTER.len() <= 80);
        assert!(HELP_FOOTER_PRIMARY.len() <= 80);
        assert!(HELP_FOOTER_SECONDARY.len() <= 80);
    }

    #[test]
    fn skill_manager_a_toggles_installed_group_selection() {
        let mut app = app_with_many_installed_and_available();
        let mut redraw = |_app: &mut SkillManagerApp| Ok(());

        block_on(app.handle_key(
            KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE),
            &mut redraw,
        ))
        .unwrap();

        assert_eq!(app.selected.len(), 60);
        assert!(app.selected.iter().all(|index| *index < 60));
        assert_eq!(app.status, "Selected 60 installed skill(s).");

        block_on(app.handle_key(
            KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE),
            &mut redraw,
        ))
        .unwrap();

        assert!(app.selected.is_empty());
        assert_eq!(app.status, "Cleared 60 installed skill(s).");
    }

    #[test]
    fn skill_manager_a_toggles_available_group_selection() {
        let mut app = app_with_many_installed_and_available();
        app.skill_focus = 60;
        let mut redraw = |_app: &mut SkillManagerApp| Ok(());

        block_on(app.handle_key(
            KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE),
            &mut redraw,
        ))
        .unwrap();

        assert_eq!(app.selected.len(), 20);
        assert!(app.selected.iter().all(|index| *index >= 60));
        assert_eq!(app.status, "Selected 20 available skill(s).");

        block_on(app.handle_key(
            KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE),
            &mut redraw,
        ))
        .unwrap();

        assert!(app.selected.is_empty());
        assert_eq!(app.status, "Cleared 20 available skill(s).");
    }

    #[test]
    fn skill_manager_ctrl_a_toggles_all_visible_skills() {
        let mut app = app_with_many_installed_and_available();
        app.skill_search = "available".to_string();
        let mut redraw = |_app: &mut SkillManagerApp| Ok(());

        block_on(app.handle_key(
            KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL),
            &mut redraw,
        ))
        .unwrap();

        assert_eq!(app.selected.len(), 20);
        assert!(app.selected.iter().all(|index| *index >= 60));
        assert_eq!(app.status, "Selected 20 visible skill(s).");

        block_on(app.handle_key(
            KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL),
            &mut redraw,
        ))
        .unwrap();

        assert!(app.selected.is_empty());
        assert_eq!(app.status, "Cleared 20 visible skill(s).");
    }

    #[test]
    fn skill_manager_r_inverts_visible_skill_selection() {
        let mut app = app_with_many_installed_and_available();
        app.skill_search = "available".to_string();
        app.selected.insert(60);
        let mut redraw = |_app: &mut SkillManagerApp| Ok(());

        block_on(app.handle_key(
            KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE),
            &mut redraw,
        ))
        .unwrap();

        assert_eq!(app.selected.len(), 19);
        assert!(!app.selected.contains(&60));
        assert!(app.selected.iter().all(|index| *index > 60));
        assert_eq!(app.status, "Inverted 20 visible skill(s).");
    }

    #[test]
    fn skill_manager_palette_avoids_harsh_white_for_primary_text() {
        let body = body_style().fg.expect("body color");
        let title = title_style().fg.expect("title color");
        let muted = muted_style().fg.expect("muted color");
        let focus = focus_style().fg.expect("focus color");

        assert!(matches!(body, ratatui::style::Color::Rgb(_, _, _)));
        assert!(matches!(title, ratatui::style::Color::Rgb(_, _, _)));
        assert!(matches!(muted, ratatui::style::Color::Rgb(_, _, _)));
        assert!(matches!(focus, ratatui::style::Color::Rgb(_, _, _)));
        assert_ne!(body, ratatui::style::Color::White);
        assert_ne!(title, ratatui::style::Color::White);
        assert_ne!(focus, muted);
    }

    #[test]
    fn skill_manager_details_slash_shows_status_without_search_mode() {
        let mut app = app_with_report();
        app.focus = FocusPane::Details;
        let mut redraw = |_app: &mut SkillManagerApp| Ok(());

        block_on(app.handle_key(
            KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE),
            &mut redraw,
        ))
        .unwrap();

        assert!(!app.search_mode);
        assert_eq!(
            app.status,
            "Details are scroll-only; Tab to Agents or Skills to search."
        );
    }

    #[test]
    fn skill_manager_mutation_progress_status_uses_rotating_spinner() {
        assert_eq!(
            mutation_progress_status_with_frame("Installing", 0, 4, "alpha", 0),
            "| Installing 1/4 alpha"
        );
        assert_eq!(
            mutation_progress_status_with_frame("Installing", 1, 4, "beta", 1),
            "/ Installing 2/4 beta"
        );
        assert_eq!(
            mutation_progress_status_with_frame("Deleting", 2, 4, "gamma", 2),
            "- Deleting 3/4 gamma"
        );
        assert_eq!(
            mutation_progress_status_with_frame("Deleting", 3, 4, "delta", 3),
            "\\ Deleting 4/4 delta"
        );
    }

    #[test]
    fn skill_manager_mutation_spinner_keeps_ticking_while_waiting() {
        let mut app = app_with_report();
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            thread::sleep(MUTATION_SPINNER_TICK + Duration::from_millis(40));
            tx.send(Ok(())).unwrap();
        });
        let mut statuses = Vec::new();
        let mut redraw = |app: &mut SkillManagerApp| {
            statuses.push(app.status.clone());
            Ok(())
        };

        app.wait_for_mutation_result(rx, "Installing", 4, 5, "canvas-design", &mut redraw)
            .unwrap();

        let waiting_statuses = statuses
            .iter()
            .filter(|status| status.ends_with("Installing 5/5 canvas-design"))
            .collect::<Vec<_>>();
        assert!(!waiting_statuses.is_empty());
        assert!(waiting_statuses.iter().all(|status| {
            matches!(status.as_bytes().first(), Some(b'|' | b'/' | b'-' | b'\\'))
        }));
    }

    fn buffer_text(backend: &TestBackend) -> String {
        let buffer = backend.buffer();
        let mut out = String::new();
        for y in 0..buffer.area.height {
            for x in 0..buffer.area.width {
                out.push_str(buffer[(x, y)].symbol());
            }
            out.push('\n');
        }
        out
    }

    fn block_on<T>(future: impl std::future::Future<Output = T>) -> T {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(future)
    }
}
