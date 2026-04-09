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
use tokio::runtime::{Builder, Runtime};
use tokio::task::JoinSet;

use crate::scene::primitives::{AudioSource, ImageSource, OpenverseQuery};

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
    preload_runtime: Option<Runtime>,
}

struct AssetEntry {
    path: PathBuf,
    width: u32,
    height: u32,
}

impl AssetEntry {
    fn image(path: &Path) -> Self {
        let (width, height) = read_image_dimensions(path);
        Self {
            path: path.to_path_buf(),
            width,
            height,
        }
    }

    fn audio(path: &Path) -> Self {
        Self {
            path: path.to_path_buf(),
            width: 0,
            height: 0,
        }
    }

    fn with_dimensions(path: PathBuf, width: u32, height: u32) -> Self {
        Self {
            path,
            width,
            height,
        }
    }
}

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
            preload_runtime: None,
        }
    }

    pub fn register(&mut self, path: &Path) -> AssetId {
        let id = AssetId(path.to_string_lossy().into_owned());
        self.insert_entry_if_missing(id, || AssetEntry::image(path))
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
                    self.push_missing_request(&id, &mut remote_requests, || RemoteAssetRequest {
                        id: id.clone(),
                        source: RemoteImageSource::Url(url),
                    });
                }
                ImageSource::Query(query) => {
                    let id = asset_id_for_query(&query);
                    self.push_missing_request(&id, &mut remote_requests, || RemoteAssetRequest {
                        id: id.clone(),
                        source: RemoteImageSource::Query(query),
                    });
                }
            }
        }

        if remote_requests.is_empty() {
            return Ok(());
        }

        self.ensure_cache_dir()?;
        let cache_dir = self.cache_dir.clone();
        let existing_token = self.openverse_token.clone();
        let token = {
            let runtime = self.preload_runtime("asset")?;
            runtime.block_on(fetch_openverse_token(existing_token))?
        };
        self.openverse_token = token.clone();

        let prepared = {
            let runtime = self.preload_runtime("asset")?;
            runtime.block_on(preload_remote_requests(cache_dir, token, remote_requests))?
        };

        for (id, path, width, height) in prepared {
            self.entries
                .insert(id, AssetEntry::with_dimensions(path, width, height));
        }

        Ok(())
    }

    pub fn preload_audio_sources<I>(&mut self, sources: I) -> Result<()>
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
                    self.register_audio_path(&path);
                }
                AudioSource::Url(url) => {
                    let id = asset_id_for_audio_url(&url);
                    self.push_missing_request(&id, &mut remote_requests, || RemoteAudioRequest {
                        id: id.clone(),
                        url,
                    });
                }
            }
        }

        if remote_requests.is_empty() {
            return Ok(());
        }

        self.ensure_cache_dir()?;
        let cache_dir = self.cache_dir.clone();
        let prepared = {
            let runtime = self.preload_runtime("audio")?;
            runtime.block_on(preload_remote_audio_requests(cache_dir, remote_requests))?
        };

        for (id, path) in prepared {
            self.entries.insert(id, AssetEntry::audio(&path));
        }

        Ok(())
    }

    pub fn register_image_source(&mut self, source: &ImageSource) -> Result<AssetId> {
        match source {
            ImageSource::Unset => Err(anyhow!("image source is required before rendering")),
            ImageSource::Path(path) => Ok(self.register(path)),
            ImageSource::Url(url) => {
                let id = asset_id_for_url(url);
                self.require_preloaded(id, || {
                    anyhow!("remote image source {url} was not preloaded before rendering")
                })
            }
            ImageSource::Query(query) => {
                let id = asset_id_for_query(query);
                self.require_preloaded(id, || {
                    anyhow!(
                        "Openverse query {:?} was not preloaded before rendering",
                        query.query
                    )
                })
            }
        }
    }

    pub fn ensure_image_source_entry_for_inspect(&mut self, source: &ImageSource) {
        match source {
            ImageSource::Unset => {}
            ImageSource::Path(path) => {
                let _ = self.register(path);
            }
            ImageSource::Url(url) => {
                let id = asset_id_for_url(url);
                if self.entries.contains_key(&id) {
                    return;
                }
                self.entries
                    .insert(id, AssetEntry::with_dimensions(PathBuf::from(url), 0, 0));
            }
            ImageSource::Query(query) => {
                let id = asset_id_for_query(query);
                if self.entries.contains_key(&id) {
                    return;
                }
                self.entries.insert(
                    id,
                    AssetEntry::with_dimensions(
                        PathBuf::from(format!("openverse://{}", query.query)),
                        0,
                        0,
                    ),
                );
            }
        }
    }

    pub fn register_audio_source(&mut self, source: &AudioSource) -> Result<AssetId> {
        match source {
            AudioSource::Unset => Err(anyhow!("audio source is required before rendering")),
            AudioSource::Path(path) => Ok(self.register_audio_path(path)),
            AudioSource::Url(url) => {
                let id = asset_id_for_audio_url(url);
                self.require_preloaded(id, || {
                    anyhow!("remote audio source {url} was not preloaded before rendering")
                })
            }
        }
    }

    pub fn register_dimensions(&mut self, path: &Path, width: u32, height: u32) -> AssetId {
        let id = AssetId(path.to_string_lossy().into_owned());
        self.insert_entry_if_missing(id, || {
            AssetEntry::with_dimensions(path.to_path_buf(), width, height)
        })
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

    fn register_audio_path(&mut self, path: &Path) -> AssetId {
        let id = asset_id_for_audio_path(path);
        self.insert_entry_if_missing(id, || AssetEntry::audio(path))
    }

    fn insert_entry_if_missing(
        &mut self,
        id: AssetId,
        build_entry: impl FnOnce() -> AssetEntry,
    ) -> AssetId {
        if self.entries.contains_key(&id) {
            return id;
        }

        self.entries.insert(id.clone(), build_entry());
        id
    }

    fn push_missing_request<T>(
        &self,
        id: &AssetId,
        requests: &mut Vec<T>,
        build_request: impl FnOnce() -> T,
    ) {
        if !self.entries.contains_key(id) {
            requests.push(build_request());
        }
    }

    fn require_preloaded(
        &self,
        id: AssetId,
        missing_error: impl FnOnce() -> anyhow::Error,
    ) -> Result<AssetId> {
        self.entries
            .contains_key(&id)
            .then_some(id)
            .ok_or_else(missing_error)
    }

    fn ensure_cache_dir(&self) -> Result<()> {
        fs::create_dir_all(&self.cache_dir).with_context(|| {
            format!(
                "failed to create asset cache dir {}",
                self.cache_dir.display()
            )
        })
    }

    fn build_preload_runtime(kind: &str) -> Result<Runtime> {
        Builder::new_multi_thread()
            .enable_all()
            .build()
            .with_context(|| format!("failed to build tokio runtime for {kind} preloading"))
    }

    fn preload_runtime(&mut self, kind: &str) -> Result<&Runtime> {
        if self.preload_runtime.is_none() {
            self.preload_runtime = Some(Self::build_preload_runtime(kind)?);
        }
        self.preload_runtime
            .as_ref()
            .ok_or_else(|| anyhow!("failed to initialize tokio runtime for {kind} preloading"))
    }
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

fn asset_id_for_url(url: &str) -> AssetId {
    AssetId(format!("url:{url}"))
}

fn asset_id_for_audio_path(path: &Path) -> AssetId {
    AssetId(format!("audio:path:{}", path.to_string_lossy()))
}

fn asset_id_for_audio_url(url: &str) -> AssetId {
    AssetId(format!("audio:url:{url}"))
}

fn asset_id_for_query(query: &OpenverseQuery) -> AssetId {
    AssetId(format!(
        "openverse:q={};count={};aspect_ratio={}",
        query.query,
        query.count,
        query.aspect_ratio.as_deref().unwrap_or("")
    ))
}

fn cache_file_path(cache_dir: &Path, id: &AssetId, extension: &str) -> PathBuf {
    cache_dir.join(format!("{:016x}.{extension}", stable_hash(&id.0)))
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
