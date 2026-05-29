use crate::model::Node;
use anyhow::{Context, Result};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};

use super::node_text;

#[derive(Serialize)]
struct EmbeddingRequest {
    model: String,
    input: Vec<String>,
}

#[derive(Deserialize)]
struct EmbeddingResponse {
    data: Vec<EmbeddingItem>,
}

#[derive(Deserialize)]
struct EmbeddingItem {
    embedding: Vec<f32>,
}

pub fn embed_query(api_key: &str, model: &str, query: &str) -> Result<Vec<f32>> {
    let client = Client::new();
    let response = client
        .post("https://api.openai.com/v1/embeddings")
        .bearer_auth(api_key)
        .json(&EmbeddingRequest {
            model: model.to_string(),
            input: vec![query.to_string()],
        })
        .send()
        .context("failed to call embeddings API")?;
    let status = response.status();
    let body = response
        .text()
        .context("failed to read embeddings response")?;
    anyhow::ensure!(
        status.is_success(),
        "embedding request failed with {status}: {body}"
    );
    let data = serde_json::from_str::<EmbeddingResponse>(&body)
        .with_context(|| format!("failed to parse embedding response: {body}"))?;
    Ok(data
        .data
        .into_iter()
        .next()
        .map(|item| item.embedding)
        .unwrap_or_default())
}

pub fn embed_nodes(api_key: &str, model: &str, nodes: &[Node]) -> Result<Vec<(Node, Vec<f32>)>> {
    if nodes.is_empty() {
        return Ok(Vec::new());
    }

    let client = Client::new();
    let response = client
        .post("https://api.openai.com/v1/embeddings")
        .bearer_auth(api_key)
        .json(&EmbeddingRequest {
            model: model.to_string(),
            input: nodes.iter().map(node_text).collect(),
        })
        .send()
        .context("failed to call embeddings API")?;
    let status = response.status();
    let body = response
        .text()
        .context("failed to read embeddings response")?;
    anyhow::ensure!(
        status.is_success(),
        "embedding request failed with {status}: {body}"
    );
    let data = serde_json::from_str::<EmbeddingResponse>(&body)
        .with_context(|| format!("failed to parse embedding response: {body}"))?;

    Ok(nodes
        .iter()
        .cloned()
        .zip(data.data.into_iter().map(|item| item.embedding))
        .collect())
}
