use crate::model::{EdgeKind, Node, NodeKind};
use super::index::QueryIndex;

pub struct SymbolFilter {
    pub visibility: Option<String>,
    pub no_docs: bool,
    pub dead: bool,
    pub test_only: bool,
    pub min_callers: Option<usize>,
    pub max_callers: Option<usize>,
    pub min_degree: Option<usize>,
    pub max_degree: Option<usize>,
}

impl SymbolFilter {
    /// Check whether a node passes all active filter criteria.
    pub fn matches(&self, node: &Node, index: &QueryIndex) -> bool {
        // visibility: exact match
        if let Some(ref vis) = self.visibility {
            if node.visibility.as_deref() != Some(vis.as_str()) {
                return false;
            }
        }

        // no_docs: docs is None or empty
        if self.no_docs {
            let has_docs = node
                .docs
                .as_deref()
                .is_some_and(|d| !d.trim().is_empty());
            if has_docs {
                return false;
            }
        }

        // dead: zero incoming Calls AND kind is symbol-like
        if self.dead {
            let is_symbol = matches!(
                node.kind,
                NodeKind::Function
                    | NodeKind::Method
                    | NodeKind::AsyncFunction
                    | NodeKind::AsyncMethod
                    | NodeKind::Class
            );
            if is_symbol {
                let has_callers = index
                    .edges(&node.id, false)
                    .iter()
                    .any(|e| e.kind == EdgeKind::Calls);
                if has_callers {
                    return false;
                }
            } else {
                // Not a symbol-like kind — don't filter it in dead mode
                return false;
            }
        }

        // test_only: ALL incoming Calls from test files AND at least 1 caller
        if self.test_only {
            let incoming_calls: Vec<_> = index
                .edges(&node.id, false)
                .iter()
                .filter(|e| e.kind == EdgeKind::Calls)
                .collect();
            if incoming_calls.is_empty() {
                return false;
            }
            let all_from_tests = incoming_calls.iter().all(|e| {
                index
                    .node(&e.from)
                    .and_then(|n| n.file.as_deref())
                    .is_some_and(|f| f.contains("test"))
            });
            if !all_from_tests {
                return false;
            }
        }

        // min_callers / max_callers: count unique incoming Calls edges
        if self.min_callers.is_some() || self.max_callers.is_some() {
            let caller_count = index
                .edges(&node.id, false)
                .iter()
                .filter(|e| e.kind == EdgeKind::Calls)
                .map(|e| e.from.as_str())
                .collect::<std::collections::HashSet<&str>>()
                .len();
            if let Some(min) = self.min_callers {
                if caller_count < min {
                    return false;
                }
            }
            if let Some(max) = self.max_callers {
                if caller_count > max {
                    return false;
                }
            }
        }

        // min_degree / max_degree
        let degree = index.degree(&node.id);
        if let Some(min) = self.min_degree {
            if degree < min {
                return false;
            }
        }
        if let Some(max) = self.max_degree {
            if degree > max {
                return false;
            }
        }

        true
    }

    /// Return human-readable descriptions of all active filters.
    pub fn description(&self) -> Vec<String> {
        let mut desc = Vec::new();
        if let Some(ref vis) = self.visibility {
            desc.push(format!("visibility={vis}"));
        }
        if self.no_docs {
            desc.push("no_docs".to_string());
        }
        if self.dead {
            desc.push("dead".to_string());
        }
        if self.test_only {
            desc.push("test_only".to_string());
        }
        if let Some(min) = self.min_callers {
            desc.push(format!("min_callers={min}"));
        }
        if let Some(max) = self.max_callers {
            desc.push(format!("max_callers={max}"));
        }
        if let Some(min) = self.min_degree {
            desc.push(format!("min_degree={min}"));
        }
        if let Some(max) = self.max_degree {
            desc.push(format!("max_degree={max}"));
        }
        desc
    }
}
