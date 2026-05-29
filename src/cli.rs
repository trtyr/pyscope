use clap::{Parser, Subcommand, Args};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "pyscope")]
#[command(about = "Python code satellite map — index, query, and navigate your codebase")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Build the code graph for a Python project
    Index(IndexArgs),
    /// Start the web viewer
    Serve(ServeArgs),
    /// Query the indexed graph
    #[command(subcommand)]
    Query(QueryCmd),
    /// AI-oriented navigation
    #[command(subcommand)]
    Nav(NavCmd),
    /// Static analysis
    #[command(subcommand)]
    Analyze(AnalyzeCmd),
    /// Configure API keys and model settings
    #[command(subcommand)]
    Config(ConfigCmd),
}

// ---- Index ----

#[derive(Args)]
pub struct IndexArgs {
    /// Path to the Python project
    #[arg(default_value = ".")]
    pub project: PathBuf,
    /// Custom output path for the graph file
    #[arg(long)]
    pub output: Option<PathBuf>,
    /// Exclude test directories (test/, tests/, *_test.py)
    #[arg(long)]
    pub no_tests: bool,
    /// Enable semantic enrichment via Python LSP
    #[arg(long)]
    pub semantic: bool,
    /// Disable semantic enrichment via Python LSP
    #[arg(long)]
    pub no_semantic: bool,
}

// ---- Serve ----

#[derive(Args)]
pub struct ServeArgs {
    /// Path to the Python project
    #[arg(default_value = ".")]
    pub project: PathBuf,
    /// Port to listen on
    #[arg(long, default_value = "7878")]
    pub port: u16,
    /// Host to bind to
    #[arg(long, default_value = "127.0.0.1")]
    pub host: String,
}

// ---- Config ----

#[derive(Subcommand)]
pub enum ConfigCmd {
    /// Show current configuration
    Show,
    /// Set API key for LLM features
    SetApiKey(ConfigValueArg),
    /// Set chat model
    SetModel(ConfigValueArg),
    /// Set embedding API key
    SetEmbeddingKey(ConfigValueArg),
}

#[derive(Args)]
pub struct ConfigValueArg {
    /// Value to persist in config
    pub value: String,
}

// ---- Query ----

#[derive(Subcommand)]
pub enum QueryCmd {
    /// Inspect a symbol: details + source code
    Inspect(InspectArgs),
    /// Trace call relationships: upstream or downstream
    Trace(TraceArgs),
    /// Find symbols by pattern or similarity
    Find(FindArgs),
    /// Show symbols scoped to a file or module
    Scope(ScopeArgs),
    /// Show raw source code for a symbol
    Source(SourceArgs),
    /// Find structurally similar symbols
    Similar(SimilarArgs),
    /// Export graph to DOT/Mermaid/JSON
    Export(ExportArgs),
    /// Shortest call path between two symbols
    Path(PathArgs),
    /// List symbols with optional filters
    Symbols(SymbolsArgs),
    /// Full dependency impact analysis
    Impact(ImpactArgs),
}

#[derive(Args)]
pub struct InspectArgs {
    /// Symbol name (qualified: pkg.module.name, or short: name)
    pub name: String,
    /// Graph file to query
    #[arg(long)]
    pub graph: Option<PathBuf>,
    /// Disable source code output
    #[arg(long)]
    pub no_source: bool,
}

#[derive(Args)]
pub struct TraceArgs {
    /// Symbol name
    pub name: String,
    /// Direction: up (callers), down (callees), both (default)
    #[arg(long, default_value = "both")]
    pub direction: String,
    /// Maximum traversal depth
    #[arg(long, default_value = "3")]
    pub depth: usize,
    /// Maximum results
    #[arg(long, default_value = "100")]
    pub limit: usize,
    /// Graph file to query
    #[arg(long)]
    pub graph: Option<PathBuf>,
}

#[derive(Args)]
pub struct FindArgs {
    /// Search pattern
    pub pattern: String,
    /// Search mode: text or similar
    #[arg(long, default_value = "text")]
    pub mode: String,
    /// Maximum results
    #[arg(long, default_value = "50")]
    pub limit: usize,
    /// Graph file to query
    #[arg(long)]
    pub graph: Option<PathBuf>,
}

#[derive(Args)]
pub struct ScopeArgs {
    /// Target path or module name
    pub target: String,
    /// Scope kind: file or module
    #[arg(long, default_value = "file")]
    pub kind: String,
    /// Graph file to query
    #[arg(long)]
    pub graph: Option<PathBuf>,
}

#[derive(Args)]
pub struct SourceArgs {
    /// Symbol name
    pub name: String,
    /// Graph file to query
    #[arg(long)]
    pub graph: Option<PathBuf>,
}

#[derive(Args)]
pub struct SimilarArgs {
    /// Symbol name to find similar functions for
    pub name: String,
    /// Maximum results
    #[arg(long, default_value = "20")]
    pub limit: usize,
    /// Graph file to query
    #[arg(long)]
    pub graph: Option<PathBuf>,
}

#[derive(Args)]
pub struct ExportArgs {
    /// Export format: dot, mermaid, or json
    #[arg(long, default_value = "json")]
    pub format: String,
    /// Graph file to query
    #[arg(long)]
    pub graph: Option<PathBuf>,
}

#[derive(Args)]
pub struct PathArgs {
    /// Starting symbol
    pub from: String,
    /// Target symbol
    pub to: String,
    /// Maximum search depth
    #[arg(long, default_value = "10")]
    pub depth: usize,
    /// Graph file to query
    #[arg(long)]
    pub graph: Option<PathBuf>,
}

#[derive(Args)]
pub struct SymbolsArgs {
    /// Filter by node kind (function, class, module, etc.)
    #[arg(long)]
    pub kind: Option<String>,
    /// Maximum results
    #[arg(long, default_value = "100")]
    pub limit: usize,
    /// Filter by visibility (public, private, _prefix)
    #[arg(long)]
    pub visibility: Option<String>,
    /// Only show symbols without docstrings
    #[arg(long)]
    pub no_docs: bool,
    /// Only show symbols with no callers (dead code)
    #[arg(long)]
    pub dead: bool,
    /// Only show symbols called exclusively from test files
    #[arg(long)]
    pub test_only: bool,
    /// Minimum number of unique callers
    #[arg(long)]
    pub min_callers: Option<usize>,
    /// Maximum number of unique callers
    #[arg(long)]
    pub max_callers: Option<usize>,
    /// Minimum total degree
    #[arg(long)]
    pub min_degree: Option<usize>,
    /// Maximum total degree
    #[arg(long)]
    pub max_degree: Option<usize>,
    /// Graph file to query
    #[arg(long)]
    pub graph: Option<PathBuf>,
}

#[derive(Args)]
pub struct ImpactArgs {
    /// Symbol name
    pub name: String,
    /// Maximum traversal depth
    #[arg(long, default_value = "3")]
    pub depth: usize,
    /// Maximum results
    #[arg(long, default_value = "200")]
    pub limit: usize,
    /// Graph file to query
    #[arg(long)]
    pub graph: Option<PathBuf>,
}

// ---- Nav ----

#[derive(Subcommand)]
pub enum NavCmd {
    /// Token-budgeted project overview for AI agents
    Map(MapArgs),
    /// Ask the LLM a question about the codebase
    Ask(AskArgs),
    /// Retrieve relevant nodes using lexical and optional embedding search
    Retrieve(RetrieveArgs),
    /// Architecture health: cycles, god modules, dead code
    Health(HealthArgs),
    /// Generate a markdown report
    Report(ReportArgs),
}

#[derive(Args)]
pub struct MapArgs {
    /// Include entry points and feature clusters (default: hot symbols only)
    #[arg(long)]
    pub full: bool,
    /// Token budget for the map content
    #[arg(long, default_value = "8000")]
    pub budget: usize,
    /// Graph file to query
    #[arg(long)]
    pub graph: Option<PathBuf>,
}

#[derive(Args)]
pub struct AskArgs {
    /// Natural-language question about the codebase
    pub question: String,
    /// Graph file to query
    #[arg(long)]
    pub graph: Option<PathBuf>,
}

#[derive(Args)]
pub struct RetrieveArgs {
    /// Search query
    pub query: String,
    /// Maximum results
    #[arg(long, default_value = "10")]
    pub limit: usize,
    /// Graph file to query
    #[arg(long)]
    pub graph: Option<PathBuf>,
}

#[derive(Args)]
pub struct HealthArgs {
    /// Maximum items per category
    #[arg(long, default_value = "10")]
    pub limit: usize,
    /// Graph file to query
    #[arg(long)]
    pub graph: Option<PathBuf>,
}

#[derive(Args)]
pub struct ReportArgs {
    /// Graph file to query
    #[arg(long)]
    pub graph: Option<PathBuf>,
}

// ---- Analyze ----

#[derive(Subcommand)]
pub enum AnalyzeCmd {
    /// Module dependency matrix
    Deps(DepsArgs),
    /// File-level fan-in / fan-out
    Fanout(FanoutArgs),
    /// Test impact analysis: which tests to run
    Tests(TestsArgs),
}

#[derive(Args)]
pub struct DepsArgs {
    /// Filter to dependencies from this module
    #[arg(long)]
    pub from: Option<String>,
    /// Maximum results
    #[arg(long, default_value = "100")]
    pub limit: usize,
    /// Graph file to query
    #[arg(long)]
    pub graph: Option<PathBuf>,
}

#[derive(Args)]
pub struct FanoutArgs {
    /// Maximum results
    #[arg(long, default_value = "50")]
    pub limit: usize,
    /// Graph file to query
    #[arg(long)]
    pub graph: Option<PathBuf>,
}

#[derive(Args)]
pub struct TestsArgs {
    /// Target symbol to analyze (optional, falls back to static discovery)
    pub symbol: Option<String>,
    /// Maximum test candidates
    #[arg(long, default_value = "20")]
    pub limit: usize,
    /// Graph file to query
    #[arg(long)]
    pub graph: Option<PathBuf>,
}
