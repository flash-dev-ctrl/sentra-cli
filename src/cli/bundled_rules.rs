use std::fs;
use std::path::{Path, PathBuf};

use sentra_lib::config::{
    sentra_hash_rule_dir, sentra_home, sentra_ti_rule_dir, sentra_yara_rule_dir,
};
use sentra_lib::risks::{RuleDirectoryConfig, RuleStore};
use sentra_lib::{SentraError, SentraResult};
use sha2::{Digest, Sha256};

const BUNDLED_RULES_ZIP: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/bundled-rules.zip"));
const VERSION_FILE_NAME: &str = ".bundled-rules-version";

pub(crate) fn ensure_bundled_rules(home: &Path) -> SentraResult<()> {
    let version = bundled_rules_version();
    let version_path = bundled_rules_version_file(home);

    if fs::read_to_string(&version_path)
        .map(|content| content.trim() == version)
        .unwrap_or(false)
    {
        return Ok(());
    }

    if !version_path.exists() && default_rule_files_exist(home) {
        write_version_file(&version_path, &version)?;
        return Ok(());
    }

    import_bundled_rules(home)?;
    write_version_file(&version_path, &version)
}

fn import_bundled_rules(home: &Path) -> SentraResult<()> {
    let tmp = tempfile::Builder::new()
        .prefix("sentra-bundled-rules-")
        .tempdir()
        .map_err(|err| SentraError::io(None::<PathBuf>, err))?;
    let zip_path = tmp.path().join("rules.zip");
    fs::write(&zip_path, BUNDLED_RULES_ZIP)
        .map_err(|err| SentraError::io(Some(zip_path.clone()), err))?;

    let store = RuleStore::new(RuleDirectoryConfig {
        yara: Some(sentra_yara_rule_dir(home)),
        ti: Some(sentra_ti_rule_dir(home)),
        hash: Some(sentra_hash_rule_dir(home)),
    });
    store.import(zip_path.to_string_lossy().as_ref())?;
    Ok(())
}

fn write_version_file(path: &Path, version: &str) -> SentraResult<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| SentraError::io(Some(parent.to_path_buf()), err))?;
    }
    fs::write(path, format!("{version}\n"))
        .map_err(|err| SentraError::io(Some(path.to_path_buf()), err))
}

fn default_rule_files_exist(home: &Path) -> bool {
    [
        sentra_yara_rule_dir(home),
        sentra_ti_rule_dir(home),
        sentra_hash_rule_dir(home),
    ]
    .into_iter()
    .any(|dir| contains_file(&dir))
}

fn contains_file(dir: &Path) -> bool {
    let Ok(entries) = fs::read_dir(dir) else {
        return false;
    };
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        if path.is_file() || (path.is_dir() && contains_file(&path)) {
            return true;
        }
    }
    false
}

fn bundled_rules_version_file(home: &Path) -> PathBuf {
    sentra_home(home).join(VERSION_FILE_NAME)
}

fn bundled_rules_version() -> String {
    let hash = Sha256::digest(BUNDLED_RULES_ZIP);
    format!("{}:{hash:x}", env!("CARGO_PKG_VERSION"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_use_imports_bundled_rules_and_writes_version() {
        let dir = tempfile::tempdir().unwrap();

        ensure_bundled_rules(dir.path()).unwrap();

        assert!(bundled_rules_version_file(dir.path()).is_file());
        assert!(sentra_yara_rule_dir(dir.path()).is_dir());
        assert!(sentra_ti_rule_dir(dir.path()).is_dir());
        assert!(sentra_hash_rule_dir(dir.path()).is_dir());
        assert!(contains_file(&sentra_yara_rule_dir(dir.path())));
    }

    #[test]
    fn matching_version_skips_reimport() {
        let dir = tempfile::tempdir().unwrap();
        ensure_bundled_rules(dir.path()).unwrap();

        let marker = sentra_yara_rule_dir(dir.path()).join("prompt_injection_generic.yara");
        fs::write(&marker, "local edit").unwrap();

        ensure_bundled_rules(dir.path()).unwrap();

        assert_eq!(fs::read_to_string(marker).unwrap(), "local edit");
    }

    #[test]
    fn existing_manual_rules_are_adopted_on_first_use() {
        let dir = tempfile::tempdir().unwrap();
        let manual = sentra_yara_rule_dir(dir.path()).join("manual.yar");
        fs::create_dir_all(manual.parent().unwrap()).unwrap();
        fs::write(&manual, "rule Manual { condition: true }").unwrap();

        ensure_bundled_rules(dir.path()).unwrap();

        assert_eq!(
            fs::read_to_string(bundled_rules_version_file(dir.path()))
                .unwrap()
                .trim(),
            bundled_rules_version()
        );
        assert!(
            !sentra_yara_rule_dir(dir.path())
                .join("prompt_injection_generic.yara")
                .exists()
        );
    }
}
