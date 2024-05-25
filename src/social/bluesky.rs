use anyhow::{bail, Ok, Result};
use chrono::Utc;
use regex::Regex;
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
    created_at: String,
    langs: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    facets: Option<Vec<Facet>>,
}

impl Record {
    fn new(text: String, lang: &Lang) -> Self {
        let facets = parse_facets(&text);
        Self {
            r#type: "app.bsky.feed.post",
            created_at: format_utc_date(&Utc::now()),
            text,
            langs: vec![lang.0.clone()],
            facets: if facets.is_empty() {
                None
            } else {
                Some(facets)
            },
        }
    }
}

#[derive(Deserialize)]
struct Status {
    uri: String,
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

#[derive(Deserialize)]
struct Profile {
    handle: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Index {
    byte_start: usize,
    byte_end: usize,
}

#[derive(Serialize)]
#[serde(untagged)]
enum FeatureType {
    Link(&'static str),
    Hashtag(&'static str),
}

impl FeatureType {
    fn link() -> Self {
        FeatureType::Link("app.bsky.richtext.facet#link")
    }

    fn hashtag() -> Self {
        FeatureType::Hashtag("app.bsky.richtext.facet#tag")
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
enum FeatureData {
    Uri(Url),
    Tag(String),
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Feature {
    #[serde(rename = "$type")]
    r#type: FeatureType,
    #[serde(flatten)]
    data: FeatureData,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Facet {
    index: Index,
    features: Vec<Feature>,
}

fn parse_urls(s: &str) -> Vec<Facet> {
    // partial/naive URL regex based on: https://stackoverflow.com/a/3809435
    // tweaked to disallow some training punctuation
    let reg = Regex::new(r"[$|\W](https?:\/\/(www\.)?[-a-zA-Z0-9@:%._\+~#=]{1,256}\.[a-zA-Z0-9()]{1,6}\b([-a-zA-Z0-9()@:%_\+.~#?&//=]*[-a-zA-Z0-9@%_\+~#//=])?)").unwrap();
    reg.captures_iter(s)
        .map(|c| {
            let url_match = c.get(1).expect("Failure at capturing URL");
            let url = Url::parse(url_match.as_str())
                .expect(&format!("Failed to parse `{}`", url_match.as_str()));
            Facet {
                index: Index {
                    byte_start: url_match.start(),
                    byte_end: url_match.end(),
                },
                features: vec![Feature {
                    r#type: FeatureType::link(),
                    data: FeatureData::Uri(url),
                }],
            }
        })
        .collect()
}

fn parse_tags(s: &str) -> Vec<Facet> {
    let reg = Regex::new(r"(?:^|\s)(#[^\d\s]\S*)").unwrap();
    reg.captures_iter(s)
        .map(|c| {
            let tag_match = c.get(1).expect("Failure at capturing hashtag");
            Facet {
                index: Index {
                    byte_start: tag_match.start(),
                    byte_end: tag_match.end(),
                },
                features: vec![Feature {
                    r#type: FeatureType::hashtag(),
                    data: FeatureData::Tag(tag_match.as_str()[1..].to_owned()),
                }],
            }
        })
        .collect()
}

fn parse_facets(s: &str) -> Vec<Facet> {
    let mut facets = parse_urls(s);
    let mut tags = parse_tags(s);
    facets.append(&mut tags);
    facets
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
    let reg = Regex::new(r"at://(did:plc:.+)/app\.bsky\.feed\.post/([[:alnum:]]+)").unwrap();
    let Some(captures) = reg.captures(&status.uri) else {
        bail!("Failure on retrieving `did` and `record_key`");
    };
    let did = captures.get(1).expect("No `did` in record").as_str();
    let record_id = captures.get(2).expect("No `record_key` in record").as_str();

    let response = reqwest::Client::new()
        .get(format!(
            "https://{}/xrpc/app.bsky.actor.getProfile",
            instance.server
        ))
        .bearer_auth(&session.access_jwt)
        .query(&[("actor", did)])
        .send()
        .await?;

    if response.status() != StatusCode::OK {
        let status = response.status();
        let text = response.text().await?;
        bail!("Failed to get profile: {status}, {text}");
    }

    let profile = response.json::<Profile>().await?;

    let url = format!(
        "https://bsky.app/profile/{}/post/{record_id}",
        profile.handle
    );
    Ok(Some(Url::parse(&url)?))
}
