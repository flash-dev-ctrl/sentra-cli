use std::io::{self, IsTerminal};

use crate::cli::i18n::t;
use crate::tui::theme::{AnsiStyle, paint};
use unicode_width::UnicodeWidthStr;

#[derive(Clone, Copy)]
pub(crate) enum Status {
    Info,
    Success,
    Warning,
    Error,
    Running,
    #[allow(dead_code)]
    Action,
}

impl Status {
    pub(crate) fn symbol(self) -> &'static str {
        self.symbol_for(terminal_symbols())
    }

    pub(crate) fn symbol_for(self, unicode: bool) -> &'static str {
        match (self, unicode) {
            (Status::Info, true) => "●",
            (Status::Success, true) => "✓",
            (Status::Warning, true) => "⚠",
            (Status::Error, true) => "✗",
            (Status::Running, true) => "◌",
            (Status::Action, true) => "▶",
            (Status::Info, false) => "[INFO]",
            (Status::Success, false) => "[OK]",
            (Status::Warning, false) => "[WARN]",
            (Status::Error, false) => "[ERR]",
            (Status::Running, false) => "[*]",
            (Status::Action, false) => ">",
        }
    }
}

pub(crate) fn terminal_symbols() -> bool {
    io::stderr().is_terminal()
        && std::env::var_os("NO_COLOR").is_none()
        && std::env::var_os("CI").is_none()
}

pub(crate) fn status_line(status: Status, message: impl AsRef<str>) {
    eprintln!(
        "{} {}",
        colored_symbol(status),
        paint_text(message.as_ref(), AnsiStyle::Foreground)
    );
}

pub(crate) fn context(title: &str, fields: &[(&str, String)]) {
    status_line(Status::Info, title);
    metadata(fields);
    eprintln!();
}

pub(crate) fn metadata(fields: &[(&str, String)]) {
    eprint!("{}", render_metadata(fields));
}

fn render_metadata(fields: &[(&str, String)]) -> String {
    let width = fields
        .iter()
        .filter(|(_, value)| !value.trim().is_empty())
        .map(|(label, _)| display_width(label))
        .max()
        .unwrap_or(0);
    let mut output = String::new();
    for (label, value) in fields {
        if !value.trim().is_empty() {
            let padding = " ".repeat(width.saturating_sub(display_width(label)) + 1);
            let label = paint_text(&format!("{label}:"), AnsiStyle::Muted);
            let value = paint_text(value, AnsiStyle::Secondary);
            output.push_str(&format!("  {label}{padding}{value}\n"));
        }
    }
    output
}

pub(crate) fn counted_action(current: usize, total: usize, action: &str, target: impl AsRef<str>) {
    eprintln!("{}", render_counted_action(current, total, action, target));
}

pub(crate) fn render_counted_action(
    current: usize,
    total: usize,
    action: &str,
    target: impl AsRef<str>,
) -> String {
    let counter = paint_text(&format!("[{current}/{total}]"), AnsiStyle::Muted);
    let action = paint_text(action, AnsiStyle::Foreground);
    let label = paint_text(&format!("{}:", t("Target", "目标")), AnsiStyle::Muted);
    let target = paint_text(target.as_ref(), AnsiStyle::Secondary);
    format!("  {counter} {action}\n  {label} {target}")
}

pub(crate) fn phase(status: Status, message: impl AsRef<str>) {
    eprintln!(
        "  {} {}",
        colored_symbol(status),
        paint_text(message.as_ref(), AnsiStyle::Foreground)
    );
}

pub(crate) fn result(status: Status, message: impl AsRef<str>, fields: &[(&str, String)]) {
    eprintln!();
    status_line(status, message);
    metadata(fields);
}

pub(crate) fn render_error(problem: &str, cause: &str, solution: &str) -> String {
    let title = paint_text(problem, AnsiStyle::Foreground);
    let problem_label = paint_text(t("Problem", "问题"), AnsiStyle::Muted);
    let cause_label = paint_text(t("Cause", "原因"), AnsiStyle::Muted);
    let solution_label = paint_text(t("Solution", "解决方案"), AnsiStyle::Muted);
    let problem = paint_text(problem, AnsiStyle::Secondary);
    let cause = paint_text(cause, AnsiStyle::Secondary);
    let solution = paint_text(solution, AnsiStyle::Secondary);
    format!(
        "{} {title}\n\n{problem_label}:\n  {problem}\n\n{cause_label}:\n  {cause}\n\n{solution_label}:\n  {solution}\n",
        colored_symbol(Status::Error),
    )
}

fn colored_symbol(status: Status) -> String {
    let symbol = status.symbol();
    let style = match status {
        Status::Info => AnsiStyle::Purple,
        Status::Success => AnsiStyle::Green,
        Status::Warning => AnsiStyle::Warning,
        Status::Error => AnsiStyle::DangerBold,
        Status::Running | Status::Action => AnsiStyle::Accent,
    };
    paint(symbol, style, terminal_symbols())
}

fn paint_text(value: &str, style: AnsiStyle) -> String {
    paint(value, style, terminal_symbols())
}

fn display_width(value: &str) -> usize {
    UnicodeWidthStr::width(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_symbols_have_ascii_fallbacks() {
        assert_eq!(Status::Info.symbol_for(false), "[INFO]");
        assert_eq!(Status::Success.symbol_for(false), "[OK]");
        assert_eq!(Status::Warning.symbol_for(false), "[WARN]");
        assert_eq!(Status::Error.symbol_for(false), "[ERR]");
        assert_eq!(Status::Running.symbol_for(false), "[*]");
        assert_eq!(Status::Action.symbol_for(false), ">");
    }

    #[test]
    fn error_template_uses_problem_cause_solution_sections() {
        let rendered = render_error("Install failed", "Permission denied", "Run as admin");

        assert!(rendered.starts_with("[ERR] Install failed") || rendered.starts_with("\u{1b}["));
        assert!(rendered.contains("Problem:\n  Install failed"));
        assert!(rendered.contains("Cause:\n  Permission denied"));
        assert!(rendered.contains("Solution:\n  Run as admin"));
    }

    #[test]
    fn counted_action_uses_hierarchy_without_color_in_logs() {
        let rendered = render_counted_action(1, 2, "Try npm", "opencode");

        assert_eq!(rendered, "  [1/2] Try npm\n  Target: opencode");
    }

    #[test]
    fn metadata_aligns_mixed_latin_and_cjk_labels_by_terminal_width() {
        let rendered = render_metadata(&[
            ("Agent", "sentra".to_string()),
            ("Base URL", "https://example.test/v1".to_string()),
            ("模型", "dev/gpt-5.5".to_string()),
            ("协议", "chat_completions".to_string()),
        ]);

        assert!(rendered.contains("  Agent:    sentra\n"));
        assert!(rendered.contains("  Base URL: https://example.test/v1\n"));
        assert!(rendered.contains("  模型:     dev/gpt-5.5\n"));
        assert!(rendered.contains("  协议:     chat_completions\n"));
        assert!(!rendered.contains("Agent   :"));
    }
}
