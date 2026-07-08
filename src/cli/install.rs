use sentra_lib::interfaces::{
    AgentInstallAction, AgentInstallProgress, AgentInstallProgressStage, AgentInstallResult,
};
use sentra_lib::{SentraResult, agents};

use crate::i18n::t;

pub(crate) fn run(agent: String) -> SentraResult<()> {
    eprintln!("{}: {agent}", t("installing agent", "正在安装 Agent"));
    let result = agents::install_agent_with_progress(&agent, |progress| {
        eprintln!("{}", render_install_progress(&progress));
    })?;
    println!("{}", render_install_result(&result));
    Ok(())
}

pub(crate) fn run_uninstall(agent: String) -> SentraResult<()> {
    eprintln!("{}: {agent}", t("uninstalling agent", "正在卸载 Agent"));
    let result = agents::uninstall_agent_with_progress(&agent, |progress| {
        eprintln!("{}", render_install_progress(&progress));
    })?;
    println!("{}", render_install_result(&result));
    Ok(())
}

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
        AgentInstallProgressStage::Trying => t("trying", "正在尝试"),
        AgentInstallProgressStage::Verifying => t("verifying", "正在验证"),
    };
    format!(
        "[{}/{}] {} {}",
        progress.current, progress.total, verb, progress.method
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

        assert_eq!(rendered, "[2/3] trying WinGet");
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

        assert_eq!(rendered, "[2/2] verifying opencode");
    }
}
