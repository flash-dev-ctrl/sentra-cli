use std::collections::BTreeSet;
use std::ffi::OsString;
use std::fmt;
use std::path::PathBuf;

use sentra_lib::interfaces::AssetType;
use sentra_lib::{SentraError, SentraResult};

use crate::cli::i18n::t;

#[derive(Debug)]
pub(crate) enum Command {
    Help,
    Version,
    ListHelp,
    ScanHelp,
    ImportHelp,
    UpdateHelp,
    ModelHelp,
    SkillHelp,
    InstallHelp,
    UninstallHelp,
    List {
        resource: ListResource,
        home: Option<PathBuf>,
        agent: Option<String>,
        output: OutputOptions,
    },
    Scan {
        resource: ScanResource,
        path: Option<PathBuf>,
        agents: Vec<String>,
        enabled_checkers: BTreeSet<ScanChecker>,
        no_cache: bool,
        output: OutputOptions,
    },
    Import {
        sources: Vec<PathBuf>,
    },
    Config {
        action: ConfigAction,
    },
    Rule {
        action: RuleAction,
    },
    Update {
        target: UpdateTarget,
    },
    Model {
        action: ModelAction,
    },
    SkillAdd {
        source: String,
        agents: Vec<String>,
        enabled_checkers: BTreeSet<ScanChecker>,
        force: bool,
    },
    SkillList,
    Install {
        agent: String,
    },
    Uninstall {
        agent: String,
        force: bool,
    },
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum ListResource {
    Agent,
    Asset(AssetType),
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum ScanResource {
    Skill,
    Cron,
    Memory,
    Provider,
}

impl fmt::Display for ScanResource {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self {
            Self::Skill => "skill",
            Self::Cron => "cron",
            Self::Memory => "memory",
            Self::Provider => "provider",
        };
        formatter.write_str(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum ScanChecker {
    Hash,
    Yara,
    Ti,
    Llm,
    OnlineTi,
}

#[derive(Debug, Clone)]
pub(crate) struct OutputOptions {
    pub(crate) format: OutputFormat,
    pub(crate) output: Option<PathBuf>,
}

impl Default for OutputOptions {
    fn default() -> Self {
        Self {
            format: OutputFormat::Terminal,
            output: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OutputFormat {
    Json,
    Terminal,
}

#[derive(Debug, Clone)]
pub(crate) enum ModelAction {
    Interactive,
    List {
        output: OutputOptions,
    },
    Set {
        agent: String,
        base_url: String,
        api_key: String,
        model: String,
        protocol: Option<sentra_lib::protocol::WireProtocol>,
    },
    Delete {
        agent: String,
        base_url: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ConfigAction {
    Help,
    Get,
    Set { key: String, value: String },
    Del { key: String, value: Option<String> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RuleAction {
    Help,
    Get,
    Set { key: String, value: String },
    Del { key: String, value: Option<String> },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UpdateTarget {
    Auto,
    Cli,
    Rules,
}

pub(crate) fn parse_args(args: Vec<OsString>) -> SentraResult<Command> {
    if args.is_empty() || is_help(&args[0]) {
        return Ok(Command::Help);
    }
    if is_version(&args[0]) {
        return Ok(Command::Version);
    }

    match args[0].to_string_lossy().as_ref() {
        "list" => parse_list(&args[1..]),
        "scan" => parse_scan(&args[1..]),
        "import" => parse_import(&args[1..]),
        "config" => parse_config(&args[1..]),
        "rule" => parse_rule(&args[1..]),
        "update" => parse_update(&args[1..]),
        "model" => parse_model(&args[1..]),
        "skill" => parse_skill(&args[1..]),
        "install" => parse_install(&args[1..]),
        "uninstall" => parse_uninstall(&args[1..]),
        command => Err(SentraError::Message(format!(
            "{}: {command}",
            t("unknown command", "未知命令")
        ))),
    }
}

fn parse_install(args: &[OsString]) -> SentraResult<Command> {
    if args.first().is_some_and(is_help) || args.iter().skip(1).any(is_help) {
        return Ok(Command::InstallHelp);
    }
    let agent = args
        .first()
        .ok_or_else(|| {
            SentraError::Message(t("missing install agent", "缺少安装目标").to_string())
        })?
        .to_string_lossy()
        .to_string();
    if args.len() > 1 {
        return Err(SentraError::Message(format!(
            "{}: {}",
            t("unknown install argument", "未知安装参数"),
            args[1].to_string_lossy()
        )));
    }
    match agent.as_str() {
        "codex" | "claude" | "opencode" | "pi" => Ok(Command::Install { agent }),
        other => Err(SentraError::Message(format!(
            "{}: {other}",
            t("unsupported install agent", "不支持的安装目标")
        ))),
    }
}

fn parse_uninstall(args: &[OsString]) -> SentraResult<Command> {
    if args.iter().any(is_help) {
        return Ok(Command::UninstallHelp);
    }
    let mut agent = None;
    let mut force = false;
    for arg in args {
        let value = arg.to_string_lossy();
        match value.as_ref() {
            "-f" | "--force" => force = true,
            value if value.starts_with('-') => {
                return Err(SentraError::Message(format!(
                    "{}: {value}",
                    t("unknown uninstall argument", "未知卸载参数")
                )));
            }
            value if agent.is_none() => agent = Some(value.to_string()),
            value => {
                return Err(SentraError::Message(format!(
                    "{}: {value}",
                    t("unknown uninstall argument", "未知卸载参数")
                )));
            }
        }
    }
    let agent = agent.ok_or_else(|| {
        SentraError::Message(t("missing uninstall agent", "缺少卸载目标").to_string())
    })?;
    match agent.as_str() {
        "codex" | "claude" | "opencode" | "pi" => Ok(Command::Uninstall { agent, force }),
        other => Err(SentraError::Message(format!(
            "{}: {other}",
            t("unsupported uninstall agent", "不支持的卸载目标")
        ))),
    }
}

fn parse_config(args: &[OsString]) -> SentraResult<Command> {
    if args.first().is_some_and(is_help) || args.iter().skip(1).any(is_help) {
        return Ok(Command::Config {
            action: ConfigAction::Help,
        });
    }
    let action = match args.first().map(|arg| arg.to_string_lossy()) {
        None => ConfigAction::Get,
        Some(action) if action == "get" => ConfigAction::Get,
        Some(action) if action == "set" => {
            let key = args
                .get(1)
                .ok_or_else(|| {
                    SentraError::Message(t("missing config key", "缺少配置键").to_string())
                })?
                .to_string_lossy()
                .to_string();
            let value = args
                .get(2)
                .ok_or_else(|| {
                    SentraError::Message(t("missing config value", "缺少配置值").to_string())
                })?
                .to_string_lossy()
                .to_string();
            ConfigAction::Set { key, value }
        }
        Some(action) if action == "del" || action == "delete" || action == "unset" => {
            let key = args
                .get(1)
                .ok_or_else(|| {
                    SentraError::Message(t("missing config key", "缺少配置键").to_string())
                })?
                .to_string_lossy()
                .to_string();
            let value = args.get(2).map(|arg| arg.to_string_lossy().to_string());
            ConfigAction::Del { key, value }
        }
        Some(other) => {
            return Err(SentraError::Message(format!(
                "{}: {other}",
                t("unknown config action", "未知配置动作")
            )));
        }
    };
    Ok(Command::Config { action })
}

fn parse_rule(args: &[OsString]) -> SentraResult<Command> {
    if args.first().is_some_and(is_help) || args.iter().skip(1).any(is_help) {
        return Ok(Command::Rule {
            action: RuleAction::Help,
        });
    }
    let action = match args.first().map(|arg| arg.to_string_lossy()) {
        None => RuleAction::Get,
        Some(action) if action == "get" => RuleAction::Get,
        Some(action) if action == "set" => {
            let key = args
                .get(1)
                .ok_or_else(|| {
                    SentraError::Message(t("missing rule key", "缺少规则键").to_string())
                })?
                .to_string_lossy()
                .to_string();
            let value = args
                .get(2)
                .ok_or_else(|| {
                    SentraError::Message(t("missing rule source", "缺少规则来源").to_string())
                })?
                .to_string_lossy()
                .to_string();
            RuleAction::Set { key, value }
        }
        Some(action) if action == "del" || action == "delete" || action == "unset" => {
            let key = args
                .get(1)
                .ok_or_else(|| {
                    SentraError::Message(t("missing rule key", "缺少规则键").to_string())
                })?
                .to_string_lossy()
                .to_string();
            let value = args.get(2).map(|arg| arg.to_string_lossy().to_string());
            RuleAction::Del { key, value }
        }
        Some(other) => {
            return Err(SentraError::Message(format!(
                "{}: {other}",
                t("unknown rule action", "未知规则动作")
            )));
        }
    };
    Ok(Command::Rule { action })
}

fn parse_import(args: &[OsString]) -> SentraResult<Command> {
    if args.first().is_some_and(is_help) || args.iter().skip(1).any(is_help) {
        return Ok(Command::ImportHelp);
    }
    if args.is_empty() {
        return Err(SentraError::Message(
            t("missing import source", "缺少导入来源").to_string(),
        ));
    }
    let sources = args.iter().map(PathBuf::from).collect();
    Ok(Command::Import { sources })
}

fn parse_update(args: &[OsString]) -> SentraResult<Command> {
    if args.first().is_some_and(is_help) || args.iter().skip(1).any(is_help) {
        return Ok(Command::UpdateHelp);
    }
    let target = match args.first().map(|arg| arg.to_string_lossy()) {
        None => UpdateTarget::Auto,
        Some(action) if action == "rules" || action == "rule" => UpdateTarget::Rules,
        Some(action) if action == "self" || action == "cli" => UpdateTarget::Cli,
        Some(arg) => {
            return Err(SentraError::Message(format!(
                "{}: {}",
                t("unknown update argument", "未知更新参数"),
                arg
            )));
        }
    };
    if args.len() > 1 {
        return Err(SentraError::Message(format!(
            "{}: {}",
            t("unknown update argument", "未知更新参数"),
            args[1].to_string_lossy()
        )));
    }
    Ok(Command::Update { target })
}

fn parse_skill(args: &[OsString]) -> SentraResult<Command> {
    if args.first().is_some_and(is_help) || args.iter().skip(1).any(is_help) {
        return Ok(Command::SkillHelp);
    }
    let action = args
        .first()
        .ok_or_else(|| SentraError::Message(t("missing skill action", "缺少技能动作").to_string()))?
        .to_string_lossy();
    match action.as_ref() {
        "add" => parse_skill_add(&args[1..]),
        "list" => Ok(Command::SkillList),
        other => Err(SentraError::Message(format!(
            "{}: {other}",
            t("unknown skill action", "未知技能动作")
        ))),
    }
}

fn parse_skill_add(args: &[OsString]) -> SentraResult<Command> {
    let source = args
        .first()
        .ok_or_else(|| SentraError::Message(t("missing skill source", "缺少技能来源").to_string()))?
        .to_string_lossy()
        .to_string();
    let (agents, enabled_checkers, force) = parse_skill_add_options(&args[1..])?;
    Ok(Command::SkillAdd {
        source,
        agents,
        enabled_checkers,
        force,
    })
}

fn parse_list(args: &[OsString]) -> SentraResult<Command> {
    if args.first().is_some_and(is_help) || args.iter().skip(1).any(is_help) {
        return Ok(Command::ListHelp);
    }

    let (resource, options) = match args.first() {
        Some(resource) => (
            parse_list_resource(&resource.to_string_lossy())?,
            &args[1..],
        ),
        None => (ListResource::Agent, &args[0..]),
    };
    let (home, agent, output) = parse_list_options(options)?;

    Ok(Command::List {
        resource,
        home,
        agent,
        output,
    })
}

fn parse_scan(args: &[OsString]) -> SentraResult<Command> {
    if args.first().is_some_and(is_help) || args.iter().skip(1).any(is_help) {
        return Ok(Command::ScanHelp);
    }

    let resource = args
        .first()
        .ok_or_else(|| {
            SentraError::Message(t("missing scan resource", "缺少扫描资源").to_string())
        })?
        .to_string_lossy();
    let resource = parse_scan_resource(&resource)?;
    let (path, agents, enabled_checkers, no_cache, output) = parse_scan_options(&args[1..])?;
    if path.is_some() && !matches!(resource, ScanResource::Skill) {
        return Err(SentraError::Message(format!(
            "{} {resource} {}",
            t("scan", "扫描"),
            t("does not accept a path", "不接受路径参数")
        )));
    }
    Ok(Command::Scan {
        resource,
        path,
        agents,
        enabled_checkers,
        no_cache,
        output,
    })
}

fn parse_model(args: &[OsString]) -> SentraResult<Command> {
    if args.first().is_some_and(is_help) || args.iter().skip(1).any(is_help) {
        return Ok(Command::ModelHelp);
    }
    let action = match args.first().map(|arg| arg.to_string_lossy()) {
        None => ModelAction::Interactive,
        Some(action) if action == "list" => {
            let output = parse_output_options(&args[1..])?;
            ModelAction::List { output }
        }
        Some(action) if action == "set" => parse_model_set_options(&args[1..])?,
        Some(action) if action == "delete" || action == "del" || action == "remove" => {
            parse_model_delete_options(&args[1..])?
        }
        Some(other) if other.starts_with('-') => {
            let output = parse_output_options(args)?;
            ModelAction::List { output }
        }
        Some(other) => {
            return Err(SentraError::Message(format!(
                "{}: {other}",
                t("unknown model action", "未知模型动作")
            )));
        }
    };
    Ok(Command::Model { action })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn os_args(values: &[&str]) -> Vec<OsString> {
        values.iter().map(OsString::from).collect()
    }

    #[test]
    fn version_flags_print_version() {
        let short = parse_args(os_args(&["-v"])).unwrap();
        assert!(matches!(short, Command::Version));

        let long = parse_args(os_args(&["--version"])).unwrap();
        assert!(matches!(long, Command::Version));
    }

    #[test]
    fn bare_model_command_enters_interactive_mode() {
        let command = parse_args(os_args(&["model"])).unwrap();

        assert!(matches!(
            command,
            Command::Model {
                action: ModelAction::Interactive
            }
        ));
    }

    #[test]
    fn explicit_model_list_keeps_scriptable_output_options() {
        let command = parse_args(os_args(&["model", "list", "--format", "json"])).unwrap();

        assert!(matches!(
            command,
            Command::Model {
                action: ModelAction::List {
                    output: OutputOptions {
                        format: OutputFormat::Json,
                        output: None
                    }
                }
            }
        ));
    }

    #[test]
    fn import_command_accepts_multiple_sources() {
        let command = parse_args(os_args(&["import", "rules.yar", "ioc.txt"])).unwrap();

        assert!(matches!(
            command,
            Command::Import { sources }
                if sources == vec![PathBuf::from("rules.yar"), PathBuf::from("ioc.txt")]
        ));
    }

    #[test]
    fn import_command_requires_a_source() {
        let err = parse_args(os_args(&["import"])).unwrap_err();

        assert!(err.to_string().contains("missing import source"));
    }

    #[test]
    fn skill_list_enters_interactive_skill_manager() {
        let command = parse_args(os_args(&["skill", "list"])).unwrap();

        assert!(matches!(command, Command::SkillList));
    }

    #[test]
    fn install_command_accepts_supported_agents() {
        let codex = parse_args(os_args(&["install", "codex"])).unwrap();
        assert!(matches!(codex, Command::Install { agent } if agent == "codex"));

        let claude = parse_args(os_args(&["install", "claude"])).unwrap();
        assert!(matches!(claude, Command::Install { agent } if agent == "claude"));

        let opencode = parse_args(os_args(&["install", "opencode"])).unwrap();
        assert!(matches!(opencode, Command::Install { agent } if agent == "opencode"));

        let pi = parse_args(os_args(&["install", "pi"])).unwrap();
        assert!(matches!(pi, Command::Install { agent } if agent == "pi"));
    }

    #[test]
    fn install_command_rejects_unsupported_agents() {
        let err = parse_args(os_args(&["install", "gemini"])).unwrap_err();

        assert!(err.to_string().contains("unsupported install agent"));
    }

    #[test]
    fn uninstall_command_accepts_supported_agents() {
        let codex = parse_args(os_args(&["uninstall", "codex"])).unwrap();
        assert!(matches!(codex, Command::Uninstall { agent, force: false } if agent == "codex"));

        let claude = parse_args(os_args(&["uninstall", "claude"])).unwrap();
        assert!(matches!(claude, Command::Uninstall { agent, force: false } if agent == "claude"));

        let opencode = parse_args(os_args(&["uninstall", "opencode"])).unwrap();
        assert!(
            matches!(opencode, Command::Uninstall { agent, force: false } if agent == "opencode")
        );

        let pi = parse_args(os_args(&["uninstall", "pi"])).unwrap();
        assert!(matches!(pi, Command::Uninstall { agent, force: false } if agent == "pi"));
    }

    #[test]
    fn uninstall_command_accepts_force_flag() {
        let long = parse_args(os_args(&["uninstall", "codex", "--force"])).unwrap();
        assert!(matches!(long, Command::Uninstall { agent, force: true } if agent == "codex"));

        let short = parse_args(os_args(&["uninstall", "-f", "opencode"])).unwrap();
        assert!(matches!(short, Command::Uninstall { agent, force: true } if agent == "opencode"));
    }

    #[test]
    fn uninstall_command_rejects_unsupported_agents() {
        let err = parse_args(os_args(&["uninstall", "gemini"])).unwrap_err();

        let err = err.to_string();
        assert!(err.contains("unsupported uninstall agent") || err.contains("不支持的卸载目标"));
    }

    #[test]
    fn scan_skill_parses_no_cache_flag() {
        let command = parse_args(os_args(&[
            "scan",
            "skill",
            "--no-cache",
            "--format",
            "json",
        ]))
        .unwrap();

        assert!(matches!(
            command,
            Command::Scan {
                resource: ScanResource::Skill,
                no_cache: true,
                output: OutputOptions {
                    format: OutputFormat::Json,
                    ..
                },
                ..
            }
        ));
    }

    #[test]
    fn scan_skill_accepts_ts_style_short_flags() {
        let command = parse_args(os_args(&[
            "scan",
            "skill",
            "-a",
            "codex",
            "-f",
            "json",
            "--online-ti",
        ]))
        .unwrap();

        assert!(matches!(
            command,
            Command::Scan {
                agents,
                enabled_checkers,
                output: OutputOptions {
                    format: OutputFormat::Json,
                    ..
                },
                ..
            } if agents == vec!["codex"]
                && enabled_checkers.contains(&ScanChecker::OnlineTi)
        ));
    }

    #[test]
    fn scan_skill_accepts_ts_style_llm_flags() {
        let llm = parse_args(os_args(&["scan", "skill", "--llm"])).unwrap();
        assert!(matches!(
            llm,
            Command::Scan {
                enabled_checkers,
                ..
            } if enabled_checkers.contains(&ScanChecker::Llm)
                && enabled_checkers.contains(&ScanChecker::Hash)
        ));

        let llm_only = parse_args(os_args(&["scan", "skill", "--llm-only"])).unwrap();
        assert!(matches!(
            llm_only,
            Command::Scan {
                enabled_checkers,
                ..
            } if enabled_checkers == [ScanChecker::Llm].into_iter().collect()
        ));
    }

    #[test]
    fn config_command_parses_get_set_and_del_actions() {
        let get = parse_args(os_args(&["config"])).unwrap();
        assert!(matches!(
            get,
            Command::Config {
                action: ConfigAction::Get
            }
        ));
        let set = parse_args(os_args(&["config", "set", "threatbook_key", "sk-test"])).unwrap();
        assert!(matches!(
            set,
            Command::Config {
                action: ConfigAction::Set { key, value }
            } if key == "threatbook_key" && value == "sk-test"
        ));

        let del = parse_args(os_args(&["config", "del", "threatbook_key"])).unwrap();
        assert!(matches!(
            del,
            Command::Config {
                action: ConfigAction::Del { key, value: None }
            } if key == "threatbook_key"
        ));
    }

    #[test]
    fn rule_command_parses_get_set_and_del_actions() {
        let get = parse_args(os_args(&["rule"])).unwrap();
        assert!(matches!(
            get,
            Command::Rule {
                action: RuleAction::Get
            }
        ));

        let set = parse_args(os_args(&[
            "rule",
            "set",
            "rule_demo",
            "https://example.test/rules.zip",
        ]))
        .unwrap();
        assert!(matches!(
            set,
            Command::Rule {
                action: RuleAction::Set { key, value }
            } if key == "rule_demo" && value == "https://example.test/rules.zip"
        ));

        let del = parse_args(os_args(&[
            "rule",
            "del",
            "rule_demo",
            "https://example.test/rules.zip",
        ]))
        .unwrap();
        assert!(matches!(
            del,
            Command::Rule {
                action: RuleAction::Del { key, value: Some(value) }
            } if key == "rule_demo" && value == "https://example.test/rules.zip"
        ));

        let update = parse_args(os_args(&["update"])).unwrap();
        assert!(matches!(
            update,
            Command::Update {
                target: UpdateTarget::Auto
            }
        ));

        let update_cli = parse_args(os_args(&["update", "self"])).unwrap();
        assert!(matches!(
            update_cli,
            Command::Update {
                target: UpdateTarget::Cli
            }
        ));

        let update_rules = parse_args(os_args(&["update", "rules"])).unwrap();
        assert!(matches!(
            update_rules,
            Command::Update {
                target: UpdateTarget::Rules
            }
        ));
    }
}

fn parse_list_resource(resource: &str) -> SentraResult<ListResource> {
    match resource {
        "agent" => Ok(ListResource::Agent),
        "skill" => Ok(ListResource::Asset(AssetType::Skill)),
        "mcp" => Ok(ListResource::Asset(AssetType::Mcp)),
        "provider" => Ok(ListResource::Asset(AssetType::Provider)),
        "memory" => Ok(ListResource::Asset(AssetType::Memory)),
        "cron" => Ok(ListResource::Asset(AssetType::Cron)),
        "plugin" => Ok(ListResource::Asset(AssetType::Plugin)),
        other => Err(SentraError::Message(format!(
            "{}: {other}",
            t("unknown list resource", "未知列表资源")
        ))),
    }
}

fn parse_scan_resource(resource: &str) -> SentraResult<ScanResource> {
    match resource {
        "skill" => Ok(ScanResource::Skill),
        "cron" => Ok(ScanResource::Cron),
        "memory" => Ok(ScanResource::Memory),
        "provider" => Ok(ScanResource::Provider),
        other => Err(SentraError::Message(format!(
            "{}: {other}",
            t("unknown scan resource", "未知扫描资源")
        ))),
    }
}

fn parse_output_options(args: &[OsString]) -> SentraResult<OutputOptions> {
    let mut output = OutputOptions::default();
    let mut index = 0;
    while index < args.len() {
        let option = args[index].to_string_lossy();
        match option.as_ref() {
            "--format" => {
                index += 1;
                let value = args.get(index).ok_or_else(|| {
                    SentraError::Message(
                        t("missing value for --format", "缺少 --format 的值").to_string(),
                    )
                })?;
                output.format = parse_output_format(&value.to_string_lossy())?;
            }
            "-o" | "--output" => {
                index += 1;
                let value = args.get(index).ok_or_else(|| {
                    SentraError::Message(
                        t("missing value for --output", "缺少 --output 的值").to_string(),
                    )
                })?;
                output.output = Some(PathBuf::from(value));
            }
            other => {
                return Err(SentraError::Message(format!(
                    "{}: {other}",
                    t("unknown option", "未知选项")
                )));
            }
        }
        index += 1;
    }
    Ok(output)
}

fn parse_list_options(
    args: &[OsString],
) -> SentraResult<(Option<PathBuf>, Option<String>, OutputOptions)> {
    let mut home = None;
    let mut agent = None;
    let mut output = OutputOptions::default();
    let mut index = 0;
    while index < args.len() {
        let option = args[index].to_string_lossy();
        match option.as_ref() {
            "--home" => {
                index += 1;
                let value = args.get(index).ok_or_else(|| {
                    SentraError::Message(
                        t("missing value for --home", "缺少 --home 的值").to_string(),
                    )
                })?;
                home = Some(PathBuf::from(value));
            }
            "--agent" => {
                index += 1;
                let value = args.get(index).ok_or_else(|| {
                    SentraError::Message(
                        t("missing value for --agent", "缺少 --agent 的值").to_string(),
                    )
                })?;
                agent = Some(value.to_string_lossy().to_string());
            }
            "--format" => {
                index += 1;
                let value = args.get(index).ok_or_else(|| {
                    SentraError::Message(
                        t("missing value for --format", "缺少 --format 的值").to_string(),
                    )
                })?;
                output.format = parse_output_format(&value.to_string_lossy())?;
            }
            "-o" | "--output" => {
                index += 1;
                let value = args.get(index).ok_or_else(|| {
                    SentraError::Message(
                        t("missing value for --output", "缺少 --output 的值").to_string(),
                    )
                })?;
                output.output = Some(PathBuf::from(value));
            }
            other => {
                return Err(SentraError::Message(format!(
                    "{}: {other}",
                    t("unknown option", "未知选项")
                )));
            }
        }
        index += 1;
    }
    Ok((home, agent, output))
}

fn parse_model_set_options(args: &[OsString]) -> SentraResult<ModelAction> {
    let mut agent = None;
    let mut base_url = None;
    let mut api_key = None;
    let mut model = None;
    let mut protocol = None;
    let mut index = 0;

    while index < args.len() {
        let option = args[index].to_string_lossy();
        match option.as_ref() {
            "--agent" => agent = Some(option_value(args, &mut index, "--agent")?),
            "--base-url" | "--url" => {
                base_url = Some(option_value(args, &mut index, "--base-url")?)
            }
            "--api-key" | "--key" => api_key = Some(option_value(args, &mut index, "--api-key")?),
            "--model" => model = Some(option_value(args, &mut index, "--model")?),
            "--protocol" => {
                let value = option_value(args, &mut index, "--protocol")?;
                protocol = Some(sentra_lib::protocol::parse_wire_protocol(&value)?);
            }
            other => {
                return Err(SentraError::Message(format!(
                    "{}: {other}",
                    t("unknown option", "未知选项")
                )));
            }
        }
        index += 1;
    }

    Ok(ModelAction::Set {
        agent: required_option(agent, "--agent")?,
        base_url: required_option(base_url, "--base-url")?,
        api_key: required_option(api_key, "--api-key")?,
        model: required_option(model, "--model")?,
        protocol,
    })
}

fn parse_model_delete_options(args: &[OsString]) -> SentraResult<ModelAction> {
    let mut agent = None;
    let mut base_url = None;
    let mut index = 0;

    while index < args.len() {
        let option = args[index].to_string_lossy();
        match option.as_ref() {
            "--agent" => agent = Some(option_value(args, &mut index, "--agent")?),
            "--base-url" | "--url" => {
                base_url = Some(option_value(args, &mut index, "--base-url")?)
            }
            other => {
                return Err(SentraError::Message(format!(
                    "{}: {other}",
                    t("unknown option", "未知选项")
                )));
            }
        }
        index += 1;
    }

    Ok(ModelAction::Delete {
        agent: required_option(agent, "--agent")?,
        base_url: required_option(base_url, "--base-url")?,
    })
}

fn option_value(args: &[OsString], index: &mut usize, option: &str) -> SentraResult<String> {
    *index += 1;
    args.get(*index)
        .map(|value| value.to_string_lossy().to_string())
        .ok_or_else(|| {
            SentraError::Message(format!("{} {option}", t("missing value for", "缺少值:")))
        })
}

fn required_option(value: Option<String>, option: &str) -> SentraResult<String> {
    value.ok_or_else(|| {
        SentraError::Message(format!(
            "{}: {option}",
            t("missing required option", "缺少必需选项")
        ))
    })
}

fn parse_scan_options(
    args: &[OsString],
) -> SentraResult<(
    Option<PathBuf>,
    Vec<String>,
    BTreeSet<ScanChecker>,
    bool,
    OutputOptions,
)> {
    let mut path = None;
    let mut agents = Vec::new();
    let mut enabled = default_scan_checkers();
    let mut no_cache = false;
    let mut output = OutputOptions::default();
    let mut index = 0;

    while index < args.len() {
        let option = args[index].to_string_lossy();
        match option.as_ref() {
            "-a" | "--agent" => {
                index += 1;
                let value = args.get(index).ok_or_else(|| {
                    SentraError::Message(
                        t("missing value for --agent", "缺少 --agent 的值").to_string(),
                    )
                })?;
                agents.push(value.to_string_lossy().to_string());
            }
            "-f" | "--format" => {
                index += 1;
                let value = args.get(index).ok_or_else(|| {
                    SentraError::Message(
                        t("missing value for --format", "缺少 --format 的值").to_string(),
                    )
                })?;
                output.format = parse_output_format(&value.to_string_lossy())?;
            }
            "-o" | "--output" => {
                index += 1;
                let value = args.get(index).ok_or_else(|| {
                    SentraError::Message(
                        t("missing value for --output", "缺少 --output 的值").to_string(),
                    )
                })?;
                output.output = Some(PathBuf::from(value));
            }
            "--no-cache" => {
                no_cache = true;
            }
            "--llm" => {
                enabled.insert(ScanChecker::Llm);
            }
            "--llm-only" => {
                enabled.clear();
                enabled.insert(ScanChecker::Llm);
            }
            "--online-ti" => {
                enabled.insert(ScanChecker::OnlineTi);
            }
            _ if option.starts_with("--with-") => {
                enabled.insert(parse_scan_checker(&option["--with-".len()..])?);
            }
            _ if option.starts_with("--without-") => {
                enabled.remove(&parse_scan_checker(&option["--without-".len()..])?);
            }
            other if option.starts_with('-') => {
                return Err(SentraError::Message(format!(
                    "{}: {other}",
                    t("unknown option", "未知选项")
                )));
            }
            _ => {
                if path.is_some() {
                    return Err(SentraError::Message(
                        t("scan accepts only one path", "扫描只接受一个路径").to_string(),
                    ));
                }
                path = Some(PathBuf::from(args[index].clone()));
            }
        }
        index += 1;
    }

    if path.is_some() && !agents.is_empty() {
        return Err(SentraError::Message(
            t(
                "--agent cannot be used when scanning a skill path",
                "扫描技能路径时不能使用 --agent",
            )
            .to_string(),
        ));
    }

    Ok((path, agents, enabled, no_cache, output))
}

fn parse_skill_add_options(
    args: &[OsString],
) -> SentraResult<(Vec<String>, BTreeSet<ScanChecker>, bool)> {
    let mut agents = Vec::new();
    let mut enabled = default_scan_checkers();
    let mut force = false;
    let mut index = 0;

    while index < args.len() {
        let option = args[index].to_string_lossy();
        match option.as_ref() {
            "--agent" => {
                index += 1;
                let value = args.get(index).ok_or_else(|| {
                    SentraError::Message(
                        t("missing value for --agent", "缺少 --agent 的值").to_string(),
                    )
                })?;
                agents.push(value.to_string_lossy().to_string());
            }
            "-f" | "--force" => {
                force = true;
            }
            _ if option.starts_with("--with-") => {
                enabled.insert(parse_scan_checker(&option["--with-".len()..])?);
            }
            _ if option.starts_with("--without-") => {
                enabled.remove(&parse_scan_checker(&option["--without-".len()..])?);
            }
            other => {
                return Err(SentraError::Message(format!(
                    "{}: {other}",
                    t("unknown option", "未知选项")
                )));
            }
        }
        index += 1;
    }

    Ok((agents, enabled, force))
}

fn default_scan_checkers() -> BTreeSet<ScanChecker> {
    [ScanChecker::Hash, ScanChecker::Yara, ScanChecker::Ti]
        .into_iter()
        .collect()
}

fn parse_scan_checker(value: &str) -> SentraResult<ScanChecker> {
    match value {
        "hash" => Ok(ScanChecker::Hash),
        "yara" => Ok(ScanChecker::Yara),
        "ti" => Ok(ScanChecker::Ti),
        "llm" => Ok(ScanChecker::Llm),
        "online-ti" => Ok(ScanChecker::OnlineTi),
        other => Err(SentraError::Message(format!(
            "{}: {other}",
            t("unknown scan checker", "未知扫描检查器")
        ))),
    }
}

fn parse_output_format(value: &str) -> SentraResult<OutputFormat> {
    match value {
        "json" => Ok(OutputFormat::Json),
        "terminal" | "table" => Ok(OutputFormat::Terminal),
        other => Err(SentraError::Message(format!(
            "{}: {other}",
            t("unknown output format", "未知输出格式")
        ))),
    }
}

pub(crate) fn print_help() {
    println!(
        "{}\n{}",
        version_line(),
        t(
            "\
Usage:
  sentra <command> [args...]

Commands:
  list       List discovered assets and agents
  scan       Scan skills and other assets for risks
  import     Import local or remote rule files
  rule       View and modify rule sources
  update     Update Sentra CLI or configured rule sources
  config     View and modify Sentra configuration
  model      View and modify model providers
  skill      Install skills
  install    Install or update an agent CLI
  uninstall  Uninstall an agent CLI

Options:
  -v, --version   Show version
  -h, --help      Show help
  --lang <en|zh>  Display language

Use 'sentra <command> --help' for command-specific usage.",
            "\
用法:
  sentra <命令> [参数...]

命令:
  list       列出发现的资产和 Agent
  scan       扫描技能和其他资产风险
  import     导入本地或远程规则文件
  rule       查看和修改规则来源
  update     更新 Sentra CLI 或已配置的规则来源
  config     查看和修改 Sentra 配置
  model      查看和修改模型供应商
  skill      安装技能
  install    安装或更新 Agent CLI
  uninstall  卸载 Agent CLI

选项:
  -v, --version   显示版本
  -h, --help      显示帮助
  --lang <en|zh>  显示语言

使用 'sentra <命令> --help' 查看命令帮助。"
        )
    );
}

pub(crate) fn print_version() {
    println!("{}", version_line());
}

fn version_line() -> String {
    format!("sentra {}", env!("CARGO_PKG_VERSION"))
}

pub(crate) fn print_install_help() {
    println!(
        "{}",
        t(
            "\
Usage:
  sentra install <codex|claude|opencode|pi>

Description:
  Install an agent CLI. If it is already installed, update it.

Options:
  -h, --help  Show help

Examples:
  sentra install codex
  sentra install claude
  sentra install opencode
  sentra install pi",
            "\
用法:
  sentra install <codex|claude|opencode|pi>

说明:
  安装 Agent CLI；如果已经安装则更新。

选项:
  -h, --help  显示帮助

示例:
  sentra install codex
  sentra install claude
  sentra install opencode
  sentra install pi"
        )
    );
}

pub(crate) fn print_uninstall_help() {
    println!(
        "{}",
        t(
            "\
Usage:
  sentra uninstall <codex|claude|opencode|pi> [--force]

Description:
  Uninstall an agent CLI. By default, Sentra asks whether to delete local configuration data.

Options:
  -f, --force  Delete configuration data without asking
  -h, --help   Show help

Examples:
  sentra uninstall codex
  sentra uninstall claude
  sentra uninstall opencode --force
  sentra uninstall pi",
            "\
用法:
  sentra uninstall <codex|claude|opencode|pi> [--force]

说明:
  卸载 Agent CLI。默认会询问是否删除本地配置数据。

选项:
  -f, --force  不询问并直接删除配置数据
  -h, --help   显示帮助

示例:
  sentra uninstall codex
  sentra uninstall claude
  sentra uninstall opencode --force
  sentra uninstall pi"
        )
    );
}

pub(crate) fn print_list_help() {
    println!("{}", t(
        "\
Usage:
  sentra list <skill|mcp|provider|memory|agent|cron|plugin> [--home <path>] [--agent <name>] [--format <terminal|json>] [--output <file>]

Description:
  List discovered Sentra assets or configured agents.

Options:
  --home <path>       Read agent homes under this user home
  --agent <name>      Filter assets to an agent
  --format <format>   Output format: terminal, json
  -o, --output <file> Write command output to a file
  -h, --help          Show help

Examples:
  sentra list skill
  sentra list provider --home ./fixtures/provider/account-home --format json"
    ,
        "\
用法:
  sentra list <skill|mcp|provider|memory|agent|cron|plugin> [--home <路径>] [--agent <名称>] [--format <terminal|json>] [--output <文件>]

说明:
  列出发现的 Sentra 资产或已配置的 Agent。

选项:
  --home <路径>      从指定用户主目录读取 Agent
  --agent <名称>     按 Agent 过滤资产
  --format <格式>    输出格式: terminal, json
  -o, --output <文件> 将命令输出写入文件
  -h, --help         显示帮助

示例:
  sentra list skill
  sentra list provider --home ./fixtures/provider/account-home --format json"
    ));
}

pub(crate) fn print_scan_help() {
    println!("{}", t(
        "\
Usage:
  sentra scan <skill|cron|memory|provider> [path] [--agent <name> ...] [--format <terminal|json>] [--output <file>] [--with-xxx ...] [--without-xxx ...]

Description:
  Scan assets or a skill path for security risks.

Options:
  -a, --agent <name>  Filter scans to one or more agents
  -f, --format <fmt>  Output format: terminal, table, json
  -o, --output <file> Write command output to a file
  --llm               Enable LLM checker
  --llm-only          Use only the LLM checker
  --online-ti         Enable online threat intelligence checker
  --with-xxx          Enable checker: hash, yara, ti, llm, online-ti
  --without-xxx       Disable checker: hash, yara, ti, llm, online-ti
  -h, --help          Show help

Examples:
  sentra scan skill
  sentra scan skill ./fixtures/skill --with-llm
  sentra scan provider --agent codex --format json"
    ,
        "\
用法:
  sentra scan <skill|cron|memory|provider> [路径] [--agent <名称> ...] [--format <terminal|json>] [--output <文件>] [--with-xxx ...] [--without-xxx ...]

说明:
  扫描资产或技能路径中的安全风险。

选项:
  -a, --agent <名称> 按一个或多个 Agent 过滤扫描
  -f, --format <格式> 输出格式: terminal, table, json
  -o, --output <文件> 将命令输出写入文件
  --llm              启用 LLM 检查器
  --llm-only         仅使用 LLM 检查器
  --online-ti        启用在线威胁情报检查器
  --with-xxx         启用检查器: hash, yara, ti, llm, online-ti
  --without-xxx      禁用检查器: hash, yara, ti, llm, online-ti
  -h, --help         显示帮助

示例:
  sentra scan skill
  sentra scan skill ./fixtures/skill --with-llm
  sentra scan provider --agent codex --format json"
    ));
}

pub(crate) fn print_import_help() {
    println!(
        "{}",
        t(
            "\
Usage:
  sentra import <files...>

Description:
  Import YARA, threat intelligence, and hash rule files into the local rule store.

Options:
  -h, --help  Show help

Examples:
  sentra import ./rules/yara ./rules/ti ./rules/hash
  sentra import ./rules.zip",
            "\
用法:
  sentra import <文件...>

说明:
  将 YARA、威胁情报和哈希规则文件导入本地规则库。

选项:
  -h, --help  显示帮助

示例:
  sentra import ./rules/yara ./rules/ti ./rules/hash
  sentra import ./rules.zip"
        )
    );
}

pub(crate) fn print_update_help() {
    println!(
        "{}",
        t(
            "\
Usage:
  sentra update
  sentra update self
  sentra update rules

Description:
  Update configured rule sources when present; otherwise update Sentra CLI to the latest GitHub release.
  Use 'sentra update self' to update Sentra CLI explicitly.
  Use 'sentra update rules' to download and import configured rule sources.
  Configure sources with: sentra rule set rule_<name> <url>

Options:
  -h, --help  Show help

Examples:
  sentra update
  sentra update self
  sentra rule set rule_public https://example.test/rules.zip
  sentra update rules",
            "\
用法:
  sentra update
  sentra update self
  sentra update rules

说明:
  已配置规则来源时更新规则，否则将 Sentra CLI 更新到 GitHub 最新版本。
  使用 'sentra update self' 显式更新 Sentra CLI。
  使用 'sentra update rules' 下载并导入已配置的规则来源。
  使用以下命令配置来源: sentra rule set rule_<名称> <url>

选项:
  -h, --help  显示帮助

示例:
  sentra update
  sentra update self
  sentra rule set rule_public https://example.test/rules.zip
  sentra update rules"
        )
    );
}

pub(crate) fn print_model_help() {
    println!("{}", t(
        "\
Usage:
  sentra model [list] [--format <terminal|json>] [--output <file>]
  sentra model set --agent <name> --base-url <url> --api-key <key> --model <id> [--protocol <protocol>]
  sentra model delete --agent <name> --base-url <url>

Description:
  View and modify model provider configuration.

Options:
  --agent <name>      Agent name
  --base-url <url>    Provider base URL
  --api-key <key>     Provider API key
  --model <id>        Model identifier
  --protocol <value>  Wire protocol
  --format <format>   Output format: terminal, json
  -o, --output <file> Write command output to a file
  -h, --help          Show help

Examples:
  sentra model list
  sentra model set --agent codex --base-url https://example.test/v1 --api-key sk-test --model demo"
    ,
        "\
用法:
  sentra model [list] [--format <terminal|json>] [--output <文件>]
  sentra model set --agent <名称> --base-url <url> --api-key <key> --model <id> [--protocol <协议>]
  sentra model delete --agent <名称> --base-url <url>

说明:
  查看和修改模型供应商配置。

选项:
  --agent <名称>      Agent 名称
  --base-url <url>    供应商 Base URL
  --api-key <key>     供应商 API key
  --model <id>        模型标识
  --protocol <值>     通信协议
  --format <格式>     输出格式: terminal, json
  -o, --output <文件> 将命令输出写入文件
  -h, --help          显示帮助

示例:
  sentra model list
  sentra model set --agent codex --base-url https://example.test/v1 --api-key sk-test --model demo"
    ));
}

pub(crate) fn print_skill_help() {
    println!(
        "{}",
        t(
            "\
Usage:
  sentra skill list
  sentra skill add <url> [--agent <name> ...] [--force] [--with-xxx ...] [--without-xxx ...]

Description:
  Manage skills or install a skill after scanning it for risks.

Options:
  --agent <name> Filter installation to one or more agents
  -f, --force    Allow installing skills with risk findings
  --with-xxx     Enable checker: hash, yara, ti, llm, online-ti
  --without-xxx  Disable checker: hash, yara, ti, llm, online-ti
  -h, --help     Show help

Examples:
  sentra skill list
  sentra skill add https://example.test/skill.zip
  sentra skill add https://example.test/skill.zip --agent codex --force",
            "\
用法:
  sentra skill list
  sentra skill add <url> [--agent <名称> ...] [--force] [--with-xxx ...] [--without-xxx ...]

说明:
  管理技能，或在风险扫描后安装技能。

选项:
  --agent <名称> 按一个或多个 Agent 过滤安装
  -f, --force    允许安装包含风险发现的技能
  --with-xxx     启用检查器: hash, yara, ti, llm, online-ti
  --without-xxx  禁用检查器: hash, yara, ti, llm, online-ti
  -h, --help     显示帮助

示例:
  sentra skill list
  sentra skill add https://example.test/skill.zip
  sentra skill add https://example.test/skill.zip --agent codex --force"
        )
    );
}

fn is_help(arg: &OsString) -> bool {
    matches!(arg.to_string_lossy().as_ref(), "-h" | "--help")
}

fn is_version(arg: &OsString) -> bool {
    matches!(arg.to_string_lossy().as_ref(), "-v" | "--version")
}
