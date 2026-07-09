use std::collections::BTreeSet;
use std::io::Write;
use std::path::Path;

use sentra_lib::config::{
    sentra_config_file, sentra_hash_rule_dir, sentra_ti_rule_dir, sentra_yara_rule_dir,
};
use sentra_lib::risks::types::{CheckerConfig, LlmConfig, OnlineTiConfig, ScanCacheConfig};
use sentra_lib::risks::{RiskScanner, RuleDirectoryConfig, RuleLoadSummary, RuleType, ScanOptions};
use sentra_lib::{SentraError, SentraResult};

use crate::cli::args::ScanChecker;
use crate::cli::feedback::{self, Status};
use crate::cli::i18n::t;
use crate::core::bundled_rules::ensure_bundled_rules;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RuleLoadOutput {
    Interactive,
    Plain,
    Silent,
}

impl RuleLoadOutput {
    pub(crate) fn for_terminal(interactive: bool) -> Self {
        if interactive {
            Self::Interactive
        } else {
            Self::Plain
        }
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct CheckerSelection {
    hash: Option<bool>,
    yara: Option<bool>,
    ti: Option<bool>,
    llm: Option<bool>,
    online_ti: Option<bool>,
}

pub(crate) fn checker_selection(enabled: &BTreeSet<ScanChecker>) -> CheckerSelection {
    CheckerSelection {
        hash: Some(enabled.contains(&ScanChecker::Hash)),
        yara: Some(enabled.contains(&ScanChecker::Yara)),
        ti: Some(enabled.contains(&ScanChecker::Ti)),
        llm: Some(enabled.contains(&ScanChecker::Llm)),
        online_ti: Some(enabled.contains(&ScanChecker::OnlineTi)),
    }
}

pub(crate) fn build_scan_options(
    home: &Path,
    checkers: &CheckerSelection,
) -> SentraResult<ScanOptions> {
    build_scan_options_with_cache(home, checkers, false)
}

pub(crate) fn build_scan_options_with_cache(
    home: &Path,
    checkers: &CheckerSelection,
    no_cache: bool,
) -> SentraResult<ScanOptions> {
    let mut options = load_scan_config(home)?;
    if options.rules.is_none() {
        ensure_bundled_rules(home)?;
    }
    apply_default_rule_dirs(home, &mut options);
    options.cache = Some(ScanCacheConfig {
        path: None,
        skip_cache: no_cache,
    });
    options.checker = Some(CheckerConfig {
        enable_hash: checkers.hash,
        enable_yara: checkers.yara,
        enable_local_ti: checkers.ti,
        enable_llm: checkers.llm,
        enable_online_ti: checkers.online_ti,
    });
    Ok(options)
}

fn load_scan_config(home: &Path) -> SentraResult<ScanOptions> {
    let path = sentra_config_file(home);
    let Ok(content) = std::fs::read_to_string(&path) else {
        return Ok(ScanOptions::default());
    };
    let value: serde_json::Value =
        serde_json::from_str(&content).map_err(|err| SentraError::Message(err.to_string()))?;
    let raw = value.get("scan").unwrap_or(&value);
    let mut options: ScanOptions =
        serde_json::from_value(raw.clone()).map_err(|err| SentraError::Message(err.to_string()))?;
    merge_legacy_sentra_config(&value, &mut options);
    Ok(options)
}

fn apply_default_rule_dirs(home: &Path, options: &mut ScanOptions) {
    let rules = options
        .rules
        .get_or_insert_with(RuleDirectoryConfig::default);
    if rules.hash.is_none() {
        rules.hash = Some(sentra_hash_rule_dir(home));
    }
    if rules.yara.is_none() {
        rules.yara = Some(sentra_yara_rule_dir(home));
    }
    if rules.ti.is_none() {
        rules.ti = Some(sentra_ti_rule_dir(home));
    }
}

fn merge_legacy_sentra_config(config: &serde_json::Value, options: &mut ScanOptions) {
    if let Some(llm) = config.get("llm").and_then(|value| value.as_object()) {
        let options_llm = options.llm.get_or_insert_with(LlmConfig::default);
        if options_llm.api_url.is_none() {
            options_llm.api_url = llm
                .get("apiUrl")
                .or_else(|| llm.get("api"))
                .and_then(|value| value.as_str())
                .map(str::to_string);
        }
        if options_llm.api_key.is_none() {
            options_llm.api_key = llm
                .get("apiKey")
                .or_else(|| llm.get("key"))
                .and_then(|value| value.as_str())
                .map(str::to_string);
        }
        if options_llm.model.is_none() {
            options_llm.model = llm
                .get("model")
                .and_then(|value| value.as_str())
                .map(str::to_string);
        }
        if options_llm.protocol.is_none() {
            options_llm.protocol = llm
                .get("protocol")
                .cloned()
                .and_then(|value| serde_json::from_value(value).ok());
        }
        if options_llm.max_tokens.is_none() {
            options_llm.max_tokens = llm
                .get("maxTokens")
                .and_then(|value| value.as_u64())
                .map(|value| value as usize);
        }
        if options_llm.max_prompt_chars.is_none() {
            options_llm.max_prompt_chars = llm
                .get("maxPromptChars")
                .and_then(|value| value.as_u64())
                .map(|value| value as usize);
        }
        if options_llm.timeout_ms.is_none() {
            options_llm.timeout_ms = llm.get("timeoutMs").and_then(|value| value.as_u64());
        }
        if options_llm.stream.is_none() {
            options_llm.stream = llm.get("stream").and_then(|value| value.as_bool());
        }
        if options_llm.prompt.is_none() {
            options_llm.prompt = llm
                .get("prompt")
                .and_then(|value| value.as_str())
                .map(str::to_string);
        }
    }
    if options.rules.is_none()
        && let Some(rules) = config.get("rules").cloned()
    {
        options.rules = serde_json::from_value::<RuleDirectoryConfig>(rules).ok();
    }
    merge_legacy_online_ti_config(config, options);
}

fn merge_legacy_online_ti_config(config: &serde_json::Value, options: &mut ScanOptions) {
    let Some(rule) = config.get("rule").and_then(|value| value.as_object()) else {
        return;
    };
    let online_ti = options
        .online_ti
        .get_or_insert_with(OnlineTiConfig::default);
    if online_ti.cloudflare_url.is_none() {
        online_ti.cloudflare_url = rule
            .get("cloudflare_dns")
            .and_then(|value| value.as_str())
            .map(str::to_string);
    }
    if online_ti.threatbook_key.is_none() {
        online_ti.threatbook_key = rule
            .get("threatbook_key")
            .and_then(|value| value.as_str())
            .map(str::to_string);
    }
    if online_ti.threatbook_url.is_none() {
        online_ti.threatbook_url = rule
            .get("threatbook_url")
            .and_then(|value| value.as_str())
            .map(str::to_string);
    }
    if online_ti.chaitin_key.is_none() {
        online_ti.chaitin_key = rule
            .get("chaitin_key")
            .and_then(|value| value.as_str())
            .map(str::to_string);
    }
    if online_ti.chaitin_url.is_none() {
        online_ti.chaitin_url = rule
            .get("chaitin_url")
            .and_then(|value| value.as_str())
            .map(str::to_string);
    }
}

pub(crate) fn scan_progress_message(
    item_label: &str,
    current: usize,
    total: usize,
    name: &str,
) -> String {
    let percent = progress_percent(current, total);
    format!(
        "{} {item_label} {current}/{total} ({percent}%): {name}",
        t("Scan", "扫描")
    )
}

pub(crate) fn emit_scan_progress(
    item_label: &str,
    tty_label: &str,
    current: usize,
    total: usize,
    name: &str,
    interactive: bool,
    previous_width: &mut usize,
) -> SentraResult<()> {
    if !interactive {
        feedback::phase(
            Status::Running,
            scan_progress_message(item_label, current, total, name),
        );
        return Ok(());
    }

    let percent = progress_percent(current, total);
    let message = format!(
        "  {} {tty_label} {current}/{total} ({percent}%) {name}",
        Status::Running.symbol()
    );
    let padding = previous_width.saturating_sub(message.len());
    *previous_width = message.len();
    let mut stderr = std::io::stderr().lock();
    write!(stderr, "\r{message}{}", " ".repeat(padding))
        .map_err(|err| SentraError::io(None, err))?;
    stderr.flush().map_err(|err| SentraError::io(None, err))
}

pub(crate) fn load_scanner_rules(
    scanner: &mut RiskScanner,
    output: RuleLoadOutput,
) -> SentraResult<RuleLoadSummary> {
    load_rules_with_progress(scanner.enabled_rule_types(), output, |rule_type| {
        scanner.load_rule(rule_type)
    })
}

fn load_rules_with_progress(
    rule_types: Vec<RuleType>,
    output: RuleLoadOutput,
    mut load_rule: impl FnMut(RuleType) -> SentraResult<RuleLoadSummary>,
) -> SentraResult<RuleLoadSummary> {
    let mut previous_width = 0usize;
    let total = rule_types.len();
    let mut summary = RuleLoadSummary::default();
    for (index, rule_type) in rule_types.into_iter().enumerate() {
        let current = index + 1;
        match output {
            RuleLoadOutput::Interactive => {
                write_rule_load_progress(current, total, rule_type, &mut previous_width)?;
            }
            RuleLoadOutput::Plain => {
                feedback::phase(
                    Status::Running,
                    rule_load_progress_message(current, total, rule_type),
                );
            }
            RuleLoadOutput::Silent => {}
        }
        summary = merge_rule_load_summary(summary, load_rule(rule_type)?);
    }

    let message = rule_load_summary_message(&summary);
    match output {
        RuleLoadOutput::Interactive => {
            let padding = previous_width.saturating_sub(message.len());
            let mut stderr = std::io::stderr().lock();
            writeln!(stderr, "\r{message}{}", " ".repeat(padding))
                .map_err(|err| SentraError::io(None, err))?;
        }
        RuleLoadOutput::Plain => {
            feedback::phase(Status::Success, message);
        }
        RuleLoadOutput::Silent => {}
    }
    Ok(summary)
}

fn merge_rule_load_summary(mut left: RuleLoadSummary, right: RuleLoadSummary) -> RuleLoadSummary {
    left.yara = left.yara.max(right.yara);
    left.ti_ips = left.ti_ips.max(right.ti_ips);
    left.ti_domains = left.ti_domains.max(right.ti_domains);
    left.hash_blacklist = left.hash_blacklist.max(right.hash_blacklist);
    left.hash_whitelist = left.hash_whitelist.max(right.hash_whitelist);
    left
}

fn write_rule_load_progress(
    current: usize,
    total: usize,
    rule_type: RuleType,
    previous_width: &mut usize,
) -> SentraResult<()> {
    let message = rule_load_progress_message(current, total, rule_type);
    let message = format!("  {} {message}", Status::Running.symbol());
    let padding = previous_width.saturating_sub(message.len());
    *previous_width = message.len();
    let mut stderr = std::io::stderr().lock();
    write!(stderr, "\r{message}{}", " ".repeat(padding))
        .map_err(|err| SentraError::io(None, err))?;
    stderr.flush().map_err(|err| SentraError::io(None, err))
}

pub(crate) fn rule_load_progress_message(
    current: usize,
    total: usize,
    rule_type: RuleType,
) -> String {
    let percent = progress_percent(current, total);
    let stage = rule_load_stage_label(rule_type);
    format!(
        "{} {current}/{total} ({percent}%): {stage}",
        t("Load risk rules", "加载风险规则")
    )
}

fn rule_load_stage_label(rule_type: RuleType) -> &'static str {
    match rule_type {
        RuleType::Yara => t("Load YARA rules", "加载 YARA 规则"),
        RuleType::ThreatIntel => t("Load threat intel rules", "加载威胁情报规则"),
        RuleType::Hash => t("Load hash rules", "加载哈希规则"),
    }
}

pub(crate) fn rule_load_summary_message(summary: &RuleLoadSummary) -> String {
    format!(
        "{}: yara={} ti={} hash={}",
        t("Risk rules loaded", "风险规则已加载"),
        summary.yara,
        summary.ti_ips + summary.ti_domains,
        summary.hash_blacklist + summary.hash_whitelist
    )
}

pub(crate) fn finish_scan_progress(
    total: usize,
    singular_label: &str,
    plural_label: &str,
    interactive: bool,
    previous_width: usize,
) -> SentraResult<()> {
    if !interactive {
        return Ok(());
    }

    let label = if total == 1 {
        singular_label
    } else {
        plural_label
    };
    let message = format!("{} {total} {label}", t("Scanned", "已扫描"));
    let message = format!("  {} {message}", Status::Success.symbol());
    let padding = previous_width.saturating_sub(message.len());
    let mut stderr = std::io::stderr().lock();
    writeln!(stderr, "\r{message}{}", " ".repeat(padding)).map_err(|err| SentraError::io(None, err))
}

fn progress_percent(current: usize, total: usize) -> usize {
    if total == 0 {
        0
    } else {
        ((current as f64 / total as f64) * 100.0).round() as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_rules_with_progress_loads_each_stage_after_its_progress_message() {
        let mut loaded = Vec::new();

        let summary = load_rules_with_progress(
            vec![RuleType::Hash, RuleType::Yara, RuleType::ThreatIntel],
            RuleLoadOutput::Silent,
            |rule_type| {
                loaded.push(rule_type);
                Ok(RuleLoadSummary {
                    yara: usize::from(rule_type == RuleType::Yara),
                    ti_ips: usize::from(rule_type == RuleType::ThreatIntel),
                    ti_domains: 0,
                    hash_blacklist: usize::from(rule_type == RuleType::Hash),
                    hash_whitelist: 0,
                })
            },
        )
        .unwrap();

        assert_eq!(
            loaded,
            vec![RuleType::Hash, RuleType::Yara, RuleType::ThreatIntel]
        );
        assert_eq!(
            summary,
            RuleLoadSummary {
                yara: 1,
                ti_ips: 1,
                ti_domains: 0,
                hash_blacklist: 1,
                hash_whitelist: 0,
            }
        );
    }
}
