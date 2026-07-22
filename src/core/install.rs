#[cfg(test)]
use sentra_lib::interfaces::AgentInstallResult;
use sentra_lib::interfaces::AgentUninstallOptions;
use sentra_lib::interfaces::{AgentInstallAction, AgentInstallProgress, AgentInstallProgressStage};
use sentra_lib::{SentraResult, agents};
use std::io::{self, IsTerminal, Write};

use crate::cli::feedback::{self, Status};
use crate::cli::i18n::t;

pub(crate) fn run(agent: String) -> SentraResult<()> {
    feedback::context(
        t("Install agent", "安装 Agent"),
        &[(t("Agent", "Agent"), agent.clone())],
    );
    let result = agents::install_agent_with_progress(&agent, |progress| {
        eprintln!("{}", render_install_progress(&progress));
    })?;
    feedback::result(
        Status::Success,
        install_result_title(result.action),
        &[(t("Name", "名称"), result.agent.clone())],
    );
    Ok(())
}

pub(crate) fn run_uninstall(agent: String, force: bool) -> SentraResult<()> {
    feedback::context(
        t("Uninstall agent", "卸载 Agent"),
        &[(t("Agent", "Agent"), agent.clone())],
    );
    let delete_config = force || confirm_delete_config(&agent)?;
    feedback::phase(
        if delete_config {
            Status::Warning
        } else {
            Status::Info
        },
        if delete_config {
            t("Configuration data will be deleted.", "将删除配置数据。")
        } else {
            t("Configuration data will be kept.", "将保留配置数据。")
        },
    );
    let result = agents::uninstall_agent_with_options_and_progress(
        &agent,
        AgentUninstallOptions { delete_config },
        |progress| {
            eprintln!("{}", render_install_progress(&progress));
        },
    )?;
    feedback::result(
        Status::Success,
        t("Agent uninstalled", "Agent 已卸载"),
        &[(t("Name", "名称"), result.agent.clone())],
    );
    Ok(())
}

fn confirm_delete_config(agent: &str) -> SentraResult<bool> {
    if !io::stdin().is_terminal() || !io::stderr().is_terminal() {
        feedback::phase(
            Status::Info,
            t(
                "Non-interactive input detected; configuration data will be kept. Use --force to delete it.",
                "检测到非交互输入；将保留配置数据。如需删除，请使用 --force。",
            ),
        );
        return Ok(false);
    }

    feedback::phase(
        Status::Warning,
        t(
            "Uninstall will remove the agent program. You can choose whether to delete configuration data.",
            "卸载会移除 Agent 程序。你可以选择是否删除配置数据。",
        ),
    );
    let prompt = t(
        "Delete configuration data for {agent}? Type y to delete, n to keep [y/N]:",
        "是否删除 {agent} 的配置数据？输入 y 删除，输入 n 保留 [y/N]:",
    )
    .replace("{agent}", agent);
    eprint!("  {} ", prompt,);
    io::stderr()
        .flush()
        .map_err(|err| sentra_lib::SentraError::io(None, err))?;

    let mut answer = String::new();
    io::stdin()
        .read_line(&mut answer)
        .map_err(|err| sentra_lib::SentraError::io(None, err))?;
    Ok(is_delete_config_confirmation_yes(&answer))
}

fn is_delete_config_confirmation_yes(answer: &str) -> bool {
    matches!(answer.trim().to_ascii_lowercase().as_str(), "y" | "yes")
}

fn install_result_title(action: AgentInstallAction) -> &'static str {
    match action {
        AgentInstallAction::Install => t("Agent installed", "Agent 已安装"),
        AgentInstallAction::Update => t("Agent updated", "Agent 已更新"),
        AgentInstallAction::Uninstall => t("Agent uninstalled", "Agent 已卸载"),
    }
}

#[cfg(test)]
fn render_install_result(result: &AgentInstallResult) -> String {
    let action = match result.action {
        AgentInstallAction::Install => t("Installed", "已安装"),
        AgentInstallAction::Update => t("Updated", "已更新"),
        AgentInstallAction::Uninstall => t("Uninstalled", "已卸载"),
    };
    format!("{action} {}", result.agent)
}

fn render_install_progress(progress: &AgentInstallProgress) -> String {
    let verb = match progress.stage {
        AgentInstallProgressStage::Trying => t("Try", "尝试"),
        AgentInstallProgressStage::Verifying => t("Verify", "验证"),
    };
    feedback::render_counted_action(
        progress.current,
        progress.total,
        &format!("{verb} {}", progress.method),
        &progress.agent,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_result_output_distinguishes_update() {
        let rendered = render_install_result(&AgentInstallResult {
            agent: "codex-cli".to_string(),
            action: AgentInstallAction::Update,
            command: "sh -c installer".to_string(),
            exit_code: Some(0),
            stdout: String::new(),
            stderr: String::new(),
        });

        assert!(rendered.starts_with("Updated codex"));
        assert!(!rendered.contains("command:"));
    }

    #[test]
    fn install_result_output_distinguishes_uninstall() {
        let rendered = render_install_result(&AgentInstallResult {
            agent: "claude-cli".to_string(),
            action: AgentInstallAction::Uninstall,
            command: "sh -c remover".to_string(),
            exit_code: Some(0),
            stdout: String::new(),
            stderr: String::new(),
        });

        assert!(
            rendered.starts_with("Uninstalled claude-cli")
                || rendered.starts_with("已卸载 claude-cli")
        );
        assert!(!rendered.contains("command:"));
    }

    #[test]
    fn install_progress_output_is_short_and_staged() {
        let rendered = render_install_progress(&AgentInstallProgress {
            agent: "codex-cli".to_string(),
            action: AgentInstallAction::Install,
            current: 2,
            total: 3,
            method: "WinGet".to_string(),
            stage: AgentInstallProgressStage::Trying,
        });

        assert_eq!(rendered, "  [2/3] Try WinGet\n  Target: codex");
        assert!(!rendered.contains("command:"));
    }

    #[test]
    fn install_progress_output_renders_verification_stage() {
        let rendered = render_install_progress(&AgentInstallProgress {
            agent: "opencode".to_string(),
            action: AgentInstallAction::Update,
            current: 2,
            total: 2,
            method: "opencode".to_string(),
            stage: AgentInstallProgressStage::Verifying,
        });

        assert_eq!(rendered, "  [2/2] Verify opencode\n  Target: opencode");
    }

    #[test]
    fn install_result_title_distinguishes_update_without_duplicate_stdout() {
        assert_eq!(
            install_result_title(AgentInstallAction::Update),
            "Agent updated"
        );
    }

    #[test]
    fn delete_config_confirmation_requires_explicit_yes() {
        assert!(is_delete_config_confirmation_yes("y"));
        assert!(is_delete_config_confirmation_yes("YES\n"));
        assert!(!is_delete_config_confirmation_yes(""));
        assert!(!is_delete_config_confirmation_yes("n"));
        assert!(!is_delete_config_confirmation_yes("codex"));
    }
}
