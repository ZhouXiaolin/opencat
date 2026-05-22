//! Platform facade trait — aggregates script, resource, media, and draw roles.
//!
//! Each platform crate (engine, web) implements `Platform`, core pipeline
//! monomorphizes via `RenderSession<P: Platform, C: Canvas2D>`.

use crate::platform::video::VideoFrameProvider;
use crate::scene::script::ScriptHost;
use super::resource::ResourcePlatform;
use super::media::MediaPlatform;
use super::draw::DrawPlatform;

/// Platform facade: each backend provides script, resource, media, and draw roles.
/// The `Video` bridge is kept for backward compatibility during migration.
pub trait Platform: 'static {
    type Script: ScriptHost;
    type Resource: ResourcePlatform;
    type Media: MediaPlatform;
    type Draw: DrawPlatform<
        PreparedFrameMedia = <Self::Media as MediaPlatform>::PreparedFrameMedia,
    >;
    /// Legacy video bridge — will be folded into Media/Draw in a future phase.
    type Video: VideoFrameProvider;

    fn script(&mut self) -> &mut Self::Script;
    fn resources(&mut self) -> &mut Self::Resource;
    fn media(&mut self) -> &mut Self::Media;
    fn draw(&mut self) -> &mut Self::Draw;
    /// Legacy bridge — equivalent to the old video_source().
    fn video_source(&mut self) -> &mut Self::Video;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame_ctx::ScriptFrameCtx;
    use crate::platform::video::{FrameBitmap, VideoFrameProvider};
    use crate::resource::asset_id::AssetId;
    use crate::resource::{AssetResolver, BlobStore, AssetSink, UrlFetcher};
    use crate::scene::script::{ScriptDriverId, ScriptTextSource};
    use crate::script::recorder::MutationRecorder;
    use crate::draw::cache::CachedDrawRange;
    use crate::draw::frame::DrawOpFrame;
    use crate::platform::draw::{RenderSessionHeader, DrawStats, DrawError};
    use crate::platform::media::{FrameMediaPlan, AudioPlanSlice, MediaPrepareMode, AudioPrepareMode, MediaError};
    use anyhow::Result;
    use std::future::Future;

    // ---- Mock script -------------------------------------------------------

    struct MockScript;
    impl ScriptHost for MockScript {
        fn install(&mut self, _: &str) -> Result<ScriptDriverId> {
            Ok(ScriptDriverId(1))
        }
        fn register_text_source(&mut self, _: &str, _: ScriptTextSource) {}
        fn clear_text_sources(&mut self) {}
        fn run_frame(
            &mut self,
            _: ScriptDriverId,
            _: &ScriptFrameCtx,
            _: Option<&str>,
            _: &mut dyn MutationRecorder,
        ) -> Result<()> {
            Ok(())
        }
    }

    // ---- Mock resource -----------------------------------------------------

    struct MockFetcher;
    impl UrlFetcher for MockFetcher {
        fn fetch_bytes(
            &mut self,
            _id: &AssetId,
            _url: &str,
        ) -> impl Future<Output = Result<Vec<u8>>> {
            async { Ok(Vec::new()) }
        }
    }

    struct MockSink;
    impl AssetSink for MockSink {
        fn store(&mut self, _id: &AssetId, _bytes: Vec<u8>) {}
    }

    struct MockResolver {
        fetcher: MockFetcher,
        sink: MockSink,
    }
    impl AssetResolver for MockResolver {
        type Fetcher = MockFetcher;
        type Sink = MockSink;

        fn parts(&mut self) -> (&mut Self::Fetcher, &mut Self::Sink) {
            (&mut self.fetcher, &mut self.sink)
        }
    }

    struct MockBlobStore;
    impl BlobStore for MockBlobStore {
        fn read(&self, _id: &AssetId) -> Option<Vec<u8>> {
            None
        }
    }

    struct MockResourcePlatform {
        resolver: MockResolver,
        blob_store: MockBlobStore,
    }
    impl ResourcePlatform for MockResourcePlatform {
        type Resolver = MockResolver;
        type BlobStore = MockBlobStore;

        fn resolver(&mut self) -> &mut Self::Resolver {
            &mut self.resolver
        }
        fn blob_store(&self) -> Option<&Self::BlobStore> {
            Some(&self.blob_store)
        }
    }

    // ---- Mock media --------------------------------------------------------

    struct MockPreparedFrameMedia;
    struct MockPreparedAudioSlice;

    struct MockMediaPlatform;
    impl MediaPlatform for MockMediaPlatform {
        type PreparedFrameMedia = MockPreparedFrameMedia;
        type PreparedAudioSlice = MockPreparedAudioSlice;

        fn prepare_frame(
            &mut self,
            _plan: &FrameMediaPlan,
            _mode: MediaPrepareMode,
        ) -> Result<Self::PreparedFrameMedia, MediaError> {
            Ok(MockPreparedFrameMedia)
        }

        fn prepare_audio_slice(
            &mut self,
            _slice: &AudioPlanSlice,
            _mode: AudioPrepareMode,
        ) -> Result<Self::PreparedAudioSlice, MediaError> {
            Ok(MockPreparedAudioSlice)
        }
    }

    // ---- Mock draw ---------------------------------------------------------

    struct MockTarget;

    struct MockDrawPlatform;
    impl DrawPlatform for MockDrawPlatform {
        type Target = MockTarget;
        type PreparedFrameMedia = MockPreparedFrameMedia;

        fn execute(
            &mut self,
            _header: &RenderSessionHeader,
            _draw: &DrawOpFrame,
            _media: &Self::PreparedFrameMedia,
            _target: &mut Self::Target,
        ) -> Result<DrawStats, DrawError> {
            Ok(DrawStats::default())
        }

        fn compile_range(
            &mut self,
            _cached: &CachedDrawRange,
            _draw: &DrawOpFrame,
        ) -> Result<(), DrawError> {
            Ok(())
        }

        fn evict_range(&mut self, _fingerprint: u64) {}
    }

    // ---- Mock video (legacy bridge) ---------------------------------------

    struct MockVideo;
    impl VideoFrameProvider for MockVideo {
        fn frame_rgba(&mut self, _: &AssetId, _: u32) -> Result<FrameBitmap> {
            Ok(FrameBitmap {
                data: std::sync::Arc::new(vec![0; 4]),
                width: 1,
                height: 1,
            })
        }
    }

    // ---- Mock platform -----------------------------------------------------

    struct MockPlatform {
        script: MockScript,
        resources: MockResourcePlatform,
        media: MockMediaPlatform,
        draw: MockDrawPlatform,
        video: MockVideo,
    }
    impl Platform for MockPlatform {
        type Script = MockScript;
        type Resource = MockResourcePlatform;
        type Media = MockMediaPlatform;
        type Draw = MockDrawPlatform;
        type Video = MockVideo;

        fn script(&mut self) -> &mut Self::Script {
            &mut self.script
        }
        fn resources(&mut self) -> &mut Self::Resource {
            &mut self.resources
        }
        fn media(&mut self) -> &mut Self::Media {
            &mut self.media
        }
        fn draw(&mut self) -> &mut Self::Draw {
            &mut self.draw
        }
        fn video_source(&mut self) -> &mut Self::Video {
            &mut self.video
        }
    }

    #[test]
    fn platform_associated_types_resolve() {
        let mut p = MockPlatform {
            script: MockScript,
            resources: MockResourcePlatform {
                resolver: MockResolver {
                    fetcher: MockFetcher,
                    sink: MockSink,
                },
                blob_store: MockBlobStore,
            },
            media: MockMediaPlatform,
            draw: MockDrawPlatform,
            video: MockVideo,
        };
        let _script: &mut MockScript = p.script();
        let _resources: &mut MockResourcePlatform = p.resources();
        let _media: &mut MockMediaPlatform = p.media();
        let _draw: &mut MockDrawPlatform = p.draw();
        let _video: &mut MockVideo = p.video_source();
    }
}
