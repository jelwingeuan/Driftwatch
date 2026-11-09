use super::{error_item, file_item, list_dir};
use crate::Item;
use std::path::{Path, PathBuf};

pub const CATEGORY: &str = "shell";

/// Shell startup files and PATH config, each hashed. Missing files are skipped.
pub fn collect() -> Vec<Item> {
    let mut files: Vec<PathBuf> = Vec::new();
    if let Ok(home) = std::env::var("HOME") {
        for f in [
            ".zshrc",
            ".zprofile",
            ".zshenv",
            ".zlogin",
            ".bashrc",
            ".bash_profile",
            ".profile",
        ] {
            files.push(Path::new(&home).join(f));
        }
    }
    for f in ["/etc/zshrc", "/etc/zprofile", "/etc/profile", "/etc/paths"] {
        files.push(PathBuf::from(f));
    }
    collect_files(&files, &[PathBuf::from("/etc/paths.d")])
}

/// Hash each file, then every file inside each dir. Key = path.
pub fn collect_files(files: &[PathBuf], dirs: &[PathBuf]) -> Vec<Item> {
    let mut items: Vec<Item> = files
        .iter()
        .filter_map(|p| file_item(CATEGORY, p))
        .collect();
    for dir in dirs {
        match list_dir(dir) {
            Ok(paths) => items.extend(paths.iter().filter_map(|p| file_item(CATEGORY, p))),
            Err(e) => items.push(error_item(CATEGORY, &dir.to_string_lossy(), &e)),
        }
    }
    items
}
