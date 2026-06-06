pub mod asset_id;
pub mod bitmap_source;
pub mod blob_store;
pub mod catalog;
pub mod fonts;
pub mod hash_map_catalog;
pub mod host_bridge;
pub mod lottie;
pub mod manifest;
pub mod materialize;
pub mod path_store;
pub mod preload;
pub mod preload_lottie;
pub mod probe;
pub mod protocol;
pub mod resolver;
pub mod types;

pub use crate::ir::asset_id::*;
pub use crate::probe::bitmap_source::*;
pub use blob_store::{AssetPathBlobStore, BlobStore};
pub use catalog::ResourceCatalog;
pub use fonts::{
    FontFaceDecl, FontFamilyIndex, FontManifest, FontRole, FontSource, fetch_manifest_bytes,
    font_asset_id, load_faces_into_db, merge_faces_into_db, resolve_font_source_path,
};
pub use hash_map_catalog::{HashMapResourceCatalog, ResourceKind, ResourceMeta};
pub use host_bridge::provider_from_manifest;
pub use lottie::{LottieMeta, parse_lottie_meta, resolve_lottie_frame, scan_lottie_dependencies};
pub use manifest::{
    BundleDependencySource, BundleDependencySpec, ExternalResourceEntry, ExternalResourceKind,
    ExternalResourceManifest, LottieBundleSpec, LottiePrimarySource, ProviderBinding,
    build_manifest,
};
pub use materialize::{
    ByteSource, CoreBlobStoreSource, bundle_primary_json, hydrate_provider_from_bytes,
    map_bundle_dep_to_flat_lookup, skottie_assets_for_bundle,
};
pub use path_store::AssetPathStore;
pub use preload::preload_all;
pub use probe::{ImageDims, VideoProbe, probe_image_dims, probe_video};
pub use protocol::{
    ByteStore, IndexedResourceProvider, MapResourceProvider, ResourceLookup, ResourceProvider,
    TypefaceRequest,
};
pub use resolver::{AssetResolver, AssetSink, AudioMeta, ImageMeta, UrlFetcher, VideoMeta};
pub use types::{VideoFrameRequest, VideoFrameTiming, VideoPreviewQuality};
