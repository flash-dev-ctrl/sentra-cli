#[path = "cli/args.rs"]
mod args;
#[path = "cli/bundled_rules.rs"]
mod bundled_rules;
#[path = "cli/config.rs"]
mod config;
#[path = "cli/i18n.rs"]
mod i18n;
#[path = "cli/import.rs"]
mod import;
#[path = "cli/install.rs"]
mod install;
#[path = "cli/list.rs"]
mod list;
#[path = "cli/model.rs"]
mod model;
#[path = "cli/output.rs"]
mod output;
#[path = "cli/scan.rs"]
mod scan;
#[path = "cli/scan_support.rs"]
mod scan_support;
#[path = "cli/skill.rs"]
mod skill;
#[path = "cli/skill_inventory.rs"]
mod skill_inventory;
#[path = "cli/skill_manager.rs"]
mod skill_manager;
#[path = "cli/updater.rs"]
mod updater;

use std::ffi::OsString;

use sentra_lib::SentraResult;

use args::{Command, UpdateTarget};

fn main() {
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
            eprintln!("{}: {err}", i18n::t("error", "错误"));
            std::process::exit(2);
        }
    }
}

async fn run(args: Vec<OsString>) -> SentraResult<()> {
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
            agent,
            output,
        } => {
            config::initialize()?;
            list::run(*resource, agent.as_deref(), output.clone()).await
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
        Command::Uninstall { agent } => install::run_uninstall(agent.clone()),
    }
}
