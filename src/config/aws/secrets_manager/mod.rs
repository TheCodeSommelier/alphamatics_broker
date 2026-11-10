use std::{collections::HashMap, sync::OnceLock};

use anyhow::{Context, Result, anyhow};

static SECRETS: OnceLock<HashMap<String, String>> = OnceLock::new();

pub async fn init(name: &str) -> Result<()> {
    let config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
    let client = aws_sdk_secretsmanager::Client::new(&config);
    let response = client
        .get_secret_value()
        .secret_id(name)
        .send()
        .await
        .with_context(|| format!("failed to load AWS secret {name}"))?;

    let secret = response
        .secret_string()
        .ok_or_else(|| anyhow!("AWS secret {name} has no string value"))?;
    let values: HashMap<String, String> = serde_json::from_str(secret)
        .with_context(|| format!("AWS secret {name} is not a JSON object of strings"))?;

    SECRETS
        .set(values)
        .map_err(|_| anyhow!("AWS secrets were already initialized"))?;
    Ok(())
}

pub fn secrets() -> &'static HashMap<String, String> {
    SECRETS
        .get()
        .expect("AWS secrets must be loaded at startup")
}

pub fn var(name: &str) -> dotenvy::Result<String> {
    if SECRETS.get().is_some() {
        if let Some(value) = secrets().get(name) {
            return Ok(value.clone());
        }
    }

    dotenvy::var(name)
}
