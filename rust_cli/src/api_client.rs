// API client for interacting with the Python API will go here
use anyhow::Result;
use reqwest::Client;
use std::time::Duration;
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

#[derive(Debug, Deserialize, Clone)]
pub struct RouteResponse {
    pub mode: String,
    pub framed: String,
    pub note: Option<String>,
}

pub async fn send_query(api_url: &str, input: &str, history: Vec<String>) -> Result<LlmResponse> {
    let client = Client::builder()
        .timeout(Duration::from_secs(35))
        .build()?;
    let res = client
        .post(format!("{}/query", api_url))
        .json(&Query { input, history })
        .send()
        .await?;

    let res = res.error_for_status()?;
    let out = res.json::<LlmResponse>().await?;
    Ok(out)
}

#[derive(Serialize)]
pub struct RouteIn<'a> {
    pub input: &'a str,
    pub history: Vec<String>,
}

pub async fn route_prompt(api_url: &str, input: &str, history: Vec<String>) -> Result<RouteResponse> {
    let client = Client::builder()
        .timeout(Duration::from_secs(20))
        .build()?;
    let res = client
        .post(format!("{}/route", api_url))
        .json(&RouteIn { input, history })
        .send()
        .await?;
    let res = res.error_for_status()?;
    let out = res.json::<RouteResponse>().await?;
    Ok(out)
}
