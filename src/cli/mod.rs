pub(crate) mod args;
pub(crate) mod feedback;
pub(crate) mod i18n;
pub(crate) mod output;

use std::ffi::OsString;

use sentra_lib::SentraResult;

use crate::core::{config, import, install, list, model, scan, skill, updater};

use args::{Command, UpdateTarget};

pub fn main() {
    let (args, language) = i18n::strip_language_args(std::env::args_os().skip(1).collect());
    i18n::init(language.as_deref());
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect(i18n::t(
            "failed to start async runtime",
            "启动异步运行时失败",
        ));
    match runtime.block_on(run(args)) {
        Ok(()) => {}
        Err(err) => {
            eprintln!(
                "{}",
                feedback::render_error(
                    i18n::t("Command failed", "命令执行失败"),
                    &err.to_string(),
                    i18n::t(
                        "Review the message above, adjust the command or configuration, then retry.",
                        "请检查上方信息，调整命令或配置后重试。",
                    ),
                )
            );
            std::process::exit(2);
        }
    }
}

pub async fn run(args: Vec<OsString>) -> SentraResult<()> {
    let command = args::parse_args(args)?;
    updater::maybe_prompt_auto_update(&command).await;
    execute(&command).await?;
    Ok(())
}

async fn execute(command: &Command) -> SentraResult<()> {
    match command {
        Command::Help => {
            args::print_help();
            Ok(())
        }
        Command::Version => {
            args::print_version();
            Ok(())
        }
        Command::ListHelp => {
            args::print_list_help();
            Ok(())
        }
        Command::ScanHelp => {
            args::print_scan_help();
            Ok(())
        }
        Command::ImportHelp => {
            args::print_import_help();
            Ok(())
        }
        Command::UpdateHelp => {
            args::print_update_help();
            Ok(())
        }
        Command::ModelHelp => {
            args::print_model_help();
            Ok(())
        }
        Command::SkillHelp => {
            args::print_skill_help();
            Ok(())
        }
        Command::InstallHelp => {
            args::print_install_help();
            Ok(())
        }
        Command::UninstallHelp => {
            args::print_uninstall_help();
            Ok(())
        }
        Command::List {
            resource,
            home,
            agent,
            output,
        } => {
            let home = list::resolve_home(home.as_deref())?;
            config::initialize_at(&home)?;
            list::run(*resource, &home, agent.as_deref(), output.clone()).await
        }
        Command::Scan {
            resource,
            path,
            agents,
            enabled_checkers,
            no_cache,
            output,
        } => {
            config::initialize()?;
            scan::run(
                *resource,
                path.clone(),
                agents.clone(),
                enabled_checkers.clone(),
                *no_cache,
                output.clone(),
            )
            .await
        }
        Command::Import { sources } => {
            config::initialize()?;
            import::run(sources.clone())
        }
        Command::Config { action } => {
            config::initialize()?;
            config::run(action.clone())
        }
        Command::Rule { action } => {
            config::initialize()?;
            config::run_rule(action.clone())
        }
        Command::Update { target } => {
            config::initialize()?;
            match target {
                UpdateTarget::Auto => {
                    if config::has_rule_sources()? {
                        config::update_rules()
                    } else {
                        updater::manual_update().await
                    }
                }
                UpdateTarget::Cli => updater::manual_update().await,
                UpdateTarget::Rules => config::update_rules(),
            }
        }
        Command::Model { action } => {
            config::initialize()?;
            model::run(action.clone()).await
        }
        Command::SkillAdd {
            source,
            agents,
            enabled_checkers,
            force,
        } => {
            config::initialize()?;
            skill::add(
                source.clone(),
                agents.clone(),
                enabled_checkers.clone(),
                *force,
            )
            .await
        }
        Command::SkillList => {
            config::initialize()?;
            skill::list().await
        }
        Command::Install { agent } => install::run(agent.clone()),
        Command::Uninstall { agent, force } => install::run_uninstall(agent.clone(), *force),
    }
}
