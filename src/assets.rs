use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::env;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use reqwest::header::CONTENT_TYPE;
use serde::Deserialize;
use tokio::runtime::Builder;
use tokio::task::JoinSet;

use crate::nodes::{ImageSource, OpenverseQuery};

const OPENVERSE_IMAGES_ENDPOINT: &str = "https://api.openverse.org/v1/images/";
const OPENVERSE_TOKEN_ENDPOINT: &str = "https://api.openverse.org/v1/auth_tokens/token/";
const OPENVERSE_CLIENT_ID_ENV: &str = "OPENVERSE_CLIENT_ID";
const OPENVERSE_CLIENT_SECRET_ENV: &str = "OPENVERSE_CLIENT_SECRET";
const HTTP_USER_AGENT: &str = "OpenCat/0.1 (+https://github.com/solaren/opencat)";

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct AssetId(pub String);

pub struct AssetsMap {
    entries: HashMap<AssetId, AssetEntry>,
    cache_dir: PathBuf,
    openverse_token: Option<String>,
}

struct AssetEntry {
    path: PathBuf,
    width: u32,
    height: u32,
}

#[derive(Clone)]
struct RemoteAssetRequest {
    id: AssetId,
    source: RemoteImageSource,
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

impl AssetsMap {
    pub fn new() -> Self {
        let cache_dir = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".opencat")
            .join("assets");
        Self {
            entries: HashMap::new(),
            cache_dir,
            openverse_token: None,
        }
    }

    pub fn register(&mut self, path: &Path) -> AssetId {
        let id = AssetId(path.to_string_lossy().into_owned());
        if self.entries.contains_key(&id) {
            return id;
        }

        let (width, height) = read_image_dimensions(path);
        self.entries.insert(
            id.clone(),
            AssetEntry {
                path: path.to_path_buf(),
                width,
                height,
            },
        );
        id
    }

    pub fn preload_image_sources<I>(&mut self, sources: I) -> Result<()>
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
                    self.register(&path);
                }
                ImageSource::Url(url) => {
                    let id = asset_id_for_url(&url);
                    if !self.entries.contains_key(&id) {
                        remote_requests.push(RemoteAssetRequest {
                            id,
                            source: RemoteImageSource::Url(url),
                        });
                    }
                }
                ImageSource::Query(query) => {
                    let id = asset_id_for_query(&query);
                    if !self.entries.contains_key(&id) {
                        remote_requests.push(RemoteAssetRequest {
                            id,
                            source: RemoteImageSource::Query(query),
                        });
                    }
                }
            }
        }

        if remote_requests.is_empty() {
            return Ok(());
        }

        fs::create_dir_all(&self.cache_dir).with_context(|| {
            format!(
                "failed to create asset cache dir {}",
                self.cache_dir.display()
            )
        })?;

        let runtime = Builder::new_multi_thread()
            .enable_all()
            .build()
            .context("failed to build tokio runtime for asset preloading")?;

        let token = runtime.block_on(fetch_openverse_token(self.openverse_token.clone()))?;
        self.openverse_token = token.clone();

        let prepared = runtime.block_on(preload_remote_requests(
            self.cache_dir.clone(),
            token,
            remote_requests,
        ))?;

        for (id, path, width, height) in prepared {
            self.entries.insert(
                id,
                AssetEntry {
                    path,
                    width,
                    height,
                },
            );
        }

        Ok(())
    }

    pub fn register_image_source(&mut self, source: &ImageSource) -> Result<AssetId> {
        match source {
            ImageSource::Unset => Err(anyhow!("image source is required before rendering")),
            ImageSource::Path(path) => Ok(self.register(path)),
            ImageSource::Url(url) => {
                let id = asset_id_for_url(url);
                self.entries.get(&id).map(|_| id).ok_or_else(|| {
                    anyhow!("remote image source {url} was not preloaded before rendering")
                })
            }
            ImageSource::Query(query) => {
                let id = asset_id_for_query(query);
                self.entries.get(&id).map(|_| id).ok_or_else(|| {
                    anyhow!(
                        "Openverse query {:?} was not preloaded before rendering",
                        query.query
                    )
                })
            }
        }
    }

    pub fn register_dimensions(&mut self, path: &Path, width: u32, height: u32) -> AssetId {
        let id = AssetId(path.to_string_lossy().into_owned());
        if self.entries.contains_key(&id) {
            return id;
        }

        self.entries.insert(
            id.clone(),
            AssetEntry {
                path: path.to_path_buf(),
                width,
                height,
            },
        );
        id
    }

    pub fn alias(&mut self, alias: AssetId, target: &AssetId) -> Result<()> {
        if self.entries.contains_key(&alias) {
            return Ok(());
        }

        let entry = self
            .entries
            .get(target)
            .ok_or_else(|| anyhow!("cannot alias missing asset {}", target.0))?;
        self.entries.insert(
            alias,
            AssetEntry {
                path: entry.path.clone(),
                width: entry.width,
                height: entry.height,
            },
        );
        Ok(())
    }

    pub fn dimensions(&self, id: &AssetId) -> (u32, u32) {
        self.entries
            .get(id)
            .map(|e| (e.width, e.height))
            .unwrap_or((0, 0))
    }

    pub fn path(&self, id: &AssetId) -> Option<&Path> {
        self.entries.get(id).map(|e| e.path.as_path())
    }
}

async fn preload_remote_requests(
    cache_dir: PathBuf,
    openverse_token: Option<String>,
    requests: Vec<RemoteAssetRequest>,
) -> Result<Vec<(AssetId, PathBuf, u32, u32)>> {
    let client = reqwest::Client::builder()
        .user_agent(HTTP_USER_AGENT)
        .timeout(Duration::from_secs(20))
        .build()
        .context("failed to build async http client")?;

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
    let path = cache_file_path(&cache_dir, &request.id);

    if !path.exists() {
        let resolved_url = match &request.source {
            RemoteImageSource::Url(url) => url.clone(),
            RemoteImageSource::Query(query) => {
                search_openverse_image(&client, openverse_token.as_deref(), query).await?
            }
        };

        let bytes = client
            .get(&resolved_url)
            .send()
            .await
            .with_context(|| format!("failed to download image asset from {resolved_url}"))?
            .error_for_status()
            .with_context(|| format!("image download failed for {resolved_url}"))?
            .bytes()
            .await
            .with_context(|| {
                format!("failed to read downloaded image bytes from {resolved_url}")
            })?;

        tokio::fs::write(&path, &bytes)
            .await
            .with_context(|| format!("failed to write cached image {}", path.display()))?;
    }

    let (width, height) = read_image_dimensions(&path);
    Ok((request.id, path, width, height))
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
            let client = reqwest::Client::builder()
                .user_agent(HTTP_USER_AGENT)
                .timeout(Duration::from_secs(20))
                .build()
                .context("failed to build async http client for Openverse token")?;

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

fn asset_id_for_url(url: &str) -> AssetId {
    AssetId(format!("url:{url}"))
}

fn asset_id_for_query(query: &OpenverseQuery) -> AssetId {
    AssetId(format!(
        "openverse:q={};count={};aspect_ratio={}",
        query.query,
        query.count,
        query.aspect_ratio.as_deref().unwrap_or("")
    ))
}

fn cache_file_path(cache_dir: &Path, id: &AssetId) -> PathBuf {
    cache_dir.join(format!("{:016x}.img", stable_hash(&id.0)))
}

fn read_image_dimensions(path: &Path) -> (u32, u32) {
    let Ok(bytes) = fs::read(path) else {
        return (0, 0);
    };
    let Ok(image) = image::load_from_memory(&bytes) else {
        return (0, 0);
    };
    (image.width(), image.height())
}

fn stable_hash(value: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}
