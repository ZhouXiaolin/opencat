//! 兼容别名 + 远程预加载入口；纯映射逻辑迁移到 asset_catalog.rs。

use std::env;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use reqwest::header::CONTENT_TYPE;
use serde::Deserialize;
use tokio::runtime::Builder;
use tokio::task::JoinSet;

use crate::resource::asset_catalog::{
    AssetCatalog, AssetEntry,
    asset_id_for_audio_url,
    asset_id_for_query, asset_id_for_url,
    cache_file_path, read_image_dimensions,
};
use crate::scene::primitives::{AudioSource, ImageSource, OpenverseQuery};

pub use crate::resource::asset_catalog::{AssetCatalog as AssetsMap, AssetId};

const OPENVERSE_IMAGES_ENDPOINT: &str = "https://api.openverse.org/v1/images/";
const OPENVERSE_TOKEN_ENDPOINT: &str = "https://api.openverse.org/v1/auth_tokens/token/";
const OPENVERSE_CLIENT_ID_ENV: &str = "OPENVERSE_CLIENT_ID";
const OPENVERSE_CLIENT_SECRET_ENV: &str = "OPENVERSE_CLIENT_SECRET";
const HTTP_USER_AGENT: &str = "OpenCat/0.1 (+https://github.com/solaren/opencat)";

#[derive(Clone)]
struct RemoteAssetRequest {
    id: AssetId,
    source: RemoteImageSource,
}

#[derive(Clone)]
struct RemoteAudioRequest {
    id: AssetId,
    url: String,
}

#[derive(Clone)]
enum RemoteImageSource {
    Url(String),
    Query(OpenverseQuery),
}

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

pub fn preload_image_sources<I>(catalog: &mut AssetCatalog, sources: I) -> Result<()>
where
    I: IntoIterator<Item = ImageSource>,
{
    let mut remote_requests = Vec::new();

    for source in sources {
        match source {
            ImageSource::Unset => {
                return Err(anyhow!("image source is required before rendering"));
            }
            ImageSource::Path(path) => {
                catalog.register(&path);
            }
            ImageSource::Url(url) => {
                let id = asset_id_for_url(&url);
                catalog.push_missing_request(&id, &mut remote_requests, || RemoteAssetRequest {
                    id: id.clone(),
                    source: RemoteImageSource::Url(url.clone()),
                });
            }
            ImageSource::Query(query) => {
                let id = asset_id_for_query(&query);
                catalog.push_missing_request(&id, &mut remote_requests, || RemoteAssetRequest {
                    id: id.clone(),
                    source: RemoteImageSource::Query(query.clone()),
                });
            }
        }
    }

    if remote_requests.is_empty() {
        return Ok(());
    }

    catalog.ensure_cache_dir()?;
    let cache_dir = catalog.cache_dir.clone();
    let existing_token = catalog.openverse_token.clone();
    let rt = build_preload_runtime("asset")?;
    let token = rt.block_on(fetch_openverse_token(existing_token))?;
    catalog.openverse_token = token.clone();

    let prepared = rt.block_on(preload_remote_requests(cache_dir, token, remote_requests))?;

    for (id, path, width, height) in prepared {
        catalog
            .entries
            .insert(id, AssetEntry::with_dimensions(path, width, height));
    }

    Ok(())
}

pub fn preload_audio_sources<I>(catalog: &mut AssetCatalog, sources: I) -> Result<()>
where
    I: IntoIterator<Item = AudioSource>,
{
    let mut remote_requests = Vec::new();

    for source in sources {
        match source {
            AudioSource::Unset => {
                return Err(anyhow!("audio source is required before rendering"));
            }
            AudioSource::Path(path) => {
                catalog.register_audio_path(&path);
            }
            AudioSource::Url(url) => {
                let id = asset_id_for_audio_url(&url);
                catalog.push_missing_request(&id, &mut remote_requests, || RemoteAudioRequest {
                    id: id.clone(),
                    url: url.clone(),
                });
            }
        }
    }

    if remote_requests.is_empty() {
        return Ok(());
    }

    catalog.ensure_cache_dir()?;
    let cache_dir = catalog.cache_dir.clone();
    let rt = build_preload_runtime("audio")?;
    let prepared = rt.block_on(preload_remote_audio_requests(cache_dir, remote_requests))?;

    for (id, path) in prepared {
        catalog.entries.insert(id, AssetEntry::audio(&path));
    }

    Ok(())
}

fn build_preload_runtime(kind: &str) -> Result<tokio::runtime::Runtime> {
    Builder::new_multi_thread()
        .enable_all()
        .build()
        .with_context(|| format!("failed to build tokio runtime for {kind} preloading"))
}

async fn preload_remote_requests(
    cache_dir: PathBuf,
    openverse_token: Option<String>,
    requests: Vec<RemoteAssetRequest>,
) -> Result<Vec<(AssetId, PathBuf, u32, u32)>> {
    let client = build_http_client("failed to build async http client")?;

    let mut tasks = JoinSet::new();
    for request in requests {
        let client = client.clone();
        let cache_dir = cache_dir.clone();
        let token = openverse_token.clone();
        tasks.spawn(async move { prepare_remote_asset(client, cache_dir, token, request).await });
    }

    let mut prepared = Vec::new();
    while let Some(result) = tasks.join_next().await {
        prepared.push(
            result
                .context("asset preload task failed to join")?
                .context("asset preload task failed")?,
        );
    }
    Ok(prepared)
}

async fn prepare_remote_asset(
    client: reqwest::Client,
    cache_dir: PathBuf,
    openverse_token: Option<String>,
    request: RemoteAssetRequest,
) -> Result<(AssetId, PathBuf, u32, u32)> {
    let path = cache_file_path(&cache_dir, &request.id, "img");

    if !path.exists() {
        let resolved_url = match &request.source {
            RemoteImageSource::Url(url) => url.clone(),
            RemoteImageSource::Query(query) => {
                search_openverse_image(&client, openverse_token.as_deref(), query).await?
            }
        };

        download_to_cache(&client, &resolved_url, &path, "image").await?;
    }

    let (width, height) = read_image_dimensions(&path);
    Ok((request.id, path, width, height))
}

async fn preload_remote_audio_requests(
    cache_dir: PathBuf,
    requests: Vec<RemoteAudioRequest>,
) -> Result<Vec<(AssetId, PathBuf)>> {
    let client = build_http_client("failed to build async http client")?;

    let mut tasks = JoinSet::new();
    for request in requests {
        let client = client.clone();
        let cache_dir = cache_dir.clone();
        tasks.spawn(async move { prepare_remote_audio_asset(client, cache_dir, request).await });
    }

    let mut prepared = Vec::new();
    while let Some(result) = tasks.join_next().await {
        prepared.push(
            result
                .context("audio preload task failed to join")?
                .context("audio preload task failed")?,
        );
    }
    Ok(prepared)
}

async fn prepare_remote_audio_asset(
    client: reqwest::Client,
    cache_dir: PathBuf,
    request: RemoteAudioRequest,
) -> Result<(AssetId, PathBuf)> {
    let path = cache_file_path(&cache_dir, &request.id, "audio");

    if !path.exists() {
        download_to_cache(&client, &request.url, &path, "audio").await?;
    }

    Ok((request.id, path))
}

async fn search_openverse_image(
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

async fn fetch_openverse_token(existing: Option<String>) -> Result<Option<String>> {
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

fn build_http_client(context: &str) -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .user_agent(HTTP_USER_AGENT)
        .timeout(Duration::from_secs(20))
        .build()
        .with_context(|| context.to_string())
}

async fn download_to_cache(
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
