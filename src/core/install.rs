#[cfg(test)]
use sentra_lib::interfaces::AgentInstallResult;
use sentra_lib::interfaces::{AgentInstallAction, AgentInstallProgress, AgentInstallProgressStage};
use sentra_lib::{SentraResult, agents};

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

pub(crate) fn run_uninstall(agent: String) -> SentraResult<()> {
    feedback::context(
        t("Uninstall agent", "卸载 Agent"),
        &[(t("Agent", "Agent"), agent.clone())],
    );
    let result = agents::uninstall_agent_with_progress(&agent, |progress| {
        eprintln!("{}", render_install_progress(&progress));
    })?;
    feedback::result(
        Status::Success,
        t("Agent uninstalled", "Agent 已卸载"),
        &[(t("Name", "名称"), result.agent.clone())],
    );
    Ok(())
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
            agent: "codex".to_string(),
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

        assert!(rendered.starts_with("Uninstalled claude-cli"));
        assert!(!rendered.contains("command:"));
    }

    #[test]
    fn install_progress_output_is_short_and_staged() {
        let rendered = render_install_progress(&AgentInstallProgress {
            agent: "codex".to_string(),
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
}
