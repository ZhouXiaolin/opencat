//! HTTP helpers + [`EngineFetcher`] 实现：reqwest client、cache 命中、字节下载。

use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result};
use tokio::runtime::Builder;

use crate::resource::utils::cache_file_path;
use opencat_core::resource::asset_id::AssetId;

const HTTP_USER_AGENT: &str = "OpenCat/0.1 (+https://github.com/solaren/opencat)";

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

pub(crate) async fn download_bytes(client: &reqwest::Client, url: &str) -> Result<Vec<u8>> {
    let bytes = client
        .get(url)
        .send()
        .await
        .with_context(|| format!("failed to download asset from {url}"))?
        .error_for_status()
        .with_context(|| format!("download failed for {url}"))?
        .bytes()
        .await
        .with_context(|| format!("failed to read downloaded bytes from {url}"))?;
    Ok(bytes.to_vec())
}

/// Engine 端 URL → 字节下载器，内置 cache_dir 命中/写盘。
pub struct EngineFetcher {
    client: reqwest::Client,
    cache_dir: PathBuf,
}

impl EngineFetcher {
    pub fn new(cache_dir: PathBuf) -> Result<Self> {
        Ok(Self {
            client: build_http_client("failed to build async http client")?,
            cache_dir,
        })
    }

    pub fn client(&self) -> &reqwest::Client {
        &self.client
    }

    pub fn cache_dir(&self) -> &std::path::Path {
        &self.cache_dir
    }

    pub async fn fetch_bytes(&mut self, id: &AssetId, url: &str) -> Result<Vec<u8>> {
        let path = cache_file_path(&self.cache_dir, id);
        let client = self.client.clone();
        let url = url.to_string();
        if path.exists() {
            let bytes = tokio::fs::read(&path)
                .await
                .with_context(|| format!("failed to read cached asset {}", path.display()))?;
            return Ok(bytes);
        }
        let bytes = download_bytes(&client, &url).await?;
        tokio::fs::write(&path, &bytes)
            .await
            .with_context(|| format!("failed to write cache {}", path.display()))?;
        Ok(bytes)
    }
}
