//! `preload_all` — core 编排：把 [`ResourceRequests`] 里的每个 source 变体
//! 分发给 [`AssetResolver`] 的对应方法，结果灌进 [`ResourceCatalog`]。
//!
//! v1：纯串行 `await`，不做并发。各 platform 想加并发请在 trait 实现内部
//! override 或后续加 batch 方法。
//!
//! 内容字节/路径的存储是各 platform 实现的内部副作用，core 不感知。

use anyhow::Result;

use crate::resource::catalog::ResourceCatalog;
use crate::resource::resolver::AssetResolver;
use crate::runtime::preflight_collect::ResourceRequests;
use crate::scene::primitives::{AudioSource, ImageSource};

pub async fn preload_all<R: AssetResolver, C: ResourceCatalog>(
    requests: ResourceRequests,
    resolver: &mut R,
    catalog: &mut C,
) -> Result<()> {
    for src in requests.image_sources {
        let meta = match src {
            ImageSource::Unset => continue,
            ImageSource::Url(url) => resolver.resolve_image_url(&url).await?,
            ImageSource::Path(path) => resolver.resolve_image_path(&path).await?,
            ImageSource::Query(query) => resolver.resolve_image_query(&query).await?,
        };
        catalog.register_dimensions(&meta.id.0, meta.width, meta.height);
    }

    for src in requests.audio_sources {
        match src {
            AudioSource::Unset => continue,
            AudioSource::Url(url) => {
                let meta = resolver.resolve_audio_url(&url).await?;
                catalog.register_audio(&meta.id.0);
            }
            AudioSource::Path(path) => {
                let meta = resolver.resolve_audio_path(&path).await?;
                catalog.register_audio(&meta.id.0);
            }
        }
    }

    for url in requests.video_urls {
        let meta = resolver.resolve_video_url(&url).await?;
        catalog.register_video_dimensions(&meta.id.0, meta.width, meta.height, meta.duration_secs);
    }

    for path in requests.video_paths {
        let meta = resolver.resolve_video_path(&path).await?;
        catalog.register_video_dimensions(&meta.id.0, meta.width, meta.height, meta.duration_secs);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::future::Future;
    use std::path::{Path, PathBuf};

    use anyhow::Result;

    use super::*;
    use crate::resource::asset_id::{
        AssetId, asset_id_for_audio_url, asset_id_for_url, asset_id_for_video_url,
    };
    use crate::resource::hash_map_catalog::HashMapResourceCatalog;
    use crate::resource::resolver::{AssetSink, AudioMeta, ImageMeta, UrlFetcher, VideoMeta};
    use crate::scene::primitives::OpenverseQuery;

    /// 测试用空实现，永远不被实际调用（MockResolver override 了所有方法）。
    #[derive(Default)]
    struct NullFetcher;
    impl UrlFetcher for NullFetcher {
        fn fetch_bytes(
            &mut self,
            _id: &AssetId,
            _url: &str,
        ) -> impl Future<Output = Result<Vec<u8>>> {
            async { anyhow::bail!("NullFetcher unused") }
        }
    }
    #[derive(Default)]
    struct NullSink;
    impl AssetSink for NullSink {
        fn store(&mut self, _id: &AssetId, _bytes: Vec<u8>) {}
    }

    /// 记录每次方法调用的 mock，断言「编排路由对了」。
    #[derive(Default)]
    struct MockResolver {
        image_urls: Vec<String>,
        image_paths: Vec<PathBuf>,
        image_queries: Vec<OpenverseQuery>,
        audio_urls: Vec<String>,
        audio_paths: Vec<PathBuf>,
        video_urls: Vec<String>,
        video_paths: Vec<PathBuf>,
        null_fetcher: NullFetcher,
        null_sink: NullSink,
    }

    impl AssetResolver for MockResolver {
        type Fetcher = NullFetcher;
        type Sink = NullSink;

        fn parts(&mut self) -> (&mut NullFetcher, &mut NullSink) {
            (&mut self.null_fetcher, &mut self.null_sink)
        }

        fn resolve_image_url(&mut self, url: &str) -> impl Future<Output = Result<ImageMeta>> {
            self.image_urls.push(url.to_string());
            let id = asset_id_for_url(url);
            async move {
                Ok(ImageMeta {
                    id,
                    width: 100,
                    height: 50,
                })
            }
        }
        fn resolve_image_path(&mut self, path: &Path) -> impl Future<Output = Result<ImageMeta>> {
            self.image_paths.push(path.to_path_buf());
            let id = AssetId(path.to_string_lossy().into_owned());
            async move {
                Ok(ImageMeta {
                    id,
                    width: 200,
                    height: 80,
                })
            }
        }
        fn resolve_image_query(
            &mut self,
            query: &OpenverseQuery,
        ) -> impl Future<Output = Result<ImageMeta>> {
            self.image_queries.push(query.clone());
            let id = crate::resource::asset_id::asset_id_for_query(query);
            async move {
                Ok(ImageMeta {
                    id,
                    width: 320,
                    height: 240,
                })
            }
        }
        fn resolve_audio_url(&mut self, url: &str) -> impl Future<Output = Result<AudioMeta>> {
            self.audio_urls.push(url.to_string());
            let id = asset_id_for_audio_url(url);
            async move { Ok(AudioMeta { id }) }
        }
        fn resolve_audio_path(&mut self, path: &Path) -> impl Future<Output = Result<AudioMeta>> {
            self.audio_paths.push(path.to_path_buf());
            let id = AssetId(path.to_string_lossy().into_owned());
            async move { Ok(AudioMeta { id }) }
        }
        fn resolve_video_url(&mut self, url: &str) -> impl Future<Output = Result<VideoMeta>> {
            self.video_urls.push(url.to_string());
            let id = asset_id_for_video_url(url);
            async move {
                Ok(VideoMeta {
                    id,
                    width: 1920,
                    height: 1080,
                    duration_secs: Some(12.5),
                })
            }
        }
        fn resolve_video_path(&mut self, path: &Path) -> impl Future<Output = Result<VideoMeta>> {
            self.video_paths.push(path.to_path_buf());
            let id = AssetId(path.to_string_lossy().into_owned());
            async move {
                Ok(VideoMeta {
                    id,
                    width: 640,
                    height: 360,
                    duration_secs: None,
                })
            }
        }
    }

    fn block_on<F: Future>(f: F) -> F::Output {
        // 不引入 tokio 依赖，用一个简陋的同步阻塞器；测试场景下 mock 不会真挂起。
        futures_lite_block_on(f)
    }

    /// 极简的 block_on：只能跑「立即就绪」的 future（mock 都满足）。
    fn futures_lite_block_on<F: Future>(mut f: F) -> F::Output {
        use std::pin::Pin;
        use std::task::{Context, Poll, Wake, Waker};

        struct NoopWake;
        impl Wake for NoopWake {
            fn wake(self: std::sync::Arc<Self>) {}
        }

        let waker: Waker = std::sync::Arc::new(NoopWake).into();
        let mut cx = Context::from_waker(&waker);
        // SAFETY: `f` lives on this stack frame for the duration of the poll loop.
        let mut pinned = unsafe { Pin::new_unchecked(&mut f) };
        loop {
            match pinned.as_mut().poll(&mut cx) {
                Poll::Ready(out) => return out,
                Poll::Pending => panic!("mock future yielded Pending; not supported in tests"),
            }
        }
    }

    #[test]
    fn routes_image_url_and_registers_dims() {
        let mut requests = ResourceRequests::default();
        requests
            .image_sources
            .insert(ImageSource::Url("https://example.com/a.png".to_string()));
        let mut resolver = MockResolver::default();
        let mut catalog = HashMapResourceCatalog::from_json("{}").unwrap();

        block_on(preload_all(requests, &mut resolver, &mut catalog)).unwrap();

        assert_eq!(resolver.image_urls, vec!["https://example.com/a.png"]);
        let id = asset_id_for_url("https://example.com/a.png");
        assert_eq!(catalog.dimensions(&id), (100, 50));
    }

    #[test]
    fn routes_video_url_with_duration() {
        let mut requests = ResourceRequests::default();
        requests
            .video_urls
            .insert("https://example.com/c.mp4".to_string());
        let mut resolver = MockResolver::default();
        let mut catalog = HashMapResourceCatalog::from_json("{}").unwrap();

        block_on(preload_all(requests, &mut resolver, &mut catalog)).unwrap();

        let id = asset_id_for_video_url("https://example.com/c.mp4");
        let info = catalog.video_info(&id).expect("video info registered");
        assert_eq!(info.width, 1920);
        assert_eq!(info.height, 1080);
        assert_eq!(info.duration_secs, Some(12.5));
    }

    #[test]
    fn skips_unset_image_source() {
        let mut requests = ResourceRequests::default();
        requests.image_sources.insert(ImageSource::Unset);
        let mut resolver = MockResolver::default();
        let mut catalog = HashMapResourceCatalog::from_json("{}").unwrap();

        block_on(preload_all(requests, &mut resolver, &mut catalog)).unwrap();

        assert!(resolver.image_urls.is_empty());
        assert!(resolver.image_paths.is_empty());
    }

    #[test]
    fn routes_all_source_kinds() {
        let mut requests = ResourceRequests::default();
        requests
            .image_sources
            .insert(ImageSource::Url("https://example.com/img.png".into()));
        requests
            .image_sources
            .insert(ImageSource::Path(PathBuf::from("/tmp/local.png")));
        requests
            .audio_sources
            .insert(AudioSource::Url("https://example.com/a.mp3".into()));
        requests
            .video_urls
            .insert("https://example.com/v.mp4".into());
        requests.video_paths.insert(PathBuf::from("/tmp/local.mp4"));

        let mut resolver = MockResolver::default();
        let mut catalog = HashMapResourceCatalog::from_json("{}").unwrap();

        block_on(preload_all(requests, &mut resolver, &mut catalog)).unwrap();

        assert_eq!(resolver.image_urls.len(), 1);
        assert_eq!(resolver.image_paths.len(), 1);
        assert_eq!(resolver.audio_urls.len(), 1);
        assert_eq!(resolver.video_urls.len(), 1);
        assert_eq!(resolver.video_paths.len(), 1);

        // dims for both image variants registered
        assert_eq!(
            catalog.dimensions(&asset_id_for_url("https://example.com/img.png")),
            (100, 50)
        );
        assert_eq!(
            catalog.dimensions(&AssetId("/tmp/local.png".into())),
            (200, 80)
        );

        // video dims registered for both variants
        assert_eq!(
            catalog
                .video_info(&asset_id_for_video_url("https://example.com/v.mp4"))
                .unwrap()
                .width,
            1920
        );
        assert_eq!(
            catalog
                .video_info(&AssetId("/tmp/local.mp4".into()))
                .unwrap()
                .width,
            640
        );
    }

    #[test]
    fn empty_requests_is_noop() {
        let mut resolver = MockResolver::default();
        let mut catalog = HashMapResourceCatalog::from_json("{}").unwrap();
        block_on(preload_all(
            ResourceRequests::default(),
            &mut resolver,
            &mut catalog,
        ))
        .unwrap();
        assert!(resolver.image_urls.is_empty());
    }

    // 抑制未使用警告：上面的 `HashSet` use 是为构造 ResourceRequests 用的字面量推断。
    #[allow(dead_code)]
    fn _suppress_hashset_unused(_: HashSet<ImageSource>) {}
}
