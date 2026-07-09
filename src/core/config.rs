use std::path::Path;

use sentra_lib::config::{
    sentra_config_file, sentra_hash_rule_dir, sentra_home, sentra_ti_rule_dir, sentra_yara_rule_dir,
};
use sentra_lib::{SentraError, SentraResult};

use crate::cli::args::{ConfigAction, RuleAction};
use crate::cli::feedback::{self, Status};
use crate::cli::i18n::t;
use crate::cli::output::{self, stdout_color_enabled};
use crate::core::import;
use crate::tui::theme::{AnsiStyle, paint};

const DEFAULT_CONFIG: &str = "{\n}\n";

pub(crate) fn initialize() -> SentraResult<()> {
    let home = home::home_dir().ok_or_else(|| {
        SentraError::Message(
            t(
                "could not determine current user home",
                "无法确定当前用户主目录",
            )
            .to_string(),
        )
    })?;
    initialize_at(&home)
}

fn initialize_at(home: &Path) -> SentraResult<()> {
    let sentra_home = sentra_home(home);
    std::fs::create_dir_all(&sentra_home)
        .map_err(|err| SentraError::io(Some(sentra_home.clone()), err))?;

    let config_path = sentra_config_file(home);
    if config_path.exists() {
        return Ok(());
    }

    std::fs::write(&config_path, DEFAULT_CONFIG)
        .map_err(|err| SentraError::io(Some(config_path), err))
}

pub(crate) fn run(action: ConfigAction) -> SentraResult<()> {
    let home = home::home_dir().ok_or_else(|| {
        SentraError::Message(
            t(
                "could not determine current user home",
                "无法确定当前用户主目录",
            )
            .to_string(),
        )
    })?;
    match action {
        ConfigAction::Help => {
            print_help();
            Ok(())
        }
        ConfigAction::Get => get(&home),
        ConfigAction::Set { key, value } => set(&home, &key, &value),
        ConfigAction::Del { key, value } => del(&home, &key, value.as_deref()),
    }
}

pub(crate) fn print_help() {
    println!(
        "{}",
        t(
            "\
Usage:
  sentra config get
  sentra config set cloudflare_dns <url>
  sentra config set threatbook_key <key>
  sentra config set chaitin_key <key>

Description:
  View and modify Sentra configuration.

Examples:
  sentra config get
  sentra config set threatbook_key sk-test
  sentra config set chaitin_key sk-test",
            "\
用法:
  sentra config get
  sentra config set cloudflare_dns <url>
  sentra config set threatbook_key <key>
  sentra config set chaitin_key <key>

说明:
  查看和修改 Sentra 配置。

示例:
  sentra config get
  sentra config set threatbook_key sk-test
  sentra config set chaitin_key sk-test"
        )
    );
}

pub(crate) fn run_rule(action: RuleAction) -> SentraResult<()> {
    let home = home::home_dir().ok_or_else(|| {
        SentraError::Message(
            t(
                "could not determine current user home",
                "无法确定当前用户主目录",
            )
            .to_string(),
        )
    })?;
    match action {
        RuleAction::Help => {
            print_rule_help();
            Ok(())
        }
        RuleAction::Get => get_rules(&home),
        RuleAction::Set { key, value } => set_rule_source(&home, &key, &value),
        RuleAction::Del { key, value } => del_rule_source(&home, &key, value.as_deref()),
    }
}

pub(crate) fn update_rules() -> SentraResult<()> {
    let home = home::home_dir().ok_or_else(|| {
        SentraError::Message(
            t(
                "could not determine current user home",
                "无法确定当前用户主目录",
            )
            .to_string(),
        )
    })?;
    update(&home)
}

pub(crate) fn has_rule_sources() -> SentraResult<bool> {
    let home = home::home_dir().ok_or_else(|| {
        SentraError::Message(
            t(
                "could not determine current user home",
                "无法确定当前用户主目录",
            )
            .to_string(),
        )
    })?;
    let config = load_json_config(&home)?;
    Ok(!rule_sources(&config).is_empty())
}

fn print_rule_help() {
    println!(
        "{}",
        t(
            "\
Usage:
  sentra rule get
  sentra rule set rule_<name> <url>
  sentra rule del rule_<name> [url]
  sentra update rules

Description:
  View, modify, and update Sentra rule sources.

Examples:
  sentra rule get
  sentra rule set rule_public https://example.test/rules.zip
  sentra rule del rule_public https://example.test/rules.zip
  sentra update rules",
            "\
用法:
  sentra rule get
  sentra rule set rule_<名称> <url>
  sentra rule del rule_<名称> [url]
  sentra update rules

说明:
  查看、修改并更新 Sentra 规则来源。

示例:
  sentra rule get
  sentra rule set rule_public https://example.test/rules.zip
  sentra rule del rule_public https://example.test/rules.zip
  sentra update rules"
        )
    );
}

fn get(home: &Path) -> SentraResult<()> {
    let config = load_json_config(home)?;

    print_page_header(
        t("View configuration", "查看配置"),
        &[(
            t("Config", "配置"),
            sentra_config_file(home).display().to_string(),
        )],
    );
    print_section(t("LLM", "大模型"));
    print_llm_config(&config);

    print_section(t("Intel", "情报"));
    print_intel_config(&config);

    print_section(t("YARA Rules", "YARA 规则"));
    print_rule_dir(&sentra_yara_rule_dir(home), &["yar", "yara"]);

    print_section(t("Threat Intelligence", "威胁情报"));
    print_rule_dir(&sentra_ti_rule_dir(home), &["txt", "csv"]);

    print_section(t("File Hash Lists", "文件哈希列表"));
    print_rule_dir(&sentra_hash_rule_dir(home), &["txt", "csv", "json"]);
    Ok(())
}

fn set(home: &Path, key: &str, value: &str) -> SentraResult<()> {
    let mut config = load_json_config(home)?;
    if is_rule_source_key(key) {
        return Err(SentraError::Message(format!(
            "{} sentra rule set {key} <url>",
            t(
                "rule source keys must be managed with",
                "规则来源键必须使用以下命令管理:"
            )
        )));
    } else if is_intel_key(key) {
        set_object_string(&mut config, "rule", key, value);
    } else if let Some(llm_key) = key.strip_prefix("llm.") {
        set_object_string(&mut config, "llm", llm_key, value);
    } else {
        return Err(SentraError::Message(format!(
            "{}: {key}",
            t("unknown config key", "未知配置键")
        )));
    }
    save_json_config(home, &config)?;
    let display = if is_secret_key(key) {
        mask_secret(value)
    } else {
        value.to_string()
    };
    feedback::result(
        Status::Success,
        t("Configuration updated", "配置已更新"),
        &[
            (t("Key", "键"), key.to_string()),
            (t("Value", "值"), display),
        ],
    );
    Ok(())
}

fn del(home: &Path, key: &str, _value: Option<&str>) -> SentraResult<()> {
    let mut config = load_json_config(home)?;
    if is_rule_source_key(key) {
        return Err(SentraError::Message(format!(
            "{} sentra rule del {key}",
            t(
                "rule source keys must be managed with",
                "规则来源键必须使用以下命令管理:"
            )
        )));
    } else if is_intel_key(key) {
        delete_object_key(&mut config, "rule", key);
    } else if let Some(llm_key) = key.strip_prefix("llm.") {
        delete_object_key(&mut config, "llm", llm_key);
    } else {
        return Err(SentraError::Message(format!(
            "{}: {key}",
            t("unknown config key", "未知配置键")
        )));
    }
    save_json_config(home, &config)?;
    feedback::result(
        Status::Success,
        t("Configuration unset", "配置已取消设置"),
        &[(t("Key", "键"), key.to_string())],
    );
    Ok(())
}

fn update(home: &Path) -> SentraResult<()> {
    let config = load_json_config(home)?;
    let sources = rule_sources(&config);
    if sources.is_empty() {
        return Err(SentraError::Message(
            t(
                "no rule sources configured; use sentra rule set rule_<name> <url>",
                "未配置规则来源；请使用 sentra rule set rule_<名称> <url>",
            )
            .to_string(),
        ));
    }

    import::run_sources_at(home, sources, "updating", "update")
}

fn load_json_config(home: &Path) -> SentraResult<serde_json::Value> {
    let path = sentra_config_file(home);
    let content =
        std::fs::read_to_string(&path).map_err(|err| SentraError::io(Some(path.clone()), err))?;
    serde_json::from_str(&content).map_err(|err| SentraError::Message(err.to_string()))
}

fn save_json_config(home: &Path, config: &serde_json::Value) -> SentraResult<()> {
    let path = sentra_config_file(home);
    let content = serde_json::to_string_pretty(config)
        .map_err(|err| SentraError::Message(err.to_string()))?;
    std::fs::write(&path, format!("{content}\n")).map_err(|err| SentraError::io(Some(path), err))
}

fn print_llm_config(config: &serde_json::Value) {
    let Some(llm) = config.get("llm").and_then(|value| value.as_object()) else {
        print_empty(t("(no configuration)", "(无配置)"));
        return;
    };
    if llm.is_empty() {
        print_empty(t("(no configuration)", "(无配置)"));
        return;
    }
    print_optional_value(llm, "api", "llm.api", false);
    print_optional_value(llm, "key", "llm.key", true);
    print_optional_value(llm, "model", "llm.model", false);
    print_optional_value(llm, "protocol", "llm.protocol", false);
}

fn print_intel_config(config: &serde_json::Value) {
    let mut rows = Vec::new();
    if let Some(intel) = config.get("intel").and_then(|value| value.as_object()) {
        for (key, value) in intel {
            if let Some(value) = value.as_str() {
                rows.push((
                    format!("intel.{key}"),
                    value.to_string(),
                    is_secret_key(key),
                ));
            }
        }
    }
    if let Some(rule) = config.get("rule").and_then(|value| value.as_object()) {
        for (key, value) in rule {
            if is_rule_source_key(key) {
                continue;
            }
            for source in string_values(value) {
                let label = if is_intel_key(key) {
                    format!("intel.{key}")
                } else {
                    format!("rule.{key}")
                };
                rows.push((label, source, is_secret_key(key)));
            }
        }
    }
    if rows.is_empty() {
        print_empty(t("(no configuration)", "(无配置)"));
        return;
    }
    rows.sort_by(|left, right| left.0.cmp(&right.0).then(left.1.cmp(&right.1)));
    for (key, value, secret) in rows {
        let display = if secret { mask_secret(&value) } else { value };
        print_key_value(&key, &display);
    }
}

fn print_optional_value(
    object: &serde_json::Map<String, serde_json::Value>,
    key: &str,
    label: &str,
    secret: bool,
) {
    if let Some(value) = object.get(key).and_then(|value| value.as_str()) {
        let display = if secret {
            mask_secret(value)
        } else {
            value.to_string()
        };
        print_key_value(label, &display);
    }
}

fn get_rules(home: &Path) -> SentraResult<()> {
    let config = load_json_config(home)?;

    print_page_header(
        t("View rule sources", "查看规则来源"),
        &[(
            t("Config", "配置"),
            sentra_config_file(home).display().to_string(),
        )],
    );
    print_section(t("Rule Sources", "规则来源"));
    print_rule_sources(&config);

    print_section(t("YARA Rules", "YARA 规则"));
    print_rule_dir(&sentra_yara_rule_dir(home), &["yar", "yara"]);

    print_section(t("Threat Intelligence", "威胁情报"));
    print_rule_dir(&sentra_ti_rule_dir(home), &["txt", "csv"]);

    print_section(t("File Hash Lists", "文件哈希列表"));
    print_rule_dir(&sentra_hash_rule_dir(home), &["txt", "csv", "json"]);
    Ok(())
}

fn print_page_header(title: &str, fields: &[(&str, String)]) {
    let color = stdout_color_enabled();
    let mut output = String::new();
    output.push_str(&paint(
        if color { "●" } else { "[INFO]" },
        AnsiStyle::Purple,
        color,
    ));
    output.push(' ');
    output.push_str(&paint(title, AnsiStyle::Foreground, color));
    output.push('\n');
    for (label, value) in fields {
        output.push_str("  ");
        output.push_str(&paint(&format!("{label}:"), AnsiStyle::Muted, color));
        output.push(' ');
        output.push_str(&paint(value, AnsiStyle::Secondary, color));
        output.push('\n');
    }
    let _ = output::write_stdout(&output);
}

fn print_section(title: &str) {
    let color = stdout_color_enabled();
    let _ = output::write_stdout(&format!(
        "\n{}\n",
        paint(title, AnsiStyle::Foreground, color)
    ));
}

fn set_rule_source(home: &Path, key: &str, value: &str) -> SentraResult<()> {
    if !is_rule_source_key(key) {
        return Err(SentraError::Message(format!(
            "{}: {key}; {} rule_<name>",
            t("unknown rule key", "未知规则键"),
            t("expected", "期望")
        )));
    }
    let mut config = load_json_config(home)?;
    append_rule_source(&mut config, key, value);
    save_json_config(home, &config)?;
    feedback::result(
        Status::Success,
        t("Rule source updated", "规则来源已更新"),
        &[
            (t("Key", "键"), key.to_string()),
            (t("Source", "来源"), value.to_string()),
        ],
    );
    Ok(())
}

fn del_rule_source(home: &Path, key: &str, value: Option<&str>) -> SentraResult<()> {
    if !is_rule_source_key(key) {
        return Err(SentraError::Message(format!(
            "{}: {key}; {} rule_<name>",
            t("unknown rule key", "未知规则键"),
            t("expected", "期望")
        )));
    }
    let mut config = load_json_config(home)?;
    delete_rule_source(&mut config, key, value);
    save_json_config(home, &config)?;
    feedback::result(
        Status::Success,
        t("Rule source unset", "规则来源已取消设置"),
        &[(t("Key", "键"), key.to_string())],
    );
    Ok(())
}

fn print_rule_sources(config: &serde_json::Value) {
    let mut rows = Vec::new();
    if let Some(rule) = config.get("rule").and_then(|value| value.as_object()) {
        for (key, value) in rule {
            if is_rule_source_key(key) {
                for source in string_values(value) {
                    rows.push((key.to_string(), source));
                }
            }
        }
    }
    if rows.is_empty() {
        print_empty(t("(no configuration)", "(无配置)"));
        return;
    }
    rows.sort_by(|left, right| left.0.cmp(&right.0).then(left.1.cmp(&right.1)));
    for (key, value) in rows {
        print_key_value(&format!("rule.{key}"), &value);
    }
}

fn print_rule_dir(dir: &std::path::Path, extensions: &[&str]) {
    let files = list_rule_files(dir, extensions);
    if files.is_empty() {
        print_empty(t("(none)", "(无)"));
        return;
    }
    let color = stdout_color_enabled();
    let mut output = String::new();
    for (name, size) in files {
        output.push_str("  ");
        output.push_str(&paint(&name, AnsiStyle::Secondary, color));
        output.push(' ');
        output.push_str(&paint(
            &format!("({:.1} KB)", size as f64 / 1024.0),
            AnsiStyle::Muted,
            color,
        ));
        output.push('\n');
    }
    let _ = output::write_stdout(&output);
}

fn print_empty(value: &str) {
    let color = stdout_color_enabled();
    let _ = output::write_stdout(&format!("  {}\n", paint(value, AnsiStyle::Muted, color)));
}

fn print_key_value(key: &str, value: &str) {
    let color = stdout_color_enabled();
    let mut output = String::new();
    output.push_str("  ");
    output.push_str(&paint(key, AnsiStyle::Muted, color));
    output.push(' ');
    output.push_str(&paint("=", AnsiStyle::Muted, color));
    output.push(' ');
    output.push_str(&paint(value, AnsiStyle::Secondary, color));
    output.push('\n');
    let _ = output::write_stdout(&output);
}

fn list_rule_files(dir: &std::path::Path, extensions: &[&str]) -> Vec<(String, u64)> {
    let mut files = Vec::new();
    if !dir.is_dir() {
        return files;
    }
    for entry in walkdir::WalkDir::new(dir)
        .into_iter()
        .filter_map(Result::ok)
    {
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        let ext = path
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default();
        if !extensions
            .iter()
            .any(|candidate| ext.eq_ignore_ascii_case(candidate))
        {
            continue;
        }
        let relative = path.strip_prefix(dir).unwrap_or(path).to_string_lossy();
        let size = entry.metadata().map(|metadata| metadata.len()).unwrap_or(0);
        files.push((relative.to_string(), size));
    }
    files.sort_by(|left, right| left.0.cmp(&right.0));
    files
}

fn append_rule_source(config: &mut serde_json::Value, key: &str, value: &str) {
    let object = ensure_object(config);
    let rule = object
        .entry("rule")
        .or_insert_with(|| serde_json::json!({}));
    let rule = ensure_object(rule);
    let entry = rule
        .entry(key)
        .or_insert_with(|| serde_json::Value::Array(Vec::new()));
    if !entry.is_array() {
        let previous = entry.as_str().map(str::to_string);
        *entry = serde_json::Value::Array(Vec::new());
        if let Some(previous) = previous {
            entry
                .as_array_mut()
                .unwrap()
                .push(serde_json::json!(previous));
        }
    }
    let values = entry.as_array_mut().unwrap();
    if !values.iter().any(|item| item.as_str() == Some(value)) {
        values.push(serde_json::json!(value));
    }
}

fn delete_rule_source(config: &mut serde_json::Value, key: &str, value: Option<&str>) {
    let Some(rule) = config
        .get_mut("rule")
        .and_then(|value| value.as_object_mut())
    else {
        return;
    };
    if let Some(value) = value {
        if let Some(entry) = rule.get_mut(key) {
            if let Some(values) = entry.as_array_mut() {
                values.retain(|item| item.as_str() != Some(value));
                if values.is_empty() {
                    rule.remove(key);
                }
            } else if entry.as_str() == Some(value) {
                rule.remove(key);
            }
        }
    } else {
        rule.remove(key);
    }
}

fn set_object_string(config: &mut serde_json::Value, section: &str, key: &str, value: &str) {
    let object = ensure_object(config);
    let section = object
        .entry(section)
        .or_insert_with(|| serde_json::json!({}));
    ensure_object(section).insert(key.to_string(), serde_json::json!(value));
}

fn delete_object_key(config: &mut serde_json::Value, section: &str, key: &str) {
    if let Some(object) = config
        .get_mut(section)
        .and_then(|value| value.as_object_mut())
    {
        object.remove(key);
    }
}

fn ensure_object(value: &mut serde_json::Value) -> &mut serde_json::Map<String, serde_json::Value> {
    if !value.is_object() {
        *value = serde_json::json!({});
    }
    value.as_object_mut().unwrap()
}

fn rule_sources(config: &serde_json::Value) -> Vec<String> {
    let mut sources = Vec::new();
    if let Some(rule) = config.get("rule").and_then(|value| value.as_object()) {
        for (key, value) in rule {
            if is_rule_source_key(key) {
                sources.extend(string_values(value));
            }
        }
    }
    sources.sort();
    sources.dedup();
    sources
}

fn string_values(value: &serde_json::Value) -> Vec<String> {
    match value {
        serde_json::Value::String(value) => vec![value.clone()],
        serde_json::Value::Array(values) => values
            .iter()
            .filter_map(|value| value.as_str().map(str::to_string))
            .collect(),
        _ => Vec::new(),
    }
}

fn is_rule_source_key(key: &str) -> bool {
    key.starts_with("rule_")
}

fn is_intel_key(key: &str) -> bool {
    matches!(
        key,
        "cloudflare_dns" | "threatbook_key" | "threatbook_url" | "chaitin_key" | "chaitin_url"
    )
}

fn is_secret_key(key: &str) -> bool {
    key == "key" || key.ends_with("_key") || key == "llm.key"
}

fn mask_secret(value: &str) -> String {
    let chars = value.chars().collect::<Vec<_>>();
    if chars.len() <= 8 {
        return format!("{}****", chars.iter().take(2).collect::<String>());
    }
    format!(
        "{}****{}",
        chars.iter().take(4).collect::<String>(),
        chars
            .iter()
            .skip(chars.len().saturating_sub(4))
            .collect::<String>()
    )
}

#[cfg(test)]
mod tests {
    use super::initialize_at;

    #[test]
    fn creates_empty_config_without_rules() {
        let dir = tempfile::tempdir().unwrap();

        initialize_at(dir.path()).unwrap();

        let content = std::fs::read_to_string(dir.path().join(".sentra").join("config.json"))
            .expect("read config");
        let value: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert_eq!(value, serde_json::json!({}));
    }

    #[test]
    fn keeps_existing_config() {
        let dir = tempfile::tempdir().unwrap();
        let sentra_home = dir.path().join(".sentra");
        std::fs::create_dir_all(&sentra_home).unwrap();
        std::fs::write(
            sentra_home.join("config.json"),
            r#"{"scan":{"enabled":true}}"#,
        )
        .unwrap();

        initialize_at(dir.path()).unwrap();

        let content = std::fs::read_to_string(sentra_home.join("config.json")).unwrap();
        assert_eq!(content, r#"{"scan":{"enabled":true}}"#);
    }
}
