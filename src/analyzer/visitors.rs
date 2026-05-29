use crate::analyzer::builder::Builder;
use crate::analyzer::helpers;
use crate::model::*;
use rustpython_parser::ast::*;
use rustpython_parser::{Mode, parse};
use std::collections::BTreeSet;
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

fn visit_stmt(
    builder: &mut Builder,
    stmt: &Stmt,
    source: &str,
    file: &str,
    parent_qname: &str,
    module_id: &str,
) {
    match stmt {
        Stmt::FunctionDef(fd) => {
            let name = fd.name.to_string();
            let qname = format!("{}.{}", parent_qname, name);
            let sig = build_signature(&name, &fd.args, fd.returns.as_deref());
            let docs = helpers::docstring(&fd.body);
            let vis = helpers::visibility(&name);
            let start_line = helpers::offset_line(source, fd.range.start().to_usize());
            let end_line = helpers::find_block_end(source, start_line);
            let kind = classify_callable_kind(&name, &fd.body, NodeKind::Function);
            let func_id = builder.add_node(
                kind,
                &name,
                &qname,
                Some(file),
                start_line,
                end_line,
                vis.as_deref(),
                Some(&sig),
                docs.as_deref(),
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
            let kind = classify_callable_kind(&name, &fd.body, NodeKind::AsyncFunction);
            let func_id = builder.add_node(
                kind,
                &name,
                &qname,
                Some(file),
                start_line,
                end_line,
                vis.as_deref(),
                Some(&sig),
                docs.as_deref(),
            );
            builder.edge(module_id, &func_id, EdgeKind::Declares, None, None, None);
            visit_body_calls(builder, &fd.body, source, file, &func_id);
            visit_decorators(builder, &fd.decorator_list, source, file, &qname, &func_id);
        }
        Stmt::ClassDef(cd) => visit_class(builder, cd, source, file, parent_qname, module_id),
        Stmt::Import(imp) => visit_import(builder, imp, source, file, parent_qname, module_id),
        Stmt::ImportFrom(impf) => {
            visit_import_from(builder, impf, source, file, parent_qname, module_id)
        }
        Stmt::Assign(assign) => {
            visit_assign(builder, assign, source, file, parent_qname, module_id)
        }
        Stmt::AnnAssign(ann) => {
            if let Expr::Name(name) = ann.target.as_ref() {
                let qname = format!("{}.{}", parent_qname, name.id.to_string());
                let line = helpers::offset_line(source, ann.range.start().to_usize());
                let var_id = builder.add_node(
                    NodeKind::Variable,
                    &name.id.to_string(),
                    &qname,
                    Some(file),
                    line,
                    line,
                    helpers::visibility(&name.id.to_string()).as_deref(),
                    None,
                    None,
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
        if let Some(dec_name) = extract_qualified_name(dec).or_else(|| extract_name(dec)) {
            let dec_qname = format!("{}.@{}", parent_qname, dec_name);
            let dec_line = helpers::offset_line(source, dec.range().start().to_usize());
            let dec_id = builder.add_node(
                NodeKind::Decorator,
                &dec_name,
                &dec_qname,
                Some(file),
                dec_line,
                dec_line,
                None,
                None,
                None,
            );
            builder.edge(target_id, &dec_id, EdgeKind::Declares, None, None, None);
            builder.edge(target_id, &dec_id, EdgeKind::Decorates, None, None, None);
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
    let kind = classify_class(cd);

    let class_id = builder.add_node(
        kind,
        &name,
        &qname,
        Some(file),
        start_line,
        end_line,
        vis.as_deref(),
        None,
        docs.as_deref(),
    );

    builder.edge(module_id, &class_id, EdgeKind::Declares, None, None, None);
    visit_decorators(builder, &cd.decorator_list, source, file, &qname, &class_id);

    // Inheritance
    let mut base_names = Vec::new();
    for base in &cd.bases {
        if let Some(base_name) = extract_qualified_name(base).or_else(|| extract_name(base)) {
            let loc = helpers::location(file, source, base.range().start().to_usize());
            builder.add_pending(
                &class_id,
                &base_name,
                EdgeKind::InheritsFrom,
                Some(loc),
                None,
            );
            base_names.push(base_name);
        }
    }

    // Walk body for methods and class-level assignments
    for stmt in &cd.body {
        match stmt {
            Stmt::FunctionDef(fd) => {
                let method_kind = classify_method(fd);
                visit_method(
                    builder,
                    fd,
                    source,
                    file,
                    &qname,
                    &class_id,
                    &base_names,
                    method_kind,
                );
            }
            Stmt::AsyncFunctionDef(fd) => {
                let method_kind = classify_async_method(fd);
                visit_async_method(
                    builder,
                    fd,
                    source,
                    file,
                    &qname,
                    &class_id,
                    &base_names,
                    method_kind,
                );
            }
            Stmt::Assign(assign) => {
                for target in &assign.targets {
                    if let Expr::Name(name) = target {
                        let n = name.id.to_string();
                        let var_qname = format!("{}.{}", qname, n);
                        let line = helpers::offset_line(source, assign.range.start().to_usize());
                        let field_id = builder.add_node(
                            classify_class_variable(&n),
                            &n,
                            &var_qname,
                            Some(file),
                            line,
                            line,
                            helpers::visibility(&n).as_deref(),
                            None,
                            None,
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
                        classify_class_variable(&n),
                        &n,
                        &var_qname,
                        Some(file),
                        line,
                        line,
                        helpers::visibility(&n).as_deref(),
                        None,
                        None,
                    );
                    builder.edge(&class_id, &field_id, EdgeKind::HasField, None, None, None);
                }
            }
            _ => {}
        }
    }
}

fn classify_method(fd: &StmtFunctionDef) -> NodeKind {
    for dec in &fd.decorator_list {
        if let Some(name) = extract_name(dec) {
            match name.as_str() {
                "classmethod" => {
                    return classify_callable_kind(
                        &fd.name.to_string(),
                        &fd.body,
                        NodeKind::ClassMethod,
                    );
                }
                "staticmethod" => {
                    return classify_callable_kind(
                        &fd.name.to_string(),
                        &fd.body,
                        NodeKind::StaticMethod,
                    );
                }
                "property" => {
                    return classify_callable_kind(
                        &fd.name.to_string(),
                        &fd.body,
                        NodeKind::Property,
                    );
                }
                _ => {}
            }
        }
    }
    classify_callable_kind(&fd.name.to_string(), &fd.body, NodeKind::Method)
}

fn classify_async_method(fd: &StmtAsyncFunctionDef) -> NodeKind {
    for dec in &fd.decorator_list {
        if let Some(name) = extract_name(dec) {
            match name.as_str() {
                "classmethod" => {
                    return classify_callable_kind(
                        &fd.name.to_string(),
                        &fd.body,
                        NodeKind::ClassMethod,
                    );
                }
                "staticmethod" => {
                    return classify_callable_kind(
                        &fd.name.to_string(),
                        &fd.body,
                        NodeKind::StaticMethod,
                    );
                }
                "property" => {
                    return classify_callable_kind(
                        &fd.name.to_string(),
                        &fd.body,
                        NodeKind::Property,
                    );
                }
                _ => {}
            }
        }
    }
    classify_callable_kind(&fd.name.to_string(), &fd.body, NodeKind::AsyncMethod)
}

fn visit_method(
    builder: &mut Builder,
    fd: &StmtFunctionDef,
    source: &str,
    file: &str,
    class_qname: &str,
    class_id: &str,
    base_names: &[String],
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
        kind,
        &name,
        &qname,
        Some(file),
        start_line,
        end_line,
        vis.as_deref(),
        Some(&sig),
        docs.as_deref(),
    );

    builder.edge(class_id, &method_id, EdgeKind::HasMethod, None, None, None);
    visit_body_calls(builder, &fd.body, source, file, &method_id);
    visit_decorators(
        builder,
        &fd.decorator_list,
        source,
        file,
        &qname,
        &method_id,
    );

    if is_constructor(&name) {
        visit_instance_variables(builder, &fd.body, source, file, class_qname, class_id);
    }

    maybe_add_override_edges(
        builder,
        &method_id,
        &name,
        base_names,
        file,
        source,
        fd.range.start().to_usize(),
    );
}

fn visit_async_method(
    builder: &mut Builder,
    fd: &StmtAsyncFunctionDef,
    source: &str,
    file: &str,
    class_qname: &str,
    class_id: &str,
    base_names: &[String],
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
        kind,
        &name,
        &qname,
        Some(file),
        start_line,
        end_line,
        vis.as_deref(),
        Some(&sig),
        docs.as_deref(),
    );

    builder.edge(class_id, &method_id, EdgeKind::HasMethod, None, None, None);
    visit_body_calls(builder, &fd.body, source, file, &method_id);
    visit_decorators(
        builder,
        &fd.decorator_list,
        source,
        file,
        &qname,
        &method_id,
    );

    if is_constructor(&name) {
        visit_instance_variables(builder, &fd.body, source, file, class_qname, class_id);
    }

    maybe_add_override_edges(
        builder,
        &method_id,
        &name,
        base_names,
        file,
        source,
        fd.range.start().to_usize(),
    );
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
            NodeKind::Import,
            &import_name,
            &qname,
            Some(file),
            line,
            line,
            None,
            None,
            None,
        );
        builder.edge(module_id, &import_id, EdgeKind::Declares, None, None, None);
        builder.add_pending(
            &import_id,
            &alias.name.to_string(),
            EdgeKind::Imports,
            Some(helpers::location(
                file,
                source,
                imp.range.start().to_usize(),
            )),
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
    let module_prefix = impf
        .module
        .as_ref()
        .map(|m| m.to_string())
        .unwrap_or_default();
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
            NodeKind::Import,
            &import_name,
            &qname,
            Some(file),
            line,
            line,
            None,
            None,
            None,
        );
        builder.edge(module_id, &import_id, EdgeKind::Declares, None, None, None);
        builder.add_pending(
            &import_id,
            &full_import,
            EdgeKind::FromImports,
            Some(helpers::location(
                file,
                source,
                impf.range.start().to_usize(),
            )),
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
                NodeKind::Variable,
                &n,
                &qname,
                Some(file),
                line,
                line,
                helpers::visibility(&n).as_deref(),
                None,
                None,
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

fn visit_expr_calls(builder: &mut Builder, expr: &Expr, source: &str, file: &str, caller_id: &str) {
    match expr {
        Expr::Call(call) => {
            let loc = helpers::location(file, source, call.range.start().to_usize());
            match call.func.as_ref() {
                Expr::Name(name) => {
                    builder.add_pending(
                        caller_id,
                        &name.id.to_string(),
                        EdgeKind::Calls,
                        Some(loc.clone()),
                        Some("direct"),
                    );
                }
                Expr::Attribute(attr) => {
                    builder.add_pending(
                        caller_id,
                        &attr.attr.to_string(),
                        EdgeKind::Calls,
                        Some(loc.clone()),
                        Some("method"),
                    );
                }
                _ => {
                    builder.add_pending(caller_id, "?", EdgeKind::Calls, Some(loc), Some("direct"));
                }
            }
            for arg in &call.args {
                visit_expr_calls(builder, arg, source, file, caller_id);
            }
            for kw in &call.keywords {
                visit_expr_calls(builder, &kw.value, source, file, caller_id);
            }
        }
        Expr::Await(await_expr) => {
            visit_expr_calls(builder, &await_expr.value, source, file, caller_id);

            if let Expr::Call(call) = await_expr.value.as_ref() {
                let loc = helpers::location(file, source, await_expr.range.start().to_usize());
                match call.func.as_ref() {
                    Expr::Name(name) => {
                        builder.add_pending(
                            caller_id,
                            &name.id.to_string(),
                            EdgeKind::AwaitCalls,
                            Some(loc.clone()),
                            Some("direct"),
                        );
                    }
                    Expr::Attribute(attr) => {
                        builder.add_pending(
                            caller_id,
                            &attr.attr.to_string(),
                            EdgeKind::AwaitCalls,
                            Some(loc.clone()),
                            Some("method"),
                        );
                    }
                    _ => {
                        builder.add_pending(
                            caller_id,
                            "?",
                            EdgeKind::AwaitCalls,
                            Some(loc),
                            Some("direct"),
                        );
                    }
                }
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
            format!(
                "{}[{}]",
                expr_to_string(&sub.value),
                expr_to_string(&sub.slice)
            )
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
            format!(
                "{} {} {}",
                expr_to_string(&binop.left),
                op_str,
                expr_to_string(&binop.right)
            )
        }
        Expr::UnaryOp(unary) => {
            format!("?{}", expr_to_string(&unary.operand))
        }
        _ => "?".to_string(),
    }
}

fn classify_class(cd: &StmtClassDef) -> NodeKind {
    for dec in &cd.decorator_list {
        if let Some(name) = extract_qualified_name(dec).or_else(|| extract_name(dec)) {
            if name == "dataclass"
                || name == "dataclasses.dataclass"
                || last_name_segment(&name) == "dataclass"
            {
                return NodeKind::DataType;
            }
        }
    }

    for base in &cd.bases {
        if let Some(name) = extract_qualified_name(base).or_else(|| extract_name(base)) {
            match last_name_segment(&name) {
                "Enum" => return NodeKind::Enum,
                "Protocol" => return NodeKind::Protocol,
                "ABC" => return NodeKind::ABC,
                "NamedTuple" => return NodeKind::NamedTuple,
                _ => {}
            }
        }
    }

    for base in &cd.bases {
        if let Expr::Call(call) = base {
            if let Some(name) = extract_qualified_name(call.func.as_ref())
                .or_else(|| extract_name(call.func.as_ref()))
            {
                if last_name_segment(&name) == "namedtuple" {
                    return NodeKind::NamedTuple;
                }
            }
        }
    }

    NodeKind::Class
}

fn classify_callable_kind(name: &str, body: &[Stmt], default_kind: NodeKind) -> NodeKind {
    if has_yield(body) {
        NodeKind::Generator
    } else if is_constructor(name) {
        NodeKind::Constructor
    } else if is_dunder(name) {
        NodeKind::Dunder
    } else {
        default_kind
    }
}

fn classify_class_variable(name: &str) -> NodeKind {
    if is_constant_name(name) {
        NodeKind::Constant
    } else {
        NodeKind::ClassVariable
    }
}

fn has_yield(body: &[Stmt]) -> bool {
    body.iter().any(stmt_contains_yield)
}

fn stmt_contains_yield(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Expr(expr_stmt) => expr_contains_yield(&expr_stmt.value),
        Stmt::Assign(assign) => {
            assign.targets.iter().any(expr_contains_yield) || expr_contains_yield(&assign.value)
        }
        Stmt::AnnAssign(ann) => {
            expr_contains_yield(ann.target.as_ref())
                || expr_contains_yield(&ann.annotation)
                || ann
                    .value
                    .as_ref()
                    .is_some_and(|expr| expr_contains_yield(expr))
        }
        Stmt::AugAssign(assign) => {
            expr_contains_yield(assign.target.as_ref()) || expr_contains_yield(&assign.value)
        }
        Stmt::Return(ret) => ret
            .value
            .as_ref()
            .is_some_and(|expr| expr_contains_yield(expr)),
        Stmt::If(if_stmt) => {
            expr_contains_yield(&if_stmt.test)
                || has_yield(&if_stmt.body)
                || has_yield(&if_stmt.orelse)
        }
        Stmt::For(for_stmt) => {
            expr_contains_yield(for_stmt.target.as_ref())
                || expr_contains_yield(&for_stmt.iter)
                || has_yield(&for_stmt.body)
                || has_yield(&for_stmt.orelse)
        }
        Stmt::AsyncFor(for_stmt) => {
            expr_contains_yield(for_stmt.target.as_ref())
                || expr_contains_yield(&for_stmt.iter)
                || has_yield(&for_stmt.body)
                || has_yield(&for_stmt.orelse)
        }
        Stmt::While(while_stmt) => {
            expr_contains_yield(&while_stmt.test)
                || has_yield(&while_stmt.body)
                || has_yield(&while_stmt.orelse)
        }
        Stmt::With(with_stmt) => {
            with_stmt.items.iter().any(|item| {
                expr_contains_yield(&item.context_expr)
                    || item
                        .optional_vars
                        .as_ref()
                        .is_some_and(|expr| expr_contains_yield(expr))
            }) || has_yield(&with_stmt.body)
        }
        Stmt::AsyncWith(with_stmt) => {
            with_stmt.items.iter().any(|item| {
                expr_contains_yield(&item.context_expr)
                    || item
                        .optional_vars
                        .as_ref()
                        .is_some_and(|expr| expr_contains_yield(expr))
            }) || has_yield(&with_stmt.body)
        }
        Stmt::Try(try_stmt) => {
            has_yield(&try_stmt.body)
                || try_stmt.handlers.iter().any(except_handler_contains_yield)
                || has_yield(&try_stmt.orelse)
                || has_yield(&try_stmt.finalbody)
        }
        Stmt::Match(match_stmt) => {
            expr_contains_yield(&match_stmt.subject)
                || match_stmt.cases.iter().any(|case| {
                    case.guard
                        .as_ref()
                        .is_some_and(|expr| expr_contains_yield(expr))
                        || has_yield(&case.body)
                })
        }
        Stmt::FunctionDef(_) | Stmt::AsyncFunctionDef(_) | Stmt::ClassDef(_) => false,
        _ => false,
    }
}

fn except_handler_contains_yield(handler: &ExceptHandler) -> bool {
    let ExceptHandler::ExceptHandler(handler) = handler;
    handler
        .type_
        .as_ref()
        .is_some_and(|expr| expr_contains_yield(expr))
        || has_yield(&handler.body)
}

fn expr_contains_yield(expr: &Expr) -> bool {
    match expr {
        Expr::Yield(_) | Expr::YieldFrom(_) => true,
        Expr::Await(await_expr) => expr_contains_yield(&await_expr.value),
        Expr::Call(call) => {
            expr_contains_yield(call.func.as_ref())
                || call.args.iter().any(expr_contains_yield)
                || call
                    .keywords
                    .iter()
                    .any(|kw| expr_contains_yield(&kw.value))
        }
        Expr::Attribute(attr) => expr_contains_yield(&attr.value),
        Expr::BinOp(binop) => expr_contains_yield(&binop.left) || expr_contains_yield(&binop.right),
        Expr::UnaryOp(unary) => expr_contains_yield(&unary.operand),
        Expr::BoolOp(boolop) => boolop.values.iter().any(expr_contains_yield),
        Expr::Compare(cmp) => {
            expr_contains_yield(&cmp.left) || cmp.comparators.iter().any(expr_contains_yield)
        }
        Expr::IfExp(ifexp) => {
            expr_contains_yield(&ifexp.test)
                || expr_contains_yield(&ifexp.body)
                || expr_contains_yield(&ifexp.orelse)
        }
        Expr::Subscript(sub) => expr_contains_yield(&sub.value) || expr_contains_yield(&sub.slice),
        Expr::List(list) => list.elts.iter().any(expr_contains_yield),
        Expr::Tuple(tuple) => tuple.elts.iter().any(expr_contains_yield),
        Expr::Set(set) => set.elts.iter().any(expr_contains_yield),
        Expr::Dict(dict) => {
            dict.keys.iter().flatten().any(expr_contains_yield)
                || dict.values.iter().any(expr_contains_yield)
        }
        Expr::ListComp(comp) => expr_contains_yield(&comp.elt),
        Expr::SetComp(comp) => expr_contains_yield(&comp.elt),
        Expr::GeneratorExp(comp) => expr_contains_yield(&comp.elt),
        Expr::DictComp(comp) => expr_contains_yield(&comp.key) || expr_contains_yield(&comp.value),
        Expr::Lambda(lambda) => expr_contains_yield(&lambda.body),
        Expr::NamedExpr(named) => {
            expr_contains_yield(named.target.as_ref()) || expr_contains_yield(&named.value)
        }
        Expr::Starred(star) => expr_contains_yield(&star.value),
        Expr::Slice(slice) => {
            slice
                .lower
                .as_ref()
                .is_some_and(|expr| expr_contains_yield(expr))
                || slice
                    .upper
                    .as_ref()
                    .is_some_and(|expr| expr_contains_yield(expr))
                || slice
                    .step
                    .as_ref()
                    .is_some_and(|expr| expr_contains_yield(expr))
        }
        Expr::FormattedValue(value) => expr_contains_yield(&value.value),
        Expr::JoinedStr(joined) => joined.values.iter().any(expr_contains_yield),
        _ => false,
    }
}

fn visit_instance_variables(
    builder: &mut Builder,
    body: &[Stmt],
    source: &str,
    file: &str,
    class_qname: &str,
    class_id: &str,
) {
    let mut seen = BTreeSet::new();
    let mut vars = Vec::new();
    collect_instance_variables(body, source, &mut vars);

    for (name, line) in vars {
        if !seen.insert(name.clone()) {
            continue;
        }

        let qname = format!("{}.{}", class_qname, name);
        let field_id = builder.add_node(
            NodeKind::InstanceVariable,
            &name,
            &qname,
            Some(file),
            line,
            line,
            helpers::visibility(&name).as_deref(),
            None,
            None,
        );
        builder.edge(class_id, &field_id, EdgeKind::HasField, None, None, None);
    }
}

fn collect_instance_variables(body: &[Stmt], source: &str, out: &mut Vec<(String, usize)>) {
    for stmt in body {
        match stmt {
            Stmt::Assign(assign) => {
                for target in &assign.targets {
                    if let Some(name) = extract_self_attribute_name(target) {
                        let line = helpers::offset_line(source, assign.range.start().to_usize());
                        out.push((name, line));
                    }
                }
                collect_instance_variables_from_expr(&assign.value, source, out);
            }
            Stmt::AnnAssign(ann) => {
                if let Some(name) = extract_self_attribute_name(ann.target.as_ref()) {
                    let line = helpers::offset_line(source, ann.range.start().to_usize());
                    out.push((name, line));
                }
                if let Some(value) = &ann.value {
                    collect_instance_variables_from_expr(value, source, out);
                }
            }
            Stmt::If(if_stmt) => {
                collect_instance_variables(&if_stmt.body, source, out);
                collect_instance_variables(&if_stmt.orelse, source, out);
            }
            Stmt::For(for_stmt) => {
                collect_instance_variables(&for_stmt.body, source, out);
                collect_instance_variables(&for_stmt.orelse, source, out);
            }
            Stmt::AsyncFor(for_stmt) => {
                collect_instance_variables(&for_stmt.body, source, out);
                collect_instance_variables(&for_stmt.orelse, source, out);
            }
            Stmt::While(while_stmt) => {
                collect_instance_variables(&while_stmt.body, source, out);
                collect_instance_variables(&while_stmt.orelse, source, out);
            }
            Stmt::With(with_stmt) => collect_instance_variables(&with_stmt.body, source, out),
            Stmt::AsyncWith(with_stmt) => collect_instance_variables(&with_stmt.body, source, out),
            Stmt::Try(try_stmt) => {
                collect_instance_variables(&try_stmt.body, source, out);
                for handler in &try_stmt.handlers {
                    let ExceptHandler::ExceptHandler(handler) = handler;
                    collect_instance_variables(&handler.body, source, out);
                }
                collect_instance_variables(&try_stmt.orelse, source, out);
                collect_instance_variables(&try_stmt.finalbody, source, out);
            }
            Stmt::Match(match_stmt) => {
                for case in &match_stmt.cases {
                    collect_instance_variables(&case.body, source, out);
                }
            }
            Stmt::FunctionDef(_) | Stmt::AsyncFunctionDef(_) | Stmt::ClassDef(_) => {}
            _ => {}
        }
    }
}

fn collect_instance_variables_from_expr(expr: &Expr, source: &str, out: &mut Vec<(String, usize)>) {
    match expr {
        Expr::IfExp(ifexp) => {
            collect_instance_variables_from_expr(&ifexp.body, source, out);
            collect_instance_variables_from_expr(&ifexp.orelse, source, out);
        }
        Expr::BoolOp(boolop) => {
            for value in &boolop.values {
                collect_instance_variables_from_expr(value, source, out);
            }
        }
        Expr::Call(call) => {
            for arg in &call.args {
                collect_instance_variables_from_expr(arg, source, out);
            }
            for kw in &call.keywords {
                collect_instance_variables_from_expr(&kw.value, source, out);
            }
        }
        Expr::Await(await_expr) => {
            collect_instance_variables_from_expr(&await_expr.value, source, out)
        }
        Expr::List(list) => {
            for elt in &list.elts {
                collect_instance_variables_from_expr(elt, source, out);
            }
        }
        Expr::Tuple(tuple) => {
            for elt in &tuple.elts {
                collect_instance_variables_from_expr(elt, source, out);
            }
        }
        Expr::Dict(dict) => {
            for key in dict.keys.iter().flatten() {
                collect_instance_variables_from_expr(key, source, out);
            }
            for value in &dict.values {
                collect_instance_variables_from_expr(value, source, out);
            }
        }
        Expr::NamedExpr(named) => {
            if let Some(name) = extract_self_attribute_name(named.target.as_ref()) {
                let line = helpers::offset_line(source, named.range.start().to_usize());
                out.push((name, line));
            }
            collect_instance_variables_from_expr(&named.value, source, out);
        }
        _ => {}
    }
}

fn extract_self_attribute_name(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Attribute(attr) => {
            if let Expr::Name(name) = attr.value.as_ref() {
                if name.id.as_str() == "self" {
                    return Some(attr.attr.to_string());
                }
            }
            None
        }
        _ => None,
    }
}

fn maybe_add_override_edges(
    builder: &mut Builder,
    method_id: &str,
    method_name: &str,
    base_names: &[String],
    file: &str,
    source: &str,
    offset: usize,
) {
    if base_names.is_empty() || !is_common_override_method(method_name) {
        return;
    }

    let base_suffixes: Vec<String> = base_names
        .iter()
        .map(|name| format!(".{}.{}", last_name_segment(name), method_name))
        .collect();

    let target_ids: Vec<String> = builder
        .nodes
        .iter()
        .filter(|node| is_method_like(&node.kind) && node.name == method_name)
        .filter(|node| {
            base_suffixes
                .iter()
                .any(|suffix| node.qualified_name.ends_with(suffix))
        })
        .map(|node| node.id.clone())
        .collect();

    let loc = helpers::location(file, source, offset);

    if target_ids.is_empty() {
        builder.add_pending(method_id, method_name, EdgeKind::Overrides, Some(loc), None);
        return;
    }

    for target_id in target_ids {
        builder.edge(
            method_id,
            &target_id,
            EdgeKind::Overrides,
            None,
            Some(loc.clone()),
            None,
        );
    }
}

fn is_method_like(kind: &NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::Method
            | NodeKind::AsyncMethod
            | NodeKind::ClassMethod
            | NodeKind::StaticMethod
            | NodeKind::Property
            | NodeKind::Dunder
            | NodeKind::Constructor
            | NodeKind::Generator
    )
}

fn is_dunder(name: &str) -> bool {
    name.starts_with("__") && name.ends_with("__") && name.len() > 4
}

fn is_constructor(name: &str) -> bool {
    name == "__init__"
}

fn is_constant_name(name: &str) -> bool {
    !name.is_empty()
        && name.chars().any(|ch| ch.is_ascii_uppercase())
        && !name.chars().any(|ch| ch.is_ascii_lowercase())
}

fn is_common_override_method(name: &str) -> bool {
    matches!(
        name,
        "__init__"
            | "__str__"
            | "__repr__"
            | "__eq__"
            | "__ne__"
            | "__lt__"
            | "__le__"
            | "__gt__"
            | "__ge__"
            | "__hash__"
            | "__len__"
            | "__iter__"
            | "__next__"
            | "__enter__"
            | "__exit__"
            | "__call__"
            | "__getitem__"
            | "__setitem__"
    )
}

fn last_name_segment(name: &str) -> &str {
    name.rsplit('.').next().unwrap_or(name)
}

fn extract_qualified_name(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Name(name) => Some(name.id.to_string()),
        Expr::Attribute(attr) => extract_qualified_name(&attr.value)
            .map(|value| format!("{}.{}", value, attr.attr))
            .or_else(|| Some(attr.attr.to_string())),
        Expr::Call(call) => extract_qualified_name(&call.func),
        _ => None,
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
