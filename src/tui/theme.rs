use ratatui::style::{Color, Modifier, Style};
use sentra_lib::interfaces::RiskSeverity;

pub(crate) fn body_style() -> Style {
    Style::default().fg(token_color(Token::Foreground))
}

pub(crate) fn muted_style() -> Style {
    Style::default().fg(token_color(Token::Muted))
}

pub(crate) fn secondary_style() -> Style {
    Style::default().fg(token_color(Token::Secondary))
}

pub(crate) fn title_style() -> Style {
    Style::default().fg(token_color(Token::Primary))
}

pub(crate) fn focus_style() -> Style {
    Style::default()
        .fg(token_color(Token::Primary))
        .add_modifier(Modifier::BOLD)
}

pub(crate) fn success_style() -> Style {
    Style::default().fg(token_color(Token::Success))
}

pub(crate) fn warning_style() -> Style {
    Style::default().fg(token_color(Token::Warning))
}

pub(crate) fn info_style() -> Style {
    Style::default().fg(token_color(Token::Info))
}

pub(crate) fn border_style(focused: bool) -> Style {
    if focused {
        Style::default().fg(token_color(Token::Primary))
    } else {
        Style::default().fg(token_color(Token::Muted))
    }
}

pub(crate) fn severity_style(severity: RiskSeverity) -> Style {
    let token = match severity {
        RiskSeverity::Critical => Token::Error,
        RiskSeverity::High => Token::Error,
        RiskSeverity::Medium => Token::Warning,
        RiskSeverity::Low => Token::Primary,
        RiskSeverity::Info => Token::Info,
    };
    Style::default().fg(token_color(token))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AnsiStyle {
    Accent,
    Blue,
    DangerBold,
    Foreground,
    Green,
    High,
    Muted,
    Purple,
    Secondary,
    Warning,
    WarningBold,
}

pub(crate) fn paint(value: &str, style: AnsiStyle, color: bool) -> String {
    if !color {
        return value.to_string();
    }
    let (token, bold) = match style {
        AnsiStyle::Accent => (Token::Primary, false),
        AnsiStyle::Blue => (Token::Primary, false),
        AnsiStyle::DangerBold => (Token::Error, true),
        AnsiStyle::Foreground => (Token::Foreground, false),
        AnsiStyle::Green => (Token::Success, false),
        AnsiStyle::High => (Token::Error, false),
        AnsiStyle::Muted => (Token::Muted, false),
        AnsiStyle::Purple => (Token::Info, false),
        AnsiStyle::Secondary => (Token::Secondary, false),
        AnsiStyle::Warning => (Token::Warning, false),
        AnsiStyle::WarningBold => (Token::Warning, true),
    };
    let color = rgb(token);
    let bold = if bold { "1;" } else { "" };
    format!(
        "\x1b[{bold}38;2;{};{};{}m{value}\x1b[0m",
        color.0, color.1, color.2
    )
}

pub(crate) fn severity_ansi_style(severity: &str) -> AnsiStyle {
    match severity {
        "CRITICAL" => AnsiStyle::DangerBold,
        "HIGH" => AnsiStyle::DangerBold,
        "MEDIUM" => AnsiStyle::WarningBold,
        "LOW" => AnsiStyle::Blue,
        _ => AnsiStyle::Purple,
    }
}

fn token_color(token: Token) -> Color {
    let (r, g, b) = rgb(token);
    Color::Rgb(r, g, b)
}

fn rgb(token: Token) -> (u8, u8, u8) {
    match token {
        Token::Foreground => (184, 190, 202),
        Token::Primary => (123, 159, 200),
        Token::Secondary => (142, 152, 168),
        Token::Muted => (100, 109, 122),
        Token::Success => (132, 179, 138),
        Token::Warning => (202, 167, 95),
        Token::Error => (204, 112, 112),
        Token::Info => (111, 159, 189),
    }
}

#[derive(Clone, Copy)]
enum Token {
    Foreground,
    Primary,
    Secondary,
    Muted,
    Success,
    Warning,
    Error,
    Info,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn palette_uses_calm_dark_terminal_tokens() {
        assert_eq!(token_color(Token::Foreground), Color::Rgb(184, 190, 202));
        assert_eq!(token_color(Token::Primary), Color::Rgb(123, 159, 200));
        assert_eq!(token_color(Token::Secondary), Color::Rgb(142, 152, 168));
        assert_eq!(token_color(Token::Muted), Color::Rgb(100, 109, 122));
        assert_eq!(token_color(Token::Success), Color::Rgb(132, 179, 138));
        assert_eq!(token_color(Token::Warning), Color::Rgb(202, 167, 95));
        assert_eq!(token_color(Token::Error), Color::Rgb(204, 112, 112));
    }

    #[test]
    fn ansi_palette_avoids_bright_white() {
        let rendered = paint("ok", AnsiStyle::Green, true);

        assert!(rendered.contains("\u{1b}[38;2;132;179;138m"));
        assert!(!rendered.contains("\u{1b}[97m"));
        assert!(!rendered.contains("\u{1b}[1;97m"));
    }
}
