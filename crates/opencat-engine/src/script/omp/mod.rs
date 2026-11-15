//! `@omp` 通用循环派发 runtime：bundle 缓存 → args 解码 → worker 行并行
//! → 写回/ovals 按行序落地。
//!
//! 数据面（缓存/路由/校验/写回组包）已下沉 `opencat-core::script::omp`
//! （web host 与 desktop engine 共用）；engine 只留团队调度——rayon 池 +
//! isolate worker（V8 / QuickJS 二选一），经 `OmpTeam` 注入数据面。
//!
//! 全链确定性三保证：行产出按行号槽位合并（调度不影响字节）；f64 运算
//! 无归约无 race（行间零共享可变状态）；同参数两跑逐位一致（帧=f(t)）。
//! 任何失败（bundle 构建/解码/头部枚举/行执行/采集数不整除）都毒化缓存
//! 条目 → 后续帧 prep 直接 false → 骨架不再跑 → fallback 循环语义纯净
//! （PRNG 不被无谓消耗）。

pub use opencat_core::script::omp::{omp_prep, protocol, route, transform};

#[cfg(feature = "js-quickjs")]
pub mod rq_team;
pub mod team;
#[cfg(feature = "js-v8")]
pub mod v8_team;

use std::sync::Arc;

use serde_json::Value as JsonValue;

use opencat_core::script::recorder::MutationStore;

// isolate 团队工厂：引擎二选一（js-v8 → V8Worker / js-quickjs → RqWorker）。
#[cfg(feature = "js-quickjs")]
use rq_team::global_team;
#[cfg(feature = "js-v8")]
use v8_team::global_team;

/// `__ocOmpTake(ir, ...args, skeleton)` — 数据面在 core，engine 调用点
/// 保持原签名（isolate 团队在此注入）。
pub fn omp_take(
    store: &mut MutationStore,
    ir: &JsonValue,
    args: &[JsonValue],
    shared: Arc<[protocol::SharedBuf]>,
    prng: &[f64],
    writes: &mut Vec<protocol::IsolateWrite>,
    reds: &mut Vec<f64>,
) -> bool {
    opencat_core::script::omp::omp_take(
        global_team(),
        store,
        ir,
        args,
        shared,
        prng,
        writes,
        reds,
    )
}
