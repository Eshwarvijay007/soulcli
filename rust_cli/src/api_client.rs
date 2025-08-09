// API client for interacting with the Python API will go here
use anyhow::Result;
use reqwest::Client;
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
pub struct Query<'a> {
    pub input: &'a str,
    pub history: Vec<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct LlmResponse {
    pub text: String,
    pub emotion: Option<String>,
}

pub async fn send_query(api_url: &str, input: &str, history: Vec<String>) -> Result<LlmResponse> {
    let client = Client::new();
    let res = client
        .post(format!("{}/query", api_url))
        .json(&Query { input, history })
        .send()
        .await?;

    let out = res.json::<LlmResponse>().await?;
    Ok(out)
}
