use anyhow::{bail, Ok, Result};
use chrono::Utc;
use reqwest::{StatusCode, Url};
use serde_derive::{Deserialize, Serialize};
use tracing::{debug, info};

use crate::{config::SocialInstance, format_utc_date};

use super::{Lang, StatusContent};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Session {
    access_jwt: String,
    did: String,
}

#[derive(Serialize)]
struct Credentials {
    identifier: String,
    password: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Record {
    r#type: &'static str,
    text: String,
    create_at: String,
    langs: Vec<String>,
}

impl Record {
    fn new(text: String, lang: &Lang) -> Self {
        Self {
            r#type: "app.bsky",
            create_at: format_utc_date(&Utc::now()),
            text,
            langs: vec![lang.0.clone()],
        }
    }
}

#[derive(Deserialize)]
struct Status {
    uri: Url,
}

#[derive(Serialize)]
struct RecordCreation<'a> {
    repo: &'a str,
    collection: &'static str,
    record: Record,
}

impl<'a> RecordCreation<'a> {
    fn new(session: &'a Session, text: String, lang: &Lang) -> Self {
        Self {
            repo: &session.did,
            collection: "app.bsky.feed.post",
            record: Record::new(text, lang),
        }
    }
}

async fn login(instance: &SocialInstance) -> Result<Session> {
    debug!("Login in {}", instance.server);
    let Some(password) = std::env::var(&instance.token_var).ok() else {
        bail!("`{}` env var is not defined", instance.token_var);
    };

    let identifier = match &instance.handle_var {
        Some(var) => {
            let Some(identifier) = std::env::var(var).ok() else {
                bail!("`{}` env var is not defined", var);
            };
            identifier
        }
        None => bail!("Missing `handle_var` in Bluesky definition"),
    };

    let response = reqwest::Client::new()
        .post(&format!(
            "https://{}/xrpc/com.atproto.server.createSession",
            instance.server
        ))
        .json(&Credentials {
            identifier,
            password,
        })
        .send()
        .await?;

    if response.status() != StatusCode::OK {
        let status = response.status();
        let text = response.text().await?;
        bail!("Failed to login: {status}, {text}");
    }

    let session = response.json::<Session>().await?;
    dbg!(&session.access_jwt);
    Ok(session)
}

pub async fn push_to_bsky(
    instance: &SocialInstance,
    status: &StatusContent,
    lang: &Lang,
) -> Result<Option<Url>> {
    info!("Pushing to Bluesky");
    let session = login(instance).await?;

    let record = RecordCreation::new(&session, status.0.clone(), lang);

    dbg!(serde_json::to_string(&record).unwrap());

    let response = reqwest::Client::new()
        .post(&format!(
            "https://{}/xrpc/com.atproto.repo.createRecord",
            instance.server
        ))
        .bearer_auth(&session.access_jwt)
        .json(&record)
        .send()
        .await?;

    if response.status() != StatusCode::OK {
        let status = response.status();
        let text = response.text().await?;
        bail!("Failed to post: {status}, {text}");
    }

    let status = response.json::<Status>().await?;
    Ok(Some(status.uri))
}
