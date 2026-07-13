use std::collections::BTreeSet;
use std::io::IsTerminal;
use std::path::PathBuf;
use std::sync::Arc;

use futures::StreamExt;
use sentra_lib::interfaces::{AssetType, CronData, MemoryData, ProviderData, SkillData};
use sentra_lib::risks::{RiskAsset, RiskScanner, ScanOptions, ScanReport};
use sentra_lib::{SentraError, SentraResult, agents::discover_agents};
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::cli::args::{OutputOptions, ScanChecker, ScanResource};
use crate::cli::feedback::{self, Status};
use crate::cli::i18n::t;
use crate::cli::output::write_output;
use crate::core::model;
use crate::core::scan_support::{
    RuleLoadOutput, build_scan_options_with_cache, checker_selection, emit_scan_progress,
    finish_scan_progress, load_scanner_rules,
};

pub(crate) async fn run(
    resource: ScanResource,
    path: Option<PathBuf>,
    agent_filters: Vec<String>,
    enabled_checkers: BTreeSet<ScanChecker>,
    no_cache: bool,
    output: OutputOptions,
) -> SentraResult<()> {
    let home = current_home()?;
    let resource_name = resource.to_string();
    let mut reports = Vec::new();
    let checkers = checker_selection(&enabled_checkers);
    let mut options = build_scan_options_with_cache(&home, &checkers, no_cache)?;
    feedback::context(
        t("Scan assets", "扫描资产"),
        &[
            (t("Resource", "资源"), resource_name.clone()),
            (
                t("Target", "目标"),
                path.as_ref()
                    .map(|path| path.display().to_string())
                    .unwrap_or_else(|| t("discovered agents", "已发现的 Agent").to_string()),
            ),
        ],
    );
    if should_prompt_for_sentra_model(&enabled_checkers, &options)
        && std::io::stdout().is_terminal()
    {
        feedback::phase(
            Status::Warning,
            t(
                "scan --with-llm requires a Sentra model configuration.",
                "scan --with-llm 需要 Sentra 模型配置。",
            ),
        );
        if !model::configure_sentra_model_from_all_gateways_at(&home).await? {
            return Err(SentraError::Message(
                t(
                    "scan --with-llm requires a Sentra model configuration",
                    "scan --with-llm 需要 Sentra 模型配置",
                )
                .to_string(),
            ));
        }
        options = build_scan_options_with_cache(&home, &checkers, no_cache)?;
        if !sentra_llm_config_complete(&options) {
            return Err(SentraError::Message(
                t(
                    "Sentra model configuration is incomplete",
                    "Sentra 模型配置不完整",
                )
                .to_string(),
            ));
        }
    }
    let targets = if let Some(path) = path {
        feedback::phase(
            Status::Running,
            format!(
                "{} {resource_name} {}: {}",
                t("Discover", "发现"),
                t("targets from", "目标来源"),
                path.display()
            ),
        );
        collect_path_skill_targets(path).await?
    } else {
        feedback::phase(
            Status::Running,
            format!(
                "{} {resource_name} {}",
                t("Discover", "发现"),
                t("targets from agents", "目标，来源为 Agent")
            ),
        );
        collect_agent_targets(resource, &home, &agent_filters).await?
    };
    feedback::phase(
        Status::Success,
        format!(
            "{} {} {resource_name} {}",
            t("Discovered", "已发现"),
            targets.len(),
            t("target(s)", "个目标")
        ),
    );

    let interactive_progress = std::io::stderr().is_terminal();
    let mut scanner = RiskScanner::new(options)?;
    load_scanner_rules(
        &mut scanner,
        RuleLoadOutput::for_terminal(interactive_progress),
    )?;
    let scanner = Arc::new(scanner);
    let mut progress_width = 0usize;
    let total = targets.len();
    let concurrency = scanner.concurrency();
    let mut completed = 0usize;
    let mut stream =
        futures::stream::iter(targets.into_iter().enumerate().map(|(index, target)| {
            let scanner = Arc::clone(&scanner);
            async move {
                let display_name = target.display_name().to_string();
                let task = tokio::spawn(async move {
                    let record = scan_target_record(target, scanner.as_ref()).await?;
                    Ok::<_, SentraError>(record)
                });
                let record = task
                    .await
                    .map_err(|err| SentraError::Message(err.to_string()))??;
                Ok::<_, SentraError>((index, display_name, record))
            }
        }))
        .buffer_unordered(concurrency);

    while let Some(result) = stream.next().await {
        let (index, display_name, record) = result?;
        completed += 1;
        emit_scan_progress(
            "target",
            t("Scanning targets", "正在扫描目标"),
            completed,
            total,
            &display_name,
            interactive_progress,
            &mut progress_width,
        )?;
        reports.push((index, record));
    }
    reports.sort_by_key(|(index, _)| *index);
    let reports = reports
        .into_iter()
        .map(|(_, record)| record)
        .collect::<Vec<_>>();
    let singular = format!("{resource_name} target");
    let plural = format!("{resource_name} targets");
    finish_scan_progress(
        total,
        &singular,
        &plural,
        interactive_progress,
        progress_width,
    )?;

    write_output(reports, &output, "Scan Results")
}

async fn scan_target_record(target: ScanTarget, scanner: &RiskScanner) -> SentraResult<ScanRecord> {
    let report = target.scan(scanner).await?;
    let metadata = target.metadata();
    Ok(ScanRecord {
        user: metadata.user.clone(),
        agent: metadata.agent.clone(),
        agent_title: metadata.agent_title.clone(),
        agent_home: metadata.agent_home.clone(),
        kind: target.asset_type(),
        name: target.record_name().to_string(),
        report,
    })
}

async fn collect_agent_targets(
    resource: ScanResource,
    home: &std::path::Path,
    agent_filters: &[String],
) -> SentraResult<Vec<ScanTarget>> {
    let mut targets = Vec::new();
    for agent in discover_agents(home) {
        if !agent_filters.is_empty()
            && !agent_filters
                .iter()
                .any(|filter| agent_matches(filter, agent.name()))
        {
            continue;
        }

        feedback::phase(
            Status::Running,
            format!(
                "{} {} {} {}",
                t("Collect", "收集"),
                resource,
                t("from", "来源"),
                agent.name()
            ),
        );
        let agent_home = agent.home().to_path_buf();
        let agent_name = agent.name().to_string();
        let agent_title = agent.title().to_string();
        let user = user_from_agent_home(&agent_home);
        for asset in agent.get_assets(scan_asset_type(resource))? {
            let metadata = ScanTargetMetadata {
                user: user.clone(),
                agent: agent_name.clone(),
                agent_title: agent_title.clone(),
                agent_home: agent_home.clone(),
            };
            push_agent_scan_targets(resource, asset.data_async().await?, metadata, &mut targets)?;
        }
    }
    Ok(targets)
}

async fn collect_path_skill_targets(path: PathBuf) -> SentraResult<Vec<ScanTarget>> {
    let path = canonical_scan_path(&path)?;
    let skills = sentra_lib::collect_skills_from_dir_async(&path).await?;
    Ok(skills
        .into_iter()
        .map(|skill| {
            let skill_home = skill.home.clone().unwrap_or_else(|| path.clone());
            SkillScanTarget {
                metadata: ScanTargetMetadata {
                    user: "path".to_string(),
                    agent: "path".to_string(),
                    agent_title: skill_home.display().to_string(),
                    agent_home: skill_home,
                },
                skill,
            }
        })
        .map(ScanTarget::Skill)
        .collect())
}

fn user_from_agent_home(agent_home: &std::path::Path) -> String {
    agent_home
        .parent()
        .and_then(|path| path.file_name())
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_default()
}

fn canonical_scan_path(path: &std::path::Path) -> SentraResult<PathBuf> {
    let path = path
        .canonicalize()
        .map_err(|err| SentraError::io(Some(path.to_path_buf()), err))?;
    Ok(clean_canonical_path(path))
}

fn clean_canonical_path(path: PathBuf) -> PathBuf {
    #[cfg(windows)]
    {
        let value = path.to_string_lossy();
        if let Some(rest) = value.strip_prefix(r"\\?\UNC\") {
            return PathBuf::from(format!(r"\\{rest}"));
        }
        if let Some(rest) = value.strip_prefix(r"\\?\") {
            return PathBuf::from(rest);
        }
    }
    path
}

fn push_agent_scan_targets(
    resource: ScanResource,
    data: serde_json::Value,
    metadata: ScanTargetMetadata,
    targets: &mut Vec<ScanTarget>,
) -> SentraResult<()> {
    match resource {
        ScanResource::Skill => {
            targets.extend(
                parse_asset_items::<SkillData>(data)?
                    .into_iter()
                    .map(|skill| {
                        ScanTarget::Skill(SkillScanTarget {
                            metadata: metadata.clone(),
                            skill,
                        })
                    }),
            );
        }
        ScanResource::Cron => {
            targets.extend(
                parse_asset_items::<CronData>(data)?
                    .into_iter()
                    .map(|cron| {
                        ScanTarget::Cron(CronScanTarget {
                            metadata: metadata.clone(),
                            cron,
                        })
                    }),
            );
        }
        ScanResource::Memory => {
            targets.extend(
                parse_asset_items::<MemoryData>(data)?
                    .into_iter()
                    .map(|memory| {
                        ScanTarget::Memory(MemoryScanTarget {
                            metadata: metadata.clone(),
                            memory,
                        })
                    }),
            );
        }
        ScanResource::Provider => {
            targets.extend(
                parse_asset_items::<ProviderData>(data)?
                    .into_iter()
                    .map(|provider| {
                        ScanTarget::Provider(ProviderScanTarget {
                            metadata: metadata.clone(),
                            provider,
                        })
                    }),
            );
        }
    }
    Ok(())
}

fn parse_asset_items<TData: DeserializeOwned>(data: serde_json::Value) -> SentraResult<Vec<TData>> {
    serde_json::from_value(data).map_err(|err| SentraError::Message(err.to_string()))
}

fn scan_asset_type(resource: ScanResource) -> AssetType {
    match resource {
        ScanResource::Skill => AssetType::Skill,
        ScanResource::Cron => AssetType::Cron,
        ScanResource::Memory => AssetType::Memory,
        ScanResource::Provider => AssetType::Provider,
    }
}

fn current_home() -> SentraResult<std::path::PathBuf> {
    home::home_dir().ok_or_else(|| {
        SentraError::Message(
            t(
                "could not determine current user home",
                "无法确定当前用户主目录",
            )
            .to_string(),
        )
    })
}

fn agent_matches(filter: &str, agent_name: &str) -> bool {
    filter == agent_name || (filter == "claude" && agent_name.starts_with("claude-"))
}

fn should_prompt_for_sentra_model(
    enabled_checkers: &BTreeSet<ScanChecker>,
    options: &ScanOptions,
) -> bool {
    enabled_checkers.contains(&ScanChecker::Llm) && !sentra_llm_config_complete(options)
}

fn sentra_llm_config_complete(options: &ScanOptions) -> bool {
    let Some(llm) = &options.llm else {
        return false;
    };
    llm.api_url
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty())
        && llm
            .api_key
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty())
        && llm
            .model
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty())
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ScanRecord {
    user: String,
    agent: String,
    agent_title: String,
    agent_home: PathBuf,
    #[serde(rename = "type")]
    kind: AssetType,
    name: String,
    report: ScanReport,
}

#[derive(Clone)]
struct ScanTargetMetadata {
    user: String,
    agent: String,
    agent_title: String,
    agent_home: PathBuf,
}

enum ScanTarget {
    Skill(SkillScanTarget),
    Cron(CronScanTarget),
    Memory(MemoryScanTarget),
    Provider(ProviderScanTarget),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::scan_support::{build_scan_options, checker_selection};
    use sentra_lib::protocol::WireProtocol;
    use sentra_lib::risks::LlmConfig;

    #[test]
    fn llm_scan_with_incomplete_model_config_prompts_for_sentra_model() {
        let enabled = [ScanChecker::Llm].into_iter().collect();
        let options = ScanOptions::default();

        assert!(should_prompt_for_sentra_model(&enabled, &options));
    }

    #[test]
    fn llm_scan_with_complete_model_config_does_not_prompt_for_sentra_model() {
        let enabled = [ScanChecker::Llm].into_iter().collect();
        let options = ScanOptions {
            llm: Some(LlmConfig {
                api_url: Some("https://api.example.test/v1".to_string()),
                api_key: Some("sk-test".to_string()),
                model: Some("gpt-test".to_string()),
                ..LlmConfig::default()
            }),
            ..ScanOptions::default()
        };

        assert!(!should_prompt_for_sentra_model(&enabled, &options));
    }

    #[test]
    fn scan_without_llm_does_not_prompt_for_sentra_model() {
        let enabled = BTreeSet::new();
        let options = ScanOptions::default();

        assert!(!should_prompt_for_sentra_model(&enabled, &options));
    }

    #[test]
    fn reloaded_scan_options_read_model_config_written_by_sentra_provider() {
        let dir = tempfile::tempdir().unwrap();
        let sentra_home = dir.path().join(".sentra");
        std::fs::create_dir_all(&sentra_home).unwrap();
        std::fs::write(
            sentra_home.join("config.json"),
            serde_json::json!({
                "llm": {
                    "api": "https://api.example.test/v1",
                    "key": "sk-test",
                    "model": "gpt-test",
                    "protocol": "responses"
                }
            })
            .to_string(),
        )
        .unwrap();

        let enabled = [ScanChecker::Llm].into_iter().collect();
        let options = build_scan_options(dir.path(), &checker_selection(&enabled)).unwrap();

        assert!(sentra_llm_config_complete(&options));
        let llm = options.llm.unwrap();
        assert_eq!(llm.api_url.as_deref(), Some("https://api.example.test/v1"));
        assert_eq!(llm.api_key.as_deref(), Some("sk-test"));
        assert_eq!(llm.model.as_deref(), Some("gpt-test"));
        assert_eq!(llm.protocol, Some(WireProtocol::Responses));
    }

    #[test]
    fn scan_options_merge_legacy_online_ti_rule_config() {
        let dir = tempfile::tempdir().unwrap();
        let sentra_home = dir.path().join(".sentra");
        std::fs::create_dir_all(&sentra_home).unwrap();
        std::fs::write(
            sentra_home.join("config.json"),
            serde_json::json!({
                "rule": {
                    "cloudflare_dns": "https://cloudflare.example/dns-query",
                    "threatbook_key": "tb-key",
                    "chaitin_key": "ct-key"
                }
            })
            .to_string(),
        )
        .unwrap();

        let enabled = [ScanChecker::OnlineTi].into_iter().collect();
        let options = build_scan_options(dir.path(), &checker_selection(&enabled)).unwrap();

        let online_ti = options.online_ti.unwrap();
        assert_eq!(
            online_ti.cloudflare_url.as_deref(),
            Some("https://cloudflare.example/dns-query")
        );
        assert_eq!(online_ti.threatbook_key.as_deref(), Some("tb-key"));
        assert_eq!(online_ti.chaitin_key.as_deref(), Some("ct-key"));
    }
}

impl ScanTarget {
    fn asset_type(&self) -> AssetType {
        match self {
            Self::Skill(_) => AssetType::Skill,
            Self::Cron(_) => AssetType::Cron,
            Self::Memory(_) => AssetType::Memory,
            Self::Provider(_) => AssetType::Provider,
        }
    }

    fn metadata(&self) -> &ScanTargetMetadata {
        match self {
            Self::Skill(target) => &target.metadata,
            Self::Cron(target) => &target.metadata,
            Self::Memory(target) => &target.metadata,
            Self::Provider(target) => &target.metadata,
        }
    }

    fn display_name(&self) -> &str {
        match self {
            Self::Skill(target) => &target.skill.name,
            Self::Cron(target) => &target.cron.id,
            Self::Memory(target) => &target.memory.name,
            Self::Provider(target) => &target.provider.name,
        }
    }

    fn record_name(&self) -> &str {
        match self {
            Self::Skill(target) => &target.skill.name,
            Self::Cron(target) if !target.cron.name.is_empty() => &target.cron.name,
            Self::Cron(target) => &target.cron.id,
            Self::Memory(target) => &target.memory.name,
            Self::Provider(target) => &target.provider.name,
        }
    }

    async fn scan(&self, scanner: &RiskScanner) -> SentraResult<ScanReport> {
        match self {
            Self::Skill(target) => scanner.scan(RiskAsset::from(&target.skill)).await,
            Self::Cron(target) => scanner.scan(RiskAsset::from(&target.cron)).await,
            Self::Memory(target) => scanner.scan(RiskAsset::from(&target.memory)).await,
            Self::Provider(target) => scanner.scan(RiskAsset::from(&target.provider)).await,
        }
    }
}

struct SkillScanTarget {
    metadata: ScanTargetMetadata,
    skill: SkillData,
}

struct CronScanTarget {
    metadata: ScanTargetMetadata,
    cron: CronData,
}

struct MemoryScanTarget {
    metadata: ScanTargetMetadata,
    memory: MemoryData,
}

struct ProviderScanTarget {
    metadata: ScanTargetMetadata,
    provider: ProviderData,
}
