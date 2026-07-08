use std::collections::BTreeSet;
use std::path::PathBuf;

use sentra_lib::risks::{ImportResult, RuleStore};
use sentra_lib::{SentraError, SentraResult};

use crate::i18n::t;
use crate::scan_support::{build_scan_options, checker_selection};

pub(crate) fn run(sources: Vec<PathBuf>) -> SentraResult<()> {
    let home = home::home_dir().ok_or_else(|| {
        SentraError::Message(
            t(
                "could not determine current user home",
                "无法确定当前用户主目录",
            )
            .to_string(),
        )
    })?;
    let sources = sources
        .into_iter()
        .map(|source| source.to_string_lossy().to_string())
        .collect();
    run_sources_at(&home, sources, "importing", "import")
}

pub(crate) fn run_sources_at(
    home: &std::path::Path,
    sources: Vec<String>,
    progress_verb: &str,
    error_action: &str,
) -> SentraResult<()> {
    let options = build_scan_options(home, &checker_selection(&BTreeSet::new()))?;
    let store = RuleStore::new(options.rules.unwrap_or_default());
    let mut total = ImportResult::default();

    for source in sources {
        eprintln!(
            "{} {source}",
            match progress_verb {
                "updating" => t("updating rules from", "正在从以下来源更新规则:"),
                _ => t("importing rules from", "正在从以下来源导入规则:"),
            }
        );
        match store.import(&source) {
            Ok(result) => add_import_result(&mut total, result),
            Err(err) => {
                eprintln!(
                    "{} {source}: {err}",
                    match error_action {
                        "update" => t("failed to update from", "从以下来源更新失败:"),
                        _ => t("failed to import from", "从以下来源导入失败:"),
                    }
                );
                total.skipped += 1;
            }
        }
    }

    println!(
        "{}: yara={} ti={} hash={} skipped={}",
        t("Imported rules", "已导入规则"),
        total.yara,
        total.ti,
        total.hash,
        total.skipped
    );

    if total.skipped > 0 {
        return Err(SentraError::Message(format!(
            "{} {} {}",
            match error_action {
                "update" => t("update skipped", "更新跳过了"),
                _ => t("import skipped", "导入跳过了"),
            },
            total.skipped,
            t("source(s)", "个来源"),
        )));
    }
    Ok(())
}

fn add_import_result(total: &mut ImportResult, result: ImportResult) {
    total.yara += result.yara;
    total.ti += result.ti;
    total.hash += result.hash;
    total.skipped += result.skipped;
}
