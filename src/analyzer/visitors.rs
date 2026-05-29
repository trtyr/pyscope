use crate::analyzer::builder::Builder;
use crate::analyzer::helpers;
use crate::model::*;
use rustpython_parser::ast::*;
use rustpython_parser::{parse, Mode};
use std::path::Path;

/// Visit a single Python file and populate the builder with nodes and edges.
pub fn visit_file(
    builder: &mut Builder,
    source: &str,
    file_path: &Path,
    project_root: &Path,
) -> anyhow::Result<()> {
    let ast = parse(source, Mode::Module, &file_path.to_string_lossy())
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    let module_qname = helpers::module_name(file_path, project_root);
    let file_str = file_path
        .strip_prefix(project_root)
        .unwrap_or(file_path)
        .display()
        .to_string();

    let total_lines = source.lines().count();

    let file_id = builder.add_node(
        NodeKind::File,
        &file_str,
        &file_str,
        Some(&file_str),
        1,
        total_lines,
        None,
        None,
        None,
    );

    let module_id = builder.add_node(
        NodeKind::Module,
        &module_qname,
        &module_qname,
        Some(&file_str),
        1,
        total_lines,
        None,
        None,
        None,
    );

    builder.edge(&file_id, &module_id, EdgeKind::ModuleFile, None, None, None);

    // Attach file metrics to the file node (2nd from last, before module)
    if let Some(node) = builder.nodes.iter_mut().rev().nth(1) {
        node.metrics = helpers::file_metrics(source);
    }

    let Mod::Module(ModModule { body, .. }) = ast else {
        return Ok(());
    };

    for stmt in &body {
        visit_stmt(builder, stmt, source, &file_str, &module_qname, &module_id);
    }

    Ok(())
}

fn visit_stmt(builder: &mut Builder, stmt: &Stmt, source: &str, file: &str, parent_qname: &str, module_id: &str) {
    match stmt {
        Stmt::FunctionDef(fd) => {
            let name = fd.name.to_string();
            let qname = format!("{}.{}", parent_qname, name);
            let sig = build_signature(&name, &fd.args, fd.returns.as_deref());
            let docs = helpers::docstring(&fd.body);
            let vis = helpers::visibility(&name);
            let start_line = helpers::offset_line(source, fd.range.start().to_usize());
            let end_line = helpers::find_block_end(source, start_line);
            let func_id = builder.add_node(
                NodeKind::Function, &name, &qname, Some(file),
                start_line, end_line, vis.as_deref(), Some(&sig), docs.as_deref(),
            );
            builder.edge(module_id, &func_id, EdgeKind::Declares, None, None, None);
            visit_body_calls(builder, &fd.body, source, file, &func_id);
            visit_decorators(builder, &fd.decorator_list, source, file, &qname, &func_id);
        }
        Stmt::AsyncFunctionDef(fd) => {
            let name = fd.name.to_string();
            let qname = format!("{}.{}", parent_qname, name);
            let sig = build_signature(&name, &fd.args, fd.returns.as_deref());
            let docs = helpers::docstring(&fd.body);
            let vis = helpers::visibility(&name);
            let start_line = helpers::offset_line(source, fd.range.start().to_usize());
            let end_line = helpers::find_block_end(source, start_line);
            let func_id = builder.add_node(
                NodeKind::AsyncFunction, &name, &qname, Some(file),
                start_line, end_line, vis.as_deref(), Some(&sig), docs.as_deref(),
            );
            builder.edge(module_id, &func_id, EdgeKind::Declares, None, None, None);
            visit_body_calls(builder, &fd.body, source, file, &func_id);
            visit_decorators(builder, &fd.decorator_list, source, file, &qname, &func_id);
        }
        Stmt::ClassDef(cd) => visit_class(builder, cd, source, file, parent_qname, module_id),
        Stmt::Import(imp) => visit_import(builder, imp, source, file, parent_qname, module_id),
        Stmt::ImportFrom(impf) => visit_import_from(builder, impf, source, file, parent_qname, module_id),
        Stmt::Assign(assign) => visit_assign(builder, assign, source, file, parent_qname, module_id),
        Stmt::AnnAssign(ann) => {
            if let Expr::Name(name) = ann.target.as_ref() {
                let qname = format!("{}.{}", parent_qname, name.id.to_string());
                let line = helpers::offset_line(source, ann.range.start().to_usize());
                let var_id = builder.add_node(
                    NodeKind::Variable, &name.id.to_string(), &qname, Some(file),
                    line, line, helpers::visibility(&name.id.to_string()).as_deref(),
                    None, None,
                );
                builder.edge(module_id, &var_id, EdgeKind::Declares, None, None, None);
            }
        }
        _ => {}
    }
}

fn visit_decorators(
    builder: &mut Builder,
    decorators: &[Expr],
    source: &str,
    file: &str,
    parent_qname: &str,
    target_id: &str,
) {
    for dec in decorators {
        if let Some(dec_name) = extract_name(dec) {
            let dec_qname = format!("{}.@{}", parent_qname, dec_name);
            let dec_line = helpers::offset_line(source, dec.range().start().to_usize());
            let dec_id = builder.add_node(
                NodeKind::Decorator, &dec_name, &dec_qname, Some(file),
                dec_line, dec_line, None, None, None,
            );
            builder.edge(target_id, &dec_id, EdgeKind::Declares, None, None, None);
        }
    }
}

// ---- Class ----

fn visit_class(
    builder: &mut Builder,
    cd: &StmtClassDef,
    source: &str,
    file: &str,
    parent_qname: &str,
    module_id: &str,
) {
    let name = cd.name.to_string();
    let qname = format!("{}.{}", parent_qname, name);
    let docs = helpers::docstring(&cd.body);
    let vis = helpers::visibility(&name);
    let start_line = helpers::offset_line(source, cd.range.start().to_usize());
    let end_line = helpers::find_block_end(source, start_line);

    let class_id = builder.add_node(
        NodeKind::Class, &name, &qname, Some(file),
        start_line, end_line, vis.as_deref(), None, docs.as_deref(),
    );

    builder.edge(module_id, &class_id, EdgeKind::Declares, None, None, None);

    // Inheritance
    for base in &cd.bases {
        if let Some(base_name) = extract_name(base) {
            let loc = helpers::location(file, source, base.range().start().to_usize());
            builder.add_pending(&class_id, &base_name, EdgeKind::InheritsFrom, Some(loc), None);
        }
    }

    // Walk body for methods and class-level assignments
    for stmt in &cd.body {
        match stmt {
            Stmt::FunctionDef(fd) => {
                let (method_kind, _) = classify_method(fd);
                visit_method(builder, fd, source, file, &qname, &class_id, method_kind);
            }
            Stmt::Assign(assign) => {
                for target in &assign.targets {
                    if let Expr::Name(name) = target {
                        let n = name.id.to_string();
                        let var_qname = format!("{}.{}", qname, n);
                        let line = helpers::offset_line(source, assign.range.start().to_usize());
                        let field_id = builder.add_node(
                            NodeKind::Field, &n, &var_qname, Some(file),
                            line, line, helpers::visibility(&n).as_deref(), None, None,
                        );
                        builder.edge(&class_id, &field_id, EdgeKind::HasField, None, None, None);
                    }
                }
            }
            Stmt::AnnAssign(ann) => {
                if let Expr::Name(name) = ann.target.as_ref() {
                    let n = name.id.to_string();
                    let var_qname = format!("{}.{}", qname, n);
                    let line = helpers::offset_line(source, ann.range.start().to_usize());
                    let field_id = builder.add_node(
                        NodeKind::Field, &n, &var_qname, Some(file),
                        line, line, helpers::visibility(&n).as_deref(), None, None,
                    );
                    builder.edge(&class_id, &field_id, EdgeKind::HasField, None, None, None);
                }
            }
            _ => {}
        }
    }
}

fn classify_method(fd: &StmtFunctionDef) -> (NodeKind, bool) {
    for dec in &fd.decorator_list {
        if let Expr::Name(name) = dec {
            match name.id.to_string().as_str() {
                "classmethod" => return (NodeKind::ClassMethod, false),
                "staticmethod" => return (NodeKind::StaticMethod, false),
                "property" => return (NodeKind::Property, false),
                _ => {}
            }
        }
    }
    (NodeKind::Method, false)
}

fn visit_method(
    builder: &mut Builder,
    fd: &StmtFunctionDef,
    source: &str,
    file: &str,
    class_qname: &str,
    class_id: &str,
    kind: NodeKind,
) {
    let name = fd.name.to_string();
    let qname = format!("{}.{}", class_qname, name);
    let sig = build_signature(&name, &fd.args, fd.returns.as_deref());
    let docs = helpers::docstring(&fd.body);
    let vis = helpers::visibility(&name);
    let start_line = helpers::offset_line(source, fd.range.start().to_usize());
    let end_line = helpers::find_block_end(source, start_line);

    let method_id = builder.add_node(
        kind, &name, &qname, Some(file),
        start_line, end_line, vis.as_deref(), Some(&sig), docs.as_deref(),
    );

    builder.edge(class_id, &method_id, EdgeKind::HasMethod, None, None, None);
    visit_body_calls(builder, &fd.body, source, file, &method_id);
}

// ---- Imports ----

fn visit_import(
    builder: &mut Builder,
    imp: &StmtImport,
    source: &str,
    file: &str,
    parent_qname: &str,
    module_id: &str,
) {
    for alias in &imp.names {
        let import_name = alias.asname.as_ref().unwrap_or(&alias.name).to_string();
        let qname = format!("{}.import.{}", parent_qname, import_name);
        let line = helpers::offset_line(source, imp.range.start().to_usize());
        let import_id = builder.add_node(
            NodeKind::Import, &import_name, &qname, Some(file),
            line, line, None, None, None,
        );
        builder.edge(module_id, &import_id, EdgeKind::Declares, None, None, None);
        builder.add_pending(
            &import_id,
            &alias.name.to_string(),
            EdgeKind::Imports,
            Some(helpers::location(file, source, imp.range.start().to_usize())),
            None,
        );
    }
}

fn visit_import_from(
    builder: &mut Builder,
    impf: &StmtImportFrom,
    source: &str,
    file: &str,
    parent_qname: &str,
    module_id: &str,
) {
    let module_prefix = impf.module.as_ref().map(|m| m.to_string()).unwrap_or_default();
    for alias in &impf.names {
        let import_name = alias.asname.as_ref().unwrap_or(&alias.name).to_string();
        let full_import = if module_prefix.is_empty() {
            alias.name.to_string()
        } else {
            format!("{}.{}", module_prefix, alias.name.to_string())
        };
        let qname = format!("{}.import.{}", parent_qname, import_name);
        let line = helpers::offset_line(source, impf.range.start().to_usize());
        let import_id = builder.add_node(
            NodeKind::Import, &import_name, &qname, Some(file),
            line, line, None, None, None,
        );
        builder.edge(module_id, &import_id, EdgeKind::Declares, None, None, None);
        builder.add_pending(
            &import_id,
            &full_import,
            EdgeKind::Imports,
            Some(helpers::location(file, source, impf.range.start().to_usize())),
            None,
        );
    }
}

// ---- Assignments ----

fn visit_assign(
    builder: &mut Builder,
    assign: &StmtAssign,
    source: &str,
    file: &str,
    parent_qname: &str,
    module_id: &str,
) {
    for target in &assign.targets {
        if let Expr::Name(name) = target {
            let n = name.id.to_string();
            let qname = format!("{}.{}", parent_qname, n);
            let line = helpers::offset_line(source, assign.range.start().to_usize());
            let var_id = builder.add_node(
                NodeKind::Variable, &n, &qname, Some(file),
                line, line, helpers::visibility(&n).as_deref(), None, None,
            );
            builder.edge(module_id, &var_id, EdgeKind::Declares, None, None, None);
        }
    }
}

// ---- Call traversal ----

fn visit_body_calls(
    builder: &mut Builder,
    body: &[Stmt],
    source: &str,
    file: &str,
    caller_id: &str,
) {
    for stmt in body {
        match stmt {
            Stmt::Expr(expr_stmt) => {
                visit_expr_calls(builder, &expr_stmt.value, source, file, caller_id);
            }
            Stmt::Assign(assign) => {
                visit_expr_calls(builder, &assign.value, source, file, caller_id);
            }
            Stmt::Return(ret) => {
                if let Some(value) = &ret.value {
                    visit_expr_calls(builder, value, source, file, caller_id);
                }
            }
            Stmt::If(if_stmt) => {
                visit_expr_calls(builder, &if_stmt.test, source, file, caller_id);
                visit_body_calls(builder, &if_stmt.body, source, file, caller_id);
                visit_body_calls(builder, &if_stmt.orelse, source, file, caller_id);
            }
            Stmt::For(for_stmt) => {
                visit_body_calls(builder, &for_stmt.body, source, file, caller_id);
            }
            Stmt::While(while_stmt) => {
                visit_expr_calls(builder, &while_stmt.test, source, file, caller_id);
                visit_body_calls(builder, &while_stmt.body, source, file, caller_id);
            }
            Stmt::With(with_stmt) => {
                visit_body_calls(builder, &with_stmt.body, source, file, caller_id);
            }
            Stmt::Try(try_stmt) => {
                visit_body_calls(builder, &try_stmt.body, source, file, caller_id);
                for handler in &try_stmt.handlers {
                    let ExceptHandler::ExceptHandler(h) = handler;
                    visit_body_calls(builder, &h.body, source, file, caller_id);
                }
                visit_body_calls(builder, &try_stmt.finalbody, source, file, caller_id);
            }
            _ => {}
        }
    }
}

fn visit_expr_calls(
    builder: &mut Builder,
    expr: &Expr,
    source: &str,
    file: &str,
    caller_id: &str,
) {
    match expr {
        Expr::Call(call) => {
            let loc = helpers::location(file, source, call.range.start().to_usize());
            match call.func.as_ref() {
                Expr::Name(name) => {
                    builder.add_pending(
                        caller_id,
                        &name.id.to_string(),
                        EdgeKind::Calls,
                        Some(loc),
                        Some("direct"),
                    );
                }
                Expr::Attribute(attr) => {
                    builder.add_pending(
                        caller_id,
                        &attr.attr.to_string(),
                        EdgeKind::Calls,
                        Some(loc),
                        Some("method"),
                    );
                }
                _ => {
                    builder.add_pending(
                        caller_id, "?", EdgeKind::Calls, Some(loc), Some("direct"),
                    );
                }
            }
            for arg in &call.args {
                visit_expr_calls(builder, arg, source, file, caller_id);
            }
            for kw in &call.keywords {
                visit_expr_calls(builder, &kw.value, source, file, caller_id);
            }
        }
        Expr::Attribute(attr) => {
            visit_expr_calls(builder, &attr.value, source, file, caller_id);
        }
        Expr::BinOp(binop) => {
            visit_expr_calls(builder, &binop.left, source, file, caller_id);
            visit_expr_calls(builder, &binop.right, source, file, caller_id);
        }
        Expr::UnaryOp(unary) => {
            visit_expr_calls(builder, &unary.operand, source, file, caller_id);
        }
        Expr::BoolOp(boolop) => {
            for val in &boolop.values {
                visit_expr_calls(builder, val, source, file, caller_id);
            }
        }
        Expr::Compare(cmp) => {
            visit_expr_calls(builder, &cmp.left, source, file, caller_id);
            for comp in &cmp.comparators {
                visit_expr_calls(builder, comp, source, file, caller_id);
            }
        }
        Expr::IfExp(ifexp) => {
            visit_expr_calls(builder, &ifexp.test, source, file, caller_id);
            visit_expr_calls(builder, &ifexp.body, source, file, caller_id);
            visit_expr_calls(builder, &ifexp.orelse, source, file, caller_id);
        }
        Expr::Subscript(sub) => {
            visit_expr_calls(builder, &sub.value, source, file, caller_id);
            visit_expr_calls(builder, &sub.slice, source, file, caller_id);
        }
        Expr::ListComp(lc) => {
            visit_expr_calls(builder, &lc.elt, source, file, caller_id);
        }
        Expr::Dict(dict) => {
            for (key, val) in dict.keys.iter().zip(dict.values.iter()) {
                if let Some(k) = key {
                    visit_expr_calls(builder, k, source, file, caller_id);
                }
                visit_expr_calls(builder, val, source, file, caller_id);
            }
        }
        Expr::Starred(star) => {
            visit_expr_calls(builder, &star.value, source, file, caller_id);
        }
        _ => {}
    }
}

// ---- Signature building ----

fn build_signature(name: &str, args: &Arguments, returns: Option<&Expr>) -> String {
    let mut parts: Vec<String> = Vec::new();

    for arg in &args.posonlyargs {
        parts.push(fmt_arg(&arg.def, arg.default.as_deref()));
    }
    if !args.posonlyargs.is_empty() && !args.args.is_empty() {
        parts.push("/".to_string());
    }
    for arg in &args.args {
        parts.push(fmt_arg(&arg.def, arg.default.as_deref()));
    }
    if let Some(vararg) = &args.vararg {
        parts.push(format!("*{}", vararg.arg.to_string()));
    }
    for arg in &args.kwonlyargs {
        parts.push(fmt_arg(&arg.def, arg.default.as_deref()));
    }
    if let Some(kwarg) = &args.kwarg {
        parts.push(format!("**{}", kwarg.arg.to_string()));
    }

    let mut sig = format!("def {}({})", name, parts.join(", "));
    if let Some(ret) = returns {
        sig.push_str(&format!(" -> {}", expr_to_string(ret)));
    }
    sig
}

fn fmt_arg(arg: &Arg, default: Option<&Expr>) -> String {
    let mut s = arg.arg.to_string();
    if let Some(ann) = &arg.annotation {
        s.push_str(&format!(": {}", expr_to_string(ann)));
    }
    if let Some(def) = default {
        s.push_str(&format!(" = {}", expr_to_string(def)));
    }
    s
}

fn expr_to_string(expr: &Expr) -> String {
    match expr {
        Expr::Name(name) => name.id.to_string(),
        Expr::Constant(c) => match &c.value {
            Constant::Str(s) => format!("\"{}\"", s),
            Constant::Int(n) => n.to_string(),
            Constant::Float(f) => f.to_string(),
            Constant::Bool(b) => b.to_string(),
            Constant::None => "None".to_string(),
            Constant::Ellipsis => "...".to_string(),
            _ => "?".to_string(),
        },
        Expr::Attribute(attr) => {
            format!("{}.{}", expr_to_string(&attr.value), attr.attr.to_string())
        }
        Expr::Subscript(sub) => {
            format!("{}[{}]", expr_to_string(&sub.value), expr_to_string(&sub.slice))
        }
        Expr::Tuple(tup) => {
            let items: Vec<String> = tup.elts.iter().map(expr_to_string).collect();
            format!("({})", items.join(", "))
        }
        Expr::List(lst) => {
            let items: Vec<String> = lst.elts.iter().map(expr_to_string).collect();
            format!("[{}]", items.join(", "))
        }
        Expr::BinOp(binop) => {
            let op_str = match binop.op {
                Operator::BitOr => "|",
                _ => "?",
            };
            format!("{} {} {}", expr_to_string(&binop.left), op_str, expr_to_string(&binop.right))
        }
        Expr::UnaryOp(unary) => {
            format!("?{}", expr_to_string(&unary.operand))
        }
        _ => "?".to_string(),
    }
}

fn extract_name(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Name(name) => Some(name.id.to_string()),
        Expr::Attribute(attr) => Some(attr.attr.to_string()),
        Expr::Call(call) => extract_name(&call.func),
        _ => None,
    }
}
