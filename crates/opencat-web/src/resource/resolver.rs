//! Browser-owned resource acquisition and metadata preparation.

use std::sync::Arc;

use anyhow::{Context, Result, anyhow};
use opencat_core::parse::primitives::{AudioSource, ImageSource, OpenverseQuery, VideoSource};
use opencat_core::probe::ResourceRequests;
use opencat_core::resource::asset_id::{
    asset_id_for_audio, asset_id_for_image, asset_id_for_query, asset_id_for_video,
};
use opencat_core::resource::catalog::ResourceCatalog as _;
use opencat_core::resource::hash_map_catalog::HashMapResourceCatalog;
use opencat_core::resource::{probe_image_dims, probe_video};

use crate::resource::asset_reader;
use crate::resource::blob_store::BlobStore;
use crate::resource::fetch::fetch_bytes;

pub async fn fetch_url(url: &str) -> Result<Vec<u8>> {
    fetch_bytes(url).await
}

pub async fn preload_requests(
    requests: &ResourceRequests,
    blobs: &mut BlobStore,
    catalog: &mut HashMapResourceCatalog,
) -> Result<()> {
    for source in &requests.images {
        let Some(id) = asset_id_for_image(source) else {
            continue;
        };
        let bytes = match source {
            ImageSource::Url(url) => fetch_url(url).await?,
            ImageSource::Path(path) => asset_reader::read_path(&path.to_string_lossy()).await?,
            ImageSource::Query(query) => fetch_openverse_image(query).await?,
            ImageSource::Unset => continue,
        };
        let dims = probe_image_dims(&bytes)?;
        blobs.insert(id.clone(), Arc::from(bytes));
        catalog.register_dimensions(&id.0, dims.width, dims.height);
    }

    for source in &requests.videos {
        let id = asset_id_for_video(source);
        let bytes = match source {
            VideoSource::Url(url) => fetch_url(url).await?,
            VideoSource::Path(path) => asset_reader::read_path(&path.to_string_lossy()).await?,
        };
        let probe = probe_video(&bytes)?;
        blobs.insert(id.clone(), Arc::from(bytes));
        catalog.register_video_dimensions(
            &id.0,
            probe.width,
            probe.height,
            probe.duration_ms.map(|ms| ms as f64 / 1000.0),
        );
    }

    for source in &requests.audios {
        let Some(id) = asset_id_for_audio(source) else {
            continue;
        };
        let bytes = match source {
            AudioSource::Url(url) => fetch_url(url).await?,
            AudioSource::Path(path) => asset_reader::read_path(&path.to_string_lossy()).await?,
            AudioSource::Unset => continue,
        };
        blobs.insert(id.clone(), Arc::from(bytes));
        catalog.register_audio(&id.0);
    }

    Ok(())
}

async fn fetch_openverse_image(query: &OpenverseQuery) -> Result<Vec<u8>> {
    let search_url = build_openverse_search_url(query);
    let response = fetch_url(&search_url)
        .await
        .with_context(|| format!("failed to query Openverse for {:?}", query.query))?;
    let image_url = parse_openverse_response(&response)
        .with_context(|| format!("bad Openverse response for {:?}", query.query))?;
    let _id = asset_id_for_query(query);
    fetch_url(&image_url).await
}

fn build_openverse_search_url(query: &OpenverseQuery) -> String {
    let mut url = url::Url::parse("https://api.openverse.org/v1/images/")
        .expect("static Openverse endpoint URL is valid");
    {
        let mut params = url.query_pairs_mut();
        params.append_pair("q", &query.query);
        params.append_pair("page_size", &query.count.max(1).to_string());
        if let Some(aspect_ratio) = &query.aspect_ratio {
            params.append_pair("aspect_ratio", aspect_ratio);
        }
    }
    url.to_string()
}

fn parse_openverse_response(bytes: &[u8]) -> Result<String> {
    #[derive(serde::Deserialize)]
    struct ImageResult {
        url: Option<String>,
        thumbnail: Option<String>,
    }
    #[derive(serde::Deserialize)]
    struct SearchResponse {
        results: Vec<ImageResult>,
    }

    let response: SearchResponse = serde_json::from_slice(bytes)?;
    response
        .results
        .into_iter()
        .find_map(|result| result.url.or(result.thumbnail))
        .ok_or_else(|| anyhow!("Openverse returned no image"))
}
