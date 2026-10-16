//! 共享纹理的登记处。
//!
//! 一张 GPU 纹理跨 crate 传递时有两个问题：**谁保活**、**谁归还**。生产方
//! （wgpu 侧）建出纹理，消费方（Skia 侧）采样它，但两方都不该持有对方的类型。
//!
//! 登记处把这两件事收在一处：生产方交出「原生句柄 + 保活凭据」，拿到一个
//! [`TextureId`]；消费方凭 id 取句柄；用完归还。id 是不透明整数，可以进
//! core 的 IR，也可以跨线程。

use std::any::Any;
use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::NativeTexture;

/// 登记处发出的凭据。
///
/// 单调递增，永不复用——驱动会回收原生句柄，所以句柄本身不是稳定标识。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TextureId(u64);

impl TextureId {
    /// 从裸值重建（凭据从 IR 或 FFI 边界回来时用）。
    pub fn from_raw(raw: u64) -> Self {
        Self(raw)
    }

    /// 取裸值（凭据进 `GpuImageHandle` 时用）。
    pub fn raw(self) -> u64 {
        self.0
    }
}

struct Entry {
    native: NativeTexture,
    /// 保活凭据：生产方给的、必须活到归还的东西（如 `wgpu::Texture`）。
    /// 登记处不解释它，只在 [`TextureRegistry::release`] 时丢掉。
    _keepalive: Box<dyn Any + Send>,
}

/// 共享纹理登记处。
///
/// 生产方与消费方各持一个 `Arc<TextureRegistry>`，或者由一方持有、另一方借用。
#[derive(Default)]
pub struct TextureRegistry {
    entries: Mutex<HashMap<u64, Entry>>,
    next: AtomicU64,
}

impl TextureRegistry {
    pub fn new() -> Self {
        Self {
            entries: Mutex::new(HashMap::new()),
            next: AtomicU64::new(1),
        }
    }

    /// 登记一张原生纹理，返回凭据。
    ///
    /// `keepalive` 是纹理的所有权凭据——登记处替你保活，直到
    /// [`Self::release`]。调用方交出它之后就不该再持有。
    pub fn publish(&self, native: NativeTexture, keepalive: Box<dyn Any + Send>) -> TextureId {
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        self.entries
            .lock()
            .expect("texture registry poisoned")
            .insert(id, Entry {
                native,
                _keepalive: keepalive,
            });
        TextureId(id)
    }

    /// 凭 id 取原生句柄。id 已归还或不存在时返回 `None`。
    pub fn resolve(&self, id: TextureId) -> Option<NativeTexture> {
        self.entries
            .lock()
            .expect("texture registry poisoned")
            .get(&id.0)
            .map(|entry| entry.native)
    }

    /// 归还：丢掉保活凭据，纹理随即销毁。
    pub fn release(&self, id: TextureId) {
        let _ = self
            .entries
            .lock()
            .expect("texture registry poisoned")
            .remove(&id.0);
    }

    /// 当前登记数。泄漏排查用。
    pub fn len(&self) -> usize {
        self.entries
            .lock()
            .expect("texture registry poisoned")
            .len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}
