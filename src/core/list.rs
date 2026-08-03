use std::path::{Path, PathBuf};

use sentra_lib::interfaces::{AssetType, McpData};
use sentra_lib::{
    SentraError, SentraResult,
    agents::{Agent, discover_agents},
};
use serde::Serialize;
use serde_json::Value;

use crate::cli::args::{ListResource, OutputOptions};
use crate::cli::i18n::t;
use crate::cli::output::write_output;
use crate::core::agent_filter::{agent_matches, canonical_agent_target};

const ALL_ASSET_TYPES: &[AssetType] = &[
    AssetType::Skill,
    AssetType::Mcp,
    AssetType::Provider,
    AssetType::Memory,
    AssetType::Cron,
    AssetType::Plugin,
    AssetType::Process,
];

pub(crate) async fn run(
    resource: ListResource,
    home: &Path,
    agent_filter: Option<&str>,
    output: OutputOptions,
) -> SentraResult<()> {
    match resource {
        ListResource::All => write_output(
            asset_records(home, agent_filter, ALL_ASSET_TYPES).await?,
            &output,
            "Assets",
        ),
        ListResource::Agent => write_output(agent_records(home, agent_filter)?, &output, "Agents"),
        ListResource::Asset(asset_type) => write_output(
            asset_records(home, agent_filter, &[asset_type]).await?,
            &output,
            "Assets",
        ),
    }
}

async fn asset_records(
    home: &Path,
    agent_filter: Option<&str>,
    asset_types: &[AssetType],
) -> SentraResult<Vec<AssetRecord>> {
    if let Some(records) = direct_mcp_asset_records(home, agent_filter, asset_types)? {
        return Ok(records);
    }

    let agents = discover_agents(home)
        .into_iter()
        .filter(|agent| agent_filter.is_none_or(|filter| agent_matches(filter, agent.name())))
        .collect::<Vec<_>>();
    let mut records = Vec::new();
    for &requested_type in asset_types {
        for agent in &agents {
            let agent_title = agent.title().to_string();
            for asset in agent.get_assets(requested_type)? {
                let asset_type = asset.asset_type();
                let data = hydrate_asset_data(asset_type, asset.data_async().await?)?;
                if data.as_array().is_some_and(|items| items.is_empty()) {
                    continue;
                }
                records.push(AssetRecord {
                    asset_type,
                    kind: asset_type,
                    agent_name: asset.agent_name().to_string(),
                    agent_title: agent_title.clone(),
                    agent_home: asset.agent_home().to_path_buf(),
                    data,
                });
            }
        }
    }
    Ok(records)
}

fn hydrate_asset_data(asset_type: AssetType, data: Value) -> SentraResult<Value> {
    if !matches!(asset_type, AssetType::Mcp) {
        return Ok(data);
    }
    let items = serde_json::from_value::<Vec<McpData>>(data)
        .map_err(|err| SentraError::Message(err.to_string()))?
        .into_iter()
        .map(sentra_lib::hydrate_mcp_tools)
        .collect::<Vec<_>>();
    serde_json::to_value(items).map_err(|err| SentraError::Message(err.to_string()))
}

fn direct_mcp_asset_records(
    home: &Path,
    agent_filter: Option<&str>,
    asset_types: &[AssetType],
) -> SentraResult<Option<Vec<AssetRecord>>> {
    if asset_types != [AssetType::Mcp] {
        return Ok(None);
    }
    if let Some(filter) = agent_filter
        && !matches!(canonical_agent_target(filter), Some("codex-cli"))
    {
        return Ok(None);
    }
    let agent_home = home.join(".codex");
    let path = agent_home.join("config.toml");
    let Ok(content) = std::fs::read_to_string(&path) else {
        return Ok(Some(Vec::new()));
    };
    let Ok(value) = toml::from_str::<toml::Value>(&content) else {
        return Ok(Some(Vec::new()));
    };
    let json = serde_json::to_value(value).map_err(|err| SentraError::Message(err.to_string()))?;
    let raw = json
        .get("mcp_servers")
        .unwrap_or(&serde_json::Value::Null)
        .to_string();
    let items = sentra_lib::parse_mcp_servers_json(&raw, None)?
        .into_iter()
        .map(sentra_lib::hydrate_mcp_tools)
        .collect::<Vec<_>>();
    if items.is_empty() {
        return Ok(Some(Vec::new()));
    }
    let data = serde_json::to_value(items).map_err(|err| SentraError::Message(err.to_string()))?;
    Ok(Some(vec![AssetRecord {
        asset_type: AssetType::Mcp,
        kind: AssetType::Mcp,
        agent_name: "codex-cli".to_string(),
        agent_title: "Codex CLI".to_string(),
        agent_home,
        data,
    }]))
}

pub(crate) fn resolve_home(home: Option<&Path>) -> SentraResult<PathBuf> {
    match home {
        Some(home) => Ok(home.to_path_buf()),
        None => current_home(),
    }
}

fn current_home() -> SentraResult<PathBuf> {
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

fn agent_records(home: &Path, agent_filter: Option<&str>) -> SentraResult<Vec<AgentRecord>> {
    let mut records = Vec::new();
    for agent in discover_agents(home)
        .into_iter()
        .filter(|agent| agent_filter.is_none_or(|filter| agent_matches(filter, agent.name())))
    {
        records.push(AgentRecord {
            name: agent.name().to_string(),
            title: agent.title().to_string(),
            installed: agent_installed(&agent)?,
            home: agent.home().to_path_buf(),
        });
    }
    Ok(records)
}

fn agent_installed(agent: &Agent) -> SentraResult<bool> {
    for asset in agent.get_assets(AssetType::Meta)? {
        let data = asset.data()?;
        if let Some(installed) = data.get("installed").and_then(|value| value.as_bool()) {
            return Ok(installed);
        }
    }
    Ok(false)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentRecord {
    name: String,
    title: String,
    installed: bool,
    home: PathBuf,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AssetRecord {
    asset_type: AssetType,
    #[serde(rename = "type")]
    kind: AssetType,
    agent_name: String,
    agent_title: String,
    agent_home: PathBuf,
    data: Value,
}
