use anyhow::Result;
use reqwest::Url;
use serde_derive::{Deserialize, Serialize};
use tracing::{error, info};

use crate::config::SocialInstance;

use super::{Lang, StatusContent};

#[derive(Deserialize, Debug)]
struct Status {
    id: String,
    uri: String,
}

#[derive(Serialize, Debug)]
struct Toot<'a> {
    status: &'a str,
    visibility: &'static str,
    language: &'a str,
}

pub async fn push_to_mastodon(
    instance: &SocialInstance,
    status: &StatusContent,
    language: &Lang,
) -> Result<Option<Url>> {
    info!("Push to social Mastodon");

    let Some(token) = std::env::var(&instance.token_var).ok() else {
        error!("`{}` env var is not defined", instance.token_var);
        return Ok(None);
    };

    // publish toot
    let toot = Toot {
        status,
        visibility: "public",
        language,
    };

    use sha2::{Digest, Sha256};
    let hash = format!("{:x}", Sha256::digest(toot.status.as_bytes()));

    let status = reqwest::Client::new()
        .post(&format!("https://{}/api/v1/statuses", instance.server))
        .bearer_auth(&token)
        .header("Idempotency-Key", hash)
        .json(&toot)
        .send()
        .await?
        .json::<Status>()
        .await?;

    // bookmark it to avoid deletion and for easy retrieval
    reqwest::Client::new()
        .post(&format!(
            "https://{}/api/v1/statuses/{}/bookmark",
            instance.server, status.id
        ))
        .bearer_auth(token)
        .send()
        .await?;

    Ok(Some(Url::parse(&status.uri)?))
}
