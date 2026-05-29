use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::path::PathBuf;

const DEFAULT_API_BASE: &str = "https://api.openai.com/v1";
const DEFAULT_MODEL: &str = "gpt-4o";
const DEFAULT_EMBEDDING_MODEL: &str = "text-embedding-3-small";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CodegraphConfig {
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default = "default_api_base")]
    pub api_base: String,
    #[serde(default = "default_model")]
    pub model: String,
    #[serde(default)]
    pub embedding_key: Option<String>,
    #[serde(default = "default_embedding_model")]
    pub embedding_model: String,
    #[serde(default)]
    pub rerank_model: Option<String>,
}

impl Default for CodegraphConfig {
    fn default() -> Self {
        Self {
            api_key: None,
            api_base: default_api_base(),
            model: default_model(),
            embedding_key: None,
            embedding_model: default_embedding_model(),
            rerank_model: None,
        }
    }
}

fn default_api_base() -> String {
    DEFAULT_API_BASE.to_string()
}

fn default_model() -> String {
    DEFAULT_MODEL.to_string()
}

fn default_embedding_model() -> String {
    DEFAULT_EMBEDDING_MODEL.to_string()
}

pub fn path() -> Result<PathBuf> {
    let home = std::env::var("HOME").context("HOME is not set; cannot locate pyscope config")?;
    let path = PathBuf::from(home)
        .join(".config")
        .join("pyscope")
        .join("config.json");
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    Ok(path)
}

pub fn load() -> Result<CodegraphConfig> {
    let path = path()?;
    if !path.exists() {
        return Ok(CodegraphConfig::default());
    }
    let bytes = std::fs::read(&path)
        .with_context(|| format!("failed to read pyscope config at {}", path.display()))?;
    Ok(serde_json::from_slice(&bytes)?)
}

pub fn save(config: &CodegraphConfig) -> Result<PathBuf> {
    let path = path()?;
    std::fs::write(&path, serde_json::to_vec_pretty(config)?)
        .with_context(|| format!("failed to write {}", path.display()))?;
    set_private_permissions(&path)?;
    Ok(path)
}

pub fn show() -> Result<Value> {
    let path = path()?;
    let config = load()?;
    Ok(json!({
        "kind": "config",
        "path": path,
        "config": config,
    }))
}

pub fn set_api_key(key: &str) -> Result<Value> {
    let mut config = load()?;
    config.api_key = Some(key.to_string());
    let path = save(&config)?;
    Ok(json!({ "kind": "config", "path": path, "config": config }))
}

pub fn set_model(model: &str) -> Result<Value> {
    let mut config = load()?;
    config.model = model.to_string();
    let path = save(&config)?;
    Ok(json!({ "kind": "config", "path": path, "config": config }))
}

pub fn set_embedding_key(key: &str) -> Result<Value> {
    let mut config = load()?;
    config.embedding_key = Some(key.to_string());
    let path = save(&config)?;
    Ok(json!({ "kind": "config", "path": path, "config": config }))
}

#[cfg(unix)]
fn set_private_permissions(path: &std::path::Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
        .with_context(|| format!("failed to chmod 600 {}", path.display()))
}

#[cfg(not(unix))]
fn set_private_permissions(_: &std::path::Path) -> Result<()> {
    Ok(())
}
