pub(crate) fn agent_matches(filter: &str, agent_name: &str) -> bool {
    canonical_agent_filter(filter).is_some_and(|filter| {
        filter == agent_name || filter == "claude" && agent_name.starts_with("claude-")
    })
}

pub(crate) fn canonical_agent_target(target: &str) -> Option<&str> {
    match canonical_agent_filter(target)? {
        "claude" => Some("claude-cli"),
        target => Some(target),
    }
}

fn canonical_agent_filter(filter: &str) -> Option<&str> {
    match filter {
        "codex" => Some("codex-cli"),
        "codex-ide" => Some("codex-cli-ide"),
        "claude-ide" | "claude-code-ide" => Some("claude-cli-ide"),
        "anti-gravity" => Some("antigravity"),
        "kiro-cli" => Some("kiro"),
        "qoder-cli" | "qodercli" => Some("qoder"),
        "qoderclicn" | "lingma" => Some("qoder-cn"),
        "codebuddy-code" => Some("codebuddy"),
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
        assert!(agent_matches("anti-gravity", "antigravity"));
        assert!(agent_matches("kiro-cli", "kiro"));
        assert!(agent_matches("qoder-cli", "qoder"));
        assert!(agent_matches("qodercli", "qoder"));
        assert!(agent_matches("qoderclicn", "qoder-cn"));
        assert!(agent_matches("lingma", "qoder-cn"));
        assert!(agent_matches("codebuddy-code", "codebuddy"));
    }

    #[test]
    fn unofficial_misspellings_do_not_match() {
        assert!(!agent_matches("qocder-cli", "qoder"));
        assert!(!agent_matches("qcoder-app", "qoderwork"));
        assert!(!agent_matches("qoder-app", "qoderwork"));
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
    }
}
