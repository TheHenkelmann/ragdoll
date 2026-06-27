// SPDX-License-Identifier: AGPL-3.0-only

use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::models::bootstrap::is_valid_hf_model_name;
use crate::models::catalog::{find_catalog_entry, predefined_catalog};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CustomModelsFile {
    models: Vec<String>,
}

fn custom_models_path(config: &Config) -> PathBuf {
    config.data_dir.join("custom_models.json")
}

pub fn load_custom_models(config: &Config) -> Result<Vec<String>> {
    let path = custom_models_path(config);
    if !path.exists() {
        return Ok(Vec::new());
    }
    let content = std::fs::read_to_string(&path)
        .with_context(|| format!("read {}", path.display()))?;
    let file: CustomModelsFile = serde_json::from_str(&content)
        .with_context(|| format!("parse {}", path.display()))?;
    Ok(file.models)
}

fn save_custom_models(config: &Config, models: &[String]) -> Result<()> {
    let path = custom_models_path(config);
    let file = CustomModelsFile {
        models: models.to_vec(),
    };
    let content = serde_json::to_string_pretty(&file)?;
    std::fs::write(&path, content).with_context(|| format!("write {}", path.display()))?;
    Ok(())
}

pub fn is_custom_model(config: &Config, name: &str) -> bool {
    load_custom_models(config)
        .ok()
        .is_some_and(|models| models.iter().any(|m| m == name))
}

pub fn add_custom_model(config: &Config, name: &str) -> Result<()> {
    if !is_valid_hf_model_name(name) {
        anyhow::bail!("invalid Hugging Face model id: {name}");
    }
    if find_catalog_entry(name).is_some() {
        anyhow::bail!("model {name} is already in the predefined catalog");
    }
    let mut models = load_custom_models(config)?;
    if models.iter().any(|m| m == name) {
        return Ok(());
    }
    models.push(name.to_string());
    models.sort();
    save_custom_models(config, &models)
}

pub fn remove_custom_model(config: &Config, name: &str) -> Result<()> {
    let mut models = load_custom_models(config)?;
    let before = models.len();
    models.retain(|m| m != name);
    if models.len() == before {
        anyhow::bail!("custom model not found: {name}");
    }
    save_custom_models(config, &models)
}

pub fn all_catalog_names(config: &Config) -> Result<Vec<String>> {
    let mut names: Vec<String> = predefined_catalog()
        .iter()
        .map(|e| e.name.to_string())
        .collect();
    for custom in load_custom_models(config)? {
        if !names.iter().any(|n| n == &custom) {
            names.push(custom);
        }
    }
    names.sort();
    Ok(names)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_and_remove_custom_model() {
        let dir = tempfile::tempdir().unwrap();
        let config = Config::for_test(dir.path().to_path_buf(), "secret");
        add_custom_model(&config, "org/my-model").unwrap();
        assert!(is_custom_model(&config, "org/my-model"));
        remove_custom_model(&config, "org/my-model").unwrap();
        assert!(!is_custom_model(&config, "org/my-model"));
    }
}
