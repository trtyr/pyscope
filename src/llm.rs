use crate::config;
use crate::model::CodeGraph;
use anyhow::{Context, Result, bail};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::time::Duration;

#[derive(Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<Message>,
}

#[derive(Serialize)]
struct Message {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct ChatResponse {
    #[serde(default)]
    choices: Vec<Choice>,
    #[serde(default)]
    usage: Option<Usage>,
}

#[derive(Deserialize)]
struct Choice {
    message: ResponseMessage,
}

#[derive(Deserialize)]
struct ResponseMessage {
    content: Option<String>,
}

#[derive(Deserialize)]
struct Usage {
    #[serde(default)]
    total_tokens: Option<u64>,
}

pub fn ask(graph: &CodeGraph, question: &str) -> Result<Value> {
    let config = config::load()?;
    let api_key = config
        .api_key
        .as_deref()
        .filter(|key| !key.trim().is_empty())
        .context("pyscope config is missing api_key; run `pyscope config set-api-key <key>`")?;

    let nav_map = crate::nav::nav_map(graph, true, 6000)?;
    let project_map = nav_map
        .get("content")
        .and_then(Value::as_str)
        .context("nav_map response missing content")?;

    let system_prompt = format!(
        "You are a code navigation assistant. Use the following project structure to answer questions about the codebase.\n\n{}",
        project_map
    );

    let client = Client::builder()
        .timeout(Duration::from_secs(90))
        .build()
        .context("failed to build HTTP client")?;

    let url = format!(
        "{}/chat/completions",
        config.api_base.trim_end_matches('/')
    );
    let response = client
        .post(&url)
        .bearer_auth(api_key)
        .json(&ChatRequest {
            model: config.model.clone(),
            messages: vec![
                Message {
                    role: "system".to_string(),
                    content: system_prompt,
                },
                Message {
                    role: "user".to_string(),
                    content: question.to_string(),
                },
            ],
        })
        .send()
        .with_context(|| format!("failed to call {}", url))?;

    let status = response.status();
    let body = response.text().context("failed to read LLM response")?;
    if !status.is_success() {
        bail!("LLM request failed with {status}: {body}");
    }

    let data = serde_json::from_str::<ChatResponse>(&body)
        .with_context(|| format!("failed to parse LLM response: {body}"))?;
    let answer = data
        .choices
        .into_iter()
        .filter_map(|choice| choice.message.content)
        .collect::<Vec<_>>()
        .join("\n");

    if answer.trim().is_empty() {
        bail!("LLM returned no answer content");
    }

    Ok(json!({
        "kind": "ask",
        "question": question,
        "answer": answer,
        "model": config.model,
        "tokens_used": data.usage.and_then(|usage| usage.total_tokens),
    }))
}
