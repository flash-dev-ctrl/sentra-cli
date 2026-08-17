use std::collections::{BTreeMap, BTreeSet};
use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::Duration;

use crossterm::cursor::{Hide, Show};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap};
use ratatui::{Frame, Terminal};
use sentra_lib::agents::{
    Agent, AgentDiscoveryOptions, discover_agents_matching_with_options,
    discover_agents_with_asset_and_options,
};
use sentra_lib::interfaces::{AssetType, ProviderData, ProviderModel, ProviderProbeRequest};
use sentra_lib::protocol::{
    ModelRequestParams, WireProtocol, probe_model_request, probe_model_request_with_prompt,
    validate_model_text_response,
};
use sentra_lib::{SentraError, SentraResult};
use serde::Serialize;

use crate::cli::args::{ModelAction, OutputOptions};
use crate::cli::feedback::{self, Status};
use crate::cli::i18n::{t, yes_no};
use crate::cli::output::write_output;
use crate::core::agent_filter::canonical_agent_target;
use crate::tui::theme;
use unicode_width::UnicodeWidthStr;

pub(crate) async fn run(action: ModelAction) -> SentraResult<()> {
    match action {
        ModelAction::Interactive => interactive().await,
        ModelAction::List { output } => list(output).await,
        ModelAction::Set {
            agent,
            base_url,
            api_key,
            model,
            protocol,
        } => set(agent, base_url, api_key, model, protocol),
        ModelAction::Delete { agent, base_url } => delete(agent, base_url),
    }
}

async fn interactive() -> SentraResult<()> {
    if !std::io::stdout().is_terminal() {
        return list(OutputOptions::default()).await;
    }

    let (records, catalog) = collect_model_inventory().await?;
    let submitted = run_model_tui(&records, catalog)?;
    if let Some(input) = submitted {
        let home = current_home()?;
        set_at(
            &home,
            input.agent,
            input.base_url,
            input.api_key,
            input.model,
            input.protocol,
            input.provider_name,
            input.raw_provider_id,
        )?;
    }
    Ok(())
}

pub(crate) async fn configure_sentra_model_from_all_gateways_at(home: &Path) -> SentraResult<bool> {
    if !std::io::stdout().is_terminal() {
        return Ok(false);
    }

    let (records, catalog) = collect_model_inventory_at(home).await?;
    let catalog = sentra_only_catalog(catalog);
    let submitted = run_model_tui(&records, catalog)?;
    if let Some(mut input) = submitted {
        input.agent = "sentra".to_string();
        set_at(
            home,
            input.agent,
            input.base_url,
            input.api_key,
            input.model,
            input.protocol,
            None,
            None,
        )?;
        return Ok(true);
    }
    Ok(false)
}

async fn list(output: OutputOptions) -> SentraResult<()> {
    write_output(collect_model_records().await?, &output, "Models")
}

async fn collect_model_records() -> SentraResult<Vec<ModelRecord>> {
    let home = current_home()?;
    collect_model_records_at(&home).await
}

async fn collect_model_records_at(home: &Path) -> SentraResult<Vec<ModelRecord>> {
    collect_model_inventory_at(home)
        .await
        .map(|(records, _)| records)
}

async fn collect_model_inventory() -> SentraResult<(Vec<ModelRecord>, ModelCatalog)> {
    let home = current_home()?;
    collect_model_inventory_at(&home).await
}

async fn collect_model_inventory_at(home: &Path) -> SentraResult<(Vec<ModelRecord>, ModelCatalog)> {
    collect_model_inventory_from_agents(fast_provider_agents(home)?).await
}

fn fast_provider_agents(home: &Path) -> SentraResult<Vec<Agent>> {
    Ok(discover_agents_with_asset_and_options(
        home,
        AssetType::Provider,
        fast_agent_discovery_options(),
    ))
}

fn fast_agent_discovery_options() -> AgentDiscoveryOptions {
    AgentDiscoveryOptions {
        skip_install_probe: true,
    }
}

async fn collect_model_inventory_from_agents(
    discovered_agents: Vec<Agent>,
) -> SentraResult<(Vec<ModelRecord>, ModelCatalog)> {
    let mut records = Vec::new();
    let mut catalog_agents = Vec::new();
    let mut gateways = Vec::<ProviderRecord>::new();
    let mut seen_gateways = BTreeSet::new();
    for agent in discovered_agents {
        let agent_title = agent.title().to_string();
        let collected: SentraResult<()> = async {
            let mut probe_requests = Vec::new();
            let mut supports_provider_write = false;
            for asset in agent.get_assets(AssetType::Provider)? {
                supports_provider_write |= asset.supports_provider_write();
                let requests = asset.provider_requests(PROBE_MODEL_PLACEHOLDER);
                if !requests.is_empty() {
                    probe_requests.extend(requests.clone());
                }
                for provider in provider_items(asset.runtime_data_async().await?)? {
                    if provider.provider_type != sentra_lib::interfaces::ProviderType::Gateway {
                        continue;
                    }
                    let provider_type = provider_type_label(provider.provider_type);
                    let account = provider_account_label(provider.account.as_ref());
                    let base_url = provider.base_url.clone().unwrap_or_default();
                    let has_api_key = provider.api_key.is_some();
                    if base_url.trim().is_empty() {
                        continue;
                    }
                    let raw_provider_id = provider
                        .raw_provider_id
                        .clone()
                        .or_else(|| provider.provider_id.clone());
                    let gateway_key = provider_key(&base_url, provider.api_key.as_deref());
                    let models = provider
                        .models
                        .iter()
                        .filter(|model| model.enabled)
                        .map(|model| ModelChoice {
                            id: model.id.clone(),
                            name: model.name.clone().unwrap_or_else(|| model.id.clone()),
                            enabled: model.enabled,
                            status: ModelProbeStatus::Testing,
                            protocol: provider.protocol,
                        })
                        .collect::<Vec<_>>();

                    push_model_records(
                        &mut records,
                        &agent,
                        &agent_title,
                        &provider,
                        &provider_type,
                        &account,
                        base_url.clone(),
                        has_api_key,
                        &models,
                    );

                    if !seen_gateways.insert(gateway_key) {
                        continue;
                    }
                    let provider_record = ProviderRecord {
                        name: provider.name,
                        raw_provider_id,
                        base_url: base_url.clone(),
                        api_key: provider.api_key,
                        enabled: provider.enabled,
                        models,
                        temporary: false,
                    };
                    gateways.push(provider_record);
                }
            }
            if supports_provider_write {
                catalog_agents.push(AgentProviderEntry {
                    agent_name: agent.name().to_string(),
                    agent_title: agent.title().to_string(),
                    probe_requests,
                });
            }
            Ok(())
        }
        .await;
        if let Err(err) = collected {
            feedback::status_line(
                Status::Warning,
                format!(
                    "{}: {}",
                    agent_title,
                    t(
                        "provider inventory collection failed, agent skipped",
                        "供应商信息采集失败，已跳过该 Agent"
                    )
                ),
            );
            feedback::metadata(&[(t("Cause", "原因"), err.to_string())]);
        }
    }
    Ok((
        records,
        ModelCatalog {
            agents: catalog_agents,
            gateways,
        },
    ))
}

fn push_model_records(
    records: &mut Vec<ModelRecord>,
    agent: &Agent,
    agent_title: &str,
    provider: &ProviderData,
    provider_type: &str,
    account: &str,
    base_url: String,
    has_api_key: bool,
    models: &[ModelChoice],
) {
    for model in models.iter().filter(|model| model.enabled) {
        records.push(ModelRecord {
            agent_name: agent.name().to_string(),
            agent_title: agent_title.to_string(),
            agent_home: agent.home().to_path_buf(),
            provider_name: provider.name.clone(),
            provider_type: provider_type.to_string(),
            account: account.to_string(),
            base_url: Some(base_url.clone()),
            enabled: provider.enabled,
            has_api_key,
            model: model.id.clone(),
            protocol: model.protocol.map(|protocol| protocol.to_string()),
        });
    }
    if models.is_empty() {
        records.push(ModelRecord {
            agent_name: agent.name().to_string(),
            agent_title: agent_title.to_string(),
            agent_home: agent.home().to_path_buf(),
            provider_name: provider.name.clone(),
            provider_type: provider_type.to_string(),
            account: account.to_string(),
            base_url: Some(base_url),
            enabled: provider.enabled,
            has_api_key,
            model: "-".to_string(),
            protocol: provider.protocol.map(|protocol| protocol.to_string()),
        });
    }
}

fn sentra_only_catalog(mut catalog: ModelCatalog) -> ModelCatalog {
    catalog.agents = catalog
        .agents
        .into_iter()
        .filter(|agent| agent.agent_name == "sentra")
        .collect();
    if catalog.agents.is_empty() {
        catalog.agents.push(AgentProviderEntry {
            agent_name: "sentra".to_string(),
            agent_title: "Sentra".to_string(),
            probe_requests: default_probe_requests(),
        });
    }
    catalog
}

fn set(
    agent_name: String,
    base_url: String,
    api_key: String,
    model: String,
    protocol: Option<sentra_lib::protocol::WireProtocol>,
) -> SentraResult<()> {
    let home = current_home()?;
    set_at(
        &home, agent_name, base_url, api_key, model, protocol, None, None,
    )
}

fn set_at(
    home: &Path,
    agent_name: String,
    base_url: String,
    api_key: String,
    model: String,
    protocol: Option<sentra_lib::protocol::WireProtocol>,
    provider_name_override: Option<String>,
    raw_provider_id: Option<String>,
) -> SentraResult<()> {
    let base_url_display = base_url.clone();
    let model_display = model.clone();
    let protocol_display = protocol
        .map(|protocol| protocol.to_string())
        .unwrap_or_else(|| "-".to_string());
    let provider_name = provider_name_override
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| provider_name(&base_url));
    let provider = ProviderData {
        name: provider_name,
        raw_provider_id,
        base_url: Some(base_url),
        api_key: Some(api_key),
        enabled: true,
        models: vec![ProviderModel {
            id: model.clone(),
            name: Some(model),
            enabled: true,
        }],
        protocol,
        ..ProviderData::default()
    };
    mutate_provider_at(home, &agent_name, |asset| {
        asset.set_provider_data(provider.clone())
    })?;
    feedback::result(
        Status::Success,
        t("Model provider updated", "模型供应商已更新"),
        &[
            (t("Agent", "Agent"), agent_name),
            (t("Base URL", "Base URL"), base_url_display),
            (t("Model", "模型"), model_display),
            (t("Protocol", "协议"), protocol_display),
        ],
    );
    Ok(())
}

fn delete(agent_name: String, base_url: String) -> SentraResult<()> {
    let base_url_display = base_url.clone();
    let provider = ProviderData {
        name: provider_name(&base_url),
        base_url: Some(base_url),
        api_key: None,
        enabled: false,
        models: Vec::new(),
        protocol: None,
        ..ProviderData::default()
    };
    mutate_provider(&agent_name, |asset| asset.del_provider_data(&provider))?;
    feedback::result(
        Status::Success,
        t("Model provider deleted", "模型供应商已删除"),
        &[
            (t("Agent", "Agent"), agent_name),
            (t("Base URL", "Base URL"), base_url_display),
        ],
    );
    Ok(())
}

fn mutate_provider(
    agent_name: &str,
    apply: impl FnMut(
        &dyn sentra_lib::interfaces::ErasedAsset,
    ) -> SentraResult<sentra_lib::interfaces::AssetMutationResult>,
) -> SentraResult<()> {
    let home = current_home()?;
    mutate_provider_at(&home, agent_name, apply)
}

fn mutate_provider_at(
    home: &Path,
    agent_name: &str,
    mut apply: impl FnMut(
        &dyn sentra_lib::interfaces::ErasedAsset,
    ) -> SentraResult<sentra_lib::interfaces::AssetMutationResult>,
) -> SentraResult<()> {
    let target = canonical_agent_target(agent_name);
    for agent in
        discover_agents_matching_with_options(home, fast_agent_discovery_options(), |name| {
            target == Some(name)
        })
    {
        if target != Some(agent.name()) {
            continue;
        }
        for asset in agent.get_assets(AssetType::Provider)? {
            if !asset.supports_provider_write() {
                continue;
            }
            let result = apply(asset.as_ref())?;
            if result.changed {
                return Ok(());
            }
            if let Some(error) = result.errors.first() {
                return Err(SentraError::Message(error.message.clone()));
            }
        }
        return Err(SentraError::Message(format!(
            "{}: {agent_name}",
            t(
                "agent does not support provider configuration",
                "Agent 不支持供应商配置"
            )
        )));
    }
    Err(SentraError::Message(format!(
        "{}: {agent_name}",
        t("agent not found", "未找到 Agent")
    )))
}

fn provider_items(value: impl Serialize) -> SentraResult<Vec<ProviderData>> {
    let value = serde_json::to_value(value).map_err(|err| SentraError::Message(err.to_string()))?;
    serde_json::from_value(value).map_err(|err| SentraError::Message(err.to_string()))
}

fn provider_type_label(provider_type: sentra_lib::interfaces::ProviderType) -> String {
    match provider_type {
        sentra_lib::interfaces::ProviderType::Gateway => "gateway",
        sentra_lib::interfaces::ProviderType::CodexAccount => "codex_account",
        sentra_lib::interfaces::ProviderType::ClaudeAccount => "claude_account",
    }
    .to_string()
}

fn provider_account_label(account: Option<&sentra_lib::interfaces::ProviderAccount>) -> String {
    let Some(account) = account else {
        return "-".to_string();
    };
    account
        .email
        .as_deref()
        .or(account.display_name.as_deref())
        .or(account.organization_name.as_deref())
        .or(account.organization_id.as_deref())
        .or(account.account_id.as_deref())
        .unwrap_or("-")
        .to_string()
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

fn provider_name(base_url: &str) -> String {
    host_from_url(base_url).unwrap_or_else(|| base_url.to_string())
}

fn provider_key(base_url: &str, api_key: Option<&str>) -> String {
    format!("{}::{}", base_url.trim(), api_key.unwrap_or("").trim()).to_ascii_lowercase()
}

fn provider_fetch_key(provider: &ProviderRecord) -> Option<String> {
    provider
        .api_key
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .filter(|value| !is_masked_secret(value))
        .filter(|_| !provider.base_url.trim().is_empty())
        .map(|api_key| provider_key(&provider.base_url, Some(api_key)))
}

fn host_from_url(value: &str) -> Option<String> {
    let rest = value.split_once("://")?.1;
    rest.split(['/', '?', '#', ':'])
        .next()
        .filter(|host| !host.is_empty())
        .map(str::to_string)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ModelRecord {
    agent_name: String,
    agent_title: String,
    agent_home: PathBuf,
    provider_name: String,
    provider_type: String,
    account: String,
    #[serde(rename = "baseUrl")]
    base_url: Option<String>,
    enabled: bool,
    has_api_key: bool,
    model: String,
    protocol: Option<String>,
}

#[derive(Debug, PartialEq, Eq)]
struct ModelConfigInput {
    agent: String,
    provider_name: Option<String>,
    raw_provider_id: Option<String>,
    base_url: String,
    api_key: String,
    model: String,
    protocol: Option<WireProtocol>,
}

#[derive(Debug, Clone)]
struct ModelCatalog {
    agents: Vec<AgentProviderEntry>,
    gateways: Vec<ProviderRecord>,
}

#[derive(Debug, Clone)]
struct AgentProviderEntry {
    agent_name: String,
    agent_title: String,
    probe_requests: Vec<ProviderProbeRequest>,
}

#[derive(Debug, Clone)]
struct ProviderRecord {
    name: String,
    raw_provider_id: Option<String>,
    base_url: String,
    api_key: Option<String>,
    enabled: bool,
    models: Vec<ModelChoice>,
    temporary: bool,
}

#[derive(Debug, Clone)]
struct ModelChoice {
    id: String,
    name: String,
    enabled: bool,
    status: ModelProbeStatus,
    protocol: Option<WireProtocol>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ModelProbeStatus {
    Testing,
    Available,
    Unavailable,
}

const DEFAULT_MODEL_LIST_HEIGHT: usize = 12;
const DEFAULT_AGENT_LIST_HEIGHT: usize = 8;
const MODEL_PROBE_CONCURRENCY: usize = 8;
const MODEL_PROBE_TIMEOUT_MS: u64 = 3_000;
const MODEL_PROBE_WINDOW: usize = 16;

fn scroll_for_focus(focus: usize, scroll: usize, height: usize) -> usize {
    let height = height.max(1);
    if focus < scroll {
        focus
    } else if focus >= scroll.saturating_add(height) {
        focus.saturating_sub(height - 1)
    } else {
        scroll
    }
}

fn probe_candidate_indexes(len: usize, focus: usize) -> BTreeSet<usize> {
    if len == 0 {
        return BTreeSet::new();
    }
    let focus = focus.min(len - 1);
    let half_window = MODEL_PROBE_WINDOW / 2;
    let start = focus.saturating_sub(half_window);
    let end = start.saturating_add(MODEL_PROBE_WINDOW).min(len);
    (start..end).collect()
}

#[derive(Clone, Copy, Debug)]
struct ProbeResult {
    status: ModelProbeStatus,
    protocol: Option<WireProtocol>,
}

#[derive(Debug)]
struct ProbeMessage {
    key: String,
    result: ProbeResult,
}

#[derive(Debug)]
struct ModelFetchMessage {
    key: String,
    models: Vec<ModelChoice>,
}

const PROBE_MODEL_PLACEHOLDER: &str = "__sentra_probe_model__";

fn run_model_tui(
    records: &[ModelRecord],
    catalog: ModelCatalog,
) -> SentraResult<Option<ModelConfigInput>> {
    let mut terminal = TerminalGuard::enter()?;
    let mut state = ModelTuiState::with_records_and_catalog(records, catalog);
    let (probe_tx, probe_rx) = mpsc::channel();
    let (model_fetch_tx, model_fetch_rx) = mpsc::channel();

    loop {
        state.drain_model_fetch_results(&model_fetch_rx);
        state.drain_probe_results(&probe_rx);
        terminal.draw(|frame| render_model_tui(frame, records, &state))?;
        state.schedule_current_model_fetch(&model_fetch_tx);
        state.schedule_current_probes(&probe_tx);
        if event::poll(Duration::from_millis(120))
            .map_err(|err| SentraError::Message(err.to_string()))?
        {
            if let Some(key) = read_key_event()? {
                match state.handle_key(key) {
                    ModelTuiAction::None => {}
                    ModelTuiAction::Cancel => return Ok(None),
                    ModelTuiAction::Submit => {
                        if let Some(input) = state.submit_selected_model() {
                            return Ok(Some(input));
                        }
                    }
                }
            }
        }
    }
}

fn probe_model_with_requests(
    provider: &ProviderRecord,
    api_key: &str,
    model: &ModelChoice,
    requests: &[ProviderProbeRequest],
) -> Option<WireProtocol> {
    if requests.is_empty() {
        return None;
    }
    requests
        .iter()
        .find(|request| probe_provider_model(provider, api_key, &model.id, request))
        .map(|request| request.protocol)
}

fn probe_key(agent: &AgentProviderEntry, provider: &ProviderRecord, model: &ModelChoice) -> String {
    format!(
        "{}::{}::{}",
        agent.agent_name,
        provider_key(&provider.base_url, provider.api_key.as_deref()),
        model.id
    )
}

fn probe_requests_for_agent(
    agent: &AgentProviderEntry,
    model: &ModelChoice,
) -> Vec<ProviderProbeRequest> {
    if !agent.probe_requests.is_empty() {
        return agent
            .probe_requests
            .iter()
            .map(|request| ProviderProbeRequest {
                protocol: request.protocol,
                body: request
                    .body
                    .as_deref()
                    .map(|body| provider_probe_body_for_model(body, &model.id)),
                prompt: request.prompt.clone(),
            })
            .collect();
    }
    match model.protocol {
        Some(protocol) => vec![ProviderProbeRequest {
            protocol,
            body: None,
            prompt: None,
        }],
        None => default_probe_requests(),
    }
}

fn provider_probe_body_for_model(body: &str, model: &str) -> String {
    let Ok(mut value) = serde_json::from_str::<serde_json::Value>(body) else {
        return body.replace(PROBE_MODEL_PLACEHOLDER, model);
    };
    replace_probe_model_placeholder(&mut value, model);
    serde_json::to_string(&value).unwrap_or_else(|_| body.replace(PROBE_MODEL_PLACEHOLDER, model))
}

fn replace_probe_model_placeholder(value: &mut serde_json::Value, model: &str) {
    match value {
        serde_json::Value::String(text) if text == PROBE_MODEL_PLACEHOLDER => {
            *text = model.to_string();
        }
        serde_json::Value::Array(items) => {
            for item in items {
                replace_probe_model_placeholder(item, model);
            }
        }
        serde_json::Value::Object(items) => {
            for item in items.values_mut() {
                replace_probe_model_placeholder(item, model);
            }
        }
        _ => {}
    }
}

fn default_probe_requests() -> Vec<ProviderProbeRequest> {
    vec![ProviderProbeRequest {
        protocol: WireProtocol::ChatCompletions,
        body: None,
        prompt: None,
    }]
}

fn probe_provider_model(
    provider: &ProviderRecord,
    api_key: &str,
    model: &str,
    request: &ProviderProbeRequest,
) -> bool {
    if let Some(body) = request.body.as_deref() {
        return probe_provider_model_with_body(provider, api_key, request.protocol, body);
    }

    let params = ModelRequestParams {
        api_url: provider.base_url.clone(),
        api_key: api_key.to_string(),
        model: model.to_string(),
        protocol: request.protocol,
        max_tokens: 1024,
        stream: true,
        timeout_ms: MODEL_PROBE_TIMEOUT_MS,
    };
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map(|runtime| {
            runtime
                .block_on(async {
                    match request.prompt.as_ref() {
                        Some(prompt) => probe_model_request_with_prompt(&params, prompt).await,
                        None => probe_model_request(&params).await,
                    }
                })
                .is_ok()
        })
        .unwrap_or(false)
}

fn probe_provider_model_with_body(
    provider: &ProviderRecord,
    api_key: &str,
    protocol: WireProtocol,
    body: &str,
) -> bool {
    let url = format!(
        "{}/{}",
        provider.base_url.trim_end_matches('/'),
        probe_endpoint_path(protocol)
    );
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_millis(MODEL_PROBE_TIMEOUT_MS))
        .build();
    let response = agent
        .post(&url)
        .set("Authorization", &format!("Bearer {api_key}"))
        .set("Content-Type", "application/json")
        .send_string(body);
    let Ok(response) = response else {
        return false;
    };
    let Ok(raw) = response.into_string() else {
        return false;
    };
    validate_model_text_response(protocol, &raw).is_ok()
}

fn probe_endpoint_path(protocol: WireProtocol) -> &'static str {
    match protocol {
        WireProtocol::Responses => "responses",
        WireProtocol::ChatCompletions => "chat/completions",
        WireProtocol::AnthropicMessages => "messages",
    }
}

fn fetch_gateway_models(provider: &ProviderRecord) -> Vec<ModelChoice> {
    let Some(api_key) = provider
        .api_key
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .filter(|value| !is_masked_secret(value))
    else {
        return Vec::new();
    };
    if provider.base_url.trim().is_empty() {
        return Vec::new();
    }
    let url = format!("{}/models", provider.base_url.trim_end_matches('/'));
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(2))
        .build();
    let response = agent
        .get(&url)
        .set("Authorization", &format!("Bearer {api_key}"))
        .call();
    let Ok(response) = response else {
        return Vec::new();
    };
    let Ok(body) = response.into_string() else {
        return Vec::new();
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&body) else {
        return Vec::new();
    };
    parse_model_list(&value)
}

fn is_masked_secret(value: &str) -> bool {
    value.contains("****")
}

fn parse_model_list(value: &serde_json::Value) -> Vec<ModelChoice> {
    let items = value
        .get("data")
        .and_then(|data| data.as_array())
        .or_else(|| value.as_array());
    items
        .into_iter()
        .flatten()
        .filter_map(|item| {
            let id = item
                .get("id")
                .or_else(|| item.get("name"))
                .and_then(|value| value.as_str())?;
            let name = item
                .get("name")
                .and_then(|value| value.as_str())
                .unwrap_or(id);
            Some(ModelChoice {
                id: id.to_string(),
                name: name.to_string(),
                enabled: true,
                status: ModelProbeStatus::Testing,
                protocol: None,
            })
        })
        .collect()
}

fn merge_model_choices(existing: &mut Vec<ModelChoice>, fetched: Vec<ModelChoice>) {
    let fallback_protocol = existing.iter().find_map(|model| model.protocol);
    let mut seen = existing
        .iter()
        .map(|model| model.id.clone())
        .collect::<BTreeSet<_>>();
    for mut model in fetched {
        if seen.insert(model.id.clone()) {
            if model.protocol.is_none() {
                model.protocol = fallback_protocol;
            }
            existing.push(model);
        }
    }
    existing.sort_by(|left, right| left.name.cmp(&right.name));
}

fn render_model_tui(frame: &mut Frame<'_>, records: &[ModelRecord], state: &ModelTuiState) {
    let area = frame.area();
    frame.render_widget(Clear, area);

    if area.width < 80 || area.height < 24 {
        let message = Paragraph::new(vec![
            Line::from(Span::styled("Sentra Model", theme::title_style())),
            Line::from(""),
            Line::styled(
                t(
                    "Terminal too small. Resize to at least 80x24.",
                    "终端太小，请调整到至少 80x24。",
                ),
                theme::muted_style(),
            ),
        ])
        .block(model_chrome_block(false))
        .style(theme::body_style());
        frame.render_widget(message, area);
        return;
    }

    match state.mode {
        ModelTuiMode::AgentSelect => render_agent_select(frame, area, records, state),
        ModelTuiMode::Configure => render_configure_model(frame, area, state),
        ModelTuiMode::AddGatewayDialog => {
            render_configure_model(frame, area, state);
            let popup = centered_rect(area, 76, 12);
            frame.render_widget(Clear, popup);
            render_add_gateway_dialog(frame, popup, state);
        }
    }
}

fn render_agent_select(
    frame: &mut Frame<'_>,
    area: Rect,
    records: &[ModelRecord],
    state: &ModelTuiState,
) {
    let popup = centered_rect(area, 84, 22);
    frame.render_widget(Clear, popup);
    let [header_area, list_area, detail_area, footer_area] = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(8),
        Constraint::Length(7),
        Constraint::Length(2),
    ])
    .areas(popup);

    render_panel_header(
        frame,
        header_area,
        "Sentra Model",
        t(
            "Choose an agent to configure its model provider",
            "选择要配置模型供应商的 Agent",
        ),
    );

    let visible_height = list_area.height.saturating_sub(2).max(1) as usize;
    let agent_scroll = scroll_for_focus(state.agent_focus, state.agent_scroll, visible_height);
    let items = state
        .agents
        .iter()
        .enumerate()
        .skip(agent_scroll)
        .take(visible_height)
        .map(|(index, agent)| {
            let configured = state.configured_provider_count_for(agent);
            let suffix = if configured == 0 {
                t("no configured provider", "未配置供应商").to_string()
            } else {
                format!("{}: {configured}", t("configured", "已配置"))
            };
            agent_select_item(
                index == state.agent_focus,
                true,
                index + 1,
                agent,
                &suffix,
                list_area.width,
            )
        })
        .collect::<Vec<_>>();
    frame.render_widget(
        List::new(items)
            .block(model_block(t("Agents", "Agent"), true))
            .style(theme::body_style()),
        list_area,
    );

    let summary = if records.is_empty() {
        vec![
            Line::styled(
                t(
                    "No configured model providers found.",
                    "没有已配置的模型供应商。",
                ),
                theme::muted_style(),
            ),
            Line::styled(
                t(
                    "Use Add Gateway to add baseUrl + key, then choose agent and model.",
                    "使用新增网关添加 baseUrl 和 key，然后选择 Agent 与模型。",
                ),
                theme::muted_style(),
            ),
        ]
    } else {
        records
            .iter()
            .take(5)
            .map(|record| {
                Line::from(vec![
                    Span::styled(format!("{:<12}", record.agent_name), theme::body_style()),
                    Span::styled(record.provider_name.clone(), theme::secondary_style()),
                    Span::raw("  "),
                    Span::styled(record.model.clone(), theme::body_style()),
                ])
            })
            .collect()
    };
    frame.render_widget(
        Paragraph::new(summary)
            .block(model_block(t("Current", "当前"), false))
            .style(theme::body_style())
            .wrap(Wrap { trim: false }),
        detail_area,
    );
    render_footer(
        frame,
        footer_area,
        t(
            "[Enter] configure  [j/k] move  [q/Esc] quit",
            "[Enter] 配置  [j/k] 移动  [q/Esc] 退出",
        ),
        state,
    );
}

fn render_add_gateway_dialog(frame: &mut Frame<'_>, area: Rect, state: &ModelTuiState) {
    let [header_area, form_area, footer_area] = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(7),
        Constraint::Length(2),
    ])
    .areas(area);
    render_panel_header(
        frame,
        header_area,
        t("Add Gateway", "新增网关"),
        t(
            "Enter baseUrl + key for this gateway",
            "填写此网关的 baseUrl 和 key",
        ),
    );

    let fields = [
        ("baseUrl", state.add_base_url.as_str(), false),
        ("key", state.add_api_key.as_str(), true),
    ];
    frame.render_widget(
        Paragraph::new(field_lines(&fields, state.add_focus))
            .block(model_block(t("Gateway", "网关"), true))
            .style(theme::body_style())
            .wrap(Wrap { trim: false }),
        form_area,
    );
    render_footer(
        frame,
        footer_area,
        t(
            "[Tab/Up/Down] field  [Enter] next/save  [Esc] back",
            "[Tab/上/下] 字段  [Enter] 下一项/保存  [Esc] 返回",
        ),
        state,
    );
}

fn render_configure_model(frame: &mut Frame<'_>, area: Rect, state: &ModelTuiState) {
    let layout = configure_layout(area);
    let title = state
        .current_agent()
        .map(agent_column_title)
        .unwrap_or_else(|| t("No agent", "无 Agent").to_string());
    let header_title = format!("{} - {title}", t("Configure Model", "配置模型"));
    render_panel_header(
        frame,
        layout.header,
        &header_title,
        &format!("{}: {title}", t("Agent", "Agent")),
    );

    let [provider_area, model_area] =
        Layout::horizontal([Constraint::Percentage(38), Constraint::Percentage(62)])
            .spacing(1)
            .areas(layout.body);
    render_provider_column(frame, provider_area, state);
    render_model_column(frame, model_area, state);
    render_gateway_status(frame, layout.status, state);
    render_footer(
        frame,
        layout.footer,
        t(
            "[Left/Right] focus  [j/k] move  [Tab/n] next available  [Enter] select/save  [Esc] back",
            "[左/右] 切换焦点  [j/k] 移动  [Tab/n] 下一个可用  [Enter] 选择/保存  [Esc] 返回",
        ),
        state,
    );
}

#[derive(Clone, Copy, Debug)]
struct ConfigureLayout {
    header: Rect,
    status: Rect,
    body: Rect,
    footer: Rect,
}

fn configure_layout(area: Rect) -> ConfigureLayout {
    let [header, status, body, footer] = Layout::vertical([
        Constraint::Length(3),
        Constraint::Length(3),
        Constraint::Min(10),
        Constraint::Length(2),
    ])
    .areas(area);
    ConfigureLayout {
        header,
        status,
        body,
        footer,
    }
}

fn render_provider_column(frame: &mut Frame<'_>, area: Rect, state: &ModelTuiState) {
    let mut items = state
        .gateways
        .iter()
        .enumerate()
        .map(|(index, provider)| {
            provider_switch_item(
                index == state.provider_focus,
                state.focus_pane == 0,
                state,
                index + 1,
                provider,
                area.width,
            )
        })
        .collect::<Vec<_>>();
    items.push(add_gateway_item(
        state.is_add_gateway_focused(),
        state.focus_pane == 0,
    ));
    frame.render_widget(
        List::new(items)
            .block(model_block(t("Providers", "供应商"), state.focus_pane == 0))
            .style(theme::body_style()),
        area,
    );
}

fn render_model_column(frame: &mut Frame<'_>, area: Rect, state: &ModelTuiState) {
    let models = state
        .current_provider()
        .map(|provider| provider.models.as_slice())
        .unwrap_or(&[]);
    if models.is_empty() {
        let empty_message = if state.current_provider_model_fetch_in_flight() {
            t(
                "Fetching models from this provider...",
                "正在从此供应商获取模型列表...",
            )
        } else {
            t(
                "No models returned by this provider.",
                "此供应商未返回模型列表。",
            )
        };
        frame.render_widget(
            Paragraph::new(empty_message)
                .block(model_block(t("Models", "模型"), state.focus_pane == 1))
                .style(theme::muted_style())
                .alignment(Alignment::Center)
                .wrap(Wrap { trim: false }),
            area,
        );
        return;
    }
    let visible_height = area.height.saturating_sub(2).max(1) as usize;
    let items = models
        .iter()
        .enumerate()
        .skip(state.model_scroll)
        .take(visible_height)
        .map(|(index, model)| {
            let result = state.model_probe_result(model);
            model_switch_item(
                index == state.model_focus,
                state.focus_pane == 1,
                index + 1,
                model,
                result.status,
            )
        })
        .collect::<Vec<_>>();
    frame.render_widget(
        List::new(items)
            .block(model_block(t("Models", "模型"), state.focus_pane == 1))
            .style(theme::body_style()),
        area,
    );
}

fn render_gateway_status(frame: &mut Frame<'_>, area: Rect, state: &ModelTuiState) {
    let line = gateway_status_line(state);
    frame.render_widget(
        Paragraph::new(line)
            .block(model_block(t("Status", "状态"), false))
            .style(theme::body_style())
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn gateway_status_line(state: &ModelTuiState) -> String {
    state
        .current_provider()
        .map(|provider| {
            let (available, total) = gateway_available_ratio(state, provider);
            let agent = state
                .current_agent()
                .map(agent_column_title)
                .unwrap_or_else(|| "-".to_string());
            format!(
                "{}: {agent}    {}: {}    Key: {}    {}: {available}/{total}    {}: {}",
                t("Agent", "Agent"),
                t("Base URL", "Base URL"),
                provider.base_url,
                provider
                    .api_key
                    .as_deref()
                    .map(mask_secret)
                    .unwrap_or_else(|| "-".to_string()),
                t("Models", "模型"),
                t("Enabled", "启用"),
                yes_no(provider.enabled)
            )
        })
        .unwrap_or_else(|| {
            format!(
                "{}: -    {}: -    Key: -",
                t("Agent", "Agent"),
                t("Base URL", "Base URL")
            )
        })
}

fn agent_column_title(agent: &AgentProviderEntry) -> String {
    agent.agent_title.clone()
}

fn agent_select_item(
    selected: bool,
    focused: bool,
    index: usize,
    agent: &AgentProviderEntry,
    suffix: &str,
    area_width: u16,
) -> ListItem<'static> {
    let text_style = switch_text_style(selected, focused);
    let content_width = area_width.saturating_sub(2) as usize;
    let index_text = format!("{index}.");
    let suffix_text = format!("[{suffix}]");
    let title_width = content_width
        .saturating_sub(2) // selection pointer
        .saturating_sub(index_text.len() + 1)
        .saturating_sub(2)
        .saturating_sub(UnicodeWidthStr::width(suffix_text.as_str()))
        .max(8);
    let spans = vec![
        Span::styled(format!("{index_text} "), theme::muted_style()),
        Span::styled(
            pad_display_width(&agent_column_title(agent), title_width),
            text_style,
        ),
        Span::raw("  "),
        Span::styled(suffix_text, theme::muted_style()),
    ];
    ListItem::new(switch_line(selected, focused, spans)).style(switch_row_style(selected, focused))
}

#[cfg(test)]
fn model_column_title(model: &ModelChoice) -> String {
    format!("{}  [{}]", model.name, model_status_label(model.status))
}

#[cfg(test)]
fn model_column_title_with_index(
    model: &ModelChoice,
    index: usize,
    status: ModelProbeStatus,
) -> String {
    format!("{index}. {}  [{}]", model.name, model_status_label(status))
}

#[cfg(test)]
fn gateway_column_title(state: &ModelTuiState, provider: &ProviderRecord) -> String {
    let marker = provider_marker(state, provider);
    let (available, total) = gateway_available_ratio(state, provider);
    format!("{marker} {}  {available}/{total}", provider.name)
}

fn provider_switch_item(
    selected: bool,
    focused: bool,
    state: &ModelTuiState,
    index: usize,
    provider: &ProviderRecord,
    area_width: u16,
) -> ListItem<'static> {
    let text_style = switch_text_style(selected, focused);
    let marker_style = if state.selected_agent_has_provider(provider) {
        theme::success_style()
    } else if provider.temporary {
        theme::info_style()
    } else if selected && focused {
        theme::focus_style()
    } else {
        theme::muted_style()
    };
    let (available, total) = gateway_available_ratio(state, provider);
    let ratio = format!("{available}/{total}");
    let status = provider_status_label(state, provider);
    let index_text = format!("{index:>2}.");
    let content_width = area_width.saturating_sub(2) as usize;
    let name_width = content_width
        .saturating_sub(2) // selection pointer
        .saturating_sub(4) // sequence column
        .saturating_sub(3) // marker + separating space
        .saturating_sub(8) // models ratio column
        .saturating_sub(13) // status column
        .max(8);
    let spans = vec![
        Span::styled(format!("{index_text} "), theme::muted_style()),
        Span::styled(
            format!("{} ", provider_marker(state, provider)),
            marker_style,
        ),
        Span::styled(pad_display_width(&provider.name, name_width), text_style),
        Span::raw(" "),
        Span::styled(left_pad_display_width(&ratio, 7), theme::muted_style()),
        Span::raw(" "),
        Span::styled(pad_display_width(status, 12), theme::muted_style()),
    ];
    ListItem::new(switch_line(selected, focused, spans)).style(switch_row_style(selected, focused))
}

fn provider_status_label(state: &ModelTuiState, provider: &ProviderRecord) -> &'static str {
    if state.selected_agent_has_provider(provider) {
        t("configured", "已配置")
    } else if provider.temporary {
        t("new", "新增")
    } else {
        ""
    }
}

fn provider_marker(state: &ModelTuiState, provider: &ProviderRecord) -> &'static str {
    match (
        state.selected_agent_has_provider(provider),
        provider.temporary,
    ) {
        (true, true) => "*+",
        (true, false) => "* ",
        (false, true) => "+ ",
        (false, false) => "  ",
    }
}

fn add_gateway_item(selected: bool, focused: bool) -> ListItem<'static> {
    let spans = vec![Span::styled(
        format!("+ {}", t("Add Gateway", "新增网关")),
        switch_text_style(selected, focused),
    )];
    ListItem::new(switch_line(selected, focused, spans)).style(switch_row_style(selected, focused))
}

fn gateway_available_ratio(state: &ModelTuiState, provider: &ProviderRecord) -> (usize, usize) {
    let Some(agent) = state.current_agent() else {
        return (0, provider.models.len());
    };
    let available = provider
        .models
        .iter()
        .filter(|model| {
            let key = probe_key(agent, provider, model);
            state
                .probe_results
                .get(&key)
                .map(|result| result.status == ModelProbeStatus::Available)
                .unwrap_or(model.status == ModelProbeStatus::Available)
        })
        .count();
    (available, provider.models.len())
}

fn configured_gateways_by_agent(records: &[ModelRecord]) -> BTreeMap<String, BTreeSet<String>> {
    let mut configured = BTreeMap::<String, BTreeSet<String>>::new();
    for record in records {
        let Some(base_url) = record.base_url.as_deref() else {
            continue;
        };
        if base_url.trim().is_empty() {
            continue;
        }
        configured
            .entry(record.agent_name.clone())
            .or_default()
            .insert(provider_config_key(base_url));
    }
    configured
}

fn provider_config_key(base_url: &str) -> String {
    base_url.trim().to_ascii_lowercase()
}

fn pad_display_width(value: &str, width: usize) -> String {
    let truncated = truncate_display_width(value, width);
    let padding = width.saturating_sub(UnicodeWidthStr::width(truncated.as_str()));
    format!("{truncated}{}", " ".repeat(padding))
}

fn left_pad_display_width(value: &str, width: usize) -> String {
    let truncated = truncate_display_width(value, width);
    let padding = width.saturating_sub(UnicodeWidthStr::width(truncated.as_str()));
    format!("{}{truncated}", " ".repeat(padding))
}

fn truncate_display_width(value: &str, max_width: usize) -> String {
    if UnicodeWidthStr::width(value) <= max_width {
        return value.to_string();
    }
    if max_width == 0 {
        return String::new();
    }
    if max_width == 1 {
        return "…".to_string();
    }
    let target = max_width - 1;
    let mut result = String::new();
    let mut width = 0;
    for ch in value.chars() {
        let ch_width = UnicodeWidthStr::width(ch.to_string().as_str());
        if width + ch_width > target {
            break;
        }
        width += ch_width;
        result.push(ch);
    }
    result.push('…');
    result
}

fn mask_secret(value: &str) -> String {
    let chars = value.chars().collect::<Vec<_>>();
    if chars.is_empty() {
        return String::new();
    }
    if chars.len() <= 2 {
        return "*".repeat(chars.len());
    }
    let edge = if chars.len() <= 8 { 1 } else { 4 };
    let prefix = chars.iter().take(edge).collect::<String>();
    let suffix = chars
        .iter()
        .skip(chars.len().saturating_sub(edge))
        .collect::<String>();
    format!("{prefix}****{suffix}")
}

fn model_status_label(status: ModelProbeStatus) -> &'static str {
    match status {
        ModelProbeStatus::Testing => t("testing", "测试中"),
        ModelProbeStatus::Available => t("available", "可用"),
        ModelProbeStatus::Unavailable => t("unavailable", "不可用"),
    }
}

fn model_switch_item(
    selected: bool,
    focused: bool,
    index: usize,
    model: &ModelChoice,
    status: ModelProbeStatus,
) -> ListItem<'static> {
    let text_style = switch_text_style(selected, focused);
    let status_style = if selected && focused {
        theme::selection_style()
    } else if selected {
        theme::inactive_selection_style()
    } else {
        match status {
            ModelProbeStatus::Testing => theme::warning_style(),
            ModelProbeStatus::Available => theme::success_style(),
            ModelProbeStatus::Unavailable => theme::muted_style(),
        }
    };
    let spans = vec![
        Span::styled(format!("{index}. {}  ", model.name), text_style),
        Span::styled(format!("[{}]", model_status_label(status)), status_style),
    ];
    ListItem::new(switch_line(selected, focused, spans)).style(switch_row_style(selected, focused))
}

fn render_panel_header(frame: &mut Frame<'_>, area: Rect, title: &str, subtitle: &str) {
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(Span::styled(title.to_string(), theme::title_style())),
            Line::from(Span::styled(subtitle.to_string(), theme::muted_style())),
        ])
        .block(model_chrome_block(false))
        .style(theme::body_style())
        .alignment(Alignment::Left),
        area,
    );
}

fn render_footer(frame: &mut Frame<'_>, area: Rect, keys: &str, state: &ModelTuiState) {
    let status = state.status.as_deref().unwrap_or(keys);
    let style = if state.status.is_some() {
        theme::info_style()
    } else {
        theme::muted_style()
    };
    frame.render_widget(Paragraph::new(status).style(style), area);
}

fn model_chrome_block(focused: bool) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_style(theme::border_style(focused))
}

fn model_block(title: &'static str, focused: bool) -> Block<'static> {
    model_chrome_block(focused)
        .title(title)
        .title_style(if focused {
            theme::focus_style()
        } else {
            theme::title_style()
        })
}

fn switch_line(
    selected: bool,
    focused: bool,
    spans: impl IntoIterator<Item = Span<'static>>,
) -> Line<'static> {
    let pointer = if selected { "→ " } else { "  " };
    let pointer_style = if selected && focused {
        theme::selection_style()
    } else if selected {
        theme::inactive_selection_style()
    } else {
        theme::muted_style()
    };
    let mut line_spans = vec![Span::styled(pointer, pointer_style)];
    line_spans.extend(spans);
    Line::from(line_spans).style(switch_row_style(selected, focused))
}

fn switch_text_style(selected: bool, focused: bool) -> Style {
    if selected && focused {
        theme::selection_style()
    } else if selected {
        theme::inactive_selection_style()
    } else {
        theme::body_style()
    }
}

fn switch_row_style(selected: bool, focused: bool) -> Style {
    if selected && focused {
        theme::selection_style()
    } else if selected {
        theme::inactive_selection_style()
    } else {
        Style::default()
    }
}

fn field_lines(fields: &[(&str, &str, bool)], focus: usize) -> Vec<Line<'static>> {
    fields
        .iter()
        .enumerate()
        .flat_map(|(index, (label, value, secret))| {
            let marker = if index == focus { "→ " } else { "  " };
            let style = if index == focus {
                theme::focus_style()
            } else {
                theme::body_style()
            };
            let display = if *secret && !value.is_empty() {
                mask_secret(value)
            } else {
                (*value).to_string()
            };
            [
                Line::from(vec![
                    Span::styled(marker.to_string(), style),
                    Span::styled(format!("{label:<8} "), theme::muted_style()),
                    Span::styled(display, style),
                ]),
                Line::from(""),
            ]
        })
        .collect()
}

fn centered_rect(area: Rect, width: u16, height: u16) -> Rect {
    let width = width.min(area.width);
    let height = height.min(area.height);
    Rect {
        x: area.x + area.width.saturating_sub(width) / 2,
        y: area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    }
}

fn read_key_event() -> SentraResult<Option<KeyEvent>> {
    if let Event::Key(key) = event::read().map_err(|err| SentraError::Message(err.to_string()))?
        && key.kind == KeyEventKind::Press
    {
        return Ok(Some(key));
    }
    Ok(None)
}

struct TerminalGuard {
    terminal: Terminal<CrosstermBackend<std::io::Stdout>>,
}

impl TerminalGuard {
    fn enter() -> SentraResult<Self> {
        enable_raw_mode().map_err(|err| SentraError::Message(err.to_string()))?;
        let mut stdout = std::io::stdout();
        execute!(stdout, EnterAlternateScreen, Hide)
            .map_err(|err| SentraError::Message(err.to_string()))?;
        let terminal = Terminal::new(CrosstermBackend::new(stdout))?;
        Ok(Self { terminal })
    }

    fn draw<F>(&mut self, render: F) -> SentraResult<()>
    where
        F: FnOnce(&mut Frame<'_>),
    {
        self.terminal.draw(render)?;
        Ok(())
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = execute!(self.terminal.backend_mut(), Show, LeaveAlternateScreen);
        let _ = disable_raw_mode();
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ModelTuiMode {
    AgentSelect,
    Configure,
    AddGatewayDialog,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ModelTuiAction {
    None,
    Cancel,
    Submit,
}

#[derive(Debug)]
struct ModelTuiState {
    mode: ModelTuiMode,
    focus_pane: usize,
    agent_focus: usize,
    agent_scroll: usize,
    provider_focus: usize,
    model_focus: usize,
    model_scroll: usize,
    add_focus: usize,
    agents: Vec<AgentProviderEntry>,
    gateways: Vec<ProviderRecord>,
    probe_results: BTreeMap<String, ProbeResult>,
    in_flight: BTreeSet<String>,
    model_fetch_in_flight: BTreeSet<String>,
    model_fetch_attempted: BTreeSet<String>,
    add_base_url: String,
    add_api_key: String,
    configured_gateways: BTreeMap<String, BTreeSet<String>>,
    status: Option<String>,
}

impl ModelTuiState {
    fn new() -> Self {
        Self {
            mode: ModelTuiMode::AgentSelect,
            focus_pane: 0,
            agent_focus: 0,
            agent_scroll: 0,
            provider_focus: 0,
            model_focus: 0,
            model_scroll: 0,
            add_focus: 0,
            agents: Vec::new(),
            gateways: Vec::new(),
            probe_results: BTreeMap::new(),
            in_flight: BTreeSet::new(),
            model_fetch_in_flight: BTreeSet::new(),
            model_fetch_attempted: BTreeSet::new(),
            add_base_url: String::new(),
            add_api_key: String::new(),
            configured_gateways: BTreeMap::new(),
            status: None,
        }
    }

    fn with_catalog(catalog: ModelCatalog) -> Self {
        let mut state = Self::new();
        state.agents = catalog.agents;
        state.gateways = catalog.gateways;
        state
    }

    fn with_records_and_catalog(records: &[ModelRecord], catalog: ModelCatalog) -> Self {
        let mut state = Self::with_catalog(catalog);
        state.configured_gateways = configured_gateways_by_agent(records);
        state
    }

    fn handle_key(&mut self, key: KeyEvent) -> ModelTuiAction {
        match self.mode {
            ModelTuiMode::AgentSelect => self.handle_agent_select_key(key),
            ModelTuiMode::Configure => self.handle_configure_key(key),
            ModelTuiMode::AddGatewayDialog => self.handle_add_gateway_dialog_key(key),
        }
    }

    fn handle_agent_select_key(&mut self, key: KeyEvent) -> ModelTuiAction {
        match key {
            KeyEvent {
                code: KeyCode::Esc, ..
            }
            | KeyEvent {
                code: KeyCode::Char('q'),
                modifiers: KeyModifiers::NONE,
                ..
            }
            | KeyEvent {
                code: KeyCode::Char('c'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => ModelTuiAction::Cancel,
            KeyEvent {
                code: KeyCode::Up, ..
            }
            | KeyEvent {
                code: KeyCode::Char('k'),
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                self.agent_focus = move_index(self.agent_focus, self.agents.len(), -1);
                self.ensure_agent_visible(DEFAULT_AGENT_LIST_HEIGHT);
                ModelTuiAction::None
            }
            KeyEvent {
                code: KeyCode::Down,
                ..
            }
            | KeyEvent {
                code: KeyCode::Char('j'),
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                self.agent_focus = move_index(self.agent_focus, self.agents.len(), 1);
                self.ensure_agent_visible(DEFAULT_AGENT_LIST_HEIGHT);
                ModelTuiAction::None
            }
            KeyEvent {
                code: KeyCode::Enter,
                ..
            } => {
                if self.agents.is_empty() {
                    self.status = Some(
                        t(
                            "No agent supports provider configuration.",
                            "没有支持供应商配置的 Agent。",
                        )
                        .to_string(),
                    );
                    return ModelTuiAction::None;
                }
                self.mode = ModelTuiMode::Configure;
                self.focus_pane = 0;
                self.provider_focus = self.provider_focus.min(self.gateways.len());
                self.model_focus = 0;
                self.model_scroll = 0;
                self.status = None;
                ModelTuiAction::None
            }
            _ => ModelTuiAction::None,
        }
    }

    fn handle_add_gateway_dialog_key(&mut self, key: KeyEvent) -> ModelTuiAction {
        match key {
            KeyEvent {
                code: KeyCode::Esc, ..
            } => {
                self.mode = ModelTuiMode::Configure;
                self.status = None;
            }
            KeyEvent {
                code: KeyCode::Char('c'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => return ModelTuiAction::Cancel,
            KeyEvent {
                code: KeyCode::Tab, ..
            }
            | KeyEvent {
                code: KeyCode::Down,
                ..
            } => self.add_focus = (self.add_focus + 1).min(1),
            KeyEvent {
                code: KeyCode::Up, ..
            } => self.add_focus = self.add_focus.saturating_sub(1),
            KeyEvent {
                code: KeyCode::Backspace,
                ..
            } => self.edit_add_field(|value| {
                value.pop();
            }),
            KeyEvent {
                code: KeyCode::Enter,
                ..
            } if self.add_focus == 0 => {
                self.add_focus = 1;
                self.status = None;
            }
            KeyEvent {
                code: KeyCode::Enter,
                ..
            } => self.continue_from_gateway(),
            KeyEvent {
                code: KeyCode::Char(ch),
                modifiers,
                ..
            } if !modifiers.contains(KeyModifiers::CONTROL)
                && !modifiers.contains(KeyModifiers::ALT) =>
            {
                self.edit_add_field(|value| value.push(ch));
            }
            _ => {}
        }
        ModelTuiAction::None
    }

    fn handle_configure_key(&mut self, key: KeyEvent) -> ModelTuiAction {
        match key {
            KeyEvent {
                code: KeyCode::Esc, ..
            } => {
                self.mode = ModelTuiMode::AgentSelect;
                self.status = None;
                ModelTuiAction::None
            }
            KeyEvent {
                code: KeyCode::Char('q'),
                modifiers: KeyModifiers::NONE,
                ..
            }
            | KeyEvent {
                code: KeyCode::Char('c'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => ModelTuiAction::Cancel,
            KeyEvent {
                code: KeyCode::Left,
                ..
            } => {
                self.focus_pane = self.focus_pane.saturating_sub(1);
                ModelTuiAction::None
            }
            KeyEvent {
                code: KeyCode::Tab, ..
            } if self.focus_pane == 1 => {
                self.jump_available_model(1);
                ModelTuiAction::None
            }
            KeyEvent {
                code: KeyCode::BackTab,
                ..
            }
            | KeyEvent {
                code: KeyCode::Char('N'),
                modifiers: KeyModifiers::SHIFT,
                ..
            } if self.focus_pane == 1 => {
                self.jump_available_model(-1);
                ModelTuiAction::None
            }
            KeyEvent {
                code: KeyCode::Char('n'),
                modifiers: KeyModifiers::NONE,
                ..
            } if self.focus_pane == 1 => {
                self.jump_available_model(1);
                ModelTuiAction::None
            }
            KeyEvent {
                code: KeyCode::Right,
                ..
            }
            | KeyEvent {
                code: KeyCode::Tab, ..
            } => {
                if !(self.focus_pane == 0 && self.is_add_gateway_focused()) {
                    self.focus_pane = (self.focus_pane + 1).min(1);
                }
                ModelTuiAction::None
            }
            KeyEvent {
                code: KeyCode::Up, ..
            }
            | KeyEvent {
                code: KeyCode::Char('k'),
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                self.move_configure_focus(-1);
                ModelTuiAction::None
            }
            KeyEvent {
                code: KeyCode::Down,
                ..
            }
            | KeyEvent {
                code: KeyCode::Char('j'),
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                self.move_configure_focus(1);
                ModelTuiAction::None
            }
            KeyEvent {
                code: KeyCode::Enter,
                ..
            } if self.focus_pane == 0 && self.is_add_gateway_focused() => {
                self.mode = ModelTuiMode::AddGatewayDialog;
                self.add_focus = 0;
                self.status = None;
                ModelTuiAction::None
            }
            KeyEvent {
                code: KeyCode::Enter,
                ..
            } if self.focus_pane == 0 => {
                self.focus_pane = 1;
                ModelTuiAction::None
            }
            KeyEvent {
                code: KeyCode::Enter,
                ..
            } => {
                if self.submit_selected_model().is_some() {
                    ModelTuiAction::Submit
                } else {
                    ModelTuiAction::None
                }
            }
            _ => ModelTuiAction::None,
        }
    }

    fn edit_add_field(&mut self, edit: impl FnOnce(&mut String)) {
        if self.add_focus == 0 {
            edit(&mut self.add_base_url);
        } else {
            edit(&mut self.add_api_key);
        }
    }

    fn continue_from_gateway(&mut self) {
        if self.add_base_url.trim().is_empty() || self.add_api_key.trim().is_empty() {
            self.status =
                Some(t("baseUrl and key are required.", "baseUrl 和 key 是必需的。").to_string());
            return;
        }
        let base_url = self.add_base_url.clone();
        let api_key = self.add_api_key.clone();
        self.add_gateway(&base_url, &api_key);
        self.add_base_url.clear();
        self.add_api_key.clear();
        self.add_focus = 0;
        self.focus_pane = 0;
        self.mode = ModelTuiMode::Configure;
        self.status = Some(
            t(
                "Gateway added. Probing model candidates.",
                "网关已添加，正在探测候选模型。",
            )
            .to_string(),
        );
    }

    fn current_agent(&self) -> Option<&AgentProviderEntry> {
        self.agents.get(self.agent_focus)
    }

    fn current_provider(&self) -> Option<&ProviderRecord> {
        self.gateways.get(self.provider_focus)
    }

    fn is_add_gateway_focused(&self) -> bool {
        self.provider_focus == self.gateways.len()
    }

    fn provider_row_count(&self) -> usize {
        self.gateways.len() + 1
    }

    fn current_provider_model_fetch_in_flight(&self) -> bool {
        self.current_provider()
            .and_then(provider_fetch_key)
            .map(|key| self.model_fetch_in_flight.contains(&key))
            .unwrap_or(false)
    }

    fn current_model(&self) -> Option<&ModelChoice> {
        self.current_provider()?.models.get(self.model_focus)
    }

    fn current_probe_key(&self, model: &ModelChoice) -> Option<String> {
        Some(probe_key(
            self.current_agent()?,
            self.current_provider()?,
            model,
        ))
    }

    fn model_probe_result(&self, model: &ModelChoice) -> ProbeResult {
        self.current_probe_key(model)
            .and_then(|key| self.probe_results.get(&key).copied())
            .unwrap_or(ProbeResult {
                status: model.status,
                protocol: model.protocol,
            })
    }

    fn drain_probe_results(&mut self, probe_rx: &Receiver<ProbeMessage>) {
        while let Ok(message) = probe_rx.try_recv() {
            self.in_flight.remove(&message.key);
            self.probe_results.insert(message.key, message.result);
        }
    }

    fn drain_model_fetch_results(&mut self, model_fetch_rx: &Receiver<ModelFetchMessage>) {
        while let Ok(message) = model_fetch_rx.try_recv() {
            self.model_fetch_in_flight.remove(&message.key);
            self.model_fetch_attempted.insert(message.key.clone());
            if message.models.is_empty() {
                continue;
            }
            let Some(provider) = self.gateways.iter_mut().find(|provider| {
                provider_fetch_key(provider).as_deref() == Some(message.key.as_str())
            }) else {
                continue;
            };
            merge_model_choices(&mut provider.models, message.models);
            self.model_focus = self
                .model_focus
                .min(provider.models.len().saturating_sub(1));
            self.ensure_model_visible(DEFAULT_MODEL_LIST_HEIGHT);
        }
    }

    fn schedule_current_model_fetch(&mut self, model_fetch_tx: &Sender<ModelFetchMessage>) {
        if self.mode != ModelTuiMode::Configure || self.is_add_gateway_focused() {
            return;
        }
        let Some(provider) = self.current_provider().cloned() else {
            return;
        };
        let Some(key) = provider_fetch_key(&provider) else {
            return;
        };
        if self.model_fetch_in_flight.contains(&key) || self.model_fetch_attempted.contains(&key) {
            return;
        }
        self.model_fetch_in_flight.insert(key.clone());
        let tx = model_fetch_tx.clone();
        thread::spawn(move || {
            let models = fetch_gateway_models(&provider);
            let _ = tx.send(ModelFetchMessage { key, models });
        });
    }

    fn schedule_current_probes(&mut self, probe_tx: &Sender<ProbeMessage>) {
        if self.mode != ModelTuiMode::Configure {
            return;
        }
        let Some(agent) = self.current_agent().cloned() else {
            return;
        };
        let Some(provider) = self.current_provider().cloned() else {
            return;
        };
        let Some(api_key) = provider
            .api_key
            .clone()
            .filter(|value| !value.trim().is_empty())
        else {
            self.mark_current_models_unavailable();
            return;
        };
        if provider.base_url.trim().is_empty() {
            self.mark_current_models_unavailable();
            return;
        }

        let available_slots = MODEL_PROBE_CONCURRENCY.saturating_sub(self.in_flight.len());
        if available_slots == 0 {
            return;
        }
        let model_indexes = probe_candidate_indexes(provider.models.len(), self.model_focus);
        let tasks = provider
            .models
            .iter()
            .enumerate()
            .filter(|(index, _)| model_indexes.contains(index))
            .filter(|(_, model)| self.model_probe_result(model).status == ModelProbeStatus::Testing)
            .filter_map(|(_, model)| {
                let key = probe_key(&agent, &provider, model);
                if self.in_flight.contains(&key) || self.probe_results.contains_key(&key) {
                    return None;
                }
                Some((key, model.clone(), probe_requests_for_agent(&agent, model)))
            })
            .take(available_slots)
            .collect::<Vec<_>>();

        for (key, model, requests) in tasks {
            self.in_flight.insert(key.clone());
            let tx = probe_tx.clone();
            let provider = provider.clone();
            let api_key = api_key.clone();
            thread::spawn(move || {
                let protocol = probe_model_with_requests(&provider, &api_key, &model, &requests);
                let status = if protocol.is_some() {
                    ModelProbeStatus::Available
                } else {
                    ModelProbeStatus::Unavailable
                };
                let _ = tx.send(ProbeMessage {
                    key,
                    result: ProbeResult { status, protocol },
                });
            });
        }
    }

    fn mark_current_models_unavailable(&mut self) {
        let Some(agent) = self.current_agent().cloned() else {
            return;
        };
        let Some(provider) = self.current_provider().cloned() else {
            return;
        };
        for model in &provider.models {
            let key = probe_key(&agent, &provider, model);
            self.probe_results.insert(
                key,
                ProbeResult {
                    status: ModelProbeStatus::Unavailable,
                    protocol: None,
                },
            );
        }
    }

    fn add_gateway(&mut self, base_url: &str, api_key: &str) {
        if self.agents.is_empty() {
            self.agents.push(AgentProviderEntry {
                agent_name: "sentra".to_string(),
                agent_title: "Sentra".to_string(),
                probe_requests: default_probe_requests(),
            });
        }
        let mut provider = ProviderRecord {
            name: provider_name(base_url.trim()),
            raw_provider_id: None,
            base_url: base_url.trim().to_string(),
            api_key: Some(api_key.trim().to_string()),
            enabled: true,
            models: Vec::new(),
            temporary: true,
        };
        provider.models = fetch_gateway_models(&provider);
        if let Some(key) = provider_fetch_key(&provider) {
            self.model_fetch_attempted.insert(key);
        }
        self.gateways.push(provider);
        self.provider_focus = self.gateways.len().saturating_sub(1);
        self.model_focus = 0;
    }

    fn move_configure_focus(&mut self, delta: isize) {
        match self.focus_pane {
            0 => {
                let previous = self.provider_focus;
                self.provider_focus =
                    move_index(self.provider_focus, self.provider_row_count(), delta);
                if self.provider_focus != previous {
                    self.model_focus = 0;
                    self.model_scroll = 0;
                }
            }
            _ => {
                if self.is_add_gateway_focused() {
                    return;
                }
                let len = self
                    .current_provider()
                    .map(|provider| provider.models.len())
                    .unwrap_or(0);
                self.model_focus = move_index(self.model_focus, len, delta);
                self.ensure_model_visible(DEFAULT_MODEL_LIST_HEIGHT);
            }
        }
        self.status = None;
    }

    fn configured_provider_count_for(&self, agent: &AgentProviderEntry) -> usize {
        self.configured_gateways
            .get(&agent.agent_name)
            .map(BTreeSet::len)
            .unwrap_or(0)
    }

    fn selected_agent_has_provider(&self, provider: &ProviderRecord) -> bool {
        let Some(agent) = self.current_agent() else {
            return false;
        };
        self.configured_gateways
            .get(&agent.agent_name)
            .map(|gateways| gateways.contains(&provider_config_key(&provider.base_url)))
            .unwrap_or(false)
    }

    #[cfg(test)]
    fn configured_provider_count_for_current_agent(&self) -> usize {
        self.current_agent()
            .map(|agent| self.configured_provider_count_for(agent))
            .unwrap_or(0)
    }

    #[cfg(test)]
    fn current_provider_is_configured_for_selected_agent(&self) -> bool {
        self.current_provider()
            .map(|provider| self.selected_agent_has_provider(provider))
            .unwrap_or(false)
    }

    fn jump_available_model(&mut self, direction: isize) {
        let Some(provider) = self.current_provider() else {
            return;
        };
        let available = provider
            .models
            .iter()
            .enumerate()
            .filter(|(_, model)| {
                self.model_probe_result(model).status == ModelProbeStatus::Available
            })
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        if available.is_empty() {
            self.status = Some(
                t(
                    "No available model in this gateway yet.",
                    "此网关暂时没有可用模型。",
                )
                .to_string(),
            );
            return;
        }
        let next = if direction < 0 {
            available
                .iter()
                .rev()
                .copied()
                .find(|index| *index < self.model_focus)
                .unwrap_or_else(|| *available.last().unwrap())
        } else {
            available
                .iter()
                .copied()
                .find(|index| *index > self.model_focus)
                .unwrap_or(available[0])
        };
        self.model_focus = next;
        self.ensure_model_visible(DEFAULT_MODEL_LIST_HEIGHT);
        self.status = None;
    }

    fn ensure_model_visible(&mut self, height: usize) {
        self.model_scroll = scroll_for_focus(self.model_focus, self.model_scroll, height);
    }

    fn ensure_agent_visible(&mut self, height: usize) {
        self.agent_scroll = scroll_for_focus(self.agent_focus, self.agent_scroll, height);
    }

    fn submit_selected_model(&mut self) -> Option<ModelConfigInput> {
        let agent = self.current_agent()?;
        let provider = self.current_provider()?;
        let model = self.current_model()?;
        let result = self.model_probe_result(model);
        if result.status == ModelProbeStatus::Unavailable {
            self.status = Some(
                t(
                    "Unavailable models cannot be configured.",
                    "不能配置不可用模型。",
                )
                .to_string(),
            );
            return None;
        }
        let api_key = provider.api_key.clone().unwrap_or_default();
        if provider.base_url.trim().is_empty() || api_key.trim().is_empty() {
            self.status = Some(
                t(
                    "Selected provider is missing baseUrl or key.",
                    "所选供应商缺少 baseUrl 或 key。",
                )
                .to_string(),
            );
            return None;
        }
        Some(ModelConfigInput {
            agent: agent.agent_name.clone(),
            provider_name: Some(provider.name.clone()),
            raw_provider_id: provider.raw_provider_id.clone(),
            base_url: provider.base_url.clone(),
            api_key,
            model: model.id.clone(),
            protocol: result.protocol.or(model.protocol),
        })
    }
}

fn move_index(current: usize, len: usize, delta: isize) -> usize {
    if len == 0 {
        return 0;
    }
    if delta < 0 {
        current.saturating_sub(delta.unsigned_abs()).min(len - 1)
    } else {
        current.saturating_add(delta as usize).min(len - 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::style::Color;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn claude_alias_mutates_only_cli_provider() {
        let home = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(home.path().join(".claude")).unwrap();
        std::fs::create_dir_all(home.path().join("AppData/Local/Claude")).unwrap();
        let mut seen = Vec::new();

        mutate_provider_at(home.path(), "claude", |asset| {
            seen.push(asset.agent_name().to_string());
            Ok(sentra_lib::interfaces::AssetMutationResult::changed())
        })
        .unwrap();

        assert_eq!(seen, ["claude-cli"]);
    }

    #[test]
    fn model_inventory_skips_agent_with_corrupt_provider_config() {
        let home = tempfile::tempdir().unwrap();
        let claude_home = home.path().join(".claude");
        std::fs::create_dir_all(&claude_home).unwrap();
        std::fs::write(
            claude_home.join("settings.json"),
            r#"{"env": {"ANTHROPIC_BASE_URL": "https://example.test"}"#,
        )
        .unwrap();

        let codex_home = home.path().join(".codex");
        std::fs::create_dir_all(&codex_home).unwrap();
        std::fs::write(
            codex_home.join("config.toml"),
            r#"model_provider = "test"
model = "gpt-test"

[model_providers.test]
name = "Test"
base_url = "https://provider.example.test/v1"
experimental_bearer_token = "sk-test"
"#,
        )
        .unwrap();

        let (records, _) = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(collect_model_inventory_at(home.path()))
            .unwrap();

        assert!(records.iter().any(|record| {
            record.agent_name == "codex-cli"
                && record.base_url.as_deref() == Some("https://provider.example.test/v1")
        }));
    }

    fn assert_cell_fg(backend: &TestBackend, text: &str, color: Color) {
        let (x, y) = find_text(backend, text);
        assert_eq!(backend.buffer()[(x, y)].fg, color, "{text}");
    }

    fn assert_cell_bg(backend: &TestBackend, text: &str, color: Color) {
        let (x, y) = find_text(backend, text);
        assert_eq!(backend.buffer()[(x, y)].bg, color, "{text}");
    }

    fn find_text(backend: &TestBackend, needle: &str) -> (u16, u16) {
        let buffer = backend.buffer();
        for y in 0..buffer.area.height {
            let mut line = String::new();
            for x in 0..buffer.area.width {
                line.push_str(buffer[(x, y)].symbol());
            }
            if let Some(x) = line.find(needle) {
                return (x as u16, y);
            }
        }
        panic!("did not find {needle:?}");
    }

    struct ProbeServer {
        base_url: String,
        rx: Receiver<ObservedProbeRequest>,
        handle: thread::JoinHandle<()>,
    }

    impl ProbeServer {
        fn request(self) -> ObservedProbeRequest {
            let request = self.rx.recv().unwrap();
            self.handle.join().unwrap();
            request
        }
    }

    struct ObservedProbeRequest {
        path: String,
        body: serde_json::Value,
    }

    fn run_probe_server(status: u16, body: &str) -> ProbeServer {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let base_url = format!("http://{}", listener.local_addr().unwrap());
        let body = body.to_string();
        let (tx, rx) = mpsc::channel();
        let handle = thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            handle_probe_connection(stream, status, &body, tx);
        });
        ProbeServer {
            base_url,
            rx,
            handle,
        }
    }

    fn handle_probe_connection(
        mut stream: std::net::TcpStream,
        status: u16,
        response_body: &str,
        tx: Sender<ObservedProbeRequest>,
    ) {
        use std::io::{BufRead, BufReader, Read, Write};

        let mut reader = BufReader::new(stream.try_clone().unwrap());
        let mut request_line = String::new();
        reader.read_line(&mut request_line).unwrap();
        let path = request_line
            .split_whitespace()
            .nth(1)
            .unwrap_or_default()
            .to_string();

        let mut content_length = 0usize;
        loop {
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            let trimmed = line.trim_end_matches(['\r', '\n']);
            if trimmed.is_empty() {
                break;
            }
            if let Some((key, value)) = trimmed.split_once(':')
                && key.trim().eq_ignore_ascii_case("content-length")
            {
                content_length = value.trim().parse().unwrap();
            }
        }

        let mut request_body = vec![0; content_length];
        reader.read_exact(&mut request_body).unwrap();
        let request_body = if request_body.is_empty() {
            serde_json::Value::Null
        } else {
            serde_json::from_slice(&request_body).unwrap()
        };
        tx.send(ObservedProbeRequest {
            path,
            body: request_body,
        })
        .unwrap();

        let reason = if status == 200 { "OK" } else { "Error" };
        write!(
            stream,
            "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            response_body.len(),
            response_body
        )
        .unwrap();
    }

    #[test]
    fn model_tui_starts_at_agent_select() {
        let state = ModelTuiState::new();

        assert_eq!(state.mode, ModelTuiMode::AgentSelect);
        assert_eq!(state.focus_pane, 0);
    }

    #[test]
    fn model_tui_agent_select_uses_theme_colors_for_visible_chrome() {
        let records = vec![ModelRecord {
            agent_name: "codex-cli".to_string(),
            agent_title: "Codex".to_string(),
            agent_home: PathBuf::from("/home/codex"),
            provider_name: "gateway.example.test".to_string(),
            provider_type: "gateway".to_string(),
            account: "-".to_string(),
            base_url: Some("https://gateway.example.test/api".to_string()),
            enabled: true,
            has_api_key: true,
            model: "gpt-test".to_string(),
            protocol: Some("responses".to_string()),
        }];
        let state = ModelTuiState::with_records_and_catalog(
            &records,
            ModelCatalog {
                agents: vec![AgentProviderEntry {
                    agent_name: "codex-cli".to_string(),
                    agent_title: "Codex".to_string(),
                    probe_requests: Vec::new(),
                }],
                gateways: Vec::new(),
            },
        );
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| render_model_tui(frame, &records, &state))
            .unwrap();

        let backend = terminal.backend();
        assert_cell_fg(
            backend,
            "Sentra Model",
            theme::title_style().fg.expect("title color"),
        );
        assert_cell_fg(
            backend,
            t("Agents", "Agent"),
            theme::focus_style().fg.expect("focus color"),
        );
        assert_cell_fg(
            backend,
            "codex-cli",
            theme::body_style().fg.expect("body color"),
        );
        assert_cell_fg(
            backend,
            "gateway.example.test",
            theme::secondary_style().fg.expect("secondary color"),
        );
        assert_cell_fg(
            backend,
            "[Enter]",
            theme::muted_style().fg.expect("muted color"),
        );
    }

    #[test]
    fn enter_on_agent_select_opens_model_configure_view() {
        let mut state = ModelTuiState::with_catalog(ModelCatalog {
            agents: vec![AgentProviderEntry {
                agent_name: "codex-cli".to_string(),
                agent_title: "Codex".to_string(),
                probe_requests: Vec::new(),
            }],
            gateways: Vec::new(),
        });

        state.handle_key(key(KeyCode::Enter));

        assert_eq!(state.mode, ModelTuiMode::Configure);
    }

    #[test]
    fn agent_select_scrolls_to_keep_focused_agent_visible() {
        let agents = (0..10)
            .map(|index| AgentProviderEntry {
                agent_name: format!("agent-{index:02}"),
                agent_title: format!("Agent {index:02}"),
                probe_requests: Vec::new(),
            })
            .collect::<Vec<_>>();
        let mut state = ModelTuiState::with_catalog(ModelCatalog {
            agents,
            gateways: Vec::new(),
        });

        for _ in 0..8 {
            state.handle_key(key(KeyCode::Down));
        }

        assert_eq!(state.agent_focus, 8);
        assert_eq!(state.agent_scroll, 1);

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| render_model_tui(frame, &[], &state))
            .unwrap();

        find_text(terminal.backend(), "Agent 08");
        find_text(terminal.backend(), "9.");
    }

    #[test]
    fn add_gateway_dialog_requires_base_url_and_key_before_configuring() {
        let mut state = ModelTuiState::with_catalog(ModelCatalog {
            agents: vec![AgentProviderEntry {
                agent_name: "codex-cli".to_string(),
                agent_title: "Codex".to_string(),
                probe_requests: Vec::new(),
            }],
            gateways: Vec::new(),
        });
        state.handle_key(key(KeyCode::Enter));
        state.handle_key(key(KeyCode::Enter));
        assert_eq!(state.mode, ModelTuiMode::AddGatewayDialog);

        state.handle_key(key(KeyCode::Enter));
        state.handle_key(key(KeyCode::Enter));

        assert_eq!(state.mode, ModelTuiMode::AddGatewayDialog);
        assert_eq!(
            state.status.as_deref(),
            Some(t(
                "baseUrl and key are required.",
                "baseUrl 和 key 是必需的。"
            ))
        );
    }

    #[test]
    fn enter_on_add_gateway_base_url_moves_to_key_field() {
        let mut state = ModelTuiState::with_catalog(ModelCatalog {
            agents: vec![AgentProviderEntry {
                agent_name: "codex-cli".to_string(),
                agent_title: "Codex".to_string(),
                probe_requests: Vec::new(),
            }],
            gateways: Vec::new(),
        });
        state.handle_key(key(KeyCode::Enter));
        state.handle_key(key(KeyCode::Enter));
        state.add_base_url = "https://gateway.example.test/api".to_string();

        state.handle_key(key(KeyCode::Enter));

        assert_eq!(state.mode, ModelTuiMode::AddGatewayDialog);
        assert_eq!(state.add_focus, 1);
        assert_eq!(state.gateways.len(), 0);
        assert_eq!(state.status, None);
    }

    #[test]
    fn escape_moves_back_one_tui_level() {
        let mut state = ModelTuiState::with_catalog(ModelCatalog {
            agents: vec![AgentProviderEntry {
                agent_name: "codex-cli".to_string(),
                agent_title: "Codex".to_string(),
                probe_requests: Vec::new(),
            }],
            gateways: Vec::new(),
        });
        state.handle_key(key(KeyCode::Enter));
        state.handle_key(key(KeyCode::Enter));
        assert_eq!(state.mode, ModelTuiMode::AddGatewayDialog);

        assert_eq!(state.handle_key(key(KeyCode::Esc)), ModelTuiAction::None);
        assert_eq!(state.mode, ModelTuiMode::Configure);
        assert_eq!(state.handle_key(key(KeyCode::Esc)), ModelTuiAction::None);
        assert_eq!(state.mode, ModelTuiMode::AgentSelect);
        assert_eq!(state.handle_key(key(KeyCode::Esc)), ModelTuiAction::Cancel);
    }

    #[test]
    fn add_gateway_continues_into_configure_model_with_added_gateway() {
        let mut state = ModelTuiState::with_catalog(ModelCatalog {
            agents: vec![AgentProviderEntry {
                agent_name: "codex-cli".to_string(),
                agent_title: "Codex".to_string(),
                probe_requests: Vec::new(),
            }],
            gateways: Vec::new(),
        });
        state.handle_key(key(KeyCode::Enter));
        state.handle_key(key(KeyCode::Enter));
        state.add_base_url = " https://gateway.example.test/api ".to_string();
        state.add_api_key = " sk-test ".to_string();

        state.handle_key(key(KeyCode::Enter));
        state.handle_key(key(KeyCode::Enter));

        assert_eq!(state.mode, ModelTuiMode::Configure);
        assert_eq!(state.focus_pane, 0);
        assert_eq!(
            state.current_provider().unwrap().base_url,
            "https://gateway.example.test/api"
        );
        assert_eq!(
            state.current_provider().unwrap().api_key.as_deref(),
            Some("sk-test")
        );
    }

    #[test]
    fn configured_provider_is_marked_for_selected_agent() {
        let records = vec![ModelRecord {
            agent_name: "codex-cli".to_string(),
            agent_title: "Codex".to_string(),
            agent_home: PathBuf::from("/home/codex"),
            provider_name: "gateway".to_string(),
            provider_type: "gateway".to_string(),
            account: "-".to_string(),
            base_url: Some(" https://gateway.example.test/api ".to_string()),
            enabled: true,
            has_api_key: true,
            model: "gpt-test".to_string(),
            protocol: Some("responses".to_string()),
        }];
        let state = ModelTuiState::with_records_and_catalog(
            &records,
            ModelCatalog {
                agents: vec![AgentProviderEntry {
                    agent_name: "codex-cli".to_string(),
                    agent_title: "Codex".to_string(),
                    probe_requests: Vec::new(),
                }],
                gateways: vec![ProviderRecord {
                    name: "gateway".to_string(),
                    raw_provider_id: None,
                    base_url: "https://gateway.example.test/api".to_string(),
                    api_key: Some("sk-test".to_string()),
                    enabled: true,
                    models: Vec::new(),
                    temporary: false,
                }],
            },
        );

        assert_eq!(state.configured_provider_count_for_current_agent(), 1);
        assert!(state.current_provider_is_configured_for_selected_agent());
        assert_eq!(
            gateway_column_title(&state, state.current_provider().unwrap()),
            "*  gateway  0/0"
        );
    }

    #[test]
    fn provider_line_uses_fixed_columns_and_truncates_long_names() {
        let records = vec![ModelRecord {
            agent_name: "codex-cli".to_string(),
            agent_title: "Codex".to_string(),
            agent_home: PathBuf::from("/home/codex"),
            provider_name: "gateway".to_string(),
            provider_type: "gateway".to_string(),
            account: "-".to_string(),
            base_url: Some("https://ai-api-gateway.app.baizhi.cloud/api/openai".to_string()),
            enabled: true,
            has_api_key: true,
            model: "gpt-test".to_string(),
            protocol: Some("responses".to_string()),
        }];
        let state = ModelTuiState::with_records_and_catalog(
            &records,
            ModelCatalog {
                agents: vec![AgentProviderEntry {
                    agent_name: "codex-cli".to_string(),
                    agent_title: "Codex".to_string(),
                    probe_requests: Vec::new(),
                }],
                gateways: vec![ProviderRecord {
                    name: "ai-api-gateway.app.baizhi.cloud".to_string(),
                    raw_provider_id: None,
                    base_url: "https://ai-api-gateway.app.baizhi.cloud/api/openai".to_string(),
                    api_key: Some("sk-test".to_string()),
                    enabled: true,
                    models: Vec::new(),
                    temporary: false,
                }],
            },
        );
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                render_provider_column(
                    frame,
                    Rect {
                        x: 0,
                        y: 0,
                        width: 52,
                        height: 5,
                    },
                    &state,
                )
            })
            .unwrap();
        let buffer = terminal.backend().buffer();
        let rendered = (0..buffer.area.height)
            .map(|y| {
                let mut line = String::new();
                for x in 0..buffer.area.width {
                    line.push_str(buffer[(x, y)].symbol());
                }
                line
            })
            .collect::<Vec<_>>()
            .join("\n");

        assert!(rendered.contains("1."));
        assert!(rendered.contains("ai-api-gateway.app"));
        assert!(rendered.contains("0/0"));
        assert_cell_bg(
            terminal.backend(),
            "ai-api-gateway.app",
            theme::selection_style().bg.expect("selection color"),
        );
    }

    #[test]
    fn selected_provider_keeps_inactive_background_when_model_pane_is_focused() {
        let state = {
            let mut state = ModelTuiState::with_catalog(ModelCatalog {
                agents: vec![AgentProviderEntry {
                    agent_name: "codex-cli".to_string(),
                    agent_title: "Codex".to_string(),
                    probe_requests: Vec::new(),
                }],
                gateways: vec![ProviderRecord {
                    name: "gateway".to_string(),
                    raw_provider_id: None,
                    base_url: "https://gateway.example.test/api".to_string(),
                    api_key: Some("sk-test".to_string()),
                    enabled: true,
                    models: Vec::new(),
                    temporary: false,
                }],
            });
            state.focus_pane = 1;
            state
        };
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                render_provider_column(
                    frame,
                    Rect {
                        x: 0,
                        y: 0,
                        width: 40,
                        height: 5,
                    },
                    &state,
                )
            })
            .unwrap();

        assert_cell_bg(
            terminal.backend(),
            "gateway",
            theme::inactive_selection_style()
                .bg
                .expect("inactive selection color"),
        );
    }

    #[test]
    fn fetched_models_are_merged_into_selected_provider() {
        let mut state = ModelTuiState::with_catalog(ModelCatalog {
            agents: vec![AgentProviderEntry {
                agent_name: "codex-cli".to_string(),
                agent_title: "Codex".to_string(),
                probe_requests: Vec::new(),
            }],
            gateways: vec![ProviderRecord {
                name: "gateway".to_string(),
                raw_provider_id: None,
                base_url: "https://gateway.example.test/api".to_string(),
                api_key: Some("sk-test".to_string()),
                enabled: true,
                models: vec![ModelChoice {
                    id: "configured-model".to_string(),
                    name: "configured-model".to_string(),
                    enabled: true,
                    status: ModelProbeStatus::Testing,
                    protocol: Some(WireProtocol::Responses),
                }],
                temporary: false,
            }],
        });
        let key = provider_fetch_key(state.current_provider().unwrap()).unwrap();
        let (tx, rx) = mpsc::channel();
        tx.send(ModelFetchMessage {
            key: key.clone(),
            models: vec![
                ModelChoice {
                    id: "fetched-a".to_string(),
                    name: "fetched-a".to_string(),
                    enabled: true,
                    status: ModelProbeStatus::Testing,
                    protocol: None,
                },
                ModelChoice {
                    id: "configured-model".to_string(),
                    name: "configured-model".to_string(),
                    enabled: true,
                    status: ModelProbeStatus::Testing,
                    protocol: None,
                },
            ],
        })
        .unwrap();

        state.drain_model_fetch_results(&rx);

        assert!(state.model_fetch_attempted.contains(&key));
        let ids = state.gateways[0]
            .models
            .iter()
            .map(|model| model.id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(ids, ["configured-model", "fetched-a"]);
        assert_eq!(
            state.gateways[0].models[1].protocol,
            Some(WireProtocol::Responses)
        );
    }

    #[test]
    fn model_catalog_keeps_agent_provider_model_columns() {
        let mut state = ModelTuiState::with_catalog(ModelCatalog {
            agents: vec![AgentProviderEntry {
                agent_name: "codex-cli".to_string(),
                agent_title: "Codex".to_string(),
                probe_requests: Vec::new(),
            }],
            gateways: vec![ProviderRecord {
                name: "gateway".to_string(),
                raw_provider_id: None,
                base_url: "https://gateway.example.test/api".to_string(),
                api_key: Some("sk-test".to_string()),
                enabled: true,
                models: vec![ModelChoice {
                    id: "gpt-test".to_string(),
                    name: "gpt-test".to_string(),
                    enabled: true,
                    status: ModelProbeStatus::Testing,
                    protocol: None,
                }],
                temporary: false,
            }],
        });
        state.mode = ModelTuiMode::Configure;

        assert_eq!(state.current_agent().unwrap().agent_name, "codex-cli");
        assert_eq!(state.current_provider().unwrap().name, "gateway");
        assert_eq!(state.current_model().unwrap().id, "gpt-test");
    }

    #[test]
    fn add_gateway_appends_temporary_provider_to_current_agent() {
        let mut state = ModelTuiState::with_catalog(ModelCatalog {
            agents: vec![AgentProviderEntry {
                agent_name: "codex-cli".to_string(),
                agent_title: "Codex".to_string(),
                probe_requests: Vec::new(),
            }],
            gateways: Vec::new(),
        });

        state.add_gateway(" https://gateway.example.test/api ", " sk-test ");

        let provider = state.current_provider().unwrap();
        assert_eq!(provider.base_url, "https://gateway.example.test/api");
        assert_eq!(provider.api_key.as_deref(), Some("sk-test"));
        assert!(provider.temporary);
    }

    #[test]
    fn add_gateway_uses_models_from_new_gateway_not_existing_gateway_union() {
        let server = run_probe_server(200, r#"{"data":[{"id":"fresh-gpt"},{"id":"fresh-mini"}]}"#);
        let base_url = server.base_url.clone();
        let mut state = ModelTuiState::with_catalog(ModelCatalog {
            agents: vec![AgentProviderEntry {
                agent_name: "codex-cli".to_string(),
                agent_title: "Codex".to_string(),
                probe_requests: Vec::new(),
            }],
            gateways: vec![ProviderRecord {
                name: "existing".to_string(),
                raw_provider_id: None,
                base_url: "https://existing.example.test/api".to_string(),
                api_key: Some("sk-existing".to_string()),
                enabled: true,
                models: vec![
                    ModelChoice {
                        id: "existing-a".to_string(),
                        name: "existing-a".to_string(),
                        enabled: true,
                        status: ModelProbeStatus::Testing,
                        protocol: None,
                    },
                    ModelChoice {
                        id: "existing-b".to_string(),
                        name: "existing-b".to_string(),
                        enabled: true,
                        status: ModelProbeStatus::Testing,
                        protocol: None,
                    },
                ],
                temporary: false,
            }],
        });

        state.add_gateway(&base_url, "sk-new");

        let request = server.request();
        assert_eq!(request.path, "/models");
        let provider = state.current_provider().unwrap();
        let model_ids = provider
            .models
            .iter()
            .map(|model| model.id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(model_ids, ["fresh-gpt", "fresh-mini"]);
        assert_eq!(gateway_column_title(&state, provider), "+  127.0.0.1  0/2");
    }

    #[test]
    fn add_gateway_does_not_fallback_to_existing_models_when_fetch_fails() {
        let mut state = ModelTuiState::with_catalog(ModelCatalog {
            agents: vec![AgentProviderEntry {
                agent_name: "codex-cli".to_string(),
                agent_title: "Codex".to_string(),
                probe_requests: Vec::new(),
            }],
            gateways: vec![ProviderRecord {
                name: "existing".to_string(),
                raw_provider_id: None,
                base_url: "https://existing.example.test/api".to_string(),
                api_key: Some("sk-existing".to_string()),
                enabled: true,
                models: vec![ModelChoice {
                    id: "existing-a".to_string(),
                    name: "existing-a".to_string(),
                    enabled: true,
                    status: ModelProbeStatus::Testing,
                    protocol: None,
                }],
                temporary: false,
            }],
        });

        state.add_gateway("not-a-real-provider", "sk-new");

        let provider = state.current_provider().unwrap();
        assert_eq!(provider.base_url, "not-a-real-provider");
        assert!(provider.models.is_empty());
    }

    #[test]
    fn submit_rejects_unavailable_model_but_allows_pending_probe() {
        let mut state = ModelTuiState::with_catalog(ModelCatalog {
            agents: vec![AgentProviderEntry {
                agent_name: "codex-cli".to_string(),
                agent_title: "Codex".to_string(),
                probe_requests: Vec::new(),
            }],
            gateways: vec![ProviderRecord {
                name: "gateway".to_string(),
                raw_provider_id: Some("gateway-id".to_string()),
                base_url: "https://gateway.example.test/api".to_string(),
                api_key: Some("sk-test".to_string()),
                enabled: true,
                models: vec![ModelChoice {
                    id: "gpt-test".to_string(),
                    name: "gpt-test".to_string(),
                    enabled: true,
                    status: ModelProbeStatus::Unavailable,
                    protocol: None,
                }],
                temporary: false,
            }],
        });
        state.mode = ModelTuiMode::Configure;

        assert_eq!(state.submit_selected_model(), None);
        state.gateways[0].models[0].status = ModelProbeStatus::Testing;
        state.gateways[0].models[0].protocol = Some(sentra_lib::protocol::WireProtocol::Responses);

        assert_eq!(
            state.submit_selected_model(),
            Some(ModelConfigInput {
                agent: "codex-cli".to_string(),
                provider_name: Some("gateway".to_string()),
                raw_provider_id: Some("gateway-id".to_string()),
                base_url: "https://gateway.example.test/api".to_string(),
                api_key: "sk-test".to_string(),
                model: "gpt-test".to_string(),
                protocol: Some(sentra_lib::protocol::WireProtocol::Responses),
            })
        );
    }

    #[test]
    fn gateways_are_aggregated_across_agents() {
        let state = ModelTuiState::with_catalog(ModelCatalog {
            agents: vec![
                AgentProviderEntry {
                    agent_name: "codex-cli".to_string(),
                    agent_title: "Codex".to_string(),
                    probe_requests: Vec::new(),
                },
                AgentProviderEntry {
                    agent_name: "claude".to_string(),
                    agent_title: "Claude".to_string(),
                    probe_requests: Vec::new(),
                },
            ],
            gateways: vec![ProviderRecord {
                name: "gateway".to_string(),
                raw_provider_id: None,
                base_url: "https://gateway.example.test/api".to_string(),
                api_key: Some("sk-test".to_string()),
                enabled: true,
                models: Vec::new(),
                temporary: false,
            }],
        });

        assert_eq!(state.agents.len(), 2);
        assert_eq!(state.gateways.len(), 1);
        assert_eq!(state.current_provider().unwrap().name, "gateway");
    }

    #[test]
    fn scan_model_catalog_keeps_only_sentra_agent_and_all_gateways() {
        let catalog = ModelCatalog {
            agents: vec![
                AgentProviderEntry {
                    agent_name: "codex-cli".to_string(),
                    agent_title: "Codex".to_string(),
                    probe_requests: default_probe_requests(),
                },
                AgentProviderEntry {
                    agent_name: "sentra".to_string(),
                    agent_title: "Sentra".to_string(),
                    probe_requests: default_probe_requests(),
                },
            ],
            gateways: vec![
                ProviderRecord {
                    name: "codex-gateway".to_string(),
                    raw_provider_id: None,
                    base_url: "https://codex.example.test/api/openai".to_string(),
                    api_key: Some("sk-codex".to_string()),
                    enabled: true,
                    models: Vec::new(),
                    temporary: false,
                },
                ProviderRecord {
                    name: "sentra-gateway".to_string(),
                    raw_provider_id: None,
                    base_url: "https://sentra.example.test/api/openai".to_string(),
                    api_key: Some("sk-sentra".to_string()),
                    enabled: true,
                    models: Vec::new(),
                    temporary: false,
                },
            ],
        };

        let catalog = sentra_only_catalog(catalog);

        assert_eq!(catalog.agents.len(), 1);
        assert_eq!(catalog.agents[0].agent_name, "sentra");
        assert_eq!(catalog.gateways.len(), 2);
        assert_eq!(catalog.gateways[0].name, "codex-gateway");
        assert_eq!(catalog.gateways[1].name, "sentra-gateway");
    }

    #[test]
    fn add_gateway_appends_to_global_gateways() {
        let mut state = ModelTuiState::with_catalog(ModelCatalog {
            agents: vec![AgentProviderEntry {
                agent_name: "codex-cli".to_string(),
                agent_title: "Codex".to_string(),
                probe_requests: Vec::new(),
            }],
            gateways: Vec::new(),
        });

        state.add_gateway(" https://gateway.example.test/api ", " sk-test ");

        assert_eq!(state.gateways.len(), 1);
        assert_eq!(
            state.current_provider().unwrap().base_url,
            "https://gateway.example.test/api"
        );
    }

    #[test]
    fn agent_title_line_does_not_include_home_path() {
        let agent = AgentProviderEntry {
            agent_name: "codex-cli".to_string(),
            agent_title: "Codex".to_string(),
            probe_requests: Vec::new(),
        };

        assert_eq!(agent_column_title(&agent), "Codex");
    }

    #[test]
    fn model_line_puts_status_after_model_name() {
        let model = ModelChoice {
            id: "gpt-test".to_string(),
            name: "gpt-test".to_string(),
            enabled: true,
            status: ModelProbeStatus::Available,
            protocol: Some(WireProtocol::Responses),
        };

        assert_eq!(
            model_column_title(&model),
            format!(
                "gpt-test  [{}]",
                model_status_label(ModelProbeStatus::Available)
            )
        );
    }

    #[test]
    fn configure_layout_places_status_above_columns() {
        let layout = configure_layout(Rect::new(0, 0, 120, 40));

        assert!(layout.header.y < layout.status.y);
        assert!(layout.status.y < layout.body.y);
        assert!(layout.body.y < layout.footer.y);
        assert_eq!(layout.status.height, 3);
    }

    #[test]
    fn secret_mask_keeps_edges_and_hides_middle() {
        assert_eq!(mask_secret("sk-1234567890"), "sk-1****7890");
        assert_eq!(mask_secret("sk12"), "s****2");
    }

    #[test]
    fn gateway_status_line_shows_full_base_url_and_masked_key() {
        let state = ModelTuiState::with_catalog(ModelCatalog {
            agents: vec![AgentProviderEntry {
                agent_name: "codex-cli".to_string(),
                agent_title: "Codex".to_string(),
                probe_requests: Vec::new(),
            }],
            gateways: vec![ProviderRecord {
                name: "gateway".to_string(),
                raw_provider_id: None,
                base_url: "https://ai-api-gateway.app.baizhi.cloud/api/openai".to_string(),
                api_key: Some("sk-1234567890".to_string()),
                enabled: true,
                models: Vec::new(),
                temporary: false,
            }],
        });

        let line = gateway_status_line(&state);

        assert!(line.contains("Agent: Codex"));
        assert!(line.contains("Base URL: https://ai-api-gateway.app.baizhi.cloud/api/openai"));
        assert!(line.contains("Key: sk-1****7890"));
        assert!(!line.contains("sk-1234567890"));
    }

    #[test]
    fn switch_view_renders_selected_gateway_base_url_and_masked_key() {
        let mut state = ModelTuiState::with_catalog(ModelCatalog {
            agents: vec![AgentProviderEntry {
                agent_name: "codex-cli".to_string(),
                agent_title: "Codex".to_string(),
                probe_requests: Vec::new(),
            }],
            gateways: vec![ProviderRecord {
                name: "gateway".to_string(),
                raw_provider_id: None,
                base_url: "https://ai-api-gateway.app.baizhi.cloud/api/openai".to_string(),
                api_key: Some("sk-1234567890".to_string()),
                enabled: true,
                models: Vec::new(),
                temporary: false,
            }],
        });
        state.mode = ModelTuiMode::Configure;
        state.focus_pane = 1;
        let backend = TestBackend::new(140, 30);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| render_model_tui(frame, &[], &state))
            .unwrap();

        let backend = terminal.backend();
        find_text(backend, "Agent: Codex");
        find_text(
            backend,
            "Base URL: https://ai-api-gateway.app.baizhi.cloud/api/openai",
        );
        find_text(backend, "Key: sk-1****7890");
        let buffer = backend.buffer();
        let rendered = (0..buffer.area.height)
            .map(|y| {
                let mut line = String::new();
                for x in 0..buffer.area.width {
                    line.push_str(buffer[(x, y)].symbol());
                }
                line
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert!(!rendered.contains("sk-1234567890"));
    }

    #[test]
    fn agent_probe_request_body_is_preserved_for_provider_probe() {
        let agent = AgentProviderEntry {
            agent_name: "sentra".to_string(),
            agent_title: "Sentra".to_string(),
            probe_requests: vec![ProviderProbeRequest {
                protocol: WireProtocol::Responses,
                body: Some(format!(r#"{{"model":"{PROBE_MODEL_PLACEHOLDER}"}}"#)),
                prompt: Some(sentra_lib::protocol::ModelPrompt {
                    system: "provider system".to_string(),
                    user: "provider user".to_string(),
                }),
            }],
        };
        let model = ModelChoice {
            id: "gpt-real".to_string(),
            name: "gpt-real".to_string(),
            enabled: true,
            status: ModelProbeStatus::Testing,
            protocol: None,
        };

        let requests = probe_requests_for_agent(&agent, &model);

        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].protocol, WireProtocol::Responses);
        let body: serde_json::Value =
            serde_json::from_str(requests[0].body.as_deref().unwrap()).unwrap();
        assert_eq!(body["model"], "gpt-real");
        assert_eq!(
            requests[0].prompt.as_ref().unwrap().system,
            "provider system"
        );
    }

    #[test]
    fn default_probe_requests_use_single_fast_protocol() {
        let requests = default_probe_requests();

        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].protocol, WireProtocol::ChatCompletions);
    }

    #[test]
    fn probe_candidate_indexes_follow_focused_model_window() {
        let indexes = probe_candidate_indexes(50, 25);

        assert_eq!(indexes.len(), MODEL_PROBE_WINDOW);
        assert!(indexes.contains(&25));
        assert!(indexes.contains(&17));
        assert!(!indexes.contains(&0));
    }

    #[test]
    fn probe_provider_model_sends_custom_body_to_protocol_endpoint() {
        let server = run_probe_server(
            200,
            r#"{"output":[{"content":[{"text":"{\"results\":[]}"}]}]}"#,
        );
        let provider = ProviderRecord {
            name: "test".to_string(),
            raw_provider_id: None,
            base_url: server.base_url.clone(),
            api_key: Some("sk-test".to_string()),
            enabled: true,
            models: Vec::new(),
            temporary: false,
        };
        let request = ProviderProbeRequest {
            protocol: WireProtocol::Responses,
            body: Some(r#"{"model":"gpt-5","input":[]}"#.to_string()),
            prompt: None,
        };

        assert!(probe_provider_model(
            &provider,
            "sk-test",
            "ignored-model",
            &request
        ));

        let observed = server.request();
        assert_eq!(observed.path, "/responses");
        assert_eq!(observed.body["model"], "gpt-5");
    }

    #[test]
    fn probe_provider_model_accepts_custom_title_response_text() {
        let server = run_probe_server(
            200,
            r#"{"choices":[{"message":{"content":"Quick check-in"}}]}"#,
        );
        let provider = ProviderRecord {
            name: "test".to_string(),
            raw_provider_id: None,
            base_url: server.base_url.clone(),
            api_key: Some("sk-test".to_string()),
            enabled: true,
            models: Vec::new(),
            temporary: false,
        };
        let request = ProviderProbeRequest {
            protocol: WireProtocol::ChatCompletions,
            body: Some(r#"{"model":"gpt-5","messages":[]}"#.to_string()),
            prompt: None,
        };

        assert!(probe_provider_model(
            &provider,
            "sk-test",
            "ignored-model",
            &request
        ));

        let observed = server.request();
        assert_eq!(observed.path, "/chat/completions");
        assert_eq!(observed.body["model"], "gpt-5");
    }

    #[test]
    fn model_line_includes_sequence_number_and_status_suffix() {
        let model = ModelChoice {
            id: "gpt-test".to_string(),
            name: "gpt-test".to_string(),
            enabled: true,
            status: ModelProbeStatus::Available,
            protocol: Some(WireProtocol::Responses),
        };

        assert_eq!(
            model_column_title_with_index(&model, 7, ModelProbeStatus::Available),
            format!(
                "7. gpt-test  [{}]",
                model_status_label(ModelProbeStatus::Available)
            )
        );
    }

    #[test]
    fn gateway_line_shows_available_model_ratio() {
        let mut state = model_state_with_three_models();
        state.probe_results.insert(
            state
                .current_probe_key(&state.gateways[0].models[0])
                .unwrap(),
            ProbeResult {
                status: ModelProbeStatus::Available,
                protocol: Some(WireProtocol::Responses),
            },
        );

        assert_eq!(
            gateway_column_title(&state, &state.gateways[0]),
            "   gateway  1/3"
        );
    }

    #[test]
    fn tab_in_model_column_jumps_between_available_models() {
        let mut state = model_state_with_three_models();
        state.mode = ModelTuiMode::Configure;
        state.focus_pane = 1;
        for index in [0, 2] {
            let key = state
                .current_probe_key(&state.gateways[0].models[index])
                .unwrap();
            state.probe_results.insert(
                key,
                ProbeResult {
                    status: ModelProbeStatus::Available,
                    protocol: Some(WireProtocol::Responses),
                },
            );
        }

        state.handle_key(key(KeyCode::Tab));

        assert_eq!(state.model_focus, 2);
    }

    #[test]
    fn probe_results_are_scoped_by_agent_gateway_and_model() {
        let mut state = ModelTuiState::with_catalog(ModelCatalog {
            agents: vec![
                AgentProviderEntry {
                    agent_name: "codex-cli".to_string(),
                    agent_title: "Codex".to_string(),
                    probe_requests: Vec::new(),
                },
                AgentProviderEntry {
                    agent_name: "claude".to_string(),
                    agent_title: "Claude".to_string(),
                    probe_requests: Vec::new(),
                },
            ],
            gateways: vec![ProviderRecord {
                name: "gateway".to_string(),
                raw_provider_id: None,
                base_url: "https://gateway.example.test/api".to_string(),
                api_key: Some("sk-test".to_string()),
                enabled: true,
                models: vec![ModelChoice {
                    id: "gpt-test".to_string(),
                    name: "gpt-test".to_string(),
                    enabled: true,
                    status: ModelProbeStatus::Testing,
                    protocol: None,
                }],
                temporary: false,
            }],
        });
        let key = state
            .current_probe_key(state.current_model().unwrap())
            .unwrap();
        state.probe_results.insert(
            key,
            ProbeResult {
                status: ModelProbeStatus::Available,
                protocol: Some(WireProtocol::Responses),
            },
        );

        assert_eq!(
            state
                .model_probe_result(state.current_model().unwrap())
                .status,
            ModelProbeStatus::Available
        );
        state.agent_focus = 1;
        assert_eq!(
            state
                .model_probe_result(state.current_model().unwrap())
                .status,
            ModelProbeStatus::Testing
        );
    }

    #[test]
    fn probe_scheduler_respects_concurrency_limit() {
        let mut state = ModelTuiState::with_catalog(ModelCatalog {
            agents: vec![AgentProviderEntry {
                agent_name: "codex-cli".to_string(),
                agent_title: "Codex".to_string(),
                probe_requests: Vec::new(),
            }],
            gateways: vec![ProviderRecord {
                name: "gateway".to_string(),
                raw_provider_id: None,
                base_url: "https://gateway.example.test/api".to_string(),
                api_key: Some("sk-test".to_string()),
                enabled: true,
                models: vec![ModelChoice {
                    id: "gpt-test".to_string(),
                    name: "gpt-test".to_string(),
                    enabled: true,
                    status: ModelProbeStatus::Testing,
                    protocol: None,
                }],
                temporary: false,
            }],
        });
        state.mode = ModelTuiMode::Configure;
        state.in_flight = ["a", "b", "c", "d", "e", "f", "g", "h"]
            .into_iter()
            .map(str::to_string)
            .collect();
        let (tx, _rx) = mpsc::channel();

        state.schedule_current_probes(&tx);

        assert_eq!(state.in_flight.len(), MODEL_PROBE_CONCURRENCY);
    }

    fn model_state_with_three_models() -> ModelTuiState {
        ModelTuiState::with_catalog(ModelCatalog {
            agents: vec![AgentProviderEntry {
                agent_name: "codex-cli".to_string(),
                agent_title: "Codex".to_string(),
                probe_requests: Vec::new(),
            }],
            gateways: vec![ProviderRecord {
                name: "gateway".to_string(),
                raw_provider_id: None,
                base_url: "https://gateway.example.test/api".to_string(),
                api_key: Some("sk-test".to_string()),
                enabled: true,
                models: (1..=3)
                    .map(|index| ModelChoice {
                        id: format!("gpt-test-{index}"),
                        name: format!("gpt-test-{index}"),
                        enabled: true,
                        status: ModelProbeStatus::Testing,
                        protocol: None,
                    })
                    .collect(),
                temporary: false,
            }],
        })
    }
}
