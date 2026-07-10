use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crossterm::cursor::MoveUp;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{self, Clear, ClearType};
use sentra_lib::config::sentra_home;
use sentra_lib::{SentraError, SentraResult};
use serde::{Deserialize, Serialize};

use crate::cli::args::{Command, ModelAction, OutputFormat, OutputOptions, UpdateTarget};
use crate::cli::feedback::{self, Status};
use crate::cli::i18n::t;
use crate::tui::theme::{AnsiStyle, paint};

const REPO: &str = "flash-dev-ctrl/sentra-cli";
const INSTALL_SH_URL: &str =
    "https://github.com/flash-dev-ctrl/sentra-cli/releases/latest/download/install.sh";
const INSTALL_PS1_URL: &str =
    "https://github.com/flash-dev-ctrl/sentra-cli/releases/latest/download/install.ps1";
const CHECK_INTERVAL: Duration = Duration::from_secs(6 * 60 * 60);
const SKIP_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct UpdateState {
    next_check_at: u64,
}

#[derive(Debug, Clone)]
struct LatestRelease {
    tag: String,
}

pub(crate) async fn maybe_prompt_auto_update(command: &Command) {
    if !allows_auto_update_prompt(command) {
        return;
    }
    if !io::stdin().is_terminal() || !io::stderr().is_terminal() {
        return;
    }
    let Some(home) = home::home_dir() else {
        return;
    };
    let state_path = update_state_file(&home);
    let mut state = load_state(&state_path).unwrap_or_default();
    let now = now_secs();
    if now < state.next_check_at {
        return;
    }

    let latest = match latest_release().await {
        Ok(latest) => latest,
        Err(_) => {
            state.next_check_at = next_time(CHECK_INTERVAL);
            let _ = save_state(&state_path, &state);
            return;
        }
    };

    if !is_newer_version(&latest.tag, current_version()) {
        state.next_check_at = next_time(CHECK_INTERVAL);
        let _ = save_state(&state_path, &state);
        return;
    }

    match prompt_update_choice(&latest.tag) {
        AutoUpdateChoice::UpdateNow => {
            if let Err(err) = install_version(&latest.tag).await {
                feedback::status_line(
                    Status::Error,
                    format!("{}: {err}", t("update failed", "更新失败")),
                );
                state.next_check_at = next_time(CHECK_INTERVAL);
                let _ = save_state(&state_path, &state);
            }
        }
        AutoUpdateChoice::Later => {
            state.next_check_at = next_time(CHECK_INTERVAL);
            let _ = save_state(&state_path, &state);
        }
        AutoUpdateChoice::SkipToday => {
            state.next_check_at = next_time(SKIP_INTERVAL);
            let _ = save_state(&state_path, &state);
        }
    }
}

pub(crate) async fn manual_update() -> SentraResult<()> {
    let latest = latest_release().await?;
    if !is_newer_version(&latest.tag, current_version()) {
        feedback::result(
            Status::Success,
            t("Sentra is already up to date", "Sentra 已是最新版本"),
            &[(t("Version", "版本"), current_version().to_string())],
        );
        return Ok(());
    }
    feedback::context(
        t("Update Sentra", "更新 Sentra"),
        &[
            (t("Current", "当前版本"), current_version().to_string()),
            (t("Latest", "最新版本"), latest.tag.clone()),
        ],
    );
    install_version(&latest.tag).await
}

fn current_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

async fn latest_release() -> SentraResult<LatestRelease> {
    tokio::task::spawn_blocking(fetch_latest_release)
        .await
        .map_err(|err| SentraError::Message(err.to_string()))?
}

fn fetch_latest_release() -> SentraResult<LatestRelease> {
    let agent = ureq::AgentBuilder::new().timeout(REQUEST_TIMEOUT).build();
    let url = format!("https://api.github.com/repos/{REPO}/releases/latest");
    let response = agent
        .get(&url)
        .set("Accept", "application/vnd.github+json")
        .set("User-Agent", "sentra-cli")
        .call()
        .map_err(|err| SentraError::Message(err.to_string()))?;
    let body = response
        .into_string()
        .map_err(|err| SentraError::Message(err.to_string()))?;
    let value: serde_json::Value =
        serde_json::from_str(&body).map_err(|err| SentraError::Message(err.to_string()))?;
    let tag = value
        .get("tag_name")
        .and_then(|value| value.as_str())
        .ok_or_else(|| SentraError::Message("missing release tag_name".to_string()))?
        .to_string();
    Ok(LatestRelease { tag })
}

async fn install_version(version: &str) -> SentraResult<()> {
    let version = version.to_string();
    tokio::task::spawn_blocking(move || run_installer(&version))
        .await
        .map_err(|err| SentraError::Message(err.to_string()))?
}

fn run_installer(version: &str) -> SentraResult<()> {
    let status = if cfg!(windows) {
        let command = format!(
            "$env:SENTRA_VERSION = '{}'; irm -TimeoutSec 15 {} | iex",
            escape_powershell_single_quoted(version),
            INSTALL_PS1_URL
        );
        ProcessCommand::new("powershell")
            .args([
                "-NoProfile",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                &command,
            ])
            .status()
    } else {
        let command = format!(
            "SENTRA_VERSION='{}'; export SENTRA_VERSION; \
             if command -v curl >/dev/null 2>&1; then \
               curl -fsSL --max-time 15 '{}' | sh; \
             else \
               wget -qO- '{}' | sh; \
             fi",
            escape_sh_single_quoted(version),
            INSTALL_SH_URL,
            INSTALL_SH_URL
        );
        ProcessCommand::new("sh").args(["-c", &command]).status()
    }
    .map_err(|err| SentraError::io(None, err))?;

    if status.success() {
        Ok(())
    } else {
        Err(SentraError::Message(format!(
            "{}: {status}",
            t("installer failed", "安装脚本失败")
        )))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AutoUpdateChoice {
    UpdateNow,
    Later,
    SkipToday,
}

fn prompt_update_choice(latest: &str) -> AutoUpdateChoice {
    eprintln!();
    feedback::context(
        t("New Sentra version available", "发现 Sentra 新版本"),
        &[
            (t("Current", "当前版本"), current_version().to_string()),
            (t("Latest", "最新版本"), latest.to_string()),
        ],
    );
    eprintln!(
        "{}",
        paint(
            t(
                "Update now? Use ↑/↓ and Enter, or press y/l/s:",
                "立即更新？使用 ↑/↓ 和 Enter，或按 y/l/s:"
            ),
            AnsiStyle::Foreground,
            feedback::terminal_symbols(),
        )
    );

    let options = update_choice_options();
    let mut selected = 0usize;
    render_update_choice_options(&options, selected);

    let Ok(_raw_mode) = RawModeGuard::new() else {
        return AutoUpdateChoice::Later;
    };

    loop {
        match read_key_event() {
            Ok(KeyEvent {
                code: KeyCode::Up, ..
            })
            | Ok(KeyEvent {
                code: KeyCode::Char('k'),
                ..
            }) => {
                selected = selected.saturating_sub(1);
                redraw_update_choice_options(&options, selected);
            }
            Ok(KeyEvent {
                code: KeyCode::Down,
                ..
            })
            | Ok(KeyEvent {
                code: KeyCode::Char('j'),
                ..
            }) => {
                selected = (selected + 1).min(options.len().saturating_sub(1));
                redraw_update_choice_options(&options, selected);
            }
            Ok(KeyEvent {
                code: KeyCode::Enter,
                ..
            }) => return options[selected].choice,
            Ok(KeyEvent {
                code: KeyCode::Char('y'),
                ..
            })
            | Ok(KeyEvent {
                code: KeyCode::Char('Y'),
                ..
            }) => return AutoUpdateChoice::UpdateNow,
            Ok(KeyEvent {
                code: KeyCode::Char('l'),
                ..
            })
            | Ok(KeyEvent {
                code: KeyCode::Char('L'),
                ..
            }) => return AutoUpdateChoice::Later,
            Ok(KeyEvent {
                code: KeyCode::Char('s'),
                ..
            })
            | Ok(KeyEvent {
                code: KeyCode::Char('S'),
                ..
            }) => return AutoUpdateChoice::SkipToday,
            Ok(KeyEvent {
                code: KeyCode::Esc, ..
            }) => return AutoUpdateChoice::Later,
            Err(_) => return AutoUpdateChoice::Later,
            _ => {}
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct UpdateChoiceOption {
    label: &'static str,
    choice: AutoUpdateChoice,
}

fn update_choice_options() -> [UpdateChoiceOption; 3] {
    [
        UpdateChoiceOption {
            label: t("Yes, update now", "是，立即更新"),
            choice: AutoUpdateChoice::UpdateNow,
        },
        UpdateChoiceOption {
            label: t("Later", "稍后提醒"),
            choice: AutoUpdateChoice::Later,
        },
        UpdateChoiceOption {
            label: t("Skip today", "今天不提示"),
            choice: AutoUpdateChoice::SkipToday,
        },
    ]
}

fn render_update_choice_options(options: &[UpdateChoiceOption], selected: usize) {
    for (index, option) in options.iter().enumerate() {
        let marker = if index == selected { ">" } else { " " };
        let color = feedback::terminal_symbols();
        let marker_style = if index == selected {
            AnsiStyle::Accent
        } else {
            AnsiStyle::Muted
        };
        let label_style = if index == selected {
            AnsiStyle::Foreground
        } else {
            AnsiStyle::Secondary
        };
        eprintln!(
            "  {} {}",
            paint(marker, marker_style, color),
            paint(option.label, label_style, color)
        );
    }
    let _ = io::stderr().flush();
}

fn redraw_update_choice_options(options: &[UpdateChoiceOption], selected: usize) {
    let mut stderr = io::stderr();
    let _ = execute!(
        stderr,
        MoveUp(options.len() as u16),
        Clear(ClearType::FromCursorDown)
    );
    render_update_choice_options(options, selected);
}

fn read_key_event() -> SentraResult<KeyEvent> {
    loop {
        if let Event::Key(key) =
            event::read().map_err(|err| SentraError::Message(err.to_string()))?
        {
            if key.kind == KeyEventKind::Press {
                return Ok(key);
            }
        }
    }
}

struct RawModeGuard;

impl RawModeGuard {
    fn new() -> SentraResult<Self> {
        terminal::enable_raw_mode().map_err(|err| SentraError::Message(err.to_string()))?;
        Ok(Self)
    }
}

impl Drop for RawModeGuard {
    fn drop(&mut self) {
        let _ = terminal::disable_raw_mode();
        eprintln!();
    }
}

fn allows_auto_update_prompt(command: &Command) -> bool {
    match command {
        Command::Help
        | Command::Version
        | Command::ListHelp
        | Command::ScanHelp
        | Command::ImportHelp
        | Command::UpdateHelp
        | Command::ModelHelp
        | Command::SkillHelp
        | Command::InstallHelp
        | Command::UninstallHelp
        | Command::Update {
            target: UpdateTarget::Auto,
        }
        | Command::Update {
            target: UpdateTarget::Cli,
        }
        | Command::Update {
            target: UpdateTarget::Rules,
        }
        | Command::Install { .. }
        | Command::Uninstall { .. } => false,
        Command::List { output, .. } | Command::Scan { output, .. } => is_terminal_output(output),
        Command::Model {
            action: ModelAction::List { output },
        } => is_terminal_output(output),
        _ => true,
    }
}

fn is_terminal_output(output: &OutputOptions) -> bool {
    output.output.is_none() && output.format == OutputFormat::Terminal
}

fn update_state_file(home: &Path) -> PathBuf {
    sentra_home(home).join("update-state.json")
}

fn load_state(path: &Path) -> SentraResult<UpdateState> {
    let content = std::fs::read_to_string(path)
        .map_err(|err| SentraError::io(Some(path.to_path_buf()), err))?;
    serde_json::from_str(&content).map_err(|err| SentraError::Message(err.to_string()))
}

fn save_state(path: &Path, state: &UpdateState) -> SentraResult<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|err| SentraError::io(Some(parent.to_path_buf()), err))?;
    }
    let content =
        serde_json::to_string_pretty(state).map_err(|err| SentraError::Message(err.to_string()))?;
    std::fs::write(path, format!("{content}\n"))
        .map_err(|err| SentraError::io(Some(path.to_path_buf()), err))
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn next_time(interval: Duration) -> u64 {
    now_secs().saturating_add(interval.as_secs())
}

fn is_newer_version(latest: &str, current: &str) -> bool {
    parse_version(latest) > parse_version(current)
}

fn parse_version(value: &str) -> Vec<u64> {
    value
        .trim_start_matches('v')
        .split('.')
        .map(|part| {
            part.chars()
                .take_while(|ch| ch.is_ascii_digit())
                .collect::<String>()
                .parse::<u64>()
                .unwrap_or(0)
        })
        .collect()
}

fn escape_sh_single_quoted(value: &str) -> String {
    value.replace('\'', "'\"'\"'")
}

fn escape_powershell_single_quoted(value: &str) -> String {
    value.replace('\'', "''")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::args::{ListResource, OutputOptions};

    #[test]
    fn compares_release_versions_with_v_prefix() {
        assert!(is_newer_version("v0.1.1", "0.1.0"));
        assert!(is_newer_version("v1.0.0", "0.9.9"));
        assert!(!is_newer_version("v0.1.0", "0.1.0"));
        assert!(!is_newer_version("v0.0.9", "0.1.0"));
    }

    #[test]
    fn auto_update_skips_machine_readable_outputs() {
        let command = Command::List {
            resource: ListResource::Agent,
            home: None,
            agent: None,
            output: OutputOptions {
                format: OutputFormat::Json,
                output: None,
            },
        };

        assert!(!allows_auto_update_prompt(&command));
    }

    #[test]
    fn update_state_round_trips_next_check_time() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("update-state.json");
        let state = UpdateState { next_check_at: 42 };

        save_state(&path, &state).unwrap();
        let loaded = load_state(&path).unwrap();

        assert_eq!(loaded.next_check_at, 42);
    }
}
