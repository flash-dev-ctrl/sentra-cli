use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use zip::write::SimpleFileOptions;

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let rules_dir = manifest_dir.join("rules");
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let zip_path = out_dir.join("bundled-rules.zip");

    println!("cargo:rerun-if-changed={}", rules_dir.display());
    create_rules_zip(&rules_dir, &zip_path).expect("failed to build bundled rules archive");
}

fn create_rules_zip(rules_dir: &Path, zip_path: &Path) -> io::Result<()> {
    let file = fs::File::create(zip_path)?;
    let mut zip = zip::ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    if rules_dir.is_dir() {
        for path in rule_files(rules_dir)? {
            println!("cargo:rerun-if-changed={}", path.display());
            let relative = path.strip_prefix(rules_dir).unwrap_or(&path);
            let name = zip_entry_name(relative);
            zip.start_file(name, options)?;
            let mut input = fs::File::open(&path)?;
            io::copy(&mut input, &mut zip)?;
        }
    }

    zip.finish()?;
    Ok(())
}

fn rule_files(dir: &Path) -> io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    collect_rule_files(dir, &mut files)?;
    files.sort();
    Ok(files)
}

fn collect_rule_files(dir: &Path, files: &mut Vec<PathBuf>) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_rule_files(&path, files)?;
        } else if path.is_file() {
            files.push(path);
        }
    }
    Ok(())
}

fn zip_entry_name(path: &Path) -> String {
    path.components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}
