use std::path::PathBuf;

use sentra_lib::interfaces::AssetType;
use sentra_lib::{SentraError, SentraResult, agents::discover_agents};
use serde::Serialize;
use serde_json::Value;

use crate::args::{ListResource, OutputOptions};
use crate::i18n::t;
use crate::output::write_output;

pub(crate) async fn run(
    resource: ListResource,
    agent_filter: Option<&str>,
    output: OutputOptions,
) -> SentraResult<()> {
    let home = current_home()?;
    match resource {
        ListResource::Agent => write_output(agent_records(&home, agent_filter), &output, "Agents"),
        ListResource::Asset(asset_type) => {
            let mut assets = Vec::new();
            for agent in discover_agents(&home) {
                if agent_filter.is_some_and(|filter| filter != agent.name()) {
                    continue;
                }
                let agent_title = agent.title().to_string();
                for asset in agent.get_assets(asset_type)? {
                    let asset_type = asset.asset_type();
                    let data = asset.data_async().await?;
                    if data.as_array().is_some_and(|items| items.is_empty()) {
                        continue;
                    }
                    assets.push(AssetRecord {
                        asset_type,
                        kind: asset_type,
                        agent_name: asset.agent_name().to_string(),
                        agent_title: agent_title.clone(),
                        agent_home: asset.agent_home().to_path_buf(),
                        data,
                    });
                }
            }
            write_output(assets, &output, "Assets")
        }
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

fn agent_records(home: &std::path::Path, agent_filter: Option<&str>) -> Vec<AgentRecord> {
    discover_agents(home)
        .into_iter()
        .filter(|agent| agent_filter.is_none_or(|filter| filter == agent.name()))
        .map(|agent| AgentRecord {
            name: agent.name().to_string(),
            title: agent.title().to_string(),
            home: agent.home().to_path_buf(),
        })
        .collect()
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentRecord {
    name: String,
    title: String,
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
