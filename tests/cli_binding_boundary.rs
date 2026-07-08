use std::fs;
use std::path::Path;

#[test]
fn cli_does_not_depend_on_binding_dtos() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut source = String::new();

    for entry in fs::read_dir(root.join("src")).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.is_file() && path.extension().is_some_and(|ext| ext == "rs") {
            source.push_str(&fs::read_to_string(path).unwrap());
        }
    }

    for entry in fs::read_dir(root.join("src/cli")).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "rs") {
            source.push_str(&fs::read_to_string(path).unwrap());
        }
    }

    for forbidden in [
        "sentra_lib::bindings",
        "bindings::c",
        "ScanRequest",
        "UnifiedAsset",
        "ScannerSelection",
    ] {
        assert!(
            !source.contains(forbidden),
            "sentra-cli must not depend on binding surface `{forbidden}`"
        );
    }
}
