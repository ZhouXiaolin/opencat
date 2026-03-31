use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct AssetId(pub String);

pub struct AssetsMap {
    entries: HashMap<AssetId, AssetEntry>,
}

struct AssetEntry {
    path: PathBuf,
    width: u32,
    height: u32,
}

impl AssetsMap {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
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
}

fn read_image_dimensions(path: &Path) -> (u32, u32) {
    let Ok(reader) = image::ImageReader::open(path) else {
        return (0, 0);
    };
    let Ok(dimensions) = reader.into_dimensions() else {
        return (0, 0);
    };
    dimensions
}
