use std::collections::BTreeSet;
use std::path::PathBuf;

use sentra_lib::risks::{ImportResult, RuleStore};
use sentra_lib::{SentraError, SentraResult};

use crate::cli::feedback::{self, Status};
use crate::cli::i18n::t;
use crate::core::scan_support::{build_scan_options, checker_selection};

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
    feedback::context(
        match progress_verb {
            "updating" => t("Update rule sources", "更新规则来源"),
            _ => t("Import rule sources", "导入规则来源"),
        },
        &[(t("Sources", "来源数"), sources.len().to_string())],
    );
    let options = build_scan_options(home, &checker_selection(&BTreeSet::new()))?;
    let store = RuleStore::new(options.rules.unwrap_or_default());
    let mut total = ImportResult::default();
    let source_count = sources.len();

    for (index, source) in sources.into_iter().enumerate() {
        feedback::counted_action(
            index + 1,
            source_count,
            match progress_verb {
                "updating" => t("Update rules from source", "从来源更新规则"),
                _ => t("Import rules from source", "从来源导入规则"),
            },
            &source,
        );
        match store.import(&source) {
            Ok(result) => {
                add_import_result(&mut total, result);
                feedback::status_line(Status::Success, t("Source processed", "来源已处理"));
            }
            Err(err) => {
                feedback::status_line(
                    Status::Warning,
                    format!(
                        "{} {source}: {err}",
                        match error_action {
                            "update" => t("failed to update from", "从以下来源更新失败:"),
                            _ => t("failed to import from", "从以下来源导入失败:"),
                        }
                    ),
                );
                total.skipped += 1;
            }
        }
    }

    let result_prefix = if total.skipped == 0 {
        Status::Success
    } else {
        Status::Warning
    };
    feedback::result(
        result_prefix,
        match progress_verb {
            "updating" => t("Rule update complete", "规则更新完成"),
            _ => t("Rule import complete", "规则导入完成"),
        },
        &[
            (t("YARA", "YARA"), total.yara.to_string()),
            (t("Threat intelligence", "威胁情报"), total.ti.to_string()),
            (t("Hash lists", "哈希列表"), total.hash.to_string()),
            (t("Skipped", "跳过"), total.skipped.to_string()),
        ],
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
