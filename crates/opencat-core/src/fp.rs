//! 字段维度标记的结构体指纹（M4-⑤ 3a）。
//!
//! `fp_struct!` 让「字段声明清单」与「指纹哈希清单」在同一处维护：每个字段
//! 在声明行就地标注所属维度（`=> [dims] mode?`），加字段必须同处选维度
//! （或显式 `=> []` 退出并写明原因）——「加字段漏指纹 = 缓存误命中、静默
//! 画错」这类最难抓的 bug 从类别上消灭（评审候选⑤的机制目标）。
//!
//! 生成物：`fp_hash_dim<H: Hasher>(&self, dim, state)`——按 [`FpDim`] 过滤
//! 声明序字段逐个哈希；单维度结构体（`@impl_hash [dim]`）额外生成
//! `impl Hash` 委托到该维度。手写的复合指纹（含 per-kind match / 枚举
//! 派发的层）继续用 std Hash 包装器，内部调 `fp_hash_dim` 取字段清单。
//!
//! 哈希值只活在进程内（ElementNode/DisplayNode 不序列化、缓存键不落盘），
//! 字段集不变而值变是安全的；渲染行为只取决于「同输入 → 同键」。

use std::hash::{Hash, Hasher};

/// 指纹维度。语义层三路（[`ElementInputFingerprints`](crate::semantic::fingerprint::ElementInputFingerprints)
/// 的 structure/layout/paint/apply——structure 是结构手写哈希，不走本机制）
/// + 文本的三份子集维度（语义 `TextLayout`/`TextPaint` 与 display 层
/// `TextPaintDisplay` 的字段集今天不同，3b 统一后收敛）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FpDim {
    /// 布局输入（ComputedLayoutStyle 全量减未标注项）。
    Layout,
    /// 绘制输入（ComputedVisualStyle / display 侧 paint style 的绘制字段）。
    Paint,
    /// composite/apply 维度（opacity / transforms / backdrop / mask）——
    /// 只进 L3 DisplayNode 复用判断，不进 paint 缓存键。
    Apply,
    /// 语义层文本布局输入。
    TextLayout,
    /// 语义层文本绘制输入。
    TextPaint,
    /// display 层文本绘制输入（TextDisplayItem.style，字段集比语义层少
    /// `wrap_text`——3b 统一）。
    TextPaintDisplay,
}

/// f32 指纹哈希：逐位（NaN 按位、-0≠+0——布局/绘制输入逐位敏感，与
/// 语义层历史 `F32Hash` 同语义）。
pub struct FpF32(pub f32);

impl Hash for FpF32 {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.to_bits().hash(state);
    }
}

/// 字段哈希展开：`bits` = f32 逐位；`opt_bits` = Option<f32> 逐位。
/// （mode 缺省的字段不走本宏——`@field` 无 mode 臂直接 `.hash`。）
/// `#[macro_export]` 仅为让 `fp_struct!` 的展开能以 `$crate::` 路径调用。
#[macro_export]
macro_rules! fp_hash_expr {
    ($sf:expr, bits, $st:ident) => {
        std::hash::Hash::hash(&$crate::fp::FpF32($sf), $st);
    };
    ($sf:expr, opt_bits, $st:ident) => {
        std::hash::Hash::hash(&$sf.map($crate::fp::FpF32), $st);
    };
}

/// 见模块文档。字段语法：
///
/// ```text
/// $(#[field 属性])* $vis $name: $ty => [$($dim),*] $($mode:ident)?,
/// ```
///
/// `=> []` = 不进任何指纹维度（必须显式写出并在注释里说明原因）；
/// mode ∈ {`bits`, `opt_bits`}（f32/Option<f32> 逐位哈希），缺省 = 值本身
/// 实现 Hash；`@impl_hash [dim]` = 单维度结构体，附带 `impl Hash` 委托到
/// 该维度。
#[macro_export]
macro_rules! fp_struct {
    // 单个字段按维度过滤展开（mode 缺省形态）。
    (@field $sf:expr, $dim:ident, $st:ident ; [ $($d:ident),* ]) => {
        $( if $dim == $crate::fp::FpDim::$d { std::hash::Hash::hash(&$sf, $st); } )*
    };
    // 单个字段按维度过滤展开（带 mode 形态：mode 尾随在 state 之后）。
    (@field $sf:expr, $dim:ident, $st:ident $m:ident ; [ $($d:ident),* ]) => {
        $( if $dim == $crate::fp::FpDim::$d { $crate::fp_hash_expr!($sf, $m, $st); } )*
    };
    // 单维度结构体：结构体 + fp_hash_dim 照常生成，另附 impl Hash 委托。
    (
        @impl_hash [$single:ident]
        $(#[$meta:meta])*
        $vis:vis struct $name:ident {
            $(
                $(#[$fmeta:meta])*
                $fvis:vis $fname:ident : $fty:ty
                    => [ $($dim:ident),* ] $($mode:ident)?
            ),* $(,)?
        }
    ) => {
        $crate::fp_struct! {
            $(#[$meta])*
            $vis struct $name {
                $(
                    $(#[$fmeta])*
                    $fvis $fname : $fty => [ $($dim),* ] $($mode)?,
                )*
            }
        }
        impl std::hash::Hash for $name {
            fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
                // 单维度契约：impl Hash = 该维度字段集（声明序）。
                self.fp_hash_dim($crate::fp::FpDim::$single, state);
            }
        }
    };
    (
        $(#[$meta:meta])*
        $vis:vis struct $name:ident {
            $(
                $(#[$fmeta:meta])*
                $fvis:vis $fname:ident : $fty:ty
                    => [ $($dim:ident),* ] $($mode:ident)?
            ),* $(,)?
        }
    ) => {
        $(#[$meta])*
        $vis struct $name {
            $(
                $(#[$fmeta])*
                $fvis $fname : $fty,
            )*
        }

        impl $name {
            /// 字段维度指纹哈希（fp_struct! 生成）：按 [`FpDim`] 过滤
            /// 声明序字段。`=> []` 字段在任何维度都不出现。
            pub fn fp_hash_dim<H: std::hash::Hasher>(
                &self,
                dim: $crate::fp::FpDim,
                state: &mut H,
            ) {
                $(
                    $crate::fp_struct!(
                        @field self.$fname, dim, state $($mode)? ;
                        [ $($dim),* ]
                    );
                )*
                let _ = dim; // 全字段 `=> []` 的结构体也需要 dim 形参
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    fp_struct! {
        #[derive(Debug)]
        struct Toy {
            a: u32 => [Layout, Paint],
            b: f32 => [Paint] bits,
            c: Option<f32> => [Layout] opt_bits,
            /// 不进任何维度（示例退出位）。
            skipped: String => [],
        }
    }

    #[test]
    fn dims_filter_fields_in_declaration_order() {
        let toy = Toy { a: 7, b: 1.5, c: Some(2.5), skipped: "x".into() };

        let mut h = ahash::AHasher::default();
        toy.fp_hash_dim(FpDim::Layout, &mut h);
        let layout = h.finish();

        let mut h = ahash::AHasher::default();
        toy.fp_hash_dim(FpDim::Paint, &mut h);
        let paint = h.finish();

        // 两维字段集不同 → 哈希不同。
        assert_ne!(layout, paint);

        // 手搓等价：Layout = a + c(opt_bits)，声明序。
        let mut h = ahash::AHasher::default();
        toy.a.hash(&mut h);
        toy.c.map(FpF32).hash(&mut h);
        assert_eq!(h.finish(), layout);

        // Paint = a + b(bits)。
        let mut h = ahash::AHasher::default();
        toy.a.hash(&mut h);
        FpF32(toy.b).hash(&mut h);
        assert_eq!(h.finish(), paint);
    }

    fp_struct! {
        @impl_hash [Paint]
        #[derive(Debug)]
        struct ToySingle {
            p: u32 => [Paint],
            q: f32 => [Paint] bits,
            dropped: bool => [],
        }
    }

    #[test]
    fn single_dim_impl_hash_delegates_and_excludes_unmarked() {
        let toy = ToySingle { p: 3, q: 0.5, dropped: true };
        let mut h = ahash::AHasher::default();
        toy.hash(&mut h);
        let via_hash = h.finish();

        let mut h = ahash::AHasher::default();
        toy.fp_hash_dim(FpDim::Paint, &mut h);
        assert_eq!(h.finish(), via_hash);

        // dropped 不进维度 → 也不进 Hash：改它不动哈希。
        let other = ToySingle { p: 3, q: 0.5, dropped: !toy.dropped };
        let mut h = ahash::AHasher::default();
        other.hash(&mut h);
        assert_eq!(h.finish(), via_hash);
    }
}
