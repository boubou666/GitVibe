use std::{collections::HashMap, path::Path, sync::OnceLock};

use eframe::egui::Color32;
use syntect::{
    easy::HighlightLines,
    highlighting::{Style, ThemeSet},
    parsing::SyntaxSet,
};

use crate::git::DiffHunk;

#[derive(Clone)]
pub struct Span {
    pub start: usize,
    pub end: usize,
    pub color: Color32,
}

#[derive(Default)]
pub struct HunkColors {
    old: HashMap<usize, Vec<Span>>,
    new: HashMap<usize, Vec<Span>>,
}

impl HunkColors {
    pub fn old_line(&self, line: Option<usize>) -> Option<&[Span]> {
        line.and_then(|line| self.old.get(&line).map(Vec::as_slice))
    }

    pub fn new_line(&self, line: Option<usize>) -> Option<&[Span]> {
        line.and_then(|line| self.new.get(&line).map(Vec::as_slice))
    }
}

static SYNTAXES: OnceLock<SyntaxSet> = OnceLock::new();
static THEMES: OnceLock<ThemeSet> = OnceLock::new();

fn syntax_set() -> &'static SyntaxSet {
    SYNTAXES.get_or_init(SyntaxSet::load_defaults_nonewlines)
}

fn theme_set() -> &'static ThemeSet {
    THEMES.get_or_init(ThemeSet::load_defaults)
}

fn spans(line: &str, highlighter: &mut HighlightLines<'_>, syntaxes: &SyntaxSet) -> Vec<Span> {
    let Ok(ranges) = highlighter.highlight_line(line, syntaxes) else {
        return Vec::new();
    };
    let mut offset = 0;
    ranges
        .into_iter()
        .map(|(Style { foreground, .. }, part)| {
            let start = offset;
            offset += part.len();
            Span {
                start,
                end: offset,
                color: Color32::from_rgb(foreground.r, foreground.g, foreground.b),
            }
        })
        .collect()
}

pub fn highlight_hunk(path: &str, hunk: &DiffHunk) -> HunkColors {
    let mut colors = HunkColors {
        old: HashMap::new(),
        new: HashMap::new(),
    };
    let syntaxes = syntax_set();
    let filename = Path::new(path).file_name().and_then(|name| name.to_str());
    let extension = Path::new(path)
        .extension()
        .and_then(|extension| extension.to_str());
    let syntax = filename
        .and_then(|filename| syntaxes.find_syntax_by_extension(filename))
        .or_else(|| extension.and_then(|extension| syntaxes.find_syntax_by_extension(extension)));
    let Some(syntax) = syntax else {
        return colors;
    };
    let themes = theme_set();
    let Some(theme) = themes.themes.get("base16-ocean.dark") else {
        return colors;
    };
    let mut old_highlighter = HighlightLines::new(syntax, theme);
    let mut new_highlighter = HighlightLines::new(syntax, theme);
    for (line, (old, new)) in hunk.lines.iter().zip(hunk.line_numbers()) {
        let Some(content) = line.strip_prefix([' ', '-', '+']) else {
            continue;
        };
        if let Some(old) = old {
            colors
                .old
                .insert(old, spans(content, &mut old_highlighter, syntaxes));
        }
        if let Some(new) = new {
            colors
                .new
                .insert(new, spans(content, &mut new_highlighter, syntaxes));
        }
    }
    colors
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn highlights_rust_and_leaves_unknown_files_plain() {
        let hunk = DiffHunk {
            heading: "@@ -1,1 +1,1 @@".into(),
            patch: String::new(),
            lines: vec!["-let old = 1;".into(), "+let new = 2;".into()],
        };
        let colors = highlight_hunk("src/main.rs", &hunk);
        assert!(!colors.old_line(Some(1)).unwrap().is_empty());
        assert!(!colors.new_line(Some(1)).unwrap().is_empty());
        assert_eq!(
            colors.new_line(Some(1)).unwrap().last().unwrap().end,
            "let new = 2;".len()
        );
        assert!(
            highlight_hunk("package.json", &hunk)
                .new_line(Some(1))
                .is_some()
        );
        assert!(
            highlight_hunk("data.unknown_extension", &hunk)
                .new_line(Some(1))
                .is_none()
        );
    }

    #[test]
    fn keeps_comment_state_between_lines_and_sides_separate() {
        let hunk = DiffHunk {
            heading: "@@ -1,3 +1,3 @@".into(),
            patch: String::new(),
            lines: vec![
                " /* comment".into(),
                "-still a comment */ let old = 1;".into(),
                "+still a comment */ let new = 2;".into(),
                " let common = 3;".into(),
            ],
        };
        let colors = highlight_hunk("example.rs", &hunk);
        let old = colors.old_line(Some(2)).unwrap();
        let new = colors.new_line(Some(2)).unwrap();
        assert_eq!(old[0].color, new[0].color);
        assert_ne!(old[0].color, old.last().unwrap().color);
    }
}
