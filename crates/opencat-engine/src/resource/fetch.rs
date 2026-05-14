//! HTTP helpers shared by [`crate::resource::resolver::EngineAssetResolver`]:
//! reqwest client construction, Openverse search/token, generic download-to-file.

use std::env;
use std::path::Path;
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use reqwest::header::CONTENT_TYPE;
use serde::Deserialize;
use tokio::runtime::Builder;

use opencat_core::scene::primitives::OpenverseQuery;

pub use crate::resource::utils::{asset_id_for_audio_path, cache_file_path, read_image_dimensions};
pub use opencat_core::resource::asset_id::AssetId;

const OPENVERSE_IMAGES_ENDPOINT: &str = "https://api.openverse.org/v1/images/";
const OPENVERSE_TOKEN_ENDPOINT: &str = "https://api.openverse.org/v1/auth_tokens/token/";
const OPENVERSE_CLIENT_ID_ENV: &str = "OPENVERSE_CLIENT_ID";
const OPENVERSE_CLIENT_SECRET_ENV: &str = "OPENVERSE_CLIENT_SECRET";
const HTTP_USER_AGENT: &str = "OpenCat/0.1 (+https://github.com/solaren/opencat)";

#[derive(Deserialize)]
struct OpenverseSearchResponse {
    results: Vec<OpenverseImageResult>,
}

#[derive(Deserialize)]
struct OpenverseImageResult {
    url: Option<String>,
    thumbnail: Option<String>,
}

#[derive(Deserialize)]
struct OpenverseTokenResponse {
    access_token: String,
}

pub(crate) fn build_preload_runtime(kind: &str) -> Result<tokio::runtime::Runtime> {
    Builder::new_multi_thread()
        .enable_all()
        .build()
        .with_context(|| format!("failed to build tokio runtime for {kind} preloading"))
}

pub(crate) fn build_http_client(context: &str) -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .user_agent(HTTP_USER_AGENT)
        .timeout(Duration::from_secs(20))
        .build()
        .with_context(|| context.to_string())
}

pub(crate) async fn download_to_cache(
    client: &reqwest::Client,
    url: &str,
    path: &Path,
    asset_kind: &str,
) -> Result<()> {
    let bytes = client
        .get(url)
        .send()
        .await
        .with_context(|| format!("failed to download {asset_kind} asset from {url}"))?
        .error_for_status()
        .with_context(|| format!("{asset_kind} download failed for {url}"))?
        .bytes()
        .await
        .with_context(|| format!("failed to read downloaded {asset_kind} bytes from {url}"))?;

    tokio::fs::write(path, &bytes)
        .await
        .with_context(|| format!("failed to write cached {asset_kind} {}", path.display()))
}

pub(crate) async fn search_openverse_image(
    client: &reqwest::Client,
    openverse_token: Option<&str>,
    query: &OpenverseQuery,
) -> Result<String> {
    let page_size = query.count.max(1).to_string();
    let mut url = reqwest::Url::parse(OPENVERSE_IMAGES_ENDPOINT)
        .context("failed to parse Openverse images endpoint")?;
    {
        let mut params = url.query_pairs_mut();
        params.append_pair("q", &query.query);
        params.append_pair("page_size", &page_size);
        if let Some(aspect_ratio) = &query.aspect_ratio {
            params.append_pair("aspect_ratio", aspect_ratio);
        }
    }

    let mut request = client.get(url);
    if let Some(token) = openverse_token {
        request = request.bearer_auth(token);
    }

    let response = request
        .send()
        .await
        .with_context(|| format!("failed to query Openverse for {:?}", query.query))?
        .error_for_status()
        .with_context(|| format!("Openverse search failed for {:?}", query.query))?;

    let payload: OpenverseSearchResponse = response
        .json()
        .await
        .with_context(|| format!("failed to decode Openverse response for {:?}", query.query))?;

    payload
        .results
        .into_iter()
        .find_map(|result| result.url.or(result.thumbnail))
        .ok_or_else(|| anyhow!("Openverse returned no image for query {:?}", query.query))
}

pub(crate) async fn fetch_openverse_token(existing: Option<String>) -> Result<Option<String>> {
    if existing.is_some() {
        return Ok(existing);
    }

    let client_id = env::var(OPENVERSE_CLIENT_ID_ENV).ok();
    let client_secret = env::var(OPENVERSE_CLIENT_SECRET_ENV).ok();

    match (client_id, client_secret) {
        (None, None) => Ok(None),
        (Some(_), None) | (None, Some(_)) => {
            bail!(
                "both {} and {} must be set to use Openverse authentication",
                OPENVERSE_CLIENT_ID_ENV,
                OPENVERSE_CLIENT_SECRET_ENV
            );
        }
        (Some(client_id), Some(client_secret)) => {
            let body = format!(
                "grant_type=client_credentials&client_id={}&client_secret={}",
                client_id, client_secret
            );
            let client =
                build_http_client("failed to build async http client for Openverse token")?;

            let token: OpenverseTokenResponse = client
                .post(OPENVERSE_TOKEN_ENDPOINT)
                .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(body)
                .send()
                .await
                .context("failed to request Openverse access token")?
                .error_for_status()
                .context("Openverse token request failed")?
                .json()
                .await
                .context("failed to decode Openverse token response")?;
            Ok(Some(token.access_token))
        }
    }
}
