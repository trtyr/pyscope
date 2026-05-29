use crate::analyzer::helpers;
use crate::model::*;
use std::collections::{BTreeMap, HashMap};

/// Pending edge — call target not yet resolved.
pub struct PendingEdge {
    pub from: String,
    pub to_name: String,
    pub kind: EdgeKind,
    pub location: Option<Location>,
    pub call_style: Option<String>,
}

/// Graph builder — accumulates nodes and edges during indexing.
pub struct Builder {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    pub pending: Vec<PendingEdge>,
    pub warnings: Vec<String>,
    /// Index: qualified_name → node id
    by_qname: HashMap<String, String>,
    /// Index: short name → list of node ids
    by_name: HashMap<String, Vec<String>>,
    /// Counter for unique id generation (base_id → count)
    id_counter: HashMap<String, usize>,
}

impl Builder {
    /// Create an empty builder.
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            edges: Vec::new(),
            pending: Vec::new(),
            warnings: Vec::new(),
            by_qname: HashMap::new(),
            by_name: HashMap::new(),
            id_counter: HashMap::new(),
        }
    }

    /// Add a node with all fields pre-computed.
    ///
    /// Generates a unique ID of the form `kind:qualified_name`.
    /// If a node with the same qualified_name already exists, a `#N`
    /// suffix is appended to disambiguate.
    ///
    /// Returns the assigned node id.
    pub fn add_node(
        &mut self,
        kind: NodeKind,
        name: &str,
        qualified_name: &str,
        file: Option<&str>,
        start_line: usize,
        end_line: usize,
        visibility: Option<&str>,
        signature: Option<&str>,
        docs: Option<&str>,
    ) -> String {
        let base_id = format!("{}:{}", kind.as_str(), qualified_name);

        // Track duplicate qualified names and append #N suffix
        let count = self.id_counter.entry(base_id.clone()).or_insert(0);
        *count += 1;
        let id = if *count > 1 {
            format!("{}#{}", base_id, count)
        } else {
            base_id
        };

        let node = Node {
            id: id.clone(),
            kind,
            name: name.to_string(),
            qualified_name: qualified_name.to_string(),
            file: file.map(|s| s.to_string()),
            range: Some(Range {
                start_line,
                end_line,
            }),
            visibility: visibility.map(|s| s.to_string()),
            signature: signature.map(|s| s.to_string()),
            docs: docs.map(|s| s.to_string()),
            metrics: BTreeMap::new(),
        };

        self.nodes.push(node);

        // Index by qualified name (latest wins — shadows previous)
        self.by_qname
            .insert(qualified_name.to_string(), id.clone());

        // Index by short name
        self.by_name
            .entry(name.to_string())
            .or_default()
            .push(id.clone());

        id
    }

    /// Add a symbol node from byte offsets.
    ///
    /// Converts `start_offset` and `end_offset` to line numbers via
    /// [`helpers::offset_line`], then delegates to [`add_node`].
    ///
    /// Returns the assigned node id.
    #[allow(dead_code)]
    pub fn symbol(
        &mut self,
        kind: NodeKind,
        name: &str,
        qualified_name: &str,
        file: &str,
        source: &str,
        start_offset: usize,
        end_offset: usize,
        visibility: Option<&str>,
        signature: Option<&str>,
        docs: Option<&str>,
    ) -> String {
        let start_line = helpers::offset_line(source, start_offset);
        let end_line = helpers::offset_line(source, end_offset);
        self.add_node(
            kind,
            name,
            qualified_name,
            Some(file),
            start_line,
            end_line,
            visibility,
            signature,
            docs,
        )
    }

    /// Create an edge between two nodes.
    ///
    /// Uses `EdgeSource::Ast`, `EdgeCertainty::Definite`, weight 1.
    pub fn edge(
        &mut self,
        from_id: &str,
        to_id: &str,
        kind: EdgeKind,
        label: Option<&str>,
        location: Option<Location>,
        call_style: Option<&str>,
    ) {
        self.edge_with_source(
            from_id,
            to_id,
            kind,
            label,
            location,
            EdgeSource::Ast,
            EdgeCertainty::Definite,
            call_style,
        );
    }

    /// Create an edge with configurable source and certainty.
    pub fn edge_with_source(
        &mut self,
        from_id: &str,
        to_id: &str,
        kind: EdgeKind,
        label: Option<&str>,
        location: Option<Location>,
        source: EdgeSource,
        certainty: EdgeCertainty,
        call_style: Option<&str>,
    ) {
        let edge = Edge {
            from: from_id.to_string(),
            to: to_id.to_string(),
            kind,
            label: label.map(|s| s.to_string()),
            evidence: location,
            weight: 1,
            source,
            certainty,
            call_style: call_style.map(|s| s.to_string()),
        };
        self.edges.push(edge);
    }

    /// Register a pending edge whose target will be resolved later.
    pub fn add_pending(
        &mut self,
        from_id: &str,
        to_name: &str,
        kind: EdgeKind,
        location: Option<Location>,
        call_style: Option<&str>,
    ) {
        self.pending.push(PendingEdge {
            from: from_id.to_string(),
            to_name: to_name.to_string(),
            kind,
            location,
            call_style: call_style.map(|s| s.to_string()),
        });
    }

    /// Resolve all pending edges.
    ///
    /// For each pending edge:
    /// - If the target is found unambiguously, create the edge.
    /// - If ambiguous (multiple matches for a short name), skip with a warning.
    /// - If not found, skip silently.
    ///
    /// Clears the pending list when done.
    pub fn resolve_pending(&mut self) {
        let pending = std::mem::take(&mut self.pending);

        for pe in pending {
            // Try exact qualified-name lookup first
            if let Some(to_id) = self.by_qname.get(&pe.to_name).cloned() {
                self.edge_with_source(
                    &pe.from,
                    &to_id,
                    pe.kind.clone(),
                    None,
                    pe.location.clone(),
                    EdgeSource::Ast,
                    EdgeCertainty::Inferred,
                    pe.call_style.as_deref(),
                );
                continue;
            }

            // Try short-name lookup
            match self.by_name.get(&pe.to_name).cloned() {
                Some(ids) if ids.len() == 1 => {
                    self.edge_with_source(
                        &pe.from,
                        &ids[0],
                        pe.kind.clone(),
                        None,
                        pe.location.clone(),
                        EdgeSource::Ast,
                        EdgeCertainty::Possible,
                        pe.call_style.as_deref(),
                    );
                }
                Some(ids) if ids.len() > 1 => {
                    self.warnings.push(format!(
                        "ambiguous target '{}' matches {} nodes — skipped",
                        pe.to_name,
                        ids.len()
                    ));
                }
                _ => {
                    // Not found — skip silently
                }
            }
        }
    }

    /// Look up a node id by qualified name.
    #[allow(dead_code)]
    pub fn find_node_id(&self, qualified_name: &str) -> Option<&String> {
        self.by_qname.get(qualified_name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builder_new() {
        let b = Builder::new();
        assert!(b.nodes.is_empty());
        assert!(b.edges.is_empty());
        assert!(b.pending.is_empty());
        assert!(b.warnings.is_empty());
    }

    #[test]
    fn test_add_node_unique() {
        let mut b = Builder::new();
        let id = b.add_node(
            NodeKind::Function,
            "foo",
            "pkg.mod.foo",
            Some("mod.py"),
            1,
            5,
            None,
            None,
            None,
        );
        assert_eq!(id, "function:pkg.mod.foo");
        assert_eq!(b.nodes.len(), 1);
        assert_eq!(b.nodes[0].id, "function:pkg.mod.foo");
    }

    #[test]
    fn test_add_node_duplicate_suffix() {
        let mut b = Builder::new();
        let id1 = b.add_node(
            NodeKind::Function,
            "foo",
            "pkg.foo",
            None,
            1,
            1,
            None,
            None,
            None,
        );
        let id2 = b.add_node(
            NodeKind::Function,
            "foo",
            "pkg.foo",
            None,
            10,
            10,
            None,
            None,
            None,
        );
        assert_eq!(id1, "function:pkg.foo");
        assert_eq!(id2, "function:pkg.foo#2");
    }

    #[test]
    fn test_symbol_converts_offsets() {
        let mut b = Builder::new();
        let src = "line1\nline2\nline3\n";
        let id = b.symbol(
            NodeKind::Function,
            "bar",
            "mod.bar",
            "mod.py",
            src,
            6,  // start of line 2
            11, // end of line 2
            None,
            None,
            None,
        );
        assert_eq!(id, "function:mod.bar");
        assert_eq!(b.nodes[0].range.as_ref().unwrap().start_line, 2);
        assert_eq!(b.nodes[0].range.as_ref().unwrap().end_line, 2);
    }

    #[test]
    fn test_edge_creation() {
        let mut b = Builder::new();
        b.add_node(NodeKind::Function, "a", "mod.a", None, 1, 1, None, None, None);
        b.add_node(NodeKind::Function, "b", "mod.b", None, 5, 5, None, None, None);
        b.edge("function:mod.a", "function:mod.b", EdgeKind::Calls, None, None, None);
        assert_eq!(b.edges.len(), 1);
        assert_eq!(b.edges[0].from, "function:mod.a");
        assert_eq!(b.edges[0].to, "function:mod.b");
        assert_eq!(b.edges[0].source, EdgeSource::Ast);
        assert_eq!(b.edges[0].certainty, EdgeCertainty::Definite);
        assert_eq!(b.edges[0].weight, 1);
    }

    #[test]
    fn test_resolve_pending_found() {
        let mut b = Builder::new();
        b.add_node(NodeKind::Function, "caller", "mod.caller", None, 1, 1, None, None, None);
        b.add_node(NodeKind::Function, "callee", "mod.callee", None, 10, 10, None, None, None);
        b.add_pending(
            "function:mod.caller",
            "mod.callee",
            EdgeKind::Calls,
            None,
            None,
        );
        b.resolve_pending();
        assert!(b.pending.is_empty());
        assert_eq!(b.edges.len(), 1);
        assert_eq!(b.edges[0].to, "function:mod.callee");
    }

    #[test]
    fn test_resolve_pending_ambiguous() {
        let mut b = Builder::new();
        b.add_node(NodeKind::Function, "caller", "mod.caller", None, 1, 1, None, None, None);
        b.add_node(NodeKind::Function, "foo", "a.foo", None, 10, 10, None, None, None);
        b.add_node(NodeKind::Function, "foo", "b.foo", None, 20, 20, None, None, None);
        b.add_pending("function:mod.caller", "foo", EdgeKind::Calls, None, None);
        b.resolve_pending();
        assert!(b.pending.is_empty());
        assert!(b.edges.is_empty());
        assert_eq!(b.warnings.len(), 1);
        assert!(b.warnings[0].contains("ambiguous"));
    }

    #[test]
    fn test_resolve_pending_not_found() {
        let mut b = Builder::new();
        b.add_node(NodeKind::Function, "caller", "mod.caller", None, 1, 1, None, None, None);
        b.add_pending(
            "function:mod.caller",
            "nonexistent",
            EdgeKind::Calls,
            None,
            None,
        );
        b.resolve_pending();
        assert!(b.pending.is_empty());
        assert!(b.edges.is_empty());
        assert!(b.warnings.is_empty());
    }

    #[test]
    fn test_find_node_id() {
        let mut b = Builder::new();
        b.add_node(NodeKind::Class, "MyClass", "pkg.MyClass", None, 1, 50, None, None, None);
        assert!(b.find_node_id("pkg.MyClass").is_some());
        assert!(b.find_node_id("other").is_none());
    }
}
