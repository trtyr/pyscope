use crate::model::Location;
use rustpython_parser::ast::{Constant, Expr, Stmt};
use std::collections::BTreeMap;
use std::path::Path;

/// Count lines, non-empty lines, comment lines in source.
pub fn file_metrics(source: &str) -> BTreeMap<String, usize> {
    let mut metrics = BTreeMap::new();
    let mut total = 0usize;
    let mut non_empty = 0usize;
    let mut comments = 0usize;

    for line in source.lines() {
        total += 1;
        let trimmed = line.trim();
        if !trimmed.is_empty() {
            non_empty += 1;
            if trimmed.starts_with('#') {
                comments += 1;
            }
        }
    }

    metrics.insert("lines".to_string(), total);
    metrics.insert("non_empty".to_string(), non_empty);
    metrics.insert("comments".to_string(), comments);
    metrics
}

/// Extract docstring from first statement of a function/class body.
///
/// In Python AST, a docstring is the first statement if it's an Expr
/// containing a Constant string.
/// Returns None if no docstring found.
pub fn docstring(body: &[Stmt]) -> Option<String> {
    let first = body.first()?;
    if let Stmt::Expr(expr_stmt) = first {
        if let Expr::Constant(const_expr) = expr_stmt.value.as_ref() {
            if let Constant::Str(s) = &const_expr.value {
                return Some(s.clone());
            }
        }
    }
    None
}

/// Python visibility: `_` prefix = private, `__` prefix = name-mangled, else public.
///
/// Returns `"private"`, `"mangled"`, or `None` (public).
pub fn visibility(name: &str) -> Option<String> {
    if name.starts_with("__") && name.ends_with("__") {
        // Dunder methods are public (special Python convention)
        None
    } else if name.starts_with("__") {
        Some("mangled".to_string())
    } else if name.starts_with('_') {
        Some("private".to_string())
    } else {
        None
    }
}

/// Derive Python module path from file path relative to project root.
///
/// - `/project/pkg/module.py` → `pkg.module`
/// - `/project/pkg/__init__.py` → `pkg`
pub fn module_name(file_path: &Path, project_root: &Path) -> String {
    let relative = file_path
        .strip_prefix(project_root)
        .unwrap_or(file_path);

    let without_ext = relative.with_extension("");

    let effective = if without_ext
        .file_name()
        .map_or(false, |f| f == "__init__")
    {
        // __init__.py represents the package itself — use parent directory
        match without_ext.parent() {
            Some(parent) if !parent.as_os_str().is_empty() => parent.to_path_buf(),
            _ => without_ext,
        }
    } else {
        without_ext
    };

    effective
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join(".")
}

/// Find line number (1-based) of the first occurrence of `needle` in `source`.
/// Returns 0 if not found.
#[allow(dead_code)]
pub fn find_line(source: &str, needle: &str) -> usize {
    match source.find(needle) {
        Some(offset) => offset_line(source, offset),
        None => 0,
    }
}

/// Given a `TextSize` byte offset and the full source string,
/// return the 1-based line number.
pub fn offset_line(source: &str, offset: usize) -> usize {
    let bounded = offset.min(source.len());
    source[..bounded].chars().filter(|&c| c == '\n').count() + 1
}

/// Given a start line (1-based), find the matching end line by tracking
/// Python indentation levels.
///
/// A block ends when indentation returns to the same or lesser level
/// as the start. Returns the 1-based line number of the last line
/// belonging to the block body.
pub fn find_block_end(source: &str, start_line: usize) -> usize {
    let lines: Vec<&str> = source.lines().collect();
    if start_line == 0 || start_line > lines.len() {
        return lines.len();
    }

    let start_idx = start_line - 1;
    let start_indent = indent_level(lines[start_idx]);
    let mut end_line = start_line; // at minimum the start line itself

    for i in (start_idx + 1)..lines.len() {
        let line = lines[i];
        if line.trim().is_empty() {
            continue; // skip blank lines — they don't end a block
        }
        let indent = indent_level(line);
        if indent <= start_indent {
            break;
        }
        end_line = i + 1; // convert 0-based index to 1-based line number
    }

    end_line
}

/// Count leading whitespace characters (spaces and tabs) in a line.
fn indent_level(line: &str) -> usize {
    line.len() - line.trim_start().len()
}

/// Create a [`Location`] from a file path, source text, and byte offset.
pub fn location(file: &str, source: &str, offset: usize) -> Location {
    Location {
        file: file.to_string(),
        line: offset_line(source, offset),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_metrics_empty() {
        let m = file_metrics("");
        assert_eq!(m["lines"], 0);
        assert_eq!(m["non_empty"], 0);
        assert_eq!(m["comments"], 0);
    }

    #[test]
    fn test_file_metrics_basic() {
        let src = "x = 1\n# a comment\n\ny = 2\n";
        let m = file_metrics(src);
        assert_eq!(m["lines"], 4);
        assert_eq!(m["non_empty"], 3);
        assert_eq!(m["comments"], 1);
    }

    #[test]
    fn test_offset_line_first_line() {
        assert_eq!(offset_line("hello\nworld", 0), 1);
        assert_eq!(offset_line("hello\nworld", 4), 1);
    }

    #[test]
    fn test_offset_line_second_line() {
        assert_eq!(offset_line("hello\nworld", 6), 2);
    }

    #[test]
    fn test_find_line_found() {
        assert_eq!(find_line("foo\nbar\nbaz", "bar"), 2);
    }

    #[test]
    fn test_find_line_not_found() {
        assert_eq!(find_line("foo\nbar", "qux"), 0);
    }

    #[test]
    fn test_visibility_public() {
        assert_eq!(visibility("foo"), None);
    }

    #[test]
    fn test_visibility_private() {
        assert_eq!(visibility("_foo"), Some("private".to_string()));
    }

    #[test]
    fn test_visibility_mangled() {
        assert_eq!(visibility("__foo"), Some("mangled".to_string()));
    }

    #[test]
    fn test_visibility_dunder() {
        assert_eq!(visibility("__init__"), None);
    }

    #[test]
    fn test_module_name_regular() {
        let file = Path::new("/project/pkg/module.py");
        let root = Path::new("/project");
        assert_eq!(module_name(file, root), "pkg.module");
    }

    #[test]
    fn test_module_name_init() {
        let file = Path::new("/project/pkg/__init__.py");
        let root = Path::new("/project");
        assert_eq!(module_name(file, root), "pkg");
    }

    #[test]
    fn test_find_block_end() {
        let src = "def foo():\n    x = 1\n    y = 2\ndef bar():\n    pass\n";
        // start at line 1 (def foo), block body is lines 2-3
        assert_eq!(find_block_end(src, 1), 3);
    }

    #[test]
    fn test_find_block_end_no_body() {
        let src = "x = 1\ny = 2\n";
        // start at line 1 — no indented body, so block is just line 1
        assert_eq!(find_block_end(src, 1), 1);
    }

    #[test]
    fn test_find_block_end_blank_lines() {
        let src = "def foo():\n    x = 1\n\n    y = 2\ndef bar():\n";
        // blank line in body is skipped; block ends at line 4
        assert_eq!(find_block_end(src, 1), 4);
    }

    #[test]
    fn test_location_basic() {
        let src = "line1\nline2\nline3\n";
        let loc = location("test.py", src, 6);
        assert_eq!(loc.file, "test.py");
        assert_eq!(loc.line, 2);
    }
}
