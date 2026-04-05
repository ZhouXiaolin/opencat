use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::env;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use reqwest::blocking::Client;
use reqwest::header::CONTENT_TYPE;
use serde::Deserialize;

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
    http: Client,
    openverse_token: Option<String>,
}

struct AssetEntry {
    path: PathBuf,
    width: u32,
    height: u32,
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
        let http = Client::builder()
            .user_agent(HTTP_USER_AGENT)
            .timeout(Duration::from_secs(20))
            .build()
            .expect("http client should build");
        Self {
            entries: HashMap::new(),
            cache_dir,
            http,
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

    pub fn register_image_source(&mut self, source: &ImageSource) -> Result<AssetId> {
        match source {
            ImageSource::Unset => Err(anyhow!("image source is required before rendering")),
            ImageSource::Path(path) => Ok(self.register(path)),
            ImageSource::Url(url) => self.register_url(url),
            ImageSource::Query(query) => self.register_openverse_query(query),
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

    pub fn dimensions(&self, id: &AssetId) -> (u32, u32) {
        self.entries
            .get(id)
            .map(|e| (e.width, e.height))
            .unwrap_or((0, 0))
    }

    pub fn path(&self, id: &AssetId) -> Option<&Path> {
        self.entries.get(id).map(|e| e.path.as_path())
    }

    fn register_url(&mut self, url: &str) -> Result<AssetId> {
        let id = AssetId(format!("url:{url}"));
        if self.entries.contains_key(&id) {
            return Ok(id);
        }

        let path = self.download_remote_image(&id, url)?;
        let (width, height) = read_image_dimensions(&path);
        self.entries
            .insert(id.clone(), AssetEntry { path, width, height });
        Ok(id)
    }

    fn register_openverse_query(&mut self, query: &OpenverseQuery) -> Result<AssetId> {
        let id = AssetId(format!(
            "openverse:q={};count={};aspect_ratio={}",
            query.query,
            query.count,
            query.aspect_ratio.as_deref().unwrap_or("")
        ));
        if self.entries.contains_key(&id) {
            return Ok(id);
        }

        let url = self.search_openverse_image(query)?;
        let path = self.download_remote_image(&id, &url)?;
        let (width, height) = read_image_dimensions(&path);
        self.entries
            .insert(id.clone(), AssetEntry { path, width, height });
        Ok(id)
    }

    fn search_openverse_image(&mut self, query: &OpenverseQuery) -> Result<String> {
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

        let mut request = self.http.get(url);

        if let Some(token) = self.openverse_token()?.map(str::to_owned) {
            request = request.bearer_auth(token);
        }

        let response = request
            .send()
            .with_context(|| format!("failed to query Openverse for {:?}", query.query))?
            .error_for_status()
            .with_context(|| format!("Openverse search failed for {:?}", query.query))?;

        let payload: OpenverseSearchResponse = response
            .json()
            .with_context(|| format!("failed to decode Openverse response for {:?}", query.query))?;

        payload
            .results
            .into_iter()
            .find_map(|result| result.url.or(result.thumbnail))
            .ok_or_else(|| anyhow!("Openverse returned no image for query {:?}", query.query))
    }

    fn download_remote_image(&self, id: &AssetId, url: &str) -> Result<PathBuf> {
        fs::create_dir_all(&self.cache_dir)
            .with_context(|| format!("failed to create asset cache dir {}", self.cache_dir.display()))?;

        let path = self.cache_file_path(id);
        if path.exists() {
            return Ok(path);
        }

        let bytes = self.http
            .get(url)
            .send()
            .with_context(|| format!("failed to download image asset from {url}"))?
            .error_for_status()
            .with_context(|| format!("image download failed for {url}"))?
            .bytes()
            .with_context(|| format!("failed to read downloaded image bytes from {url}"))?;

        fs::write(&path, &bytes)
            .with_context(|| format!("failed to write cached image {}", path.display()))?;
        Ok(path)
    }

    fn cache_file_path(&self, id: &AssetId) -> PathBuf {
        self.cache_dir.join(format!("{:016x}.img", stable_hash(&id.0)))
    }

    fn openverse_token(&mut self) -> Result<Option<&str>> {
        if self.openverse_token.is_none() {
            let client_id = env::var(OPENVERSE_CLIENT_ID_ENV).ok();
            let client_secret = env::var(OPENVERSE_CLIENT_SECRET_ENV).ok();

            match (client_id, client_secret) {
                (None, None) => {}
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
                    let token: OpenverseTokenResponse = self
                        .http
                        .post(OPENVERSE_TOKEN_ENDPOINT)
                        .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
                        .body(body)
                        .send()
                        .context("failed to request Openverse access token")?
                        .error_for_status()
                        .context("Openverse token request failed")?
                        .json()
                        .context("failed to decode Openverse token response")?;
                    self.openverse_token = Some(token.access_token);
                }
            }
        }

        Ok(self.openverse_token.as_deref())
    }
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
