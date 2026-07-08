use std::path::PathBuf;

use sentra_lib::agents::{Agent, discover_agents};
use sentra_lib::interfaces::{AssetMutationResult, AssetType, SkillData};
use sentra_lib::{SentraError, SentraResult};

use crate::i18n::t;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct AgentSkillInventory {
    pub(crate) agent_name: String,
    pub(crate) agent_title: String,
    pub(crate) agent_home: PathBuf,
    pub(crate) skills: Vec<SkillData>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SkillInventoryRow {
    pub(crate) skill: SkillData,
    pub(crate) installed: bool,
    pub(crate) source_agent: String,
    pub(crate) source_agent_title: String,
    pub(crate) source_agent_home: PathBuf,
}

pub(crate) fn grouped_skill_rows(
    inventories: &[AgentSkillInventory],
    selected_agent_index: usize,
) -> Vec<SkillInventoryRow> {
    let Some(selected) = inventories.get(selected_agent_index) else {
        return Vec::new();
    };
    let mut installed = selected
        .skills
        .iter()
        .cloned()
        .map(|skill| row_for_skill(selected, skill, true))
        .collect::<Vec<_>>();
    installed.sort_by(row_cmp);

    let installed_names = installed
        .iter()
        .map(|row| row.skill.name.clone())
        .collect::<std::collections::BTreeSet<_>>();
    let mut available = inventories
        .iter()
        .flat_map(|inventory| {
            inventory
                .skills
                .iter()
                .cloned()
                .map(|skill| row_for_skill(inventory, skill, false))
        })
        .filter(|row| !installed_names.contains(&row.skill.name))
        .collect::<Vec<_>>();
    available.sort_by(row_cmp);
    available.dedup_by(|left, right| left.skill.name == right.skill.name);

    installed.extend(available);
    installed
}

pub(crate) async fn collect_skill_inventories(
    home: &std::path::Path,
) -> SentraResult<Vec<AgentSkillInventory>> {
    let mut inventories = Vec::new();
    for agent in discover_agents(home) {
        inventories.push(inventory_for_agent(&agent).await?);
    }
    inventories.sort_by(|left, right| {
        left.agent_name
            .cmp(&right.agent_name)
            .then(left.agent_home.cmp(&right.agent_home))
    });
    Ok(inventories)
}

async fn inventory_for_agent(agent: &Agent) -> SentraResult<AgentSkillInventory> {
    let mut skills = Vec::new();
    for asset in agent.get_assets(AssetType::Skill)? {
        let data = asset.data_async().await?;
        let mut items: Vec<SkillData> =
            serde_json::from_value(data).map_err(|err| SentraError::Message(err.to_string()))?;
        skills.append(&mut items);
    }
    skills.sort_by(|left, right| left.name.cmp(&right.name).then(left.home.cmp(&right.home)));
    Ok(AgentSkillInventory {
        agent_name: agent.name().to_string(),
        agent_title: agent.title().to_string(),
        agent_home: agent.home().to_path_buf(),
        skills,
    })
}

pub(crate) fn install_skill_to_agent(
    home: &std::path::Path,
    agent_name: &str,
    skill: &SkillData,
) -> SentraResult<AssetMutationResult> {
    mutate_agent_skill(home, agent_name, |asset| {
        asset.set_skill_data(skill.clone())
    })
}

pub(crate) fn delete_skill_from_agent(
    home: &std::path::Path,
    agent_name: &str,
    skill: &SkillData,
) -> SentraResult<AssetMutationResult> {
    mutate_agent_skill(home, agent_name, |asset| asset.del_skill_data(skill))
}

fn mutate_agent_skill<F>(
    home: &std::path::Path,
    agent_name: &str,
    mutate: F,
) -> SentraResult<AssetMutationResult>
where
    F: Fn(&dyn sentra_lib::interfaces::ErasedAsset) -> SentraResult<AssetMutationResult>,
{
    let agent = discover_agents(home)
        .into_iter()
        .find(|agent| agent.name() == agent_name)
        .ok_or_else(|| {
            SentraError::Message(format!(
                "{}: {agent_name}",
                t("agent not found", "未找到 Agent")
            ))
        })?;
    let assets = agent.get_assets(AssetType::Skill)?;
    let asset = assets.first().ok_or_else(|| {
        SentraError::Message(format!(
            "{}: {agent_name}",
            t("agent has no skill asset", "Agent 没有技能资产")
        ))
    })?;
    mutate(asset.as_ref())
}

fn row_for_skill(
    inventory: &AgentSkillInventory,
    skill: SkillData,
    installed: bool,
) -> SkillInventoryRow {
    SkillInventoryRow {
        skill,
        installed,
        source_agent: inventory.agent_name.clone(),
        source_agent_title: inventory.agent_title.clone(),
        source_agent_home: inventory.agent_home.clone(),
    }
}

fn row_cmp(left: &SkillInventoryRow, right: &SkillInventoryRow) -> std::cmp::Ordering {
    left.skill
        .name
        .cmp(&right.skill.name)
        .then(left.source_agent.cmp(&right.source_agent))
        .then(left.skill.home.cmp(&right.skill.home))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn skill(name: &str, home: &str) -> SkillData {
        SkillData {
            name: name.to_string(),
            home: Some(PathBuf::from(home)),
            ..SkillData::default()
        }
    }

    fn agent(name: &str, skills: Vec<SkillData>) -> AgentSkillInventory {
        AgentSkillInventory {
            agent_name: name.to_string(),
            agent_title: name.to_string(),
            agent_home: PathBuf::from(format!("/home/{name}")),
            skills,
        }
    }

    #[test]
    fn grouped_skills_put_installed_first_then_available_difference() {
        let inventories = vec![
            agent(
                "codex",
                vec![skill("b", "/codex/b"), skill("a", "/codex/a")],
            ),
            agent(
                "sentra",
                vec![skill("c", "/sentra/c"), skill("a", "/sentra/a")],
            ),
        ];

        let rows = grouped_skill_rows(&inventories, 0);
        let rendered = rows
            .iter()
            .map(|row| {
                (
                    row.skill.name.as_str(),
                    row.installed,
                    row.source_agent.as_str(),
                )
            })
            .collect::<Vec<_>>();

        assert_eq!(
            rendered,
            vec![
                ("a", true, "codex"),
                ("b", true, "codex"),
                ("c", false, "sentra")
            ]
        );
    }

    #[test]
    fn available_duplicate_names_keep_stable_first_source() {
        let inventories = vec![
            agent("codex", vec![skill("mine", "/codex/mine")]),
            agent("b-agent", vec![skill("shared", "/b/shared")]),
            agent("a-agent", vec![skill("shared", "/a/shared")]),
        ];

        let rows = grouped_skill_rows(&inventories, 0);
        let available = rows
            .iter()
            .find(|row| !row.installed && row.skill.name == "shared")
            .unwrap();

        assert_eq!(available.source_agent, "a-agent");
        assert_eq!(
            available.skill.home.as_deref(),
            Some(PathBuf::from("/a/shared").as_path())
        );
    }
}
