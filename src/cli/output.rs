use std::io::{self, IsTerminal, Write};

use std::collections::BTreeMap;

use chrono::{DateTime, Local};
use sentra_lib::{SentraError, SentraResult};
use unicode_width::UnicodeWidthStr;

use crate::cli::args::{OutputFormat, OutputOptions};
use crate::cli::i18n::{t, yes_no};
use crate::tui::theme::{AnsiStyle, paint, severity_ansi_style};

pub(crate) fn print_json<T: serde::Serialize>(value: T) -> SentraResult<()> {
    let json = serde_json::to_string_pretty(&value)
        .map_err(|err| SentraError::Message(err.to_string()))?;
    write_stdout(&format!("{json}\n"))
}

pub(crate) fn write_output<T: serde::Serialize>(
    value: T,
    options: &OutputOptions,
    title: &str,
) -> SentraResult<()> {
    let content = match options.format {
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&value)
                .map_err(|err| SentraError::Message(err.to_string()))?;
            format!("{json}\n")
        }
        OutputFormat::Terminal => {
            let value = serde_json::to_value(&value)
                .map_err(|err| SentraError::Message(err.to_string()))?;
            format_terminal(title, &value, should_color(options))
        }
    };

    match &options.output {
        Some(path) => {
            std::fs::write(path, content).map_err(|err| SentraError::io(Some(path.clone()), err))
        }
        None => write_stdout(&content),
    }
}

pub(crate) fn write_stdout(content: &str) -> SentraResult<()> {
    let mut stdout = io::stdout().lock();
    if let Err(err) = write!(stdout, "{content}") {
        if err.kind() == io::ErrorKind::BrokenPipe {
            return Ok(());
        }
        return Err(SentraError::io(None, err));
    }
    Ok(())
}

fn should_color(options: &OutputOptions) -> bool {
    options.output.is_none() && stdout_color_enabled()
}

pub(crate) fn stdout_color_enabled() -> bool {
    io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none()
}

fn format_terminal(title: &str, value: &serde_json::Value, color: bool) -> String {
    if title == "Scan Results" {
        return format_scan_results(value, color);
    }
    if title == "Agents" {
        return format_agents(value, color);
    }
    if title == "Assets" {
        return format_assets(value, color);
    }
    if title == "Models" {
        return format_models(value, color);
    }
    format_generic(title, value)
}

fn format_agents(value: &serde_json::Value, semantic_symbols: bool) -> String {
    let Some(items) = value.as_array() else {
        return format_generic("Agents", value);
    };
    let mut rows = Vec::new();
    for item in items {
        rows.push(vec![
            string_field(item, "name"),
            string_field(item, "title"),
            bool_label(item, "installed"),
            string_field(item, "home"),
        ]);
    }
    format_list_view(
        t("List agents", "列出 Agent"),
        &format!(
            "{} {}",
            rows.len(),
            t("agent(s) discovered", "个 Agent 已发现")
        ),
        &format!(
            "{} `sentra list skill` {} `sentra scan skill`",
            t("Next:", "下一步:"),
            t("or", "或")
        ),
        format_table(
            &format!("{} ({})", t("Agents", "Agent"), rows.len()),
            &[
                t("NAME", "名称"),
                t("TITLE", "标题"),
                t("INSTALLED", "已安装"),
                t("HOME", "目录"),
            ],
            rows,
            semantic_symbols,
        ),
        semantic_symbols,
    )
}

fn format_assets(value: &serde_json::Value, semantic_symbols: bool) -> String {
    let Some(items) = value.as_array() else {
        return format_generic("Assets", value);
    };
    let asset_type = items
        .first()
        .and_then(|item| item.get("assetType"))
        .and_then(|value| value.as_str())
        .unwrap_or("asset");
    match asset_type {
        "provider" => format_provider_assets(items, semantic_symbols),
        "skill" => format_skill_assets(items, semantic_symbols),
        "mcp" => format_named_asset_items(
            "mcp",
            t("MCP Servers", "MCP 服务"),
            items,
            "MCP",
            semantic_symbols,
        ),
        "memory" => format_named_asset_items(
            "memory",
            t("Memories", "记忆"),
            items,
            t("MEMORY", "记忆"),
            semantic_symbols,
        ),
        "cron" => format_cron_assets(items, semantic_symbols),
        "plugin" => format_plugin_assets(items, semantic_symbols),
        "process" => format_process_assets(items, semantic_symbols),
        _ => format_generic("Assets", value),
    }
}

fn format_provider_assets(items: &[serde_json::Value], semantic_symbols: bool) -> String {
    let mut rows = Vec::new();
    for item in items {
        let agent = string_field(item, "agentName");
        for provider in data_items(item) {
            rows.push(vec![
                agent.clone(),
                string_field(provider, "name"),
                string_field(provider, "providerType"),
                account_label(provider),
                enabled_label(provider),
                model_names(provider),
                string_field(provider, "baseUrl"),
            ]);
        }
    }
    format_list_view(
        t("List providers", "列出供应商"),
        &format!(
            "{} {}",
            rows.len(),
            t("provider(s) discovered", "个供应商已发现")
        ),
        &format!(
            "{} `sentra model` {} `sentra scan provider`",
            t("Next:", "下一步:"),
            t("or", "或")
        ),
        format_table(
            &format!("{} ({})", t("Providers", "供应商"), rows.len()),
            &[
                t("AGENT", "AGENT"),
                t("PROVIDER", "供应商"),
                t("TYPE", "类型"),
                t("ACCOUNT", "账户"),
                t("ENABLED", "启用"),
                t("MODELS", "模型"),
                t("BASE URL", "BASE URL"),
            ],
            rows,
            semantic_symbols,
        ),
        semantic_symbols,
    )
}

fn format_named_asset_items(
    resource: &str,
    title: &str,
    items: &[serde_json::Value],
    item_header: &str,
    semantic_symbols: bool,
) -> String {
    let mut rows = Vec::new();
    for item in items {
        let agent = string_field(item, "agentName");
        for data in data_items(item) {
            rows.push(vec![
                agent.clone(),
                data.get("name")
                    .or_else(|| data.get("id"))
                    .and_then(|value| value.as_str())
                    .unwrap_or("-")
                    .to_string(),
                string_field(data, "description"),
            ]);
        }
    }
    format_list_view(
        &format!("{} {resource}", t("List", "列出")),
        &format!("{title}: {}", rows.len()),
        &format!(
            "{} `sentra scan {resource}` {} `sentra list agent`",
            t("Next:", "下一步:"),
            t("or", "或")
        ),
        format_table(
            &format!("{title} ({})", rows.len()),
            &[t("AGENT", "AGENT"), item_header, t("DESCRIPTION", "描述")],
            rows,
            semantic_symbols,
        ),
        semantic_symbols,
    )
}

fn format_cron_assets(items: &[serde_json::Value], semantic_symbols: bool) -> String {
    let mut rows = Vec::new();
    for item in items {
        let agent = string_field(item, "agentName");
        for data in data_items(item) {
            rows.push(vec![
                agent.clone(),
                data.get("name")
                    .or_else(|| data.get("id"))
                    .and_then(|value| value.as_str())
                    .unwrap_or("-")
                    .to_string(),
                enabled_label(data),
                string_field(data, "schedule"),
                display_text_field(data, &["prompt", "description"]),
            ]);
        }
    }
    format_list_view(
        t("List cron tasks", "列出定时任务"),
        &format!(
            "{} {}",
            rows.len(),
            t("cron task(s) discovered", "个定时任务已发现")
        ),
        &format!(
            "{} `sentra scan cron` {} `sentra list agent`",
            t("Next:", "下一步:"),
            t("or", "或")
        ),
        format_table(
            &format!("{} ({})", t("Crons", "定时任务"), rows.len()),
            &[
                t("AGENT", "AGENT"),
                t("CRON", "定时"),
                t("ENABLED", "启用"),
                t("SCHEDULE", "调度"),
                t("PROMPT", "提示词"),
            ],
            rows,
            semantic_symbols,
        ),
        semantic_symbols,
    )
}

fn format_plugin_assets(items: &[serde_json::Value], semantic_symbols: bool) -> String {
    let mut rows = Vec::new();
    for item in items {
        let agent = string_field(item, "agentName");
        for data in data_items(item) {
            rows.push(vec![
                agent.clone(),
                data.get("displayName")
                    .or_else(|| data.get("name"))
                    .or_else(|| data.get("id"))
                    .and_then(|value| value.as_str())
                    .unwrap_or("-")
                    .to_string(),
                string_field(data, "version"),
                plugin_source_label(data),
                enabled_label(data),
            ]);
        }
    }
    format_list_view(
        t("List plugins", "列出插件"),
        &format!(
            "{} {}",
            rows.len(),
            t("plugin(s) discovered", "个插件已发现")
        ),
        &format!("{} `sentra list agent`", t("Next:", "下一步:")),
        format_table(
            &format!("{} ({})", t("Plugins", "插件"), rows.len()),
            &[
                t("AGENT", "AGENT"),
                t("PLUGIN", "插件"),
                t("VERSION", "版本"),
                t("SOURCE", "来源"),
                t("ENABLED", "启用"),
            ],
            rows,
            semantic_symbols,
        ),
        semantic_symbols,
    )
}

fn format_process_assets(items: &[serde_json::Value], semantic_symbols: bool) -> String {
    let mut rows = Vec::new();
    for item in items {
        let agent = string_field(item, "agentName");
        for data in data_items(item) {
            rows.push(vec![
                agent.clone(),
                number_field(data, "pid"),
                started_at_field(data),
                run_time_field(data),
                string_field(data, "path"),
            ]);
        }
    }
    format_list_view(
        t("List processes", "列出进程"),
        &format!(
            "{} {}",
            rows.len(),
            t("process(es) discovered", "个进程已发现")
        ),
        &format!("{} `sentra list agent`", t("Next:", "下一步:")),
        format_table(
            &format!("{} ({})", t("Processes", "进程"), rows.len()),
            &[
                t("AGENT", "AGENT"),
                t("PID", "PID"),
                t("STARTED", "启动时间"),
                t("RUN TIME", "运行时间"),
                t("PATH", "路径"),
            ],
            rows,
            semantic_symbols,
        ),
        semantic_symbols,
    )
}

fn format_skill_assets(items: &[serde_json::Value], semantic_symbols: bool) -> String {
    let mut rows = Vec::new();
    for item in items {
        let agent = string_field(item, "agentName");
        for data in data_items(item) {
            rows.push(vec![
                agent.clone(),
                data.get("name")
                    .or_else(|| data.get("id"))
                    .and_then(|value| value.as_str())
                    .unwrap_or("-")
                    .to_string(),
            ]);
        }
    }
    format_list_view(
        t("List skills", "列出技能"),
        &format!(
            "{} {}",
            rows.len(),
            t("skill(s) discovered", "个技能已发现")
        ),
        &format!(
            "{} `sentra skill` {} `sentra scan skill`",
            t("Next:", "下一步:"),
            t("or", "或")
        ),
        format_table(
            &format!("{} ({})", t("Skills", "技能"), rows.len()),
            &[t("AGENT", "AGENT"), t("SKILL", "技能")],
            rows,
            semantic_symbols,
        ),
        semantic_symbols,
    )
}

fn format_list_view(
    title: &str,
    result: &str,
    next: &str,
    table: String,
    semantic_symbols: bool,
) -> String {
    let mut output = String::new();
    output.push_str(&paint(
        if semantic_symbols { "●" } else { "[INFO]" },
        AnsiStyle::Purple,
        semantic_symbols,
    ));
    output.push(' ');
    output.push_str(&paint(title, AnsiStyle::Foreground, semantic_symbols));
    output.push('\n');
    output.push_str(&paint(
        if semantic_symbols { "✓" } else { "[OK]" },
        AnsiStyle::Green,
        semantic_symbols,
    ));
    output.push(' ');
    output.push_str(&paint(result, AnsiStyle::Foreground, semantic_symbols));
    output.push_str("\n\n");
    output.push_str(&table);
    output.push('\n');
    output.push_str(&format_next_step(next, semantic_symbols));
    output.push('\n');
    output
}

fn format_next_step(next: &str, color: bool) -> String {
    if let Some((label, rest)) = next.split_once(':') {
        format!(
            "{}{}",
            paint(&format!("{label}:"), AnsiStyle::Muted, color),
            paint(rest, AnsiStyle::Secondary, color)
        )
    } else {
        paint(next, AnsiStyle::Secondary, color)
    }
}

fn format_scan_results(value: &serde_json::Value, color: bool) -> String {
    let Some(items) = value.as_array() else {
        return format_generic("Scan Results", value);
    };
    if items.is_empty() {
        return format!(
            "{}\n{}\n",
            t("Scan Results", "扫描结果"),
            t("No records found", "没有记录")
        );
    }
    let mut renderer = ScanTerminalRenderer::new(color);
    let risky_items = items
        .iter()
        .filter(|item| scan_asset_has_results(item))
        .collect::<Vec<_>>();
    for (index, item) in risky_items.iter().enumerate() {
        if index > 0 {
            renderer.blank();
        }
        renderer.asset_section(item);
    }
    renderer.summary(items, &risky_items);
    renderer.finish()
}

struct ScanTerminalRenderer {
    output: String,
    color: bool,
}

impl ScanTerminalRenderer {
    const RULE: &'static str = "────────────────────────────────────────────────────────────";
    const SUMMARY_WIDTHS: [usize; 6] = [10, 8, 8, 8, 8, 8];

    fn new(color: bool) -> Self {
        Self {
            output: String::new(),
            color,
        }
    }

    fn finish(self) -> String {
        self.output
    }

    fn blank(&mut self) {
        self.output.push('\n');
    }

    fn rule(&mut self, style: Option<AnsiStyle>) {
        match style {
            Some(style) => self.output.push_str(&paint(Self::RULE, style, self.color)),
            None => self.output.push_str(Self::RULE),
        }
        self.output.push('\n');
    }

    fn asset_section(&mut self, item: &serde_json::Value) {
        self.rule(None);
        self.output
            .push_str(&format!("{}\n", scan_asset_heading(item)));
        self.rule(None);

        let report = item.get("report").unwrap_or(&serde_json::Value::Null);
        let mut findings = array_field(report, "findings");
        findings.sort_by_key(|finding| severity_rank(&string_field(finding, "severity")));
        for (index, finding) in findings.iter().enumerate() {
            if index > 0 {
                self.blank();
            }
            self.finding_detail(index + 1, finding);
        }

        let errors = array_field(report, "errors");
        for error in errors {
            if !findings.is_empty() {
                self.blank();
            }
            self.scan_error(error);
        }
    }

    fn summary(&mut self, items: &[serde_json::Value], risky_items: &[&serde_json::Value]) {
        if !self.output.is_empty() {
            self.blank();
        }
        let mut by_asset = BTreeMap::<String, [usize; 5]>::new();
        for item in items {
            let report = item.get("report").unwrap_or(&serde_json::Value::Null);
            for finding in array_field(report, "findings") {
                if let Some(index) = severity_index(&string_field(finding, "severity")) {
                    by_asset.entry(scan_asset_type(item)).or_insert([0; 5])[index] += 1;
                }
            }
        }

        let risky_style = if risky_items.is_empty() {
            AnsiStyle::Green
        } else {
            AnsiStyle::DangerBold
        };
        self.output.push_str(&paint(
            t("Audit complete", "审计完成"),
            AnsiStyle::Green,
            self.color,
        ));
        self.output.push('\n');
        self.output
            .push_str(&format!("  {}: ", t("Risky assets", "风险资产")));
        self.output.push_str(&paint(
            &risky_items.len().to_string(),
            risky_style,
            self.color,
        ));
        self.output.push_str(&format!(
            "/{} {}\n",
            items.len(),
            t("(risky/total)", "(风险/总数)")
        ));
        if by_asset.is_empty() {
            self.output.push_str(&format!(
                "  {}: {}\n",
                t("Findings", "发现"),
                t("none", "无")
            ));
            return;
        }
        self.blank();
        self.output
            .push_str(&format!("{}\n", t("Findings by asset", "按资产汇总发现")));
        self.summary_header();
        for (asset_type, counts) in by_asset {
            self.summary_asset_row(&asset_type, counts);
        }
    }

    fn summary_header(&mut self) {
        self.summary_cells(
            &[
                t("Asset", "资产"),
                t("Critical", "严重"),
                t("High", "高"),
                t("Medium", "中"),
                t("Low", "低"),
                t("Info", "信息"),
            ],
            &[
                None,
                Some(AnsiStyle::DangerBold),
                Some(AnsiStyle::High),
                Some(AnsiStyle::WarningBold),
                Some(AnsiStyle::Blue),
                Some(AnsiStyle::Accent),
            ],
        );
    }

    fn summary_asset_row(&mut self, asset_type: &str, counts: [usize; 5]) {
        let cells = [
            asset_type.to_string(),
            count_or_dot(counts[0]),
            count_or_dot(counts[1]),
            count_or_dot(counts[2]),
            count_or_dot(counts[3]),
            count_or_dot(counts[4]),
        ];
        let styles = [
            None,
            Some(summary_count_style(counts[0], AnsiStyle::DangerBold)),
            Some(summary_count_style(counts[1], AnsiStyle::High)),
            Some(summary_count_style(counts[2], AnsiStyle::WarningBold)),
            Some(summary_count_style(counts[3], AnsiStyle::Blue)),
            Some(summary_count_style(counts[4], AnsiStyle::Accent)),
        ];
        self.summary_cells(&cells, &styles);
    }

    fn summary_cells<T: AsRef<str>>(&mut self, cells: &[T], styles: &[Option<AnsiStyle>]) {
        self.output.push_str("  ");
        for (index, cell) in cells.iter().enumerate() {
            let cell = cell.as_ref();
            if index > 0 {
                self.output.push_str("  ");
            }
            let padding =
                " ".repeat(Self::SUMMARY_WIDTHS[index].saturating_sub(display_width(cell)));
            match styles.get(index).and_then(|style| *style) {
                Some(style) if index == 0 => {
                    self.output.push_str(&paint(cell, style, self.color));
                    self.output.push_str(&padding);
                }
                Some(style) => {
                    self.output.push_str(&padding);
                    self.output.push_str(&paint(cell, style, self.color));
                }
                None if index == 0 => {
                    self.output.push_str(cell);
                    self.output.push_str(&padding);
                }
                None => {
                    self.output.push_str(&padding);
                    self.output.push_str(cell);
                }
            }
        }
        self.output.push('\n');
    }

    fn finding_detail(&mut self, index: usize, finding: &serde_json::Value) {
        let severity = string_field(finding, "severity");
        let severity_label = severity_label(&severity);
        self.output.push_str(&format!(
            "  {index} {}\n",
            paint(severity_label, severity_ansi_style(&severity), self.color)
        ));
        self.detail_field(t("Severity", "严重性"), severity_label);
        self.detail_field(t("Title", "标题"), &string_field(finding, "title"));
        let category = category_label(&string_field(finding, "category"));
        let checker = string_field(finding, "checker");
        if checker == "-" {
            self.detail_field(t("Category", "类别"), &category);
        } else if category != "-" {
            self.detail_field(t("Category", "类别"), &category);
            self.detail_field(t("Checker", "检查器"), &checker);
        }
        self.detail_field(t("File", "文件"), &finding_location(finding));
        self.optional_detail_field(t("Description", "描述"), finding, "description");
        self.optional_detail_field_styled(
            t("Evidence", "证据"),
            finding,
            "evidence",
            AnsiStyle::Warning,
        );
        self.optional_detail_field(t("Remediation", "修复建议"), finding, "remediation");
        self.context_detail(finding);
    }

    fn scan_error(&mut self, error: &serde_json::Value) {
        self.output.push_str(t("  Scan Error\n", "  扫描错误\n"));
        self.optional_detail_field(t("Checker", "检查器"), error, "checker");
        self.optional_detail_field(t("Source", "来源"), error, "source");
        self.optional_detail_field(t("Reason", "原因"), error, "reason");
        self.optional_detail_field(t("Message", "消息"), error, "message");
    }

    fn detail_field(&mut self, label: &str, value: &str) {
        if value.trim().is_empty() || value == "-" {
            return;
        }
        self.output.push_str(&format!("  {label}: {value}\n"));
    }

    fn optional_detail_field(&mut self, label: &str, value: &serde_json::Value, key: &str) {
        if let Some(text) = value.get(key).and_then(|value| value.as_str())
            && !text.trim().is_empty()
        {
            self.detail_field(label, text);
        }
    }

    fn optional_detail_field_styled(
        &mut self,
        label: &str,
        value: &serde_json::Value,
        key: &str,
        style: AnsiStyle,
    ) {
        if let Some(text) = value.get(key).and_then(|value| value.as_str())
            && !text.trim().is_empty()
        {
            self.detail_field(label, &paint(text, style, self.color));
        }
    }

    fn context_detail(&mut self, finding: &serde_json::Value) {
        let context = context_lines_for_finding(finding);
        if context.is_empty() {
            self.optional_detail_field(t("Context", "上下文"), finding, "context");
            return;
        }
        self.output
            .push_str(&format!("  {}:\n", t("Context", "上下文")));
        let number_width = context
            .iter()
            .filter_map(|line| line.number)
            .map(|number| number.to_string().len())
            .max()
            .unwrap_or(1);
        let evidence = finding
            .get("evidence")
            .and_then(|value| value.as_str())
            .filter(|value| !value.trim().is_empty());
        for line in context {
            self.context_line(line, number_width, evidence);
        }
    }

    fn context_line(&mut self, line: ContextLine, number_width: usize, evidence: Option<&str>) {
        let marker = if line.is_target { '>' } else { ' ' };
        let number = line
            .number
            .map(|number| format!("{number:>number_width$}"))
            .unwrap_or_else(|| " ".repeat(number_width));
        let text = if line.is_target {
            highlight_evidence(&line.text, evidence, self.color)
        } else {
            line.text
        };
        let rendered = format!("  {marker} {number} | {text}");
        if line.is_target {
            self.output
                .push_str(&paint(&rendered, AnsiStyle::WarningBold, self.color));
        } else {
            self.output.push_str(&rendered);
        }
        self.output.push('\n');
    }
}

fn scan_asset_has_results(item: &serde_json::Value) -> bool {
    let report = item.get("report").unwrap_or(&serde_json::Value::Null);
    !array_field(report, "findings").is_empty() || !array_field(report, "errors").is_empty()
}

fn array_field<'a>(value: &'a serde_json::Value, key: &str) -> Vec<&'a serde_json::Value> {
    value
        .get(key)
        .and_then(|value| value.as_array())
        .map(|items| items.iter().collect::<Vec<_>>())
        .unwrap_or_default()
}

fn severity_index(severity: &str) -> Option<usize> {
    match severity {
        "CRITICAL" => Some(0),
        "HIGH" => Some(1),
        "MEDIUM" => Some(2),
        "LOW" => Some(3),
        "INFO" => Some(4),
        _ => None,
    }
}

fn count_or_dot(count: usize) -> String {
    if count == 0 {
        "·".to_string()
    } else {
        count.to_string()
    }
}

fn summary_count_style(count: usize, severity_style: AnsiStyle) -> AnsiStyle {
    if count == 0 {
        AnsiStyle::Muted
    } else {
        severity_style
    }
}

fn scan_asset_heading(value: &serde_json::Value) -> String {
    let user = string_field(value, "user");
    let agent = value
        .get("agentTitle")
        .or_else(|| value.get("agent"))
        .and_then(|value| value.as_str())
        .unwrap_or("-");
    format!(
        "{user} / {agent} / {} \"{}\"",
        scan_asset_type(value),
        scan_target_name(value)
    )
}

fn severity_label(severity: &str) -> &'static str {
    match severity {
        "CRITICAL" => t("Critical", "严重"),
        "HIGH" => t("High", "高"),
        "MEDIUM" => t("Medium", "中"),
        "LOW" => t("Low", "低"),
        "INFO" => t("Info", "信息"),
        _ => t("Unknown", "未知"),
    }
}

fn category_label(category: &str) -> String {
    match category {
        "PROMPT_INJECTION" => t("Prompt Injection", "提示词注入").to_string(),
        "MALICIOUS_EXECUTION" => t("Malicious Execution", "恶意执行").to_string(),
        "SUPPLY_CHAIN" => t("Supply Chain", "供应链").to_string(),
        "DATA_EXFILTRATION" => t("Data Exfiltration", "数据外泄").to_string(),
        "CREDENTIAL_EXPOSURE" => t("Credential Exposure", "凭据暴露").to_string(),
        "-" => "-".to_string(),
        other => other.to_string(),
    }
}

fn severity_rank(severity: &str) -> usize {
    match severity {
        "CRITICAL" => 0,
        "HIGH" => 1,
        "MEDIUM" => 2,
        "LOW" => 3,
        "INFO" => 4,
        _ => 5,
    }
}

fn finding_location(finding: &serde_json::Value) -> String {
    let file = string_field(finding, "file");
    let line = finding
        .get("location")
        .and_then(|value| value.get("line"))
        .and_then(|value| value.as_u64())
        .unwrap_or(0);
    let column = finding
        .get("location")
        .and_then(|value| value.get("column"))
        .and_then(|value| value.as_u64());
    match column {
        Some(column) => format!("{file}:{line}:{column}"),
        None => format!("{file}:{line}"),
    }
}

fn context_lines_for_finding(finding: &serde_json::Value) -> Vec<ContextLine> {
    let line = finding
        .get("location")
        .and_then(|value| value.get("line"))
        .and_then(|value| value.as_u64())
        .map(|value| value as usize)
        .unwrap_or(0);
    if line > 0 {
        let file = string_field(finding, "file");
        if let Ok(content) = std::fs::read_to_string(file) {
            return context_window_from_content(&content, line, 2, 2);
        }
    }
    context_from_text(
        finding.get("context").and_then(|value| value.as_str()),
        line,
        finding.get("evidence").and_then(|value| value.as_str()),
    )
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
                text: truncate_detail(text, 180),
                is_target: line_number == target_line,
            }
        })
        .collect()
}

fn context_from_text(
    context: Option<&str>,
    target_line: usize,
    evidence: Option<&str>,
) -> Vec<ContextLine> {
    let lines = context
        .into_iter()
        .flat_map(str::lines)
        .take(8)
        .collect::<Vec<_>>();
    let target_index = evidence
        .and_then(|evidence| {
            let evidence = evidence.trim();
            (!evidence.is_empty())
                .then(|| lines.iter().position(|line| line.contains(evidence)))
                .flatten()
        })
        .unwrap_or(0);
    lines
        .into_iter()
        .enumerate()
        .map(|(index, text)| ContextLine {
            number: (target_line > 0).then_some(
                target_line
                    .saturating_sub(target_index)
                    .saturating_add(index),
            ),
            text: truncate_detail(text, 180),
            is_target: index == target_index,
        })
        .collect()
}

fn truncate_detail(value: &str, max_chars: usize) -> String {
    let mut out = value.chars().take(max_chars).collect::<String>();
    if value.chars().count() > max_chars {
        out.push_str("...");
    }
    out
}

fn highlight_evidence(text: &str, evidence: Option<&str>, color: bool) -> String {
    let Some(evidence) = evidence else {
        return text.to_string();
    };
    let evidence = evidence.trim();
    if evidence.is_empty() {
        return text.to_string();
    }
    let Some(start) = text.find(evidence) else {
        return text.to_string();
    };
    let end = start + evidence.len();
    format!(
        "{}{}{}",
        &text[..start],
        paint(&text[start..end], AnsiStyle::DangerBold, color),
        &text[end..]
    )
}

#[derive(Debug)]
struct ContextLine {
    number: Option<usize>,
    text: String,
    is_target: bool,
}

fn format_models(value: &serde_json::Value, semantic_symbols: bool) -> String {
    let Some(items) = value.as_array() else {
        return format_generic(t("Models", "模型"), value);
    };
    let mut rows = Vec::new();
    for item in items {
        rows.push(vec![
            string_field(item, "agentName"),
            string_field(item, "providerName"),
            string_field(item, "providerType"),
            string_field(item, "account"),
            string_field(item, "model"),
            enabled_label(item),
            string_field(item, "protocol"),
            string_field(item, "baseUrl"),
        ]);
    }
    format_list_view(
        t("List models", "列出模型"),
        &format!(
            "{} {}",
            rows.len(),
            t("model provider(s) discovered", "个模型供应商已发现")
        ),
        &format!(
            "{} `sentra model set --agent sentra --base-url <url> --api-key <key> --model <id>`",
            t("Next:", "下一步:")
        ),
        format_table(
            &format!("{} ({})", t("Models", "模型"), rows.len()),
            &[
                t("AGENT", "AGENT"),
                t("PROVIDER", "供应商"),
                t("TYPE", "类型"),
                t("ACCOUNT", "账户"),
                t("MODEL", "模型"),
                t("ENABLED", "启用"),
                t("PROTOCOL", "协议"),
                t("BASE URL", "BASE URL"),
            ],
            rows,
            semantic_symbols,
        ),
        semantic_symbols,
    )
}

fn format_generic(title: &str, value: &serde_json::Value) -> String {
    let mut output = String::new();
    output.push_str(title);
    output.push('\n');

    match value {
        serde_json::Value::Array(items) if items.is_empty() => {
            output.push_str(t("No records found\n", "没有记录\n"))
        }
        serde_json::Value::Array(items) => {
            output.push_str(&format!("{} {}\n", items.len(), t("record(s)", "条记录")));
            for item in items {
                output.push_str("- ");
                output.push_str(&item.to_string());
                output.push('\n');
            }
        }
        _ => {
            output.push_str(&value.to_string());
            output.push('\n');
        }
    }

    output
}

fn format_table(title: &str, headers: &[&str], rows: Vec<Vec<String>>, color: bool) -> String {
    let mut output = String::new();
    output.push_str(&paint(title, AnsiStyle::Foreground, color));
    output.push('\n');
    if rows.is_empty() {
        output.push_str(&paint(
            t("No records found", "没有记录"),
            AnsiStyle::Muted,
            color,
        ));
        output.push('\n');
        return output;
    }

    let mut widths: Vec<usize> = headers.iter().map(|header| display_width(header)).collect();
    for row in &rows {
        for (index, cell) in row.iter().enumerate() {
            widths[index] = widths[index].max(display_width(cell));
        }
    }

    write_table_row(&mut output, headers, &widths, Some(AnsiStyle::Muted), color);
    let separators: Vec<String> = widths.iter().map(|width| "-".repeat(*width)).collect();
    let separator_refs: Vec<&str> = separators.iter().map(String::as_str).collect();
    write_table_row(
        &mut output,
        &separator_refs,
        &widths,
        Some(AnsiStyle::Muted),
        color,
    );
    for row in rows {
        let cells: Vec<&str> = row.iter().map(String::as_str).collect();
        write_table_row(
            &mut output,
            &cells,
            &widths,
            Some(AnsiStyle::Secondary),
            color,
        );
    }
    output
}

fn write_table_row(
    output: &mut String,
    cells: &[&str],
    widths: &[usize],
    style: Option<AnsiStyle>,
    color: bool,
) {
    for (index, cell) in cells.iter().enumerate() {
        if index > 0 {
            output.push_str("  ");
        }
        match style {
            Some(style) => output.push_str(&paint(cell, style, color)),
            None => output.push_str(cell),
        }
        output.push_str(&" ".repeat(widths[index].saturating_sub(display_width(cell))));
    }
    output.push('\n');
}

fn display_width(value: &str) -> usize {
    UnicodeWidthStr::width(value)
}

fn data_items(value: &serde_json::Value) -> Vec<&serde_json::Value> {
    match value.get("data") {
        Some(serde_json::Value::Array(items)) => items.iter().collect(),
        Some(data) => vec![data],
        None => Vec::new(),
    }
}

fn string_field(value: &serde_json::Value, key: &str) -> String {
    value
        .get(key)
        .and_then(|value| value.as_str())
        .unwrap_or("-")
        .to_string()
}

fn number_field(value: &serde_json::Value, key: &str) -> String {
    value
        .get(key)
        .and_then(|value| {
            value
                .as_u64()
                .map(|value| value.to_string())
                .or_else(|| value.as_i64().map(|value| value.to_string()))
        })
        .unwrap_or_else(|| "-".to_string())
}

fn started_at_field(value: &serde_json::Value) -> String {
    let Some(seconds) = u64_field(value, "startedAt") else {
        return "-".to_string();
    };
    if seconds == 0 {
        return "-".to_string();
    }
    let Ok(seconds) = i64::try_from(seconds) else {
        return seconds.to_string();
    };
    DateTime::from_timestamp(seconds, 0)
        .map(|timestamp| {
            timestamp
                .with_timezone(&Local)
                .format("%Y-%m-%d %H:%M:%S")
                .to_string()
        })
        .unwrap_or_else(|| seconds.to_string())
}

fn run_time_field(value: &serde_json::Value) -> String {
    u64_field(value, "runTimeSeconds")
        .map(format_duration_seconds)
        .unwrap_or_else(|| "-".to_string())
}

fn u64_field(value: &serde_json::Value, key: &str) -> Option<u64> {
    value.get(key).and_then(|value| value.as_u64())
}

fn format_duration_seconds(total_seconds: u64) -> String {
    let days = total_seconds / 86_400;
    let hours = (total_seconds % 86_400) / 3_600;
    let minutes = (total_seconds % 3_600) / 60;
    let seconds = total_seconds % 60;

    if days > 0 {
        format!("{days}d {hours:02}h {minutes:02}m {seconds:02}s")
    } else if hours > 0 {
        format!("{hours}h {minutes:02}m {seconds:02}s")
    } else if minutes > 0 {
        format!("{minutes}m {seconds:02}s")
    } else {
        format!("{seconds}s")
    }
}

fn display_text_field(value: &serde_json::Value, keys: &[&str]) -> String {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(|value| value.as_str()))
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("-")
        .to_string()
}

fn account_label(value: &serde_json::Value) -> String {
    let Some(account) = value.get("account") else {
        return "-".to_string();
    };
    [
        "email",
        "displayName",
        "organizationName",
        "organizationId",
        "accountId",
    ]
    .iter()
    .find_map(|key| account.get(*key).and_then(|value| value.as_str()))
    .unwrap_or("-")
    .to_string()
}

fn enabled_label(value: &serde_json::Value) -> String {
    bool_label(value, "enabled")
}

fn bool_label(value: &serde_json::Value, key: &str) -> String {
    match value.get(key).and_then(|value| value.as_bool()) {
        Some(value) => yes_no(value).to_string(),
        None => "-".to_string(),
    }
}

fn model_names(value: &serde_json::Value) -> String {
    let names: Vec<&str> = value
        .get("models")
        .and_then(|value| value.as_array())
        .map(|models| {
            models
                .iter()
                .filter_map(|model| model.get("name").or_else(|| model.get("id")))
                .filter_map(|value| value.as_str())
                .collect()
        })
        .unwrap_or_default();
    if names.is_empty() {
        "-".to_string()
    } else {
        names.join(", ")
    }
}

fn plugin_source_label(value: &serde_json::Value) -> String {
    value
        .get("installSource")
        .and_then(|source| {
            source
                .get("reference")
                .and_then(|value| value.as_str())
                .or_else(|| source.get("kind").and_then(|value| value.as_str()))
        })
        .or_else(|| value.get("origin").and_then(|value| value.as_str()))
        .unwrap_or("-")
        .to_string()
}

fn scan_target_name(value: &serde_json::Value) -> String {
    value
        .get("name")
        .and_then(|value| value.as_str())
        .or_else(|| {
            value
                .get("data")
                .and_then(|data| data.get("name").or_else(|| data.get("id")))
                .and_then(|value| value.as_str())
        })
        .unwrap_or("-")
        .to_string()
}

fn scan_asset_type(value: &serde_json::Value) -> String {
    let asset_type = value
        .get("type")
        .or_else(|| value.get("assetType"))
        .and_then(|value| value.as_str())
        .unwrap_or("-");
    if asset_type == "-" {
        "asset".to_string()
    } else {
        asset_type.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skill_terminal_output_lists_skill_rows() {
        let value = serde_json::json!([
            {
                "assetType": "skill",
                "agentName": "claude-cli",
                "data": [
                    {
                        "name": "Spreadsheets",
                        "description": "Use this skill when a user requests to create, modify, analyze, visualize, or work with spreadsheet files (`.xlsx`, `.xls`, `.csv`, `.tsv`) or Google Sheets-targeted spreadsheet artifacts with formulas, formatting, charts, tables, and recalculation."
                    },
                    {
                        "name": "ssh-port-forward-proxy",
                        "description": "通过 SSH 端口转发配置远程主机使用本地代理。适用于在远程服务器上设置代理访问、通过隧道启用 apt/dnf/pip/docker，或需要将远程流量路由到本地代理的场景。"
                    }
                ]
            },
            {
                "assetType": "skill",
                "agentName": "claude-cli",
                "data": [
                    {
                        "name": "documents",
                        "description": "Create and edit documents."
                    }
                ]
            }
        ]);

        let output = format_assets(&value, false);

        assert!(output.contains("Skills (3)"));
        assert!(output.contains("AGENT"));
        assert!(output.contains("SKILL"));
        assert!(!output.contains("DESCRIPTION"));
        assert!(output.contains("claude-cli"));
        assert!(output.contains("Spreadsheets"), "{output}");
        assert!(output.contains("ssh-port-forward-proxy"), "{output}");
        assert!(output.contains("documents"), "{output}");
        assert!(!output.contains("Google Sheets-targeted"), "{output}");
        assert!(!output.contains("端口转发"), "{output}");
    }

    #[test]
    fn table_output_aligns_columns_with_cjk_text() {
        let output = format_table(
            "Crons (2)",
            &["AGENT", "CRON", "DESCRIPTION"],
            vec![
                vec![
                    "codex".to_string(),
                    "每日热门新闻三条".to_string(),
                    "-".to_string(),
                ],
                vec![
                    "claude-app".to_string(),
                    "daily-news-summary-permanent".to_string(),
                    "-".to_string(),
                ],
            ],
            false,
        );

        let rows = output.lines().skip(3).collect::<Vec<_>>();
        let description_columns = rows
            .iter()
            .map(|line| display_column_of(line, "-").expect(line))
            .collect::<Vec<_>>();

        assert_eq!(description_columns[0], description_columns[1], "{output}");
    }

    #[test]
    fn cron_terminal_output_uses_prompt_and_schedule_columns() {
        let value = serde_json::json!([
            {
                "assetType": "cron",
                "agentName": "claude-app",
                "data": [
                    {
                        "name": "daily-news-update",
                        "prompt": "每天早上8:30获取最新新闻",
                        "enabled": true,
                        "schedule": "30 8 * * *"
                    }
                ]
            }
        ]);

        let output = format_assets(&value, false);

        assert!(output.contains("List cron tasks"));
        assert!(output.contains("1 cron task(s) discovered"));
        assert!(output.contains("AGENT"));
        assert!(output.contains("CRON"));
        assert!(output.contains("ENABLED"));
        assert!(output.contains("SCHEDULE"));
        assert!(output.contains("PROMPT"));
        assert!(output.contains("claude-app"));
        assert!(output.contains("daily-news-update"));
        assert!(output.contains("30 8 * * *"));
        assert!(output.contains("每天早上8:30获取最新新闻"));
        assert!(!output.contains("DESCRIPTION"));
    }

    #[test]
    fn process_terminal_output_lists_process_rows_without_env() {
        let value = serde_json::json!([
            {
                "assetType": "process",
                "agentName": "codex",
                "data": [
                    {
                        "pid": 42,
                        "name": "codex.exe",
                        "path": "C:\\Users\\me\\AppData\\Local\\Programs\\OpenAI\\Codex\\codex.exe",
                        "cmdline": ["codex.exe", "--sandbox"],
                        "startedAt": 1700000000,
                        "runTimeSeconds": 3661,
                        "env": {
                            "OPENAI_API_KEY": "sk-****7890",
                            "PATH": "C:\\bin"
                        }
                    }
                ]
            }
        ]);

        let output = format_assets(&value, false);

        assert!(output.contains("List processes") || output.contains("列出进程"));
        assert!(output.contains("1 process(es) discovered") || output.contains("1 个进程已发现"));
        assert!(output.contains("AGENT"));
        assert!(output.contains("PID"));
        let started_header = if output.contains("STARTED") {
            "STARTED"
        } else {
            "启动时间"
        };
        let run_time_header = if output.contains("RUN TIME") {
            "RUN TIME"
        } else {
            "运行时间"
        };
        let path_header = if output.contains("PATH") {
            "PATH"
        } else {
            "路径"
        };
        assert!(output.contains(started_header));
        assert!(output.contains(run_time_header));
        assert!(output.contains(path_header));
        let header = output
            .lines()
            .find(|line| line.contains("AGENT") && line.contains(path_header))
            .unwrap();
        assert!(header.find(started_header).unwrap() < header.find(path_header).unwrap());
        assert!(header.find(run_time_header).unwrap() < header.find(path_header).unwrap());
        assert!(!output.contains("NAME"));
        assert!(!output.contains("CMDLINE"));
        assert!(output.contains("codex"));
        assert!(output.contains("42"));
        assert!(output.contains("2023-11"));
        assert!(output.contains("1h 01m 01s"));
        assert!(output.contains(r"C:\Users\me\AppData\Local\Programs\OpenAI\Codex\codex.exe"));
        assert!(!output.contains("--sandbox"));
        assert!(!output.contains("OPENAI_API_KEY"));
        assert!(!output.contains("sk-"));
    }

    #[test]
    fn list_terminal_output_colors_text_hierarchy_when_enabled() {
        let value = serde_json::json!([
            {
                "assetType": "cron",
                "agentName": "claude-app",
                "data": [
                    {
                        "name": "daily-news-update",
                        "prompt": "每天早上8:30获取最新新闻",
                        "enabled": true,
                        "schedule": "30 8 * * *"
                    }
                ]
            }
        ]);

        let output = format_assets(&value, true);

        assert!(output.contains("\u{1b}[38;2;184;190;202mList cron tasks\u{1b}[0m"));
        assert!(output.contains("\u{1b}[38;2;100;109;122mAGENT\u{1b}[0m"));
        assert!(output.contains("\u{1b}[38;2;142;152;168mclaude-app\u{1b}[0m"));
        assert!(!output.contains("\u{1b}[97m"));
        assert!(!output.contains("\u{1b}[1;97m"));
    }

    #[test]
    fn next_step_colon_is_part_of_muted_label() {
        let output = format_next_step("Next: `sentra scan cron`", true);

        assert!(output.starts_with("\u{1b}[38;2;100;109;122mNext:\u{1b}[0m"));
        assert!(output.contains("\u{1b}[38;2;142;152;168m `sentra scan cron`\u{1b}[0m"));
        assert!(!output.contains("\u{1b}[0m:"));
    }

    fn display_column_of(line: &str, needle: &str) -> Option<usize> {
        let byte_index = line.rfind(needle)?;
        Some(display_width_for_test(&line[..byte_index]))
    }

    fn display_width_for_test(value: &str) -> usize {
        value
            .chars()
            .map(|ch| if is_wide_for_test(ch) { 2 } else { 1 })
            .sum()
    }

    fn is_wide_for_test(ch: char) -> bool {
        matches!(
            ch as u32,
            0x1100..=0x115F
                | 0x2E80..=0xA4CF
                | 0xAC00..=0xD7A3
                | 0xF900..=0xFAFF
                | 0xFE10..=0xFE19
                | 0xFE30..=0xFE6F
                | 0xFF00..=0xFF60
                | 0xFFE0..=0xFFE6
        )
    }

    #[test]
    fn scan_terminal_output_includes_error_reason() {
        let value = serde_json::json!([
            {
                "source": "path",
                "data": {"name": "hack-skill"},
                "report": {
                    "findings": [],
                    "errors": [
                        {
                            "checker": "llm-checker",
                            "source": "SKILL.md",
                            "reason": "failed to parse model response as JSON"
                        }
                    ]
                }
            }
        ]);

        let output = format_scan_results(&value, false);

        assert!(output.contains("Scan Error"));
        assert!(output.contains("Checker"));
        assert!(output.contains("llm-checker"));
        assert!(output.contains("Reason"));
        assert!(output.contains("failed to parse model response as JSON"));
    }

    #[test]
    fn scan_terminal_output_uses_asset_section_template() {
        let value = serde_json::json!([
            {
                "user": "23741",
                "agentTitle": "Codex",
                "type": "skill",
                "name": "tui-reverse-engineering",
                "report": {
                    "findings": [
                        {
                            "severity": "HIGH",
                            "title": "胁迫式提示词注入检测",
                            "category": "PROMPT_INJECTION",
                            "checker": "yara-checker",
                            "file": "C:\\Users\\23741\\.codex\\skills\\tui-reverse-engineering\\SKILL.md",
                            "location": {"line": 544},
                            "description": "检测工具描述字段中胁迫模型改变执行顺序或窃取上下文的提示词注入",
                            "evidence": "Hidden input",
                            "remediation": "移除强制工具调用指令，确保工具描述只说明合法行为。",
                            "context": "- Color-only analysis.\n- Widget inventory without purpose.\n- Product names preserved as design patterns.\n- Hidden input during long-running work.\n- Global repaint that disrupts typing."
                        }
                    ],
                    "errors": []
                }
            }
        ]);

        let output = format_scan_results(&value, false);

        assert!(output.contains("23741 / Codex / skill \"tui-reverse-engineering\""));
        assert!(output.contains("  1 High"));
        assert!(output.contains("  Severity: High"));
        assert!(output.contains("  Title: 胁迫式提示词注入检测"));
        assert!(output.contains("  Category: Prompt Injection\n  Checker: yara-checker"));
        assert!(output.contains("SKILL.md:544"));
        assert!(output.contains(
            "  Description: 检测工具描述字段中胁迫模型改变执行顺序或窃取上下文的提示词注入"
        ));
        assert!(output.contains("  Evidence: Hidden input"));
        assert!(
            output.contains("  Remediation: 移除强制工具调用指令，确保工具描述只说明合法行为。")
        );
        assert!(output.contains("  Context:"));
        assert!(output.contains("  > 544 | - Hidden input during long-running work."));
        assert!(output.contains("Audit complete"));
        assert!(output.contains("  Risky assets: 1/1 (risky/total)"));
        assert!(output.contains("Findings by asset"));
        assert!(output.contains("  Asset       Critical      High    Medium       Low      Info"));
        assert!(output.contains("  skill              ·         1         ·         ·         ·"));
    }

    #[test]
    fn scan_terminal_output_omits_clean_asset_sections() {
        let value = serde_json::json!([
            {
                "user": "23741",
                "agentTitle": "Codex",
                "type": "skill",
                "name": "clean-skill",
                "report": {
                    "findings": [],
                    "errors": []
                }
            }
        ]);

        let output = format_scan_results(&value, false);

        assert!(!output.contains("clean-skill"));
        assert!(!output.contains("No risks found"));
        assert!(output.contains("Audit complete"));
        assert!(output.contains("  Risky assets: 0/1 (risky/total)"));
        assert!(output.contains("  Findings: none"));
        assert!(!output.contains("  Asset       Critical      High    Medium       Low      Info"));
        assert!(!output.contains("  skill"));
    }

    #[test]
    fn scan_terminal_output_sorts_findings_by_risk_within_asset() {
        let value = serde_json::json!([
            {
                "source": "path",
                "data": {"name": "mixed-skill"},
                "report": {
                    "findings": [
                        {
                            "severity": "LOW",
                            "title": "Low risk",
                            "category": "SUPPLY_CHAIN",
                            "checker": "hash-checker",
                            "file": "LOW.md",
                            "location": {"line": 1}
                        },
                        {
                            "severity": "CRITICAL",
                            "title": "Critical risk",
                            "category": "MALICIOUS_EXECUTION",
                            "checker": "llm-checker",
                            "file": "CRITICAL.md",
                            "location": {"line": 1}
                        }
                    ],
                    "errors": []
                }
            }
        ]);

        let output = format_scan_results(&value, false);

        let critical = output.find("  1 Critical").unwrap();
        let low = output.find("  2 Low").unwrap();
        assert!(critical < low);
        assert!(output.contains("  File: CRITICAL.md:1"));
        assert!(output.contains("  Category: Malicious Execution\n  Checker: llm-checker"));
    }

    #[test]
    fn scan_terminal_output_keeps_network_access_category_and_aligns_context_separator() {
        let value = serde_json::json!([
            {
                "source": "path",
                "data": {"name": "network-skill"},
                "report": {
                    "findings": [
                        {
                            "severity": "HIGH",
                            "title": "Malicious IP detected: 203.0.113.9",
                            "category": "NETWORK_ACCESS",
                            "checker": "threat-intel-checker",
                            "file": "SKILL.md",
                            "location": {"line": 5},
                            "context": "2. NEVER proceed without user approval after document creation\ncontext line 6\ncontext line 7\ncontext line 8\ncontext line 9\ncontext line 10\ncontext line 11\ncontext line 12",
                            "evidence": "NEVER proceed"
                        }
                    ],
                    "errors": []
                }
            }
        ]);

        let output = format_scan_results(&value, false);

        assert!(output.contains("  Category: NETWORK_ACCESS\n  Checker: threat-intel-checker"));
        assert!(
            output.contains(
                "  >  5 | 2. NEVER proceed without user approval after document creation"
            )
        );
        assert!(output.contains("    12 | context line 12"));
    }

    #[test]
    fn scan_terminal_output_color_avoids_bright_white_and_disables_cleanly() {
        let value = serde_json::json!([
            {
                "assetType": "skill",
                "source": "path",
                "data": {"name": "critical-skill"},
                "report": {
                    "findings": [
                        {
                            "severity": "CRITICAL",
                            "title": "Critical risk",
                            "category": "MALICIOUS_EXECUTION",
                            "checker": "llm-checker",
                            "file": "CRITICAL.md",
                            "location": {"line": 1},
                            "evidence": "danger"
                        }
                    ],
                    "errors": []
                }
            }
        ]);

        let colored = format_scan_results(&value, true);
        assert!(colored.contains("\u{1b}[1;38;2;"));
        assert!(colored.contains("\u{1b}[38;2;"));
        assert!(colored.contains("mCritical\u{1b}[0m"));
        assert!(colored.contains("mAudit complete\u{1b}[0m"));
        assert!(colored.contains("m1\u{1b}[0m/1 (risky/total)"));
        assert!(colored.contains("m·\u{1b}[0m"));
        assert!(!colored.contains("\u{1b}[97m"));
        assert!(!colored.contains("\u{1b}[1;97m"));

        let plain = format_scan_results(&value, false);
        assert!(!plain.contains("\u{1b}["));
    }
}
