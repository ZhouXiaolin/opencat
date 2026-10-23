//! 单张字段表 → WGSL 结构体 + uniform 字节。
//!
//! 与真库的分工差异：真库用 typegpu 的 TGSL，由 TypeGPU 计算布局；本 crate
//! 自己按 WGSL 规范（WebGPU §Alignment and Size）算。**一张表同时产出声明
//! 与字节**，所以「声明的偏移」与「写入的偏移」不可能漂移——不需要 naga
//! 反射去猜，也就没有 reflect 与 write 两套逻辑走偏的余地。

use anyhow::{Result, bail};

/// 标量/向量/数组的元素类型。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ty {
    F32,
    Vec2f,
    Vec4f,
    /// `array<vec4f, N>`
    Vec4fArray(u32),
    /// `array<vec3f, N>`
    Vec3fArray(u32),
}

impl Ty {
    fn wgsl(self) -> String {
        match self {
            Ty::F32 => "f32".into(),
            Ty::Vec2f => "vec2f".into(),
            Ty::Vec4f => "vec4f".into(),
            Ty::Vec4fArray(n) => format!("array<vec4f, {n}>"),
            Ty::Vec3fArray(n) => format!("array<vec3f, {n}>"),
        }
    }

    /// 标量个数（写入时按分量排布）。
    fn components(self) -> usize {
        match self {
            Ty::F32 => 1,
            Ty::Vec2f => 2,
            Ty::Vec4f => 4,
            Ty::Vec4fArray(n) => 4 * n as usize,
            Ty::Vec3fArray(n) => 3 * n as usize,
        }
    }

    /// 元素对齐（字节），按 WGSL 规则。
    fn align(self) -> usize {
        match self {
            Ty::F32 => 4,
            Ty::Vec2f => 8,
            // 数组在 uniform 空间里元素对齐向上取到 16。
            Ty::Vec4f | Ty::Vec4fArray(_) | Ty::Vec3fArray(_) => 16,
        }
    }

    fn size(self) -> usize {
        match self {
            Ty::F32 => 4,
            Ty::Vec2f => 8,
            Ty::Vec4f => 16,
            // vec4f 元素 stride 16；vec3f 元素 stride 16（对齐抬升）。
            Ty::Vec4fArray(n) => 16 * n as usize,
            Ty::Vec3fArray(n) => 16 * n as usize,
        }
    }
}

/// 一个 uniform 字段的值。数组用扁平分量表示（长度 = 分量数）。
#[derive(Clone, Debug, PartialEq)]
pub enum Val {
    F(f32),
    Fs(Vec<f32>),
}

impl Val {
    pub fn as_f32(&self) -> f32 {
        match self {
            Val::F(v) => *v,
            Val::Fs(v) => v.first().copied().unwrap_or(0.0),
        }
    }
    pub fn as_fs(&self) -> Vec<f32> {
        match self {
            Val::F(v) => vec![*v],
            Val::Fs(v) => v.clone(),
        }
    }
}

pub struct Field {
    pub name: &'static str,
    pub ty: Ty,
    pub val: Val,
}

impl Field {
    pub fn f32(name: &'static str, v: f32) -> Self {
        Field { name, ty: Ty::F32, val: Val::F(v) }
    }
    pub fn vec2(name: &'static str, x: f32, y: f32) -> Self {
        Field { name, ty: Ty::Vec2f, val: Val::Fs(vec![x, y]) }
    }
    pub fn vec4(name: &'static str, r: f32, g: f32, b: f32, a: f32) -> Self {
        Field { name, ty: Ty::Vec4f, val: Val::Fs(vec![r, g, b, a]) }
    }
    pub fn vec4s(name: &'static str, v: Vec<f32>) -> Self {
        assert_eq!(v.len(), 4, "vec4 需要恰好 4 个分量，实际 {}", v.len());
        Field { name, ty: Ty::Vec4f, val: Val::Fs(v) }
    }

    pub fn vec4a(name: &'static str, v: Vec<f32>) -> Self {
        assert_eq!(v.len() % 4, 0, "vec4 数组分量数须为 4 的倍数");
        Field { name, ty: Ty::Vec4fArray((v.len() / 4) as u32), val: Val::Fs(v) }
    }
    pub fn vec3s(name: &'static str, v: Vec<f32>) -> Self {
        assert_eq!(v.len() % 3, 0, "vec3 数组分量数须为 3 的倍数");
        Field { name, ty: Ty::Vec3fArray((v.len() / 3) as u32), val: Val::Fs(v) }
    }
}

/// 一个 struct（`n_n0` 之类的一级成员）。
pub struct Group {
    pub name: &'static str,
    pub fields: Vec<Field>,
}

impl Group {
    pub fn new(name: &'static str, fields: Vec<Field>) -> Self {
        Group { name, fields }
    }

    fn offsets(&self) -> Vec<usize> {
        let mut cur = 0usize;
        let mut out = Vec::with_capacity(self.fields.len());
        for f in &self.fields {
            let a = f.ty.align();
            cur = cur.div_ceil(a) * a;
            out.push(cur);
            cur += f.ty.size();
        }
        out
    }

    fn align(&self) -> usize {
        self.fields.iter().map(|f| f.ty.align()).max().unwrap_or(4)
    }

    /// uniform 地址空间里的占用大小：round 到 16，让下一个成员总是 16 对齐的。
    fn size(&self) -> usize {
        let offs = self.offsets();
        let last = offs
            .last()
            .zip(self.fields.last())
            .map(|(o, f)| o + f.ty.size())
            .unwrap_or(0);
        last.div_ceil(16) * 16
    }

    fn wgsl(&self) -> String {
        let mut out = format!("struct {} {{\n", self.name);
        for f in &self.fields {
            out.push_str(&format!("  {}: {},\n", f.name, f.ty.wgsl()));
        }
        out.push_str("}\n");
        out
    }
}

/// 整个 uniform：按声明顺序排列的一级成员。
///
/// 偏移按 WGSL uniform 地址空间算（struct 起始对齐到其最大成员对齐）。
pub struct Uniform {
    pub groups: Vec<Group>,
}

impl Uniform {
    pub fn new(groups: Vec<Group>) -> Self {
        Uniform { groups }
    }

    fn offsets(&self) -> Vec<usize> {
        let mut cur = 0usize;
        let mut out = Vec::with_capacity(self.groups.len());
        for g in &self.groups {
            // uniform 地址空间里 struct 成员的对齐抬到 16（WGSL `RequiredAlignOf`）——
            // 声明与字节共用这张表，所以两边一起对。
            let a = g.align().max(16);
            cur = cur.div_ceil(a) * a;
            out.push(cur);
            cur += g.size();
        }
        out
    }

    pub fn size(&self) -> usize {
        let offs = self.offsets();
        let last = offs
            .last()
            .zip(self.groups.last())
            .map(|(o, g)| o + g.size())
            .unwrap_or(0);
        // buffer 总长按 16 对齐（wgpu 要求 uniform binding size 是 16 的倍数）。
        last.div_ceil(16) * 16
    }

    /// WGSL：成员 struct 声明 + `combined` + 全局 `uniforms` 绑定。
    ///
    /// 成员 struct 直接内联展开为 `combined` 的字段，不做嵌套——WGSL 允许
    /// 嵌套 struct，但展平后偏移计算与写入共用同一张表，更不容易漂移。
    pub fn wgsl(&self) -> String {
        let offs = self.offsets();
        let mut out = String::new();
        out.push_str("struct combined {\n");
        for (g, _o) in self.groups.iter().zip(&offs) {
            out.push_str(&format!("  @align(16) {}: {},\n", g.name, struct_name(g.name)));
        }
        out.push_str("}\n\n");
        for g in &self.groups {
            out.push_str(&format!("struct {} {{\n", struct_name(g.name)));
            for f in &g.fields {
                out.push_str(&format!("  {}: {},\n", f.name, f.ty.wgsl()));
            }
            out.push_str("}\n\n");
        }
        out.push_str("@group(0) @binding(0) var<uniform> uniforms: combined;\n\n");
        out
    }

    /// 按字段表写出 uniform 字节。
    pub fn bytes(&self) -> Result<Vec<u8>> {
        let mut buf = vec![0u8; self.size()];
        let goffs = self.offsets();
        for (g, goff) in self.groups.iter().zip(&goffs) {
            let foffs = g.offsets();
            for (f, foff) in g.fields.iter().zip(&foffs) {
                let base = goff + foff;
                let want = f.ty.components();
                let vals = "f.val.as_fs()";
                let vals = eval(vals, f);
                if vals.len() != want {
                    bail!(
                        "{}.{}: {} 个分量，但类型 {:?} 需要 {want}",
                        g.name,
                        f.name,
                        vals.len(),
                        f.ty
                    );
                }
                match f.ty {
                    Ty::F32 | Ty::Vec2f | Ty::Vec4f => {
                        for (i, v) in vals.iter().enumerate() {
                            put(&mut buf, base + i * 4, *v);
                        }
                    }
                    Ty::Vec4fArray(n) | Ty::Vec3fArray(n) => {
                        let comps = match f.ty {
                            Ty::Vec4fArray(_) => 4,
                            _ => 3,
                        };
                        for i in 0..n as usize {
                            for c in 0..comps {
                                let src = i * comps + c;
                                // 元素 stride 恒 16（vec3f 也对齐到 16）。
                                put(&mut buf, base + i * 16 + c * 4, vals[src]);
                            }
                        }
                    }
                }
            }
        }
        Ok(buf)
    }
}

fn eval(_hint: &str, f: &Field) -> Vec<f32> {
    f.val.as_fs()
}

fn put(buf: &mut [u8], off: usize, v: f32) {
    buf[off..off + 4].copy_from_slice(&v.to_le_bytes());
}

fn struct_name(group: &str) -> String {
    format!("{group}_t")
}
