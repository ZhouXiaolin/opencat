//! JSX/ESM → 引擎模块脚本（oxc 前端）。
//!
//! 退役自同名的手写 JS 扫描器（`script/runtime/jsx_transform.js`，466 行字符
//! 扫描 + 逐行正则）：多行 import、完整 JS 语法、错误行列从这一步起原生支持。
//!
//! 契约与旧实现逐语义对齐（消费方：`engine/jsx.rs` 的模块图 + 每帧 driver，
//! 产物喂 V8 `new Function` 逐帧重放）：
//!
//!   import * as ns from 'x'        → const ns = __ocRequire('x');
//!   import { a, b as c } from 'x'  → const { a: a, b: c } = __ocRequire('x');
//!   import D from 'x'              → const D = __ocRequire('x').default;
//!   export default <fn/expr>       → module.exports.default = …
//!   export const/function/class N  → 剥 export 关键字 + 尾部 module.exports.N = N
//!   export { a as b }              → 尾部 module.exports.b = a（esbuild bundle 形态）
//!
//! JSX 经 oxc classic runtime 降级到 `h(...)`（`globalThis.h` 由 jsx_runtime
//! 注入），Fragment 用 `globalThis.__ocFragment`。直读 `.jsx` 与 esbuild
//! bundle 走同一入口；bundle 里 JSX 已被 esbuild 折叠，实际生效的是
//! import/export 降级。

use std::path::Path;

use anyhow::{Result, bail};
use oxc_allocator::{Allocator, Box as ArenaBox, GetAllocator, TakeIn, Vec as ArenaVec};
use oxc_ast::ast::{
    Argument, AssignmentOperator, AssignmentTarget, BindingIdentifier, BindingPattern,
    Declaration, ExportDefaultDeclarationKind, Expression, IdentifierName, IdentifierReference,
    ImportDeclarationSpecifier, MemberExpression, ModuleExportName, SimpleAssignmentTarget,
    Statement, VariableDeclarationKind,
};
use oxc_ast::builder::AstBuilder;
use oxc_codegen::Codegen;
use oxc_diagnostics::OxcDiagnostic;
use oxc_parser::Parser;
use oxc_span::{GetSpan, SourceType, Span, SPAN};
use oxc_transformer::{JsxOptions, JsxRuntime, TransformOptions, Transformer};

/// 降级产物：`code` 直接进 V8；`requires` 交给宿主按模块图解析；`exports`
/// 仅供宿主诊断（提取走 `module.exports` 运行时路径，不读这个清单）。
#[derive(Debug)]
pub struct Transformed {
    pub code: String,
    pub requires: Vec<String>,
    pub exports: Vec<String>,
}

/// JSX + ESM → 引擎模块脚本。
pub fn transform(source: &str) -> Result<Transformed> {
    let allocator = Allocator::new();
    let parsed = Parser::new(&allocator, source, SourceType::jsx()).parse();
    if parsed.diagnostics.has_errors() {
        let first = parsed
            .diagnostics
            .into_iter()
            .next()
            .expect("has_errors 保证至少一条诊断");
        bail!("jsx 解析失败: {first} @ {}", diag_loc(source, &first));
    }
    let mut program = parsed.program;

    // JSX 降级：classic runtime，pragma 对齐引擎 h() 契约。pure 关掉——
    // /*#__PURE__*/ 注释对 V8 重放毫无意义，只污染产物。
    let mut options = TransformOptions::default();
    options.jsx = JsxOptions {
        jsx_plugin: true,
        runtime: JsxRuntime::Classic,
        pragma: Some("h".to_string()),
        pragma_frag: Some("globalThis.__ocFragment".to_string()),
        pure: false,
        development: false,
        ..JsxOptions::default()
    };
    // JSX transformer 的遍历依赖 scope_id/reference_id（parser 不填），先跑语义分析。
    let semantic = oxc_semantic::SemanticBuilder::new().build(&program).semantic;
    let transformer = Transformer::new(&allocator, Path::new("entry.jsx"), &options)
        .build_with_scoping(semantic.into_scoping(), &mut program);
    if transformer.diagnostics.has_errors() {
        let first = transformer
            .diagnostics
            .into_iter()
            .next()
            .expect("has_errors 保证至少一条诊断");
        bail!("jsx 降级失败: {first} @ {}", diag_loc(source, &first));
    }

    // 模块降级：只关心顶层语句。import 展开成 const 绑定、export 剥壳，
    // 其余语句按原顺序保留，整体一次 codegen，尾部拼 module.exports 行。
    let ast = AstBuilder::new(&allocator);
    let mut requires: Vec<String> = Vec::new();
    // (local, exported)：尾部 `module.exports.{exported} = {local};`
    let mut trailer: Vec<(String, String)> = Vec::new();
    let mut kept: Vec<Statement<'_>> = Vec::new();

    let old_body = std::mem::replace(&mut program.body, ArenaVec::new_in(&ast.allocator()));
    for stmt in old_body {
        match stmt {
            Statement::ImportDeclaration(mut imp) => {
                // take_in 拿出所有权（arena Box 不可移出字段）；spec 直接借
                // arena 内的 Str，零拷贝获得 'a 生存期。
                let imp = imp.take_in(&ast.allocator());
                let spec = imp.source.value.as_str();
                requires.push(spec.to_string());
                for binding in import_bindings(&ast, spec, imp.specifiers) {
                    kept.push(binding);
                }
            }
            Statement::ExportDefaultDeclaration(mut ed) => {
                let ed = ed.take_in(&ast.allocator());
                let span = ed.span;
                match ed.declaration {
                    ExportDefaultDeclarationKind::FunctionDeclaration(f) => match f.id.as_ref() {
                        Some(id) => {
                            trailer.push((id.name.to_string(), "default".to_string()));
                            kept.push(Statement::FunctionDeclaration(f));
                        }
                        None => kept.push(export_assign(
                            &ast, "default", Expression::FunctionExpression(f), span,
                        )),
                    },
                    ExportDefaultDeclarationKind::ClassDeclaration(c) => match c.id.as_ref() {
                        Some(id) => {
                            trailer.push((id.name.to_string(), "default".to_string()));
                            kept.push(Statement::ClassDeclaration(c));
                        }
                        None => kept.push(export_assign(
                            &ast, "default", Expression::ClassExpression(c), span,
                        )),
                    },
                    // 其余全部按表达式形态处理（继承自 Expression）；非表达式
                    // （TSInterfaceDeclaration）try_from 失败后报错。
                    other => match Expression::try_from(other) {
                        Ok(e) => kept.push(export_assign(&ast, "default", e, span)),
                        Err(()) => bail!(
                            "jsx 导出降级: 不支持的 default 导出形态 @ {}",
                            span_loc(source, span)
                        ),
                    },
                }
            }
            Statement::ExportDeclaration(mut ed) => {
                let ed = ed.take_in(&ast.allocator());
                let names = declared_names(&ed.declaration).ok_or_else(|| {
                    anyhow::anyhow!(
                        "jsx 导出降级: 解构导出不支持，请逐名导出（export const a = …）@ {}",
                        span_loc(source, ed.span)
                    )
                })?;
                let inner = ed.declaration;
                kept.push(match inner {
                    Declaration::VariableDeclaration(v) => Statement::VariableDeclaration(v),
                    Declaration::FunctionDeclaration(f) => Statement::FunctionDeclaration(f),
                    Declaration::ClassDeclaration(c) => Statement::ClassDeclaration(c),
                    Declaration::TSTypeAliasDeclaration(_) => continue,
                    other => Statement::from(other),
                });
                for name in names {
                    trailer.push((name.clone(), name));
                }
            }
            Statement::ExportNamedDeclaration(mut en) => {
                let en = en.take_in(&ast.allocator());
                for spec in en.specifiers {
                    let local = export_name(&spec.local, source)?;
                    let exported = export_name(&spec.exported, source)?;
                    trailer.push((local, exported));
                }
            }
            Statement::ExportAllDeclaration(_) => {
                bail!("jsx 导出降级: export * from 不支持（模块图没有转发语义）");
            }
            Statement::ExportFromDeclaration(_) => {
                bail!("jsx 导出降级: re-export（export {{ a }} from）不支持，先在源模块导出");
            }
            other => kept.push(other),
        }
    }

    program.body = ArenaVec::from_iter_in(kept, &ast.allocator());

    let code = Codegen::new().build(&program).code;
    let mut out = String::with_capacity(code.len() + 32 * trailer.len());
    out.push_str(&code);
    let mut exports = Vec::with_capacity(trailer.len());
    for (local, exported) in &trailer {
        out.push_str(&format!("\nmodule.exports.{exported} = {local};"));
        exports.push(exported.clone());
    }
    if !trailer.is_empty() {
        out.push('\n');
    }
    Ok(Transformed { code: out, requires, exports })
}

/// 一条 import 声明 → 若干 prelude 绑定语句（const 形态，与旧实现对齐）。
fn import_bindings<'a>(
    ast: &AstBuilder<'a>,
    spec: &'a str,
    specifiers: Option<ArenaVec<'a, ImportDeclarationSpecifier<'a>>>,
) -> Vec<Statement<'a>> {
    let Some(specifiers) = specifiers else {
        // `import 'x'`：裸引入只留副作用调用。
        let call = require_call(ast, spec);
        return vec![Statement::new_expression_statement(SPAN, call, ast)];
    };
    let mut out = Vec::new();
    for s in specifiers {
        match s {
            ImportDeclarationSpecifier::ImportDefaultSpecifier(d) => {
                out.push(const_binding(
                    ast,
                    d.local.name.as_str(),
                    member_call(ast, spec, "default"),
                    d.span,
                ));
            }
            ImportDeclarationSpecifier::ImportNamespaceSpecifier(d) => {
                out.push(const_binding(
                    ast,
                    d.local.name.as_str(),
                    require_call(ast, spec),
                    d.span,
                ));
            }
            ImportDeclarationSpecifier::ImportSpecifier(d) => {
                // 字符串导出名（import { "a.b" as c }）不在引擎方言内，跳过。
                let imported: Option<&'a str> = match &d.imported {
                    ModuleExportName::IdentifierName(n) => Some(n.name.as_str()),
                    ModuleExportName::IdentifierReference(n) => Some(n.name.as_str()),
                    ModuleExportName::StringLiteral(_) => None,
                };
                let Some(imported) = imported else { continue };
                let local = d.local.name.as_str();
                // imported==local 也走成员取值：binding 需要的是该名字的导出
                // 成员，而不是 require 返回的模块对象本身（bundle 里
                // `import { h } from 'opencat/runtime'` 后直接 `h(...)` 调用）。
                let value = member_call(ast, spec, imported);
                out.push(const_binding(ast, local, value, d.span));
            }
        }
    }
    out
}

/// `const <name> = <value>;`
fn const_binding<'a>(
    ast: &AstBuilder<'a>,
    name: &'a str,
    value: Expression<'a>,
    span: Span,
) -> Statement<'a> {
    let declarator = oxc_ast::ast::VariableDeclarator::new(
        span,
        BindingPattern::BindingIdentifier(ArenaBox::new_in(
            BindingIdentifier::new(span, name, ast),
            &ast.allocator(),
        )),
        None,
        Some(value),
        false,
        ast,
    );
    let decl = oxc_ast::ast::VariableDeclaration::new(
        span,
        VariableDeclarationKind::Const,
        ArenaVec::from_iter_in([declarator], &ast.allocator()),
        false,
        ast,
    );
    Statement::VariableDeclaration(ArenaBox::new_in(decl, &ast.allocator()))
}

/// `__ocRequire('spec');`（裸副作用语句）
fn require_call<'a>(ast: &AstBuilder<'a>, spec: &'a str) -> Expression<'a> {
    let callee = ident_expr(ast, "__ocRequire", SPAN);
    let arg = Expression::new_string_literal(SPAN, spec, None, ast);
    Expression::new_call_expression(SPAN, callee, None, [Argument::from(arg)], false, ast)
}

/// `__ocRequire('spec').<prop>`
fn member_call<'a>(ast: &AstBuilder<'a>, spec: &'a str, prop: &'a str) -> Expression<'a> {
    Expression::new_static_member_expression(
        SPAN,
        require_call(ast, spec),
        IdentifierName::new(SPAN, prop, ast),
        false,
        ast,
    )
}

/// `module.exports.<prop> = <value>;`
fn export_assign<'a>(
    ast: &AstBuilder<'a>,
    prop: &'a str,
    value: Expression<'a>,
    span: Span,
) -> Statement<'a> {
    let module_expr = ident_expr(ast, "module", span);
    let exports_expr = Expression::new_static_member_expression(
        span,
        module_expr,
        IdentifierName::new(span, "exports", ast),
        false,
        ast,
    );
    let member = MemberExpression::new_static_member_expression(
        span,
        exports_expr,
        IdentifierName::new(span, prop, ast),
        false,
        ast,
    );
    let target = AssignmentTarget::from(SimpleAssignmentTarget::from(member));
    let assign = Expression::new_assignment_expression(
        span,
        AssignmentOperator::Assign,
        target,
        value,
        ast,
    );
    Statement::new_expression_statement(span, assign, ast)
}

/// 声明语句导出的名字清单；解构绑定返回 None（调用方报错）。
fn declared_names(decl: &Declaration<'_>) -> Option<Vec<String>> {
    match decl {
        Declaration::VariableDeclaration(vd) => {
            let mut names = Vec::new();
            for declarator in &vd.declarations {
                match &declarator.id {
                    BindingPattern::BindingIdentifier(id) => {
                        names.push(id.name.to_string());
                    }
                    _ => return None,
                }
            }
            Some(names)
        }
        Declaration::FunctionDeclaration(f) => f.id.as_ref().map(|id| vec![id.name.to_string()]),
        Declaration::ClassDeclaration(c) => c.id.as_ref().map(|id| vec![id.name.to_string()]),
        _ => Some(Vec::new()),
    }
}

fn export_name(name: &ModuleExportName<'_>, source: &str) -> Result<String> {
    match name {
        ModuleExportName::IdentifierName(n) => Ok(n.name.to_string()),
        ModuleExportName::IdentifierReference(n) => Ok(n.name.to_string()),
        ModuleExportName::StringLiteral(s) => anyhow::bail!(
            "jsx 导出降级: 字符串导出名不支持（{:?}）@ {}",
            s.value,
            span_loc(source, name.span())
        ),
    }
}

/// 标识符引用表达式（`Expression::Identifier` 是 `Box<IdentifierReference>`）。
fn ident_expr<'a>(ast: &AstBuilder<'a>, name: &'a str, span: Span) -> Expression<'a> {
    Expression::Identifier(ArenaBox::new_in(
        IdentifierReference::new(span, name, ast),
        &ast.allocator(),
    ))
}

/// span → `line:col`（1 起）。
fn span_loc(source: &str, span: Span) -> String {
    let offset = (span.start as usize).min(source.len());
    let before = &source[..offset];
    let line = before.matches('\n').count() + 1;
    let col = before.rsplit('\n').next().map_or(1, |l| l.chars().count() + 1);
    format!("{line}:{col}")
}

/// 诊断携带的首个 span → 行列；无 span 时返回空串。
fn diag_loc(source: &str, diag: &OxcDiagnostic) -> String {
    diag.labels
        .first()
        .map(|l| span_loc(source, l.span()))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn code(src: &str) -> String {
        transform(src).expect("transform 应成功").code
    }

    #[test]
    fn multiline_import_with_alias() {
        let out = transform(
            "import gsap, { Timeline } from 'gsap';\nimport * as THREE from 'three';\nconst x = 1;\n",
        )
        .unwrap();
        assert_eq!(out.requires, vec!["gsap".to_string(), "three".to_string()]);
        assert!(out.code.contains("const gsap = __ocRequire(\"gsap\").default;"));
        // imported == local 的具名导入同样取同名成员
        assert!(out.code.contains("const Timeline = __ocRequire(\"gsap\").Timeline;"));
        assert!(out.code.contains("const THREE = __ocRequire(\"three\");"));
    }

    #[test]
    fn import_same_name_binds_require_directly() {
        let out = transform("import { gsap } from 'gsap';\n").unwrap();
        // imported == local 同样取同名成员（绑模块对象是错的，bundle 里
        // `import { h } from 'opencat/runtime'` 后直接 `h(...)` 调用）
        assert!(out.code.contains("const gsap = __ocRequire(\"gsap\").gsap;"));
    }

    #[test]
    fn export_default_expression_and_named_list() {
        let out = code("const composition = 1;\nexport default composition;\nexport { composition as App };\n");
        assert!(out.contains("module.exports.default = composition;"));
        assert!(out.contains("module.exports.App = composition;"));
    }

    #[test]
    fn export_const_strips_keyword_and_appends_trailer() {
        let out = code("export const css = \".a{}\";\nexport function main() {}\n");
        assert!(out.contains("const css = \".a{}\";"));
        assert!(out.contains("module.exports.css = css;"));
        assert!(out.contains("module.exports.main = main;"));
    }

    #[test]
    fn jsx_lowers_to_h_and_fragment_pragma() {
        let out = code("export default <div id=\"a\"><>t</></div>;\n");
        assert!(out.contains("h(\"div\""));
        assert!(out.contains("globalThis.__ocFragment"));
    }

    #[test]
    fn spread_and_entities_pass_through() {
        let out = code("const p = {id: 1};\nexport default <div {...p} title=\"a&amp;b\" />;\n");
        assert!(out.contains("...p"));
        assert!(out.contains("\"a&b\""));
    }

    #[test]
    fn parse_error_reports_line_col() {
        let err = transform("const a = ;\nconst b = 2;\n").unwrap_err().to_string();
        assert!(err.contains("1:"), "错误应带行列: {err}");
    }

    #[test]
    fn destructured_export_rejected_with_guidance() {
        let err = transform("export const { a } = obj;\n").unwrap_err().to_string();
        assert!(err.contains("逐名导出"), "{err}");
    }
}
