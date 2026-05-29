use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

// ---- Graph ----

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CodeGraph {
    pub schema_version: u32,
    pub project: Project,
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    pub semantic: Option<SemanticInfo>,
    pub warnings: Vec<String>,
    pub generated_at_ms: u128,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Project {
    pub root: String,
    pub packages: Vec<Package>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Package {
    pub name: String,
    pub root: String,
    pub files: Vec<String>,
}

// ---- Node ----

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Node {
    pub id: String,
    pub kind: NodeKind,
    pub name: String,
    pub qualified_name: String,
    pub file: Option<String>,
    pub range: Option<Range>,
    pub visibility: Option<String>,
    pub signature: Option<String>,
    pub docs: Option<String>,
    pub metrics: BTreeMap<String, usize>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum NodeKind {
    Project,
    File,
    Module,
    Package,
    Function,
    Method,
    AsyncFunction,
    AsyncMethod,
    Class,
    ClassMethod,
    StaticMethod,
    Variable,
    Field,
    Property,
    Decorator,
    Import,
    Enum,
    NamedTuple,
    DataType,
    TypeAlias,
    Protocol,
    ABC,
    Generator,
    Dunder,
    Constructor,
    ClassVariable,
    InstanceVariable,
    Constant,
    Unknown,
}

impl NodeKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Project => "project",
            Self::File => "file",
            Self::Module => "module",
            Self::Package => "package",
            Self::Function => "function",
            Self::Method => "method",
            Self::AsyncFunction => "async_function",
            Self::AsyncMethod => "async_method",
            Self::Class => "class",
            Self::ClassMethod => "class_method",
            Self::StaticMethod => "static_method",
            Self::Variable => "variable",
            Self::Field => "field",
            Self::Property => "property",
            Self::Decorator => "decorator",
            Self::Import => "import",
            Self::Enum => "enum",
            Self::NamedTuple => "named_tuple",
            Self::DataType => "data_type",
            Self::TypeAlias => "type_alias",
            Self::Protocol => "protocol",
            Self::ABC => "abc",
            Self::Generator => "generator",
            Self::Dunder => "dunder",
            Self::Constructor => "constructor",
            Self::ClassVariable => "class_variable",
            Self::InstanceVariable => "instance_variable",
            Self::Constant => "constant",
            Self::Unknown => "unknown",
        }
    }
}

// ---- Edge ----

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Edge {
    pub from: String,
    pub to: String,
    pub kind: EdgeKind,
    pub label: Option<String>,
    pub evidence: Option<Location>,
    pub weight: usize,
    pub source: EdgeSource,
    pub certainty: EdgeCertainty,
    pub call_style: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EdgeKind {
    Contains,
    Declares,
    Imports,
    Calls,
    HasMethod,
    HasField,
    InheritsFrom,
    UsesType,
    ModuleFile,
    AwaitCalls,
    Decorates,
    FromImports,
    Overrides,
    Implements,
    Returns,
    Mixin,
    Describes,
}

impl EdgeKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Contains => "contains",
            Self::Declares => "declares",
            Self::Imports => "imports",
            Self::Calls => "calls",
            Self::HasMethod => "has_method",
            Self::HasField => "has_field",
            Self::InheritsFrom => "inherits_from",
            Self::UsesType => "uses_type",
            Self::ModuleFile => "module_file",
            Self::AwaitCalls => "await_calls",
            Self::Decorates => "decorates",
            Self::FromImports => "from_imports",
            Self::Overrides => "overrides",
            Self::Implements => "implements",
            Self::Returns => "returns",
            Self::Mixin => "mixin",
            Self::Describes => "describes",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EdgeSource {
    Ast,
    Inferred,
}

impl EdgeSource {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Ast => "ast",
            Self::Inferred => "inferred",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EdgeCertainty {
    Definite,
    Inferred,
    Possible,
}

impl EdgeCertainty {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Definite => "definite",
            Self::Inferred => "inferred",
            Self::Possible => "possible",
        }
    }
}

// ---- Common ----

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Range {
    pub start_line: usize,
    pub end_line: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Location {
    pub file: String,
    pub line: usize,
}

// ---- Stats ----

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GraphStats {
    pub nodes: usize,
    pub edges: usize,
    pub by_kind: BTreeMap<String, usize>,
    pub by_edge: BTreeMap<String, usize>,
    pub by_source: BTreeMap<String, usize>,
    pub by_certainty: BTreeMap<String, usize>,
    pub files: usize,
    pub symbols: usize,
    pub warnings: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SemanticInfo {
    pub enabled: bool,
    pub provider: String,
    pub scanned_symbols: usize,
    pub enriched_symbols: usize,
    pub confirmed_symbols: usize,
    pub enriched_edges: usize,
    pub confirmed_edges: usize,
    pub unresolved_items: usize,
    pub warnings: Vec<String>,
}

impl CodeGraph {
    pub fn stats(&self) -> GraphStats {
        let mut by_kind = BTreeMap::new();
        let mut by_edge = BTreeMap::new();
        let mut by_source = BTreeMap::new();
        let mut by_certainty = BTreeMap::new();
        for node in &self.nodes {
            *by_kind.entry(node.kind.as_str().to_string()).or_insert(0) += 1;
        }
        for edge in &self.edges {
            *by_edge.entry(edge.kind.as_str().to_string()).or_insert(0) += 1;
            *by_source
                .entry(edge.source.as_str().to_string())
                .or_insert(0) += 1;
            *by_certainty
                .entry(edge.certainty.as_str().to_string())
                .or_insert(0) += 1;
        }
        GraphStats {
            nodes: self.nodes.len(),
            edges: self.edges.len(),
            by_kind,
            by_edge,
            by_source,
            by_certainty,
            files: self
                .nodes
                .iter()
                .filter(|node| node.kind == NodeKind::File)
                .count(),
            symbols: self
                .nodes
                .iter()
                .filter(|node| !matches!(node.kind, NodeKind::Project | NodeKind::File))
                .count(),
            warnings: self.warnings.len(),
        }
    }
}
