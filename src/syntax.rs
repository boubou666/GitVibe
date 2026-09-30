use std::{
    collections::{HashMap, HashSet},
    path::Path,
    sync::OnceLock,
};

use eframe::egui::Color32;
use syntect::{
    easy::HighlightLines,
    highlighting::{Style, ThemeSet},
    parsing::SyntaxSet,
};

use crate::git::{self, DiffHunk};

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

fn highlighted_source(
    source: &str,
    wanted: &HashSet<usize>,
    syntax: &syntect::parsing::SyntaxReference,
    theme: &syntect::highlighting::Theme,
) -> HashMap<usize, (String, Vec<Span>)> {
    let syntaxes = syntax_set();
    let mut highlighter = HighlightLines::new(syntax, theme);
    let mut output = HashMap::new();
    let last = wanted.iter().copied().max().unwrap_or(0);
    for (index, line) in source.lines().take(last).enumerate() {
        let number = index + 1;
        let colored = spans(line, &mut highlighter, syntaxes);
        if wanted.contains(&number) {
            output.insert(number, (line.to_owned(), colored));
        }
    }
    output
}

/// Color the two file versions from their first line, preserving parser state
/// through unchanged lines that are omitted from the patch.
fn highlight_hunks_with_sources(
    path: &str,
    hunks: &[DiffHunk],
    old_source: Option<&str>,
    new_source: Option<&str>,
) -> Vec<HunkColors> {
    let syntaxes = syntax_set();
    let filename = Path::new(path).file_name().and_then(|name| name.to_str());
    let extension = Path::new(path)
        .extension()
        .and_then(|extension| extension.to_str());
    let syntax = filename
        .and_then(|filename| syntaxes.find_syntax_by_extension(filename))
        .or_else(|| extension.and_then(|extension| syntaxes.find_syntax_by_extension(extension)));
    let theme = theme_set().themes.get("base16-ocean.dark");
    let (Some(syntax), Some(theme)) = (syntax, theme) else {
        return (0..hunks.len()).map(|_| HunkColors::default()).collect();
    };
    let mut old_wanted = HashSet::new();
    let mut new_wanted = HashSet::new();
    for hunk in hunks {
        for (old, new) in hunk.line_numbers() {
            if let Some(old) = old {
                old_wanted.insert(old);
            }
            if let Some(new) = new {
                new_wanted.insert(new);
            }
        }
    }
    let old_lines = old_source.map(|source| highlighted_source(source, &old_wanted, syntax, theme));
    let new_lines = new_source.map(|source| highlighted_source(source, &new_wanted, syntax, theme));
    hunks
        .iter()
        .map(|hunk| {
            let mut colors = highlight_hunk(path, hunk);
            for (patch_line, (old, new)) in hunk.lines.iter().zip(hunk.line_numbers()) {
                let Some(content) = patch_line.strip_prefix([' ', '-', '+']) else {
                    continue;
                };
                if let (Some(number), Some(lines)) = (old, &old_lines)
                    && let Some((source, spans)) = lines.get(&number)
                    && source == content
                {
                    colors.old.insert(number, spans.clone());
                }
                if let (Some(number), Some(lines)) = (new, &new_lines)
                    && let Some((source, spans)) = lines.get(&number)
                    && source == content
                {
                    colors.new.insert(number, spans.clone());
                }
            }
            colors
        })
        .collect()
}

/// Precompute diff colors on the inspection worker, keeping Git I/O and syntax
/// parsing out of the UI paint loop.
pub fn highlight_diff(
    repo: &Path,
    args: &[String],
    diff: &str,
) -> HashMap<(String, String), HunkColors> {
    let Some((old_revision, new_revision)) = diff_revisions(args) else {
        return HashMap::new();
    };
    let mut output = HashMap::new();
    for section in diff.split("diff --git ").skip(1) {
        let patch = format!("diff --git {section}");
        let hunks = git::diff_hunks(&patch);
        if hunks.is_empty() {
            continue;
        }
        let old_path = section.lines().find_map(|line| line.strip_prefix("--- a/"));
        let new_path = section.lines().find_map(|line| line.strip_prefix("+++ b/"));
        let Some(path) = new_path.or(old_path) else {
            continue;
        };
        let old =
            old_path.and_then(|path| git::diff_source_text(repo, old_revision.as_deref(), path));
        let new =
            new_path.and_then(|path| git::diff_source_text(repo, new_revision.as_deref(), path));
        for (hunk, colors) in hunks.iter().zip(highlight_hunks_with_sources(
            path,
            &hunks,
            old.as_deref(),
            new.as_deref(),
        )) {
            output.insert((path.to_owned(), hunk.patch.clone()), colors);
        }
    }
    output
}

fn diff_revisions(args: &[String]) -> Option<(Option<String>, Option<String>)> {
    match args.first()?.as_str() {
        "show" if args.iter().any(|arg| arg == "--patch" || arg == "-p") => {
            let revision = args
                .iter()
                .take_while(|arg| *arg != "--")
                .skip(1)
                .find(|arg| !arg.starts_with('-'))?;
            Some((Some(format!("{revision}^")), Some(revision.clone())))
        }
        "diff" => {
            let options = args
                .iter()
                .take_while(|arg| *arg != "--")
                .skip(1)
                .collect::<Vec<_>>();
            if options
                .iter()
                .any(|arg| *arg == "--cached" || *arg == "--staged")
            {
                return Some((Some("HEAD".into()), Some(":".into())));
            }
            let revisions = options
                .into_iter()
                .filter(|arg| !arg.starts_with('-'))
                .collect::<Vec<_>>();
            match revisions.as_slice() {
                [] => Some((Some(":".into()), None)),
                [old] => Some((Some((*old).clone()), None)),
                [old, new] => Some((Some((*old).clone()), Some((*new).clone()))),
                _ => None,
            }
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

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

    #[test]
    fn recovers_comment_state_across_omitted_lines_in_diff_sources() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("gitvibe-syntax-{unique}"));
        let root = git::init(&root).unwrap();
        git::run(&root, &["config", "user.name", "GitVibe Test"]).unwrap();
        git::run(&root, &["config", "user.email", "test@example.invalid"]).unwrap();
        let original = (1..=30)
            .map(|line| match line {
                1 => "/*\n".to_owned(),
                24 => "let before = 1;\n".to_owned(),
                30 => "*/\n".to_owned(),
                _ => format!("let line_{line} = {line};\n"),
            })
            .collect::<String>();
        std::fs::write(root.join("example.rs"), &original).unwrap();
        git::run(&root, &["add", "example.rs"]).unwrap();
        git::run(&root, &["commit", "-m", "Base"]).unwrap();
        std::fs::write(
            root.join("example.rs"),
            original.replace("let before = 1;", "let after = 2;"),
        )
        .unwrap();
        let args = vec!["diff".into(), "--".into(), "example.rs".into()];
        let patch = git::run_owned(&root, &args).unwrap();
        let hunk = git::diff_hunks(&patch).remove(0);
        let fallback = highlight_hunk("example.rs", &hunk);
        let colors = highlight_diff(&root, &args, &patch);
        let recovered = colors
            .get(&("example.rs".to_owned(), hunk.patch.clone()))
            .unwrap();
        let line = hunk
            .line_numbers()
            .iter()
            .zip(&hunk.lines)
            .find_map(|((_, new), text)| (text == "+let after = 2;").then_some(*new).flatten())
            .unwrap();
        assert_ne!(
            recovered.new_line(Some(line)).unwrap()[0].color,
            fallback.new_line(Some(line)).unwrap()[0].color
        );
        assert_eq!(
            recovered.old_line(Some(line)).unwrap()[0].color,
            recovered.new_line(Some(line)).unwrap()[0].color
        );
        let expected = recovered.new_line(Some(line)).unwrap()[0].color;
        git::run(&root, &["add", "example.rs"]).unwrap();
        let staged_args = vec![
            "diff".into(),
            "--cached".into(),
            "--".into(),
            "example.rs".into(),
        ];
        let staged_patch = git::run_owned(&root, &staged_args).unwrap();
        let staged_hunk = git::diff_hunks(&staged_patch).remove(0);
        assert_eq!(
            highlight_diff(&root, &staged_args, &staged_patch)
                .get(&("example.rs".to_owned(), staged_hunk.patch))
                .unwrap()
                .new_line(Some(line))
                .unwrap()[0]
                .color,
            expected
        );
        git::run(&root, &["commit", "-m", "Change inside comment"]).unwrap();
        let commit = git::run(&root, &["rev-parse", "HEAD"]).unwrap();
        let show_args = vec![
            "show".into(),
            "--format=".into(),
            "--patch".into(),
            commit.clone(),
            "--".into(),
            "example.rs".into(),
        ];
        let show_patch = git::run_owned(&root, &show_args).unwrap();
        let show_hunk = git::diff_hunks(&show_patch).remove(0);
        assert_eq!(
            highlight_diff(&root, &show_args, &show_patch)
                .get(&("example.rs".to_owned(), show_hunk.patch))
                .unwrap()
                .new_line(Some(line))
                .unwrap()[0]
                .color,
            expected
        );
        let compare_args = vec!["diff".into(), format!("{commit}^"), commit];
        let compare_patch = git::run_owned(&root, &compare_args).unwrap();
        let compare_hunk = git::diff_hunks(&compare_patch).remove(0);
        assert_eq!(
            highlight_diff(&root, &compare_args, &compare_patch)
                .get(&("example.rs".to_owned(), compare_hunk.patch))
                .unwrap()
                .new_line(Some(line))
                .unwrap()[0]
                .color,
            expected
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}
