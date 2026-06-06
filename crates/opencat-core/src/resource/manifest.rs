//! Unified external resource manifest — OpenCat sources + Lottie bundles.
//!
//! [`ExternalResourceManifest`] is the single preflight output. Hosts materialize it into
//! a [`super::protocol::MapResourceProvider`] (or [`IndexedResourceProvider`]) for Skottie
//! and for existing image/video pipelines.

use std::collections::HashMap;

use crate::ir::asset_id::{
    AssetId, asset_id_for_audio_url, asset_id_for_query, asset_id_for_url, asset_id_for_video_url,
};
use crate::parse::primitives::LottieSource;
use crate::parse::primitives::{AudioSource, ImageSource, SubtitleSource, VideoSource};
use crate::probe::catalog::{LottieRequest, ResourceRequests};
use crate::resource::fonts::{FontFaceDecl, FontManifest, FontSource, font_asset_id};
use crate::resource::lottie::scan_lottie_dependencies;
use crate::resource::protocol::{ResourceLookup, TypefaceRequest};

/// Kind of external resource (catalog + loading policy).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExternalResourceKind {
    RasterImage,
    Video,
    Audio,
    Subtitle,
    Font,
    /// Primary JSON + named dependencies (Skottie bundle).
    LottieBundle,
    /// Single file inside a [`LottieBundleSpec`].
    BundleDependency,
}

/// How this entry is addressed through [`super::protocol::ResourceProvider`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderBinding {
    /// `load("opencat", asset_id)`
    Flat { asset_id: AssetId },
    /// Primary animation JSON; deps use [`ProviderBinding::BundleDependency`].
    BundleRoot { bundle_id: AssetId },
    /// `load(bundle_id, file_name)`
    BundleDependency {
        bundle_id: AssetId,
        file_name: String,
    },
    /// `load_typeface(name, url)`
    Typeface {
        name: String,
        url: String,
        asset_id: AssetId,
    },
}

/// One externally loaded asset.
#[derive(Debug, Clone)]
pub struct ExternalResourceEntry {
    pub kind: ExternalResourceKind,
    pub binding: ProviderBinding,
    /// Original declarative source (for fetchers); `None` for derived bundle deps.
    pub source_label: Option<String>,
}

/// Lottie bundle: primary JSON + discovered or declared dependencies.
#[derive(Debug, Clone)]
pub struct LottieBundleSpec {
    pub bundle_id: AssetId,
    pub primary: LottiePrimarySource,
    pub dependencies: Vec<BundleDependencySpec>,
}

#[derive(Debug, Clone)]
pub enum LottiePrimarySource {
    Path(std::path::PathBuf),
    Url(String),
    /// Already parsed JSON string (inline or fetched).
    InlineJson(String),
}

#[derive(Debug, Clone)]
pub struct BundleDependencySpec {
    pub file_name: String,
    pub source: BundleDependencySource,
}

#[derive(Debug, Clone)]
pub enum BundleDependencySource {
    /// Resolved later via OpenCat flat asset (image URL/path).
    OpenCatImage(ImageSource),
    /// Bytes must already live under bundle provider path.
    InlineDataUri(String),
    /// Host should fetch URL and register as bundle dep.
    Url(String),
}

/// Complete manifest for one composition document.
#[derive(Debug, Clone, Default)]
pub struct ExternalResourceManifest {
    pub entries: Vec<ExternalResourceEntry>,
    pub bundles: Vec<LottieBundleSpec>,
    lookup_index: HashMap<ResourceLookup, AssetId>,
}

impl ExternalResourceManifest {
    pub fn provider_lookup_index(&self) -> &HashMap<ResourceLookup, AssetId> {
        &self.lookup_index
    }

    pub fn push_flat(
        &mut self,
        kind: ExternalResourceKind,
        asset_id: AssetId,
        source_label: Option<String>,
    ) {
        let binding = ProviderBinding::Flat {
            asset_id: asset_id.clone(),
        };
        self.lookup_index
            .insert(ResourceLookup::opencat_flat(&asset_id), asset_id.clone());
        self.entries.push(ExternalResourceEntry {
            kind,
            binding,
            source_label,
        });
    }

    pub fn push_typeface(&mut self, face: &FontFaceDecl) {
        let url = match &face.source {
            FontSource::Url(u) => u.clone(),
            FontSource::Path(p) => p.to_string_lossy().into_owned(),
        };
        let asset_id = AssetId(font_asset_id(&face.source));
        let name = face.family.clone().unwrap_or_else(|| face.id.clone());
        self.entries.push(ExternalResourceEntry {
            kind: ExternalResourceKind::Font,
            binding: ProviderBinding::Typeface {
                name: name.clone(),
                url: url.clone(),
                asset_id,
            },
            source_label: Some(format!("font:{}", face.id)),
        });
    }

    /// Register a Lottie bundle and its dependency entries.
    pub fn push_lottie_bundle(&mut self, spec: LottieBundleSpec) {
        let bundle_id = spec.bundle_id.clone();
        self.lookup_index
            .insert(ResourceLookup::opencat_flat(&bundle_id), bundle_id.clone());
        self.entries.push(ExternalResourceEntry {
            kind: ExternalResourceKind::LottieBundle,
            binding: ProviderBinding::BundleRoot {
                bundle_id: bundle_id.clone(),
            },
            source_label: Some(format!("lottie:{}", bundle_id.0)),
        });

        for dep in &spec.dependencies {
            let dep_id = AssetId(format!("{}:dep:{}", bundle_id.0, dep.file_name));
            self.lookup_index.insert(
                ResourceLookup::bundle_dep(&bundle_id, &dep.file_name),
                dep_id.clone(),
            );
            self.entries.push(ExternalResourceEntry {
                kind: ExternalResourceKind::BundleDependency,
                binding: ProviderBinding::BundleDependency {
                    bundle_id: bundle_id.clone(),
                    file_name: dep.file_name.clone(),
                },
                source_label: Some(dep.file_name.clone()),
            });
        }
        self.bundles.push(spec);
    }

    /// Merge classic preflight [`ResourceRequests`] (images, video, audio, subtitles).
    pub fn extend_from_resource_requests(&mut self, req: &ResourceRequests) {
        for src in &req.images {
            if let Some((id, label)) = image_asset_id_and_label(src) {
                self.push_flat(ExternalResourceKind::RasterImage, id, Some(label));
            }
        }
        for src in &req.videos {
            let (id, label) = video_asset_id_and_label(src);
            self.push_flat(ExternalResourceKind::Video, id, Some(label));
        }
        for src in &req.audios {
            if let Some((id, label)) = audio_asset_id_and_label(src) {
                self.push_flat(ExternalResourceKind::Audio, id, Some(label));
            }
        }
        for src in &req.subtitles {
            let (id, label) = subtitle_asset_id_and_label(src);
            self.push_flat(ExternalResourceKind::Subtitle, id, Some(label));
        }
    }

    pub fn extend_from_font_manifest(&mut self, manifest: &FontManifest) {
        for face in &manifest.faces {
            self.push_typeface(face);
        }
    }

    /// Register `<lottie>` bundles from preflight (`lottie:{element_id}`).
    pub fn extend_from_lottie_requests(
        &mut self,
        lotties: &std::collections::HashSet<LottieRequest>,
    ) {
        for req in lotties {
            if matches!(req.source, LottieSource::Unset) {
                continue;
            }
            let bundle_id = AssetId(format!("lottie:{}", req.element_id));
            let primary = match &req.source {
                LottieSource::Path(p) => LottiePrimarySource::Path(p.clone()),
                LottieSource::Url(u) => LottiePrimarySource::Url(u.clone()),
                LottieSource::Unset => continue,
            };
            self.push_lottie_bundle(LottieBundleSpec {
                bundle_id,
                primary,
                dependencies: vec![],
            });
        }
    }

    /// After primary Lottie JSON bytes are available, discover `assets[].p` / `u` deps.
    pub fn discover_lottie_dependencies_from_json(
        &mut self,
        bundle_id: &AssetId,
        json: &str,
    ) -> anyhow::Result<()> {
        let names = scan_lottie_dependencies(json)?;
        if let Some(bundle) = self.bundles.iter_mut().find(|b| &b.bundle_id == bundle_id) {
            for name in names {
                if bundle.dependencies.iter().any(|d| d.file_name == name) {
                    continue;
                }
                let dep_id = AssetId(format!("{}:dep:{}", bundle_id.0, name));
                self.lookup_index
                    .insert(ResourceLookup::bundle_dep(bundle_id, &name), dep_id.clone());
                bundle.dependencies.push(BundleDependencySpec {
                    file_name: name.clone(),
                    source: BundleDependencySource::Url(name.clone()),
                });
                self.entries.push(ExternalResourceEntry {
                    kind: ExternalResourceKind::BundleDependency,
                    binding: ProviderBinding::BundleDependency {
                        bundle_id: bundle_id.clone(),
                        file_name: name,
                    },
                    source_label: None,
                });
            }
        }
        Ok(())
    }
}

/// Build manifest from preflight output + document-level fonts.
pub fn build_manifest(req: &ResourceRequests, fonts: &FontManifest) -> ExternalResourceManifest {
    let mut m = ExternalResourceManifest::default();
    m.extend_from_resource_requests(req);
    m.extend_from_lottie_requests(&req.lotties);
    m.extend_from_font_manifest(fonts);
    m
}

pub(crate) fn image_asset_id_and_label(src: &ImageSource) -> Option<(AssetId, String)> {
    match src {
        ImageSource::Unset => None,
        ImageSource::Url(u) => Some((asset_id_for_url(u), u.clone())),
        ImageSource::Path(p) => Some((
            AssetId(p.to_string_lossy().into_owned()),
            p.display().to_string(),
        )),
        ImageSource::Query(q) => Some((asset_id_for_query(q), format!("openverse:{}", q.query))),
    }
}

fn video_asset_id_and_label(src: &VideoSource) -> (AssetId, String) {
    match src {
        VideoSource::Url(u) => (asset_id_for_video_url(u), u.clone()),
        VideoSource::Path(p) => (
            AssetId(format!("video:path:{}", p.to_string_lossy())),
            p.display().to_string(),
        ),
    }
}

fn audio_asset_id_and_label(src: &AudioSource) -> Option<(AssetId, String)> {
    match src {
        AudioSource::Unset => None,
        AudioSource::Url(u) => Some((asset_id_for_audio_url(u), u.clone())),
        AudioSource::Path(p) => Some((
            AssetId(format!("audio:path:{}", p.to_string_lossy())),
            p.display().to_string(),
        )),
    }
}

fn subtitle_asset_id_and_label(src: &SubtitleSource) -> (AssetId, String) {
    match src {
        SubtitleSource::Path(p) => (
            AssetId(format!("subtitle:path:{}", p.to_string_lossy())),
            p.display().to_string(),
        ),
        SubtitleSource::Url(u) => (AssetId(format!("subtitle:url:{u}")), u.clone()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn manifest_unifies_image_and_font_provider_keys() {
        let mut req = ResourceRequests::default();
        req.images
            .insert(ImageSource::Url("https://example.com/a.png".to_string()));
        let mut fonts = FontManifest::default();
        fonts.faces.push(FontFaceDecl {
            id: "sans".into(),
            family: Some("Noto Sans SC".into()),
            source: FontSource::Url("https://example.com/font.otf".into()),
            role: None,
        });

        let m = build_manifest(&req, &fonts);
        assert!(
            m.entries
                .iter()
                .any(|e| matches!(e.kind, ExternalResourceKind::RasterImage))
        );
        assert!(
            m.entries
                .iter()
                .any(|e| matches!(e.kind, ExternalResourceKind::Font))
        );
        assert!(
            m.provider_lookup_index()
                .contains_key(&ResourceLookup::opencat_flat(&asset_id_for_url(
                    "https://example.com/a.png"
                )))
        );
    }
}
