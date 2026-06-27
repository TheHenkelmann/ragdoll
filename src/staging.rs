// SPDX-License-Identifier: AGPL-3.0-only

use std::path::{Path, PathBuf};

use crate::config::Config;

/// Remove on-disk staging artifacts for a source (text/file types only).
pub fn cleanup_staging_artifacts(
    config: &Config,
    source_id: &str,
    source_type: &str,
    uri: Option<&str>,
) {
    match source_type {
        "text" => {
            let path = config.staging_dir.join(format!("{source_id}.txt"));
            let _ = std::fs::remove_file(path);
        }
        "file" => {
            if let Some(uri) = uri {
                let path = PathBuf::from(uri);
                if path.starts_with(&config.staging_dir) && path.exists() {
                    let _ = std::fs::remove_file(path);
                }
            }
        }
        _ => {}
    }
}

pub fn staging_text_path(staging_dir: &Path, source_id: &str) -> PathBuf {
    staging_dir.join(format!("{source_id}.txt"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use tempfile::TempDir;

    #[test]
    fn cleanup_staging_artifacts_removes_text_file() {
        let dir = TempDir::new().unwrap();
        let mut config = Config::for_test(dir.path().to_path_buf(), "secret");
        config.staging_dir = dir.path().join("staging");
        std::fs::create_dir_all(&config.staging_dir).unwrap();
        let path = staging_text_path(&config.staging_dir, "src-1");
        std::fs::write(&path, "hello").unwrap();
        cleanup_staging_artifacts(&config, "src-1", "text", None);
        assert!(!path.exists());
    }
}
