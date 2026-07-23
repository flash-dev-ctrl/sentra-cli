const QODER_AGENT_FAMILY: &[&str] = &[
    "qoder-cli",
    "qoder-ide",
    "qoder-work",
    "qoder-cn-cli",
    "qoder-cn-ide",
    "qoder-cn-work",
];
const CODEBUDDY_AGENT_FAMILY: &[&str] = &[
    "codebuddy-cli",
    "codebuddy-ide",
    "codebuddy-cn-ide",
    "codebuddy-ide-plugin",
    "workbuddy",
];

pub(crate) fn agent_matches(filter: &str, agent_name: &str) -> bool {
    canonical_agent_filter(filter).is_some_and(|filter| {
        filter == agent_name
            || filter == "claude" && agent_name.starts_with("claude-")
            || filter == "qoder" && QODER_AGENT_FAMILY.contains(&agent_name)
            || filter == "codebuddy" && CODEBUDDY_AGENT_FAMILY.contains(&agent_name)
    })
}

pub(crate) fn canonical_agent_target(target: &str) -> Option<&str> {
    match canonical_agent_filter(target)? {
        "claude" => Some("claude-cli"),
        "qoder" => Some("qoder-cli"),
        "codebuddy" => Some("codebuddy-cli"),
        target => Some(target),
    }
}

fn canonical_agent_filter(filter: &str) -> Option<&str> {
    match filter {
        "codex" => Some("codex-cli"),
        "codex-ide" => Some("codex-cli-ide"),
        "claude-ide" | "claude-code-ide" => Some("claude-cli-ide"),
        "kimi" | "kimi-code" => Some("kimi-cli"),
        "kimi-ide" | "kimi-code-ide" => Some("kimi-cli-ide"),
        "anti-gravity" => Some("antigravity"),
        "kiro-cli" => Some("kiro"),
        "qoder" => Some("qoder"),
        "qoder-cli" | "qodercli" => Some("qoder-cli"),
        "qoderwork" | "qoder-work" => Some("qoder-work"),
        "qoder-cn" | "qoder-cn-cli" | "qoderclicn" | "lingma" => Some("qoder-cn-cli"),
        "codebuddy" => Some("codebuddy"),
        "codebuddy-code" | "codebuddy-cli" => Some("codebuddy-cli"),
        "codebuddy-ide" => Some("codebuddy-ide"),
        "codebuddy-cn" | "codebuddycn" | "codebuddy-cn-ide" => Some("codebuddy-cn-ide"),
        "codebuddy-plugin" | "codebuddy-ide-plugin" | "coding-copilot" => {
            Some("codebuddy-ide-plugin")
        }
        "qocder-cli" | "qcoder-app" | "qoder-app" => None,
        other => Some(other),
    }
}

#[cfg(test)]
mod tests {
    use super::{agent_matches, canonical_agent_target};

    #[test]
    fn canonical_aliases_match_agent_names() {
        assert!(agent_matches("codex", "codex-cli"));
        assert!(agent_matches("codex-cli", "codex-cli"));
        assert!(agent_matches("codex-ide", "codex-cli-ide"));
        assert!(agent_matches("codex-cli-ide", "codex-cli-ide"));
        assert!(agent_matches("claude", "claude-cli"));
        assert!(agent_matches("claude-ide", "claude-cli-ide"));
        assert!(agent_matches("claude-code-ide", "claude-cli-ide"));
        assert!(agent_matches("claude-cli-ide", "claude-cli-ide"));
        assert!(agent_matches("kimi", "kimi-cli"));
        assert!(agent_matches("kimi-code", "kimi-cli"));
        assert!(agent_matches("kimi-ide", "kimi-cli-ide"));
        assert!(agent_matches("kimi-code-ide", "kimi-cli-ide"));
        assert!(agent_matches("kimi-cli-ide", "kimi-cli-ide"));
        assert!(agent_matches("anti-gravity", "antigravity"));
        assert!(agent_matches("kiro-cli", "kiro"));
        assert!(agent_matches("qoder", "qoder-cli"));
        assert!(agent_matches("qoder", "qoder-ide"));
        assert!(agent_matches("qoder", "qoder-work"));
        assert!(agent_matches("qoder", "qoder-cn-cli"));
        assert!(agent_matches("qoder", "qoder-cn-ide"));
        assert!(agent_matches("qoder", "qoder-cn-work"));
        assert!(agent_matches("qoder-cli", "qoder-cli"));
        assert!(agent_matches("qodercli", "qoder-cli"));
        assert!(agent_matches("qoder-ide", "qoder-ide"));
        assert!(agent_matches("qoderwork", "qoder-work"));
        assert!(agent_matches("qoder-work", "qoder-work"));
        assert!(agent_matches("qoder-cn", "qoder-cn-cli"));
        assert!(agent_matches("qoder-cn-cli", "qoder-cn-cli"));
        assert!(agent_matches("qoderclicn", "qoder-cn-cli"));
        assert!(agent_matches("lingma", "qoder-cn-cli"));
        assert!(agent_matches("qoder-cn-ide", "qoder-cn-ide"));
        assert!(agent_matches("qoder-cn-work", "qoder-cn-work"));
        assert!(agent_matches("codebuddy", "codebuddy-cli"));
        assert!(agent_matches("codebuddy", "codebuddy-ide"));
        assert!(agent_matches("codebuddy", "codebuddy-cn-ide"));
        assert!(agent_matches("codebuddy", "codebuddy-ide-plugin"));
        assert!(agent_matches("codebuddy", "workbuddy"));
        assert!(agent_matches("codebuddy-code", "codebuddy-cli"));
        assert!(agent_matches("codebuddy-cli", "codebuddy-cli"));
        assert!(agent_matches("codebuddy-ide", "codebuddy-ide"));
        assert!(agent_matches("codebuddy-cn", "codebuddy-cn-ide"));
        assert!(agent_matches("codebuddycn", "codebuddy-cn-ide"));
        assert!(agent_matches("codebuddy-plugin", "codebuddy-ide-plugin"));
        assert!(agent_matches("coding-copilot", "codebuddy-ide-plugin"));
    }

    #[test]
    fn unofficial_misspellings_do_not_match() {
        assert!(!agent_matches("qocder-cli", "qoder-cli"));
        assert!(!agent_matches("qcoder-app", "qoder-work"));
        assert!(!agent_matches("qoder-app", "qoder-work"));
        assert!(!agent_matches("qoder-cli", "qoder-work"));
        assert!(!agent_matches("codebuddy-cli", "codebuddy-ide"));
    }

    #[test]
    fn single_target_aliases_resolve_to_canonical_names() {
        assert_eq!(canonical_agent_target("codex"), Some("codex-cli"));
        assert_eq!(canonical_agent_target("codex-ide"), Some("codex-cli-ide"));
        assert_eq!(canonical_agent_target("claude"), Some("claude-cli"));
        assert_eq!(canonical_agent_target("claude-ide"), Some("claude-cli-ide"));
        assert_eq!(
            canonical_agent_target("claude-code-ide"),
            Some("claude-cli-ide")
        );
        assert_eq!(canonical_agent_target("anti-gravity"), Some("antigravity"));
        assert_eq!(canonical_agent_target("kimi"), Some("kimi-cli"));
        assert_eq!(canonical_agent_target("kimi-code"), Some("kimi-cli"));
        assert_eq!(canonical_agent_target("kimi-ide"), Some("kimi-cli-ide"));
        assert_eq!(canonical_agent_target("qoder"), Some("qoder-cli"));
        assert_eq!(canonical_agent_target("qoderwork"), Some("qoder-work"));
        assert_eq!(canonical_agent_target("qoder-cn"), Some("qoder-cn-cli"));
        assert_eq!(canonical_agent_target("codebuddy"), Some("codebuddy-cli"));
        assert_eq!(
            canonical_agent_target("codebuddy-code"),
            Some("codebuddy-cli")
        );
        assert_eq!(
            canonical_agent_target("codebuddy-cn"),
            Some("codebuddy-cn-ide")
        );
        assert_eq!(
            canonical_agent_target("codebuddy-plugin"),
            Some("codebuddy-ide-plugin")
        );
    }
}
