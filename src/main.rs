mod analyze;
mod analyzer;
mod ci;
mod cli;
mod config;
mod llm;
mod model;
mod nav;
mod query;
mod rag;
mod report;
mod semantic;
mod store;

use anyhow::Result;
use clap::Parser;

fn main() -> Result<()> {
    let cli = cli::Cli::parse();

    match cli.command {
        // ---- Index ----
        cli::Command::Index(args) => {
            let mut graph = crate::analyzer::index::index_project(&args.project, !args.no_tests)?;
            let semantic_enabled = args.semantic || !args.no_semantic;
            if semantic_enabled {
                let semantic = crate::semantic::enrich(&mut graph, &args.project, 200)?;
                graph.semantic = Some(semantic);
            } else {
                graph.semantic = Some(crate::model::SemanticInfo {
                    enabled: false,
                    provider: String::new(),
                    scanned_symbols: 0,
                    enriched_symbols: 0,
                    confirmed_symbols: 0,
                    enriched_edges: 0,
                    confirmed_edges: 0,
                    unresolved_items: 0,
                    warnings: Vec::new(),
                });
            }
            let stats = graph.stats();
            let output = crate::store::save_project(&args.project, args.output.as_deref(), &graph)?;
            println!(
                "{}",
                serde_json::json!({
                    "kind": "index",
                    "output": output.display().to_string(),
                    "project": args.project.display().to_string(),
                    "stats": {
                        "nodes": stats.nodes,
                        "edges": stats.edges,
                        "files": stats.files,
                        "symbols": stats.symbols,
                        "by_kind": stats.by_kind,
                        "by_edge": stats.by_edge,
                        "by_source": stats.by_source,
                        "by_certainty": stats.by_certainty,
                        "warnings": stats.warnings,
                        "semantic": graph.semantic,
                    }
                })
            );
        }

        // ---- Serve (stub) ----
        cli::Command::Serve(_args) => {
            println!(
                "{}",
                serde_json::json!({"kind": "error", "message": "serve not yet implemented"})
            );
        }

        // ---- Config ----
        cli::Command::Config(cmd) => {
            let result = match cmd {
                cli::ConfigCmd::Show => crate::config::show()?,
                cli::ConfigCmd::SetApiKey(args) => crate::config::set_api_key(&args.value)?,
                cli::ConfigCmd::SetModel(args) => crate::config::set_model(&args.value)?,
                cli::ConfigCmd::SetEmbeddingKey(args) => {
                    crate::config::set_embedding_key(&args.value)?
                }
            };
            println!("{}", serde_json::to_string_pretty(&result)?);
        }

        // ---- CI ----
        cli::Command::Ci(cmd) => match cmd {
            cli::CiCmd::Check(args) => {
                let graph = crate::store::load(args.graph.as_deref())?;
                let result = crate::ci::check::check(
                    &graph,
                    args.min_health.unwrap_or(70),
                    args.max_cycles.unwrap_or(0),
                    args.max_god_modules.unwrap_or(0),
                    args.max_dead_code.unwrap_or(10),
                )?;
                let passed = result["passed"].as_bool().unwrap_or(false);
                println!("{}", serde_json::to_string_pretty(&result)?);
                if !passed {
                    std::process::exit(1);
                }
            }
        },

        // ---- Query ----
        cli::Command::Query(cmd) => match cmd {
            cli::QueryCmd::Inspect(args) => {
                let graph = crate::store::load(args.graph.as_deref())?;
                let result = crate::query::inspect(&graph, &args.name, !args.no_source)?;
                println!("{}", serde_json::to_string_pretty(&result)?);
            }
            cli::QueryCmd::Trace(args) => {
                let graph = crate::store::load(args.graph.as_deref())?;
                let direction = match args.direction.as_str() {
                    "up" => crate::query::TraceDirection::Up,
                    "down" => crate::query::TraceDirection::Down,
                    _ => crate::query::TraceDirection::Both,
                };
                let result =
                    crate::query::trace(&graph, &args.name, direction, args.depth, args.limit)?;
                println!("{}", serde_json::to_string_pretty(&result)?);
            }
            cli::QueryCmd::Find(args) => {
                let graph = crate::store::load(args.graph.as_deref())?;
                let mode = match args.mode.as_str() {
                    "text" => crate::query::FindMode::Text,
                    "similar" => crate::query::FindMode::Similar,
                    _ => crate::query::FindMode::Text,
                };
                let result = crate::query::find(&graph, &args.pattern, mode, args.limit);
                println!("{}", serde_json::to_string_pretty(&result)?);
            }
            cli::QueryCmd::Scope(args) => {
                let graph = crate::store::load(args.graph.as_deref())?;
                let kind = match args.kind.as_str() {
                    "module" => crate::query::ScopeKind::Module,
                    _ => crate::query::ScopeKind::File,
                };
                let result = crate::query::scope(&graph, &args.target, kind)?;
                println!("{}", serde_json::to_string_pretty(&result)?);
            }
            cli::QueryCmd::Source(args) => {
                let graph = crate::store::load(args.graph.as_deref())?;
                let result = crate::query::source(&graph, &args.name)?;
                println!("{}", serde_json::to_string_pretty(&result)?);
            }
            cli::QueryCmd::Similar(args) => {
                let graph = crate::store::load(args.graph.as_deref())?;
                let result = crate::query::similar(&graph, &args.name, args.limit);
                println!("{}", serde_json::to_string_pretty(&result)?);
            }
            cli::QueryCmd::Export(args) => {
                let graph = crate::store::load(args.graph.as_deref())?;
                let format = match args.format.as_str() {
                    "dot" => crate::query::ExportFormat::Dot,
                    "mermaid" => crate::query::ExportFormat::Mermaid,
                    _ => crate::query::ExportFormat::Json,
                };
                let result = crate::query::export(&graph, format)?;
                println!("{}", serde_json::to_string_pretty(&result)?);
            }
            cli::QueryCmd::Path(args) => {
                let graph = crate::store::load(args.graph.as_deref())?;
                let result = crate::query::shortest_path(&graph, &args.from, &args.to, args.depth);
                println!("{}", serde_json::to_string_pretty(&result)?);
            }
            cli::QueryCmd::Symbols(args) => {
                let graph = crate::store::load(args.graph.as_deref())?;
                let filter = crate::query::SymbolFilter {
                    visibility: args.visibility,
                    no_docs: args.no_docs,
                    dead: args.dead,
                    test_only: args.test_only,
                    dynamic: args.dynamic,
                    legacy: args.legacy,
                    async_only: args.async_only,
                    decorator: args.decorator,
                    min_callers: args.min_callers,
                    max_callers: args.max_callers,
                    min_degree: args.min_degree,
                    max_degree: args.max_degree,
                };
                let result =
                    crate::query::symbols(&graph, args.kind.as_deref(), args.limit, filter);
                println!("{}", serde_json::to_string_pretty(&result)?);
            }
            cli::QueryCmd::Impact(args) => {
                let graph = crate::store::load(args.graph.as_deref())?;
                let result = crate::query::impact(&graph, &args.name, args.depth, args.limit)?;
                println!("{}", serde_json::to_string_pretty(&result)?);
            }
            cli::QueryCmd::Risk(args) => {
                let graph = crate::store::load(args.graph.as_deref())?;
                let result = crate::query::risk(&graph, &args.name, args.depth, args.limit)?;
                println!("{}", serde_json::to_string_pretty(&result)?);
            }
        },

        // ---- Nav ----
        cli::Command::Nav(cmd) => match cmd {
            cli::NavCmd::Map(args) => {
                let graph = crate::store::load(args.graph.as_deref())?;
                let result = crate::nav::nav_map(&graph, args.full, args.budget)?;
                println!("{}", serde_json::to_string_pretty(&result)?);
            }
            cli::NavCmd::Guide(args) => {
                let graph = crate::store::load(args.graph.as_deref())?;
                let result = crate::nav::guide(&graph, args.limit);
                println!("{}", serde_json::to_string_pretty(&result)?);
            }
            cli::NavCmd::Entries(args) => {
                let graph = crate::store::load(args.graph.as_deref())?;
                let result = crate::nav::entries(&graph, args.limit);
                println!("{}", serde_json::to_string_pretty(&result)?);
            }
            cli::NavCmd::Clusters(args) => {
                let graph = crate::store::load(args.graph.as_deref())?;
                let result = crate::nav::clusters(&graph, args.limit);
                println!("{}", serde_json::to_string_pretty(&result)?);
            }
            cli::NavCmd::Quality(args) => {
                let graph = crate::store::load(args.graph.as_deref())?;
                let result = crate::nav::quality(&graph);
                println!("{}", serde_json::to_string_pretty(&result)?);
            }
            cli::NavCmd::Ask(args) => {
                let graph = crate::store::load(args.graph.as_deref())?;
                let result = crate::llm::ask(&graph, &args.question)?;
                println!("{}", serde_json::to_string_pretty(&result)?);
            }
            cli::NavCmd::Retrieve(args) => {
                let graph = crate::store::load(args.graph.as_deref())?;
                let result = crate::rag::retrieve(&graph, &args.query, args.limit)?;
                println!("{}", serde_json::to_string_pretty(&result)?);
            }
            cli::NavCmd::Health(args) => {
                let graph = crate::store::load(args.graph.as_deref())?;
                let result = crate::nav::health(&graph, args.limit);
                println!("{}", serde_json::to_string_pretty(&result)?);
            }
            cli::NavCmd::Report(args) => {
                let graph = crate::store::load(args.graph.as_deref())?;
                let result = crate::report::report(&graph)?;
                println!("{}", serde_json::to_string_pretty(&result)?);
            }
        },

        // ---- Analyze ----
        cli::Command::Analyze(cmd) => match cmd {
            cli::AnalyzeCmd::Deps(args) => {
                let graph = crate::store::load(args.graph.as_deref())?;
                let result = crate::analyze::deps(&graph, args.from.as_deref(), args.limit);
                println!("{}", serde_json::to_string_pretty(&result)?);
            }
            cli::AnalyzeCmd::Fanout(args) => {
                let graph = crate::store::load(args.graph.as_deref())?;
                let result = crate::analyze::fanout(&graph, args.limit);
                println!("{}", serde_json::to_string_pretty(&result)?);
            }
            cli::AnalyzeCmd::Tests(args) => {
                let graph = crate::store::load(args.graph.as_deref())?;
                let result =
                    crate::analyze::test_impact(&graph, args.symbol.as_deref(), args.limit);
                println!("{}", serde_json::to_string_pretty(&result)?);
            }
            cli::AnalyzeCmd::Hotspots(args) => {
                let graph = crate::store::load(args.graph.as_deref())?;
                let result = crate::analyze::hotspots(
                    &graph,
                    args.limit,
                    args.since.as_deref(),
                    args.project_root.as_deref(),
                );
                println!("{}", serde_json::to_string_pretty(&result)?);
            }
            cli::AnalyzeCmd::Diff(args) => {
                let graph = crate::store::load(args.graph.as_deref())?;
                let result = crate::analyze::diff(&graph, &args.base, args.project_root.as_deref());
                println!("{}", serde_json::to_string_pretty(&result)?);
            }
            cli::AnalyzeCmd::RefactorOrder(args) => {
                let graph = crate::store::load(args.graph.as_deref())?;
                let result = crate::analyze::refactor_order(&graph, &args.modules, args.limit);
                println!("{}", serde_json::to_string_pretty(&result)?);
            }
            cli::AnalyzeCmd::TypeCoverage(args) => {
                let graph = crate::store::load(args.graph.as_deref())?;
                let result = crate::analyze::type_coverage(&graph, args.limit);
                println!("{}", serde_json::to_string_pretty(&result)?);
            }
            cli::AnalyzeCmd::AsyncMap(args) => {
                let graph = crate::store::load(args.graph.as_deref())?;
                let result = crate::analyze::async_map(&graph, args.limit);
                println!("{}", serde_json::to_string_pretty(&result)?);
            }
            cli::AnalyzeCmd::DecoratorUsage(args) => {
                let graph = crate::store::load(args.graph.as_deref())?;
                let result = crate::analyze::decorator_usage(&graph, args.limit);
                println!("{}", serde_json::to_string_pretty(&result)?);
            }
        },
    }
    Ok(())
}
