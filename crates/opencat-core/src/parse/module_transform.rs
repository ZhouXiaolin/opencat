//! ESM dist → 引擎模块脚本（oxc 前端）。
//!
//! 作者侧 JSX/Remotion 源码先由 Vite 等打包器编译成 HTML/JS dist；这里只
//! 处理 dist 里的 ESM `import`/`export`，不解析 JSX，也不提供作者级组件或
//! 钩子运行时。

use anyhow::{Result, bail};
use oxc_allocator::{Allocator, Box as ArenaBox, GetAllocator, TakeIn, Vec as ArenaVec};
use oxc_ast::ast::{
    Argument, AssignmentOperator, AssignmentTarget, BindingIdentifier, BindingPattern, Declaration,
    ExportDefaultDeclarationKind, Expression, IdentifierName,
    IdentifierReference, ImportDeclarationSpecifier, MemberExpression, ModuleExportName,
    SimpleAssignmentTarget, Statement, VariableDeclarationKind,
};
use oxc_ast::builder::AstBuilder;
use oxc_codegen::Codegen;
use oxc_diagnostics::OxcDiagnostic;
use oxc_parser::Parser;
use oxc_span::{GetSpan, SPAN, SourceType, Span};

/// 降级产物：`code` 直接进 JS 域；`requires` 交给宿主按模块图解析。
pub struct Transformed {
    pub code: String,
    pub requires: Vec<String>,
}

/// ESM → `__ocRequire` / `module.exports`。
pub fn transform(source: &str) -> Result<Transformed> {
    let allocator = Allocator::new();
    let parsed = Parser::new(&allocator, source, SourceType::mjs()).parse();
    if parsed.diagnostics.has_errors() {
        let first = parsed
            .diagnostics
            .into_iter()
            .next()
            .expect("has_errors 保证至少一条诊断");
        bail!("ESM 解析失败: {first} @ {}", diag_loc(source, &first));
    }
    let mut program = parsed.program;
    let ast = AstBuilder::new(&allocator);
    let mut requires: Vec<String> = Vec::new();
    let mut trailer: Vec<(String, String)> = Vec::new();
    let mut kept: Vec<Statement<'_>> = Vec::new();

    let old_body = std::mem::replace(&mut program.body, ArenaVec::new_in(&ast.allocator()));
    for stmt in old_body {
        match stmt {
            Statement::ImportDeclaration(mut imp) => {
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
                            &ast,
                            "default",
                            Expression::FunctionExpression(f),
                            span,
                        )),
                    },
                    ExportDefaultDeclarationKind::ClassDeclaration(c) => match c.id.as_ref() {
                        Some(id) => {
                            trailer.push((id.name.to_string(), "default".to_string()));
                            kept.push(Statement::ClassDeclaration(c));
                        }
                        None => kept.push(export_assign(
                            &ast,
                            "default",
                            Expression::ClassExpression(c),
                            span,
                        )),
                    },
                    other => match Expression::try_from(other) {
                        Ok(e) => kept.push(export_assign(&ast, "default", e, span)),
                        Err(()) => bail!(
                            "ESM 导出降级: 不支持的 default 导出形态 @ {}",
                            span_loc(source, span)
                        ),
                    },
                }
            }
            Statement::ExportDeclaration(mut ed) => {
                let ed = ed.take_in(&ast.allocator());
                let names = declared_names(&ed.declaration).ok_or_else(|| {
                    anyhow::anyhow!(
                        "ESM 导出降级: 解构导出不支持，请逐名导出（export const a = …）@ {}",
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
            Statement::ExportFromDeclaration(mut ef) => {
                let ef = ef.take_in(&ast.allocator());
                let spec = ef.source.value.as_str();
                requires.push(spec.to_string());
                for spec_item in ef.specifiers {
                    let local = export_name(&spec_item.local, source)?;
                    let exported = export_name(&spec_item.exported, source)?;
                    kept.push(export_assign(
                        &ast,
                        &exported,
                        member_call(&ast, spec, &local),
                        spec_item.span,
                    ));
                }
            }
            Statement::ExportAllDeclaration(mut ea) => {
                let ea = ea.take_in(&ast.allocator());
                let spec = ea.source.value.as_str();
                requires.push(spec.to_string());
                let exports_expr = Expression::new_static_member_expression(
                    SPAN,
                    ident_expr(&ast, "module", SPAN),
                    IdentifierName::new(SPAN, "exports", &ast),
                    false,
                    &ast,
                );
                let call = Expression::new_call_expression(
                    SPAN,
                    ident_expr(&ast, "__ocReexportAll", SPAN),
                    None,
                    [
                        Argument::from(exports_expr),
                        Argument::from(require_call(&ast, spec)),
                    ],
                    false,
                    &ast,
                );
                kept.push(Statement::new_expression_statement(SPAN, call, &ast));
            }
            other => kept.push(other),
        }
    }

    program.body = ArenaVec::from_iter_in(kept, &ast.allocator());
    let code = Codegen::new().build(&program).code;
    let mut out = String::with_capacity(code.len() + 32 * trailer.len());
    out.push_str(&code);
    for (local, exported) in &trailer {
        out.push_str(&format!("\nmodule.exports.{exported} = {local};"));
    }
    if !trailer.is_empty() {
        out.push('\n');
    }
    let has_tla = program.body.iter().any(|s| match s {
        Statement::ExpressionStatement(e) => {
            matches!(&e.expression, Expression::AwaitExpression(_))
        }
        Statement::VariableDeclaration(v) => v.declarations.iter().any(|d| {
            d.init
                .as_ref()
                .is_some_and(|i| matches!(i, Expression::AwaitExpression(_)))
        }),
        _ => false,
    });
    if has_tla {
        out = format!(
            "(async () => {{\n{out}\n}})().catch(function (e) {{ if (typeof __ocStderr === 'function') __ocStderr('oc-tla: ' + (e && e.message)); }});"
        );
    }
    Ok(Transformed { code: out, requires })
}

fn import_bindings<'a>(
    ast: &AstBuilder<'a>,
    spec: &'a str,
    specifiers: Option<ArenaVec<'a, ImportDeclarationSpecifier<'a>>>,
) -> Vec<Statement<'a>> {
    let Some(specifiers) = specifiers else {
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
                let imported: Option<&'a str> = match &d.imported {
                    ModuleExportName::IdentifierName(n) => Some(n.name.as_str()),
                    ModuleExportName::IdentifierReference(n) => Some(n.name.as_str()),
                    ModuleExportName::StringLiteral(_) => None,
                };
                let Some(imported) = imported else { continue };
                out.push(const_binding(
                    ast,
                    d.local.name.as_str(),
                    member_call(ast, spec, imported),
                    d.span,
                ));
            }
        }
    }
    out
}

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

fn require_call<'a>(ast: &AstBuilder<'a>, spec: &'a str) -> Expression<'a> {
    let callee = ident_expr(ast, "__ocRequire", SPAN);
    let arg = Expression::new_string_literal(SPAN, spec, None, ast);
    Expression::new_call_expression(SPAN, callee, None, [Argument::from(arg)], false, ast)
}

fn member_call<'a>(ast: &AstBuilder<'a>, spec: &'a str, prop: &str) -> Expression<'a> {
    Expression::new_static_member_expression(
        SPAN,
        require_call(ast, spec),
        IdentifierName::new(SPAN, ast.allocator().alloc_str(prop), ast),
        false,
        ast,
    )
}

fn export_assign<'a>(
    ast: &AstBuilder<'a>,
    prop: &str,
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
        IdentifierName::new(span, ast.allocator().alloc_str(prop), ast),
        false,
        ast,
    );
    let target = AssignmentTarget::from(SimpleAssignmentTarget::from(member));
    let assign =
        Expression::new_assignment_expression(span, AssignmentOperator::Assign, target, value, ast);
    Statement::new_expression_statement(span, assign, ast)
}

fn declared_names(decl: &Declaration<'_>) -> Option<Vec<String>> {
    match decl {
        Declaration::VariableDeclaration(vd) => {
            let mut names = Vec::new();
            for declarator in &vd.declarations {
                match &declarator.id {
                    BindingPattern::BindingIdentifier(id) => names.push(id.name.to_string()),
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
            "ESM 导出降级: 字符串导出名不支持（{:?}）@ {}",
            s.value,
            span_loc(source, name.span())
        ),
    }
}

fn ident_expr<'a>(ast: &AstBuilder<'a>, name: &'a str, span: Span) -> Expression<'a> {
    Expression::Identifier(ArenaBox::new_in(
        IdentifierReference::new(span, name, ast),
        &ast.allocator(),
    ))
}

fn span_loc(source: &str, span: Span) -> String {
    let offset = (span.start as usize).min(source.len());
    let before = &source[..offset];
    let line = before.matches('\n').count() + 1;
    let col = before
        .rsplit('\n')
        .next()
        .map_or(1, |l| l.chars().count() + 1);
    format!("{line}:{col}")
}

fn diag_loc(source: &str, diag: &OxcDiagnostic) -> String {
    diag.labels
        .first()
        .map(|l| span_loc(source, l.span()))
        .unwrap_or_default()
}
