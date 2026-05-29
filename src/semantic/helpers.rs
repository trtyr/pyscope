use lsp_types::{Hover, HoverContents, MarkedString, ParameterInformation, Position, Url};
use std::path::{Path, PathBuf};

pub fn find_language_server() -> Option<PathBuf> {
    let candidates = if cfg!(windows) {
        ["pyright-langserver.exe", "pylsp.exe"]
    } else {
        ["pyright-langserver", "pylsp"]
    };

    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        for candidate in candidates {
            let full = dir.join(candidate);
            if full.is_file() {
                return Some(full);
            }
        }
    }
    None
}

pub fn lsp_position(_source: &str, line: usize) -> Position {
    Position {
        line: line.saturating_sub(1) as u32,
        character: 0,
    }
}

pub fn hover_type(hover: &Hover) -> Option<String> {
    match &hover.contents {
        HoverContents::Scalar(marked) => marked_string(marked),
        HoverContents::Array(items) => items.iter().find_map(marked_string),
        HoverContents::Markup(markup) => {
            let value = markup.value.trim();
            (!value.is_empty()).then(|| value.to_string())
        }
    }
}

#[allow(dead_code)]
pub fn format_signature(params: &[ParameterInformation]) -> String {
    let parts = params
        .iter()
        .map(|param| match &param.label {
            lsp_types::ParameterLabel::Simple(text) => text.clone(),
            lsp_types::ParameterLabel::LabelOffsets([start, end]) => {
                format!("param[{start}..{end}]")
            }
        })
        .collect::<Vec<_>>();
    format!("({})", parts.join(", "))
}

pub fn path_to_uri(path: &Path) -> anyhow::Result<Url> {
    Url::from_file_path(path)
        .map_err(|_| anyhow::anyhow!("failed to convert path to file URI: {}", path.display()))
}

fn marked_string(marked: &MarkedString) -> Option<String> {
    match marked {
        MarkedString::String(text) => {
            let text = text.trim();
            (!text.is_empty()).then(|| text.to_string())
        }
        MarkedString::LanguageString(value) => {
            let text = value.value.trim();
            (!text.is_empty()).then(|| text.to_string())
        }
    }
}
