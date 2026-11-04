//! OCIR schema 生成器入口：把 `ir/schema_gen.rs` 的产物无条件重写到 web 重放端
//! 的 generated 目录。
//!
//! `crates/opencat-web/web/package.json` 的 build 链第一步调用本 bin——协议
//! 常量的唯一事实源在 `ir/draw_encoding.rs` / `ir/gpu_stream.rs`，生成文件
//! 每次 web 构建前自动重写，漂移在构造上不可能发生（无检查、无测试）。

use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> std::io::Result<ExitCode> {
    let ts = opencat_core::ir::schema_gen::generate_ocir_schema_ts();
    let out = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../crates/opencat-web/web/src/generated/ocir-schema.generated.ts");
    std::fs::create_dir_all(out.parent().expect("generated 目录的父级"))?;
    std::fs::write(&out, ts)?;
    println!("ocir-schema-gen: {}", out.display());

    let js = opencat_core::ir::schema_gen::generate_gl_opcodes_js();
    let js_out = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src/script/runtime/gl-opcodes.generated.js");
    std::fs::write(&js_out, js)?;
    println!("ocir-schema-gen: {}", js_out.display());
    Ok(ExitCode::SUCCESS)
}
