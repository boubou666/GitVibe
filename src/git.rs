use std::{
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
};

#[derive(Clone, Default)]
pub struct Snapshot {
    pub root: PathBuf,
    pub branch: String,
    pub status: Vec<FileStatus>,
    pub commits: Vec<Commit>,
    pub has_more_commits: bool,
    pub branches: Vec<Ref>,
    pub remote_branches: Vec<Ref>,
    pub tags: Vec<Ref>,
    pub remotes: Vec<String>,
    pub stashes: Vec<String>,
    pub worktrees: Vec<Worktree>,
    pub submodules: Vec<Submodule>,
    pub merge_in_progress: bool,
    pub rebase_in_progress: bool,
}

#[derive(Clone)]
pub struct Worktree {
    pub path: PathBuf,
    pub head: String,
    pub branch: Option<String>,
    pub current: bool,
    pub locked: bool,
    pub prunable: bool,
}

#[derive(Clone)]
pub struct Submodule {
    pub name: String,
    pub path: String,
    pub url: String,
    pub initialized: bool,
}

#[derive(Clone)]
pub struct FileStatus {
    pub index: char,
    pub worktree: char,
    pub path: String,
}

impl FileStatus {
    pub fn staged(&self) -> bool {
        self.index != ' ' && self.index != '?'
    }
    pub fn unstaged(&self) -> bool {
        self.worktree != ' ' || self.index == '?'
    }

    pub fn conflicted(&self) -> bool {
        matches!(
            (self.index, self.worktree),
            ('D', 'D')
                | ('A', 'U')
                | ('U', 'D')
                | ('U', 'A')
                | ('D', 'U')
                | ('A', 'A')
                | ('U', 'U')
        )
    }
}

#[derive(Clone, Copy)]
pub enum ConflictSide {
    Ours,
    Theirs,
}

pub fn choose_conflict_side(repo: &Path, path: &str, side: ConflictSide) -> Result<String, String> {
    let flag = match side {
        ConflictSide::Ours => "--ours",
        ConflictSide::Theirs => "--theirs",
    };
    run(repo, &["checkout", flag, "--", path])?;
    run(repo, &["add", "-A", "--", path])?;
    Ok(format!("Resolved {path} with {}", &flag[2..]))
}

pub fn mark_conflict_resolved(repo: &Path, path: &str) -> Result<String, String> {
    let file = repo.join(path);
    if file.exists() {
        let bytes = std::fs::read(&file).map_err(|e| e.to_string())?;
        if contains_conflict_markers(&bytes) {
            return Err(format!(
                "Conflict markers remain in {path}. Remove them before marking the file resolved."
            ));
        }
    }
    run(repo, &["add", "-A", "--", path])?;
    Ok(format!("Marked {path} resolved"))
}

fn contains_conflict_markers(bytes: &[u8]) -> bool {
    bytes.split(|byte| *byte == b'\n').any(|line| {
        line.starts_with(b"<<<<<<< ")
            || line.starts_with(b"=======")
            || line.starts_with(b">>>>>>> ")
    })
}

#[derive(Clone)]
pub struct Commit {
    pub id: String,
    pub short: String,
    pub subject: String,
    pub author: String,
    pub date: String,
    pub parents: Vec<String>,
    pub lane: usize,
    pub lane_count: usize,
    pub graph_edges: Vec<(usize, usize)>,
    pub parent_edges: Vec<usize>,
}

#[derive(Clone)]
pub struct Ref {
    pub name: String,
    pub target: String,
    pub current: bool,
}

pub struct DiffHunk {
    pub heading: String,
    pub patch: String,
    pub lines: Vec<String>,
}

#[derive(Clone)]
pub struct CommitFile {
    pub status: String,
    pub path: String,
}

pub fn commit_files(repo: &Path, id: &str) -> Result<Vec<CommitFile>, String> {
    let output = git_output(
        Some(repo),
        &[
            "diff-tree",
            "--root",
            "--first-parent",
            "--no-commit-id",
            "--name-status",
            "--no-renames",
            "-r",
            "-z",
            id,
        ],
    )?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_owned());
    }
    let fields = output
        .stdout
        .split(|b| *b == 0)
        .filter(|field| !field.is_empty())
        .collect::<Vec<_>>();
    let mut files = Vec::new();
    for pair in fields.as_chunks::<2>().0 {
        files.push(CommitFile {
            status: String::from_utf8_lossy(pair[0]).trim().to_owned(),
            path: String::from_utf8_lossy(pair[1]).into_owned(),
        });
    }
    Ok(files)
}

impl DiffHunk {
    pub fn line_numbers(&self) -> Vec<(Option<usize>, Option<usize>)> {
        let heading = self.heading.split("@@").nth(1).unwrap_or("").trim();
        let mut fields = heading.split_whitespace();
        let mut old = fields
            .next()
            .and_then(|s| s.strip_prefix('-'))
            .and_then(|s| s.split(',').next())
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(0);
        let mut new = fields
            .next()
            .and_then(|s| s.strip_prefix('+'))
            .and_then(|s| s.split(',').next())
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(0);
        self.lines
            .iter()
            .map(|line| match line.as_bytes().first().copied() {
                Some(b'-') => {
                    let at = old;
                    old += 1;
                    (Some(at), None)
                }
                Some(b'+') => {
                    let at = new;
                    new += 1;
                    (None, Some(at))
                }
                Some(b' ') => {
                    let at = (Some(old), Some(new));
                    old += 1;
                    new += 1;
                    at
                }
                _ => (None, None),
            })
            .collect()
    }

    pub fn line_patch(&self, index: usize) -> Option<String> {
        let heading = self.heading.split("@@").nth(1)?.trim();
        let mut fields = heading.split_whitespace();
        let mut old = fields
            .next()?
            .strip_prefix('-')?
            .split(',')
            .next()?
            .parse::<usize>()
            .ok()?;
        let mut new = fields
            .next()?
            .strip_prefix('+')?
            .split(',')
            .next()?
            .parse::<usize>()
            .ok()?;
        let mut group_start = None;
        let mut position = None;
        for (i, line) in self.lines.iter().enumerate() {
            let kind = line.as_bytes().first().copied();
            if matches!(kind, Some(b'+') | Some(b'-')) && group_start.is_none() {
                group_start = Some(old);
            }
            if i == index {
                position = match kind {
                    Some(b'+') => Some((group_start.unwrap_or(old).saturating_sub(1), 0, new, 1)),
                    Some(b'-') => Some((old, 1, new.saturating_sub(1), 0)),
                    _ => None,
                };
                break;
            }
            match kind {
                Some(b' ') => {
                    old += 1;
                    new += 1;
                    group_start = None;
                }
                Some(b'-') => old += 1,
                Some(b'+') => new += 1,
                _ => {}
            }
        }
        let (old, old_count, new, new_count) = position?;
        let header = self.patch.split("@@ ").next()?;
        let mut patch = format!(
            "{header}@@ -{old},{old_count} +{new},{new_count} @@\n{}\n",
            self.lines[index]
        );
        if self
            .lines
            .get(index + 1)
            .is_some_and(|line| line.starts_with("\\ No newline"))
        {
            patch.push_str("\\ No newline at end of file\n");
        }
        Some(patch)
    }
}

#[derive(Clone)]
pub struct ConflictBlock {
    pub ours: Vec<String>,
    pub theirs: Vec<String>,
    pub selected_ours: Vec<bool>,
    pub selected_theirs: Vec<bool>,
    pub manual_result: Option<String>,
}

#[derive(Clone)]
pub enum ConflictPart {
    Plain(String),
    Block(ConflictBlock),
}

#[derive(Clone)]
pub struct ConflictDocument {
    pub path: String,
    pub original: String,
    pub parts: Vec<ConflictPart>,
}

impl ConflictDocument {
    pub fn result(&self) -> String {
        let mut result = String::new();
        for part in &self.parts {
            match part {
                ConflictPart::Plain(text) => result.push_str(text),
                ConflictPart::Block(block) => {
                    if let Some(manual) = &block.manual_result {
                        result.push_str(manual);
                    } else {
                        for (line, selected) in block.ours.iter().zip(&block.selected_ours) {
                            if *selected {
                                result.push_str(line);
                            }
                        }
                        for (line, selected) in block.theirs.iter().zip(&block.selected_theirs) {
                            if *selected {
                                result.push_str(line);
                            }
                        }
                    }
                }
            }
        }
        result
    }
}

pub fn parse_conflict(path: String, original: String) -> Result<ConflictDocument, String> {
    let mut parts = Vec::new();
    let mut plain = String::new();
    let mut lines = original.split_inclusive('\n').peekable();
    while let Some(line) = lines.next() {
        if !line.starts_with("<<<<<<< ") {
            plain.push_str(line);
            continue;
        }
        if !plain.is_empty() {
            parts.push(ConflictPart::Plain(std::mem::take(&mut plain)));
        }
        let mut ours = Vec::new();
        let mut theirs = Vec::new();
        let mut base = false;
        let mut divider = false;
        let mut closed = false;
        for line in lines.by_ref() {
            if line.starts_with("||||||| ") && !divider {
                base = true;
            } else if line.starts_with("=======") && !divider {
                divider = true;
                base = false;
            } else if line.starts_with(">>>>>>> ") && divider {
                closed = true;
                break;
            } else if !base && divider {
                theirs.push(line.to_owned());
            } else if !base {
                ours.push(line.to_owned());
            }
        }
        if !closed {
            return Err(format!("Incomplete conflict markers in {path}"));
        }
        let selected_ours = vec![true; ours.len()];
        let selected_theirs = vec![false; theirs.len()];
        parts.push(ConflictPart::Block(ConflictBlock {
            ours,
            theirs,
            selected_ours,
            selected_theirs,
            manual_result: None,
        }));
    }
    if !plain.is_empty() {
        parts.push(ConflictPart::Plain(plain));
    }
    if !parts
        .iter()
        .any(|part| matches!(part, ConflictPart::Block(_)))
    {
        return Err(format!("No text conflict markers found in {path}"));
    }
    Ok(ConflictDocument {
        path,
        original,
        parts,
    })
}

pub fn load_conflict(repo: &Path, path: &str) -> Result<ConflictDocument, String> {
    let bytes = std::fs::read(repo.join(path)).map_err(|e| e.to_string())?;
    if bytes.len() > 2 * 1024 * 1024 {
        return Err("Conflict file is larger than the 2 MB visual editor limit".into());
    }
    let original = String::from_utf8(bytes)
        .map_err(|_| "Visual conflict editor requires UTF-8 text".to_owned())?;
    parse_conflict(path.to_owned(), original)
}

pub fn save_conflict(repo: &Path, document: &ConflictDocument) -> Result<String, String> {
    let path = repo.join(&document.path);
    let current = std::fs::read(&path).map_err(|e| e.to_string())?;
    if current != document.original.as_bytes() {
        return Err("Conflict file changed on disk. Reopen the editor before saving.".into());
    }
    let result = document.result();
    if contains_conflict_markers(result.as_bytes()) {
        return Err("Conflict markers remain in the result".into());
    }
    std::fs::write(&path, result).map_err(|e| e.to_string())?;
    run(repo, &["add", "--", &document.path])?;
    Ok(format!("Resolved {} line by line", document.path))
}

pub fn diff_hunks(diff: &str) -> Vec<DiffHunk> {
    let lines: Vec<&str> = diff.lines().collect();
    let Some(first_hunk) = lines.iter().position(|line| line.starts_with("@@ ")) else {
        return Vec::new();
    };
    let header = &lines[..first_hunk];
    if !header.iter().any(|line| line.starts_with("diff --git "))
        || !header.iter().any(|line| line.starts_with("--- "))
        || !header.iter().any(|line| line.starts_with("+++ "))
    {
        return Vec::new();
    }
    let mut hunks = Vec::new();
    let mut start = first_hunk;
    while start < lines.len() {
        if !lines[start].starts_with("@@ ") {
            break;
        }
        let end = (start + 1..lines.len())
            .find(|&i| lines[i].starts_with("@@ ") || lines[i].starts_with("diff --git "))
            .unwrap_or(lines.len());
        let mut patch = String::new();
        for line in header.iter().chain(&lines[start..end]) {
            patch.push_str(line);
            patch.push('\n');
        }
        hunks.push(DiffHunk {
            heading: lines[start].to_owned(),
            patch,
            lines: lines[start + 1..end]
                .iter()
                .map(|line| (*line).to_owned())
                .collect(),
        });
        start = end;
    }
    hunks
}

pub fn apply_hunk(repo: &Path, patch: &str, reverse: bool) -> Result<(), String> {
    apply_patch(repo, patch, reverse, false)
}

pub fn apply_line(repo: &Path, patch: &str, reverse: bool) -> Result<(), String> {
    apply_patch(repo, patch, reverse, true)
}

fn apply_patch(repo: &Path, patch: &str, reverse: bool, zero_context: bool) -> Result<(), String> {
    let mut command = Command::new("git");
    command.arg("--no-pager").arg("-C").arg(repo).arg("apply");
    if reverse {
        command.arg("--reverse");
    }
    if zero_context {
        command.arg("--unidiff-zero");
    }
    let mut child = command
        .args(["--cached", "--whitespace=nowarn", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Could not start Git: {e}"))?;
    child
        .stdin
        .take()
        .ok_or("Could not open Git input")?
        .write_all(patch.as_bytes())
        .map_err(|e| format!("Could not send patch to Git: {e}"))?;
    let output = child.wait_with_output().map_err(|e| e.to_string())?;
    if output.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_owned())
    }
}

fn git_output(repo: Option<&Path>, args: &[&str]) -> Result<Output, String> {
    let mut command = Command::new("git");
    command.arg("--no-pager");
    if let Some(repo) = repo {
        command.arg("-C").arg(repo);
    }
    command.args(args).env("GIT_OPTIONAL_LOCKS", "0");
    command
        .output()
        .map_err(|e| format!("Could not start Git: {e}"))
}

pub fn run(repo: &Path, args: &[&str]) -> Result<String, String> {
    let output = git_output(Some(repo), args)?;
    let stdout = String::from_utf8_lossy(&output.stdout)
        .trim_end_matches(['\r', '\n'])
        .to_owned();
    if output.status.success() {
        Ok(stdout)
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        Err(if stderr.is_empty() { stdout } else { stderr })
    }
}

pub fn run_owned(repo: &Path, args: &[String]) -> Result<String, String> {
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    run(repo, &refs)
}

pub fn export_patch(repo: &Path, args: &[String], destination: &Path) -> Result<String, String> {
    let refs = args.iter().map(String::as_str).collect::<Vec<_>>();
    let output = git_output(Some(repo), &refs)?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_owned());
    }
    if output.stdout.is_empty() {
        return Err("There are no patch changes to export".to_owned());
    }
    std::fs::write(destination, output.stdout).map_err(|error| error.to_string())?;
    Ok(format!("Saved patch to {}", destination.display()))
}

pub fn discover(path: &Path) -> Result<PathBuf, String> {
    let output = git_output(Some(path), &["rev-parse", "--show-toplevel"])?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_owned());
    }
    Ok(PathBuf::from(
        String::from_utf8_lossy(&output.stdout).trim_end_matches(['\r', '\n']),
    ))
}

pub fn init(path: &Path) -> Result<PathBuf, String> {
    std::fs::create_dir_all(path).map_err(|e| e.to_string())?;
    let output = git_output(Some(path), &["init"])?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_owned());
    }
    discover(path)
}

pub fn clone_repo_with_options(
    url: &str,
    destination: &Path,
    shallow: bool,
    sparse: bool,
) -> Result<PathBuf, String> {
    let destination_arg = destination.to_string_lossy().into_owned();
    let mut args = vec!["clone"];
    if shallow {
        args.push("--depth=1");
    }
    if sparse {
        args.push("--sparse");
    }
    if shallow && Path::new(url).exists() {
        // Git ignores --depth with its default local hard-link optimization.
        args.push("--no-local");
    } else if Path::new(url).exists() {
        args.push("--local");
    }
    args.extend(["--", url, &destination_arg]);
    let output = git_output(None, &args)?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_owned());
    }
    discover(destination)
}

pub fn default_clone_directory(url: &str) -> Option<String> {
    let trimmed = url.trim().trim_end_matches(['/', '\\']);
    let final_part = trimmed.rsplit(['/', '\\', ':']).next()?;
    let name = final_part.strip_suffix(".git").unwrap_or(final_part);
    if name.is_empty() || name == "." || name == ".." {
        None
    } else {
        Some(name.to_owned())
    }
}

pub fn snapshot_with_limit(repo: &Path, limit: usize) -> Result<Snapshot, String> {
    let root = discover(repo)?;
    let branch = run(&root, &["branch", "--show-current"])?;
    let status = status(&root)?;
    let (commits, has_more_commits) = commits(&root, limit)?;
    let branches = refs(&root, "refs/heads")?;
    let remote_branches = refs(&root, "refs/remotes")?
        .into_iter()
        .filter(|r| !r.name.ends_with("/HEAD"))
        .collect();
    let tags = refs(&root, "refs/tags")?;
    let remotes = run(&root, &["remote"])?
        .lines()
        .map(str::to_owned)
        .collect();
    let stashes = run(&root, &["stash", "list", "--format=%gd  %s"])?
        .lines()
        .map(str::to_owned)
        .collect();
    let worktrees = worktrees(&root)?;
    let submodules = submodules(&root)?;
    let merge_in_progress = run(&root, &["rev-parse", "--verify", "-q", "MERGE_HEAD"]).is_ok();
    let rebase_in_progress = ["rebase-merge", "rebase-apply"]
        .into_iter()
        .filter_map(|name| run(&root, &["rev-parse", "--git-path", name]).ok())
        .any(|path| {
            let path = Path::new(&path);
            if path.is_absolute() {
                path.exists()
            } else {
                root.join(path).exists()
            }
        });
    Ok(Snapshot {
        root,
        branch,
        status,
        commits,
        has_more_commits,
        branches,
        remote_branches,
        tags,
        remotes,
        stashes,
        worktrees,
        submodules,
        merge_in_progress,
        rebase_in_progress,
    })
}

pub fn worktrees(repo: &Path) -> Result<Vec<Worktree>, String> {
    let output = git_output(Some(repo), &["worktree", "list", "--porcelain", "-z"])?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_owned());
    }
    let mut result = Vec::new();
    let mut current: Option<Worktree> = None;
    for field in output
        .stdout
        .split(|byte| *byte == 0)
        .filter(|field| !field.is_empty())
    {
        let field = String::from_utf8_lossy(field);
        if let Some(path) = field.strip_prefix("worktree ") {
            if let Some(worktree) = current.take() {
                result.push(worktree);
            }
            current = Some(Worktree {
                path: PathBuf::from(path),
                head: String::new(),
                branch: None,
                current: false,
                locked: false,
                prunable: false,
            });
        } else if let Some(worktree) = current.as_mut() {
            if let Some(head) = field.strip_prefix("HEAD ") {
                worktree.head = head.to_owned();
            } else if let Some(branch) = field.strip_prefix("branch refs/heads/") {
                worktree.branch = Some(branch.to_owned());
            } else if field.starts_with("locked") {
                worktree.locked = true;
            } else if field.starts_with("prunable") {
                worktree.prunable = true;
            }
        }
    }
    if let Some(worktree) = current {
        result.push(worktree);
    }
    let canonical_repo = repo.canonicalize().unwrap_or_else(|_| repo.to_path_buf());
    for worktree in &mut result {
        worktree.current = worktree
            .path
            .canonicalize()
            .unwrap_or_else(|_| worktree.path.clone())
            == canonical_repo;
    }
    Ok(result)
}

pub fn submodules(repo: &Path) -> Result<Vec<Submodule>, String> {
    if !repo.join(".gitmodules").is_file() {
        return Ok(Vec::new());
    }
    let output = git_output(
        Some(repo),
        &[
            "config",
            "-f",
            ".gitmodules",
            "--null",
            "--get-regexp",
            "^submodule\\..*\\.(path|url)$",
        ],
    )?;
    if !output.status.success() {
        if output.stdout.is_empty() && output.stderr.is_empty() {
            return Ok(Vec::new());
        }
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_owned());
    }
    let mut entries = std::collections::BTreeMap::<String, (String, String)>::new();
    for field in output
        .stdout
        .split(|byte| *byte == 0)
        .filter(|field| !field.is_empty())
    {
        let record = String::from_utf8_lossy(field);
        let Some((key, value)) = record.split_once('\n') else {
            continue;
        };
        let Some(key) = key.strip_prefix("submodule.") else {
            continue;
        };
        if let Some(name) = key.strip_suffix(".path") {
            entries.entry(name.to_owned()).or_default().0 = value.to_owned();
        } else if let Some(name) = key.strip_suffix(".url") {
            entries.entry(name.to_owned()).or_default().1 = value.to_owned();
        }
    }
    Ok(entries
        .into_iter()
        .filter_map(|(name, (path, url))| {
            if path.is_empty() {
                None
            } else {
                let initialized = repo.join(&path).join(".git").exists();
                Some(Submodule {
                    name,
                    path,
                    url,
                    initialized,
                })
            }
        })
        .collect())
}

fn status(repo: &Path) -> Result<Vec<FileStatus>, String> {
    let output = git_output(
        Some(repo),
        &["status", "--porcelain=v1", "-z", "--untracked-files=all"],
    )?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_owned());
    }
    parse_status(&output.stdout)
}

fn parse_status(bytes: &[u8]) -> Result<Vec<FileStatus>, String> {
    let mut fields = bytes.split(|b| *b == 0).filter(|field| !field.is_empty());
    let mut files = Vec::new();
    while let Some(field) = fields.next() {
        if field.len() < 4 || field[2] != b' ' {
            return Err("Unexpected Git status output".into());
        }
        let index = field[0] as char;
        let worktree = field[1] as char;
        let path = String::from_utf8_lossy(&field[3..]).into_owned();
        if index == 'R' || index == 'C' || worktree == 'R' || worktree == 'C' {
            fields.next().ok_or("Incomplete rename record")?;
        }
        files.push(FileStatus {
            index,
            worktree,
            path,
        });
    }
    Ok(files)
}

fn commits(repo: &Path, limit: usize) -> Result<(Vec<Commit>, bool), String> {
    if run(repo, &["rev-parse", "--verify", "HEAD"]).is_err() {
        return Ok((Vec::new(), false));
    }
    let count = limit.saturating_add(1).to_string();
    let text = run(
        repo,
        &[
            "log",
            "--all",
            "--topo-order",
            "-n",
            &count,
            "--date=short",
            "--pretty=format:%H%x1f%h%x1f%s%x1f%an%x1f%ad%x1f%P%x1e",
        ],
    )?;
    let mut commits = parse_commits(&text);
    let has_more = commits.len() > limit;
    commits.truncate(limit);
    assign_lanes(&mut commits);
    Ok((commits, has_more))
}

fn parse_commits(text: &str) -> Vec<Commit> {
    text.split('\x1e')
        .filter_map(|record| {
            let fields: Vec<&str> = record.trim().split('\x1f').collect();
            (fields.len() == 6).then(|| Commit {
                id: fields[0].to_owned(),
                short: fields[1].to_owned(),
                subject: fields[2].to_owned(),
                author: fields[3].to_owned(),
                date: fields[4].to_owned(),
                parents: fields[5].split_whitespace().map(str::to_owned).collect(),
                lane: 0,
                lane_count: 1,
                graph_edges: Vec::new(),
                parent_edges: Vec::new(),
            })
        })
        .collect()
}

pub fn search_history(repo: &Path, query: &str) -> Result<(Vec<Commit>, bool), String> {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return Ok((Vec::new(), false));
    }
    if run(repo, &["rev-parse", "--verify", "HEAD"]).is_err() {
        return Ok((Vec::new(), false));
    }
    let mut child = Command::new("git")
        .arg("--no-pager")
        .arg("-C")
        .arg(repo)
        .args([
            "log",
            "--all",
            "--topo-order",
            "--date=short",
            "--pretty=format:%H%x1f%h%x1f%s%x1f%an%x1f%ad%x1f%P%x1e",
        ])
        .env("GIT_OPTIONAL_LOCKS", "0")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Could not start Git: {e}"))?;
    let stdout = child.stdout.take().ok_or("Could not read Git history")?;
    let mut reader = BufReader::new(stdout);
    let mut hits = Vec::new();
    let mut has_more = false;
    let mut record = Vec::new();
    loop {
        record.clear();
        let read = reader
            .read_until(b'\x1e', &mut record)
            .map_err(|e| e.to_string())?;
        if read == 0 {
            break;
        }
        for commit in parse_commits(&String::from_utf8_lossy(&record)) {
            if [
                commit.id.as_str(),
                commit.short.as_str(),
                commit.subject.as_str(),
                commit.author.as_str(),
            ]
            .iter()
            .any(|field| field.to_lowercase().contains(&query))
            {
                if hits.len() == 300 {
                    has_more = true;
                    break;
                }
                hits.push(commit);
            }
        }
        if has_more {
            break;
        }
    }
    drop(reader);
    if has_more {
        let _ = child.kill();
        let _ = child.wait();
    } else {
        let output = child.wait_with_output().map_err(|e| e.to_string())?;
        if !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr).trim().to_owned());
        }
    }
    Ok((hits, has_more))
}

pub fn file_history(repo: &Path, path: &str) -> Result<Vec<Commit>, String> {
    let text = run(
        repo,
        &[
            "log",
            "--follow",
            "-n",
            "100",
            "--date=short",
            "--pretty=format:%H%x1f%h%x1f%s%x1f%an%x1f%ad%x1f%P%x1e",
            "--",
            path,
        ],
    )?;
    Ok(parse_commits(&text))
}

fn assign_lanes(commits: &mut [Commit]) {
    let mut active: Vec<String> = Vec::new();
    for commit in commits {
        let lane = active
            .iter()
            .position(|id| id == &commit.id)
            .unwrap_or_else(|| {
                active.push(commit.id.clone());
                active.len() - 1
            });
        let before = active.clone();
        commit.lane = lane;
        active.remove(lane);
        for (offset, parent) in commit.parents.iter().enumerate() {
            if !active.contains(parent) {
                active.insert((lane + offset).min(active.len()), parent.clone());
            }
        }
        commit.lane_count = before.len().max(active.len());
        commit.graph_edges = before
            .iter()
            .enumerate()
            .filter(|(_, id)| *id != &commit.id)
            .filter_map(|(from, id)| {
                active
                    .iter()
                    .position(|next| next == id)
                    .map(|to| (from, to))
            })
            .collect();
        commit.parent_edges = commit
            .parents
            .iter()
            .filter_map(|id| active.iter().position(|next| next == id))
            .collect();
    }
}

fn refs(repo: &Path, prefix: &str) -> Result<Vec<Ref>, String> {
    let text = run(
        repo,
        &[
            "for-each-ref",
            "--format=%(refname:short)%09%(objectname:short)%09%(HEAD)",
            prefix,
        ],
    )?;
    Ok(text
        .lines()
        .filter_map(|line| {
            let fields: Vec<&str> = line.split('\t').collect();
            (fields.len() == 3).then(|| Ref {
                name: fields[0].into(),
                target: fields[1].into(),
                current: fields[2] == "*",
            })
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn snapshot(repo: &Path) -> Result<Snapshot, String> {
        snapshot_with_limit(repo, 300)
    }

    #[test]
    fn parses_status_with_spaces_and_rename() {
        let files =
            parse_status(b" M a file.txt\0R  new name.txt\0old name.txt\0?? new.txt\0").unwrap();
        assert_eq!(files.len(), 3);
        assert_eq!(files[0].path, "a file.txt");
        assert_eq!(files[1].path, "new name.txt");
        assert!(files[2].unstaged());
    }

    #[test]
    fn suggests_clone_folder_for_https_ssh_and_local_repositories() {
        assert_eq!(
            default_clone_directory("https://github.com/example/my-repo.git"),
            Some("my-repo".into())
        );
        assert_eq!(
            default_clone_directory("git@github.com:example/my-repo.git"),
            Some("my-repo".into())
        );
        assert_eq!(
            default_clone_directory("C:\\projects\\local-repo"),
            Some("local-repo".into())
        );
    }

    #[test]
    fn opens_worktree_with_gitfile_and_external_git_directory() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let base =
            std::env::temp_dir().join(format!("gitvibe-gitfile-{}-{unique}", std::process::id()));
        std::fs::create_dir_all(&base).unwrap();
        let worktree = base.join("worktree");
        let metadata = base.join("metadata");
        let metadata_arg = metadata.to_string_lossy().into_owned();
        let worktree_arg = worktree.to_string_lossy().into_owned();
        let result = git_output(
            None,
            &["init", "--separate-git-dir", &metadata_arg, &worktree_arg],
        )
        .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(worktree.join(".git").is_file());
        assert!(metadata.join("HEAD").is_file());
        assert_eq!(
            discover(&worktree).unwrap().canonicalize().unwrap(),
            worktree.canonicalize().unwrap()
        );
        assert_eq!(
            snapshot_with_limit(&worktree, 20)
                .unwrap()
                .root
                .canonicalize()
                .unwrap(),
            worktree.canonicalize().unwrap()
        );
        std::fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn graph_tracks_merge_parent_lanes() {
        let make = |id: &str, parents: &[&str]| Commit {
            id: id.into(),
            short: id.into(),
            subject: id.into(),
            author: String::new(),
            date: String::new(),
            parents: parents.iter().map(|id| (*id).into()).collect(),
            lane: 0,
            lane_count: 0,
            graph_edges: Vec::new(),
            parent_edges: Vec::new(),
        };
        let mut commits = vec![
            make("merge", &["left", "right"]),
            make("left", &["base"]),
            make("right", &["base"]),
            make("base", &[]),
        ];
        assign_lanes(&mut commits);
        assert_eq!(commits[0].parent_edges.len(), 2);
        assert_eq!(commits[0].lane_count, 2);
        assert!(commits[1].graph_edges.iter().any(|(_, to)| *to == 1));
    }

    #[test]
    fn graph_keeps_all_lanes_for_large_merges() {
        let parents = (0..12).map(|n| format!("parent-{n}")).collect::<Vec<_>>();
        let mut commits = vec![Commit {
            id: "merge".into(),
            short: "merge".into(),
            subject: "merge".into(),
            author: String::new(),
            date: String::new(),
            parents,
            lane: 0,
            lane_count: 0,
            graph_edges: Vec::new(),
            parent_edges: Vec::new(),
        }];
        assign_lanes(&mut commits);
        assert_eq!(commits[0].lane_count, 12);
        assert_eq!(commits[0].parent_edges, (0..12).collect::<Vec<_>>());
    }

    #[test]
    fn snapshot_tracks_stage_commit_and_branch() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("gitvibe-test-{}-{unique}", std::process::id()));
        let root = init(&root).unwrap();
        let initial = snapshot(&root).unwrap();
        assert!(initial.commits.is_empty());
        std::fs::write(root.join("a file.txt"), "hello\n").unwrap();
        assert_eq!(snapshot(&root).unwrap().status[0].path, "a file.txt");
        run(&root, &["add", "--", "a file.txt"]).unwrap();
        assert!(snapshot(&root).unwrap().status[0].staged());
        run(
            &root,
            &[
                "-c",
                "user.name=GitVibe Test",
                "-c",
                "user.email=test@example.invalid",
                "commit",
                "-m",
                "Initial",
            ],
        )
        .unwrap();
        let committed = snapshot(&root).unwrap();
        assert_eq!(committed.commits.len(), 1);
        assert!(committed.status.is_empty());
        std::fs::write(root.join("a file.txt"), "hello again\n").unwrap();
        run(&root, &["add", "--", "a file.txt"]).unwrap();
        run(
            &root,
            &[
                "-c",
                "user.name=GitVibe Test",
                "-c",
                "user.email=test@example.invalid",
                "commit",
                "-m",
                "Second",
            ],
        )
        .unwrap();
        let limited = snapshot_with_limit(&root, 1).unwrap();
        assert_eq!(limited.commits.len(), 1);
        assert!(limited.has_more_commits);
        assert_eq!(snapshot_with_limit(&root, 2).unwrap().commits.len(), 2);
        assert!(!snapshot_with_limit(&root, 2).unwrap().has_more_commits);
        let clone_path = root.with_extension("cloned repository");
        let cloned =
            clone_repo_with_options(&root.to_string_lossy(), &clone_path, false, false).unwrap();
        let clone_snapshot = snapshot(&cloned).unwrap();
        assert_eq!(clone_snapshot.commits.len(), 2);
        assert!(!clone_snapshot.remote_branches.is_empty());
        let sparse_path = root.with_extension("sparse shallow clone");
        let sparse =
            clone_repo_with_options(&root.to_string_lossy(), &sparse_path, true, true).unwrap();
        assert_eq!(snapshot(&sparse).unwrap().commits.len(), 1);
        assert_eq!(
            run(&sparse, &["config", "--get", "core.sparseCheckout"]).unwrap(),
            "true"
        );
        std::fs::remove_dir_all(sparse).unwrap();
        std::fs::remove_dir_all(cloned).unwrap();
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn lists_linked_worktrees_and_initialized_submodules() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let base =
            std::env::temp_dir().join(format!("gitvibe-nested-{}-{unique}", std::process::id()));
        let root = init(&base.join("parent")).unwrap();
        std::fs::write(root.join("README.md"), "parent\n").unwrap();
        run(&root, &["add", "README.md"]).unwrap();
        run(
            &root,
            &[
                "-c",
                "user.name=GitVibe Test",
                "-c",
                "user.email=test@example.invalid",
                "commit",
                "-m",
                "Initial",
            ],
        )
        .unwrap();

        let linked = base.join("linked checkout");
        run(
            &root,
            &[
                "worktree",
                "add",
                "-b",
                "linked-test",
                &linked.to_string_lossy(),
            ],
        )
        .unwrap();
        let listed = worktrees(&root).unwrap();
        assert_eq!(listed.len(), 2);
        assert!(listed.iter().any(|tree| tree.path.canonicalize().unwrap()
            == linked.canonicalize().unwrap()
            && tree.branch.as_deref() == Some("linked-test")));
        assert_eq!(listed.iter().filter(|tree| tree.current).count(), 1);
        assert!(linked.join(".git").is_file());

        let child = init(&base.join("child source")).unwrap();
        std::fs::write(root.join(".gitmodules"), "").unwrap();
        assert!(submodules(&root).unwrap().is_empty());
        std::fs::write(child.join("child.txt"), "child\n").unwrap();
        run(&child, &["add", "child.txt"]).unwrap();
        run(
            &child,
            &[
                "-c",
                "user.name=GitVibe Test",
                "-c",
                "user.email=test@example.invalid",
                "commit",
                "-m",
                "Child",
            ],
        )
        .unwrap();
        run(
            &root,
            &[
                "-c",
                "protocol.file.allow=always",
                "submodule",
                "add",
                &child.to_string_lossy(),
                "modules/child",
            ],
        )
        .unwrap();
        let modules = submodules(&root).unwrap();
        assert_eq!(modules.len(), 1);
        assert_eq!(modules[0].path, "modules/child");
        assert!(modules[0].initialized);
        assert!(root.join("modules/child/.git").is_file());

        run(&root, &["worktree", "remove", &linked.to_string_lossy()]).unwrap();
        std::fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn detects_rebase_conflict_and_abort() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("gitvibe-rebase-{}-{unique}", std::process::id()));
        let root = init(&root).unwrap();
        run(&root, &["config", "user.name", "GitVibe Test"]).unwrap();
        run(&root, &["config", "user.email", "test@example.invalid"]).unwrap();
        let commit = |message: &str| {
            run(&root, &["add", "note.txt"]).unwrap();
            run(
                &root,
                &[
                    "-c",
                    "user.name=GitVibe Test",
                    "-c",
                    "user.email=test@example.invalid",
                    "commit",
                    "-m",
                    message,
                ],
            )
            .unwrap();
        };
        std::fs::write(root.join("note.txt"), "base\n").unwrap();
        commit("Base");
        let main_branch = run(&root, &["branch", "--show-current"]).unwrap();
        std::fs::write(root.join("note.txt"), "main\n").unwrap();
        commit("Main edit");
        run(&root, &["switch", "-c", "feature", "HEAD~1"]).unwrap();
        std::fs::write(root.join("note.txt"), "feature\n").unwrap();
        commit("Feature edit");
        assert!(run(&root, &["rebase", &main_branch]).is_err());
        assert!(snapshot(&root).unwrap().rebase_in_progress);
        run(&root, &["rebase", "--abort"]).unwrap();
        assert!(!snapshot(&root).unwrap().rebase_in_progress);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn exports_and_applies_working_tree_patch() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("gitvibe-patch-{}-{unique}", std::process::id()));
        let root = init(&root).unwrap();
        let file = root.join("note.txt");
        std::fs::write(&file, "before\n").unwrap();
        run(&root, &["add", "note.txt"]).unwrap();
        run(
            &root,
            &[
                "-c",
                "user.name=GitVibe Test",
                "-c",
                "user.email=test@example.invalid",
                "commit",
                "-m",
                "Before",
            ],
        )
        .unwrap();
        std::fs::write(&file, "after\n").unwrap();
        let patch = root.with_extension("patch");
        export_patch(
            &root,
            &[
                "diff".into(),
                "--binary".into(),
                "--".into(),
                "note.txt".into(),
            ],
            &patch,
        )
        .unwrap();
        assert!(
            std::fs::read_to_string(&patch)
                .unwrap()
                .contains("diff --git")
        );
        run(&root, &["restore", "note.txt"]).unwrap();
        run(&root, &["apply", "--", &patch.to_string_lossy()]).unwrap();
        assert_eq!(
            std::fs::read_to_string(&file)
                .unwrap()
                .replace("\r\n", "\n"),
            "after\n"
        );
        std::fs::remove_file(patch).unwrap();
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn manages_remote_and_annotated_tag() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("gitvibe-remotes-{}-{unique}", std::process::id()));
        let root = init(&root).unwrap();
        std::fs::write(root.join("README.md"), "hello\n").unwrap();
        run(&root, &["add", "README.md"]).unwrap();
        run(
            &root,
            &[
                "-c",
                "user.name=GitVibe Test",
                "-c",
                "user.email=test@example.invalid",
                "commit",
                "-m",
                "Initial",
            ],
        )
        .unwrap();
        run(
            &root,
            &[
                "remote",
                "add",
                "sample",
                "https://example.invalid/sample.git",
            ],
        )
        .unwrap();
        assert!(
            snapshot(&root)
                .unwrap()
                .remotes
                .iter()
                .any(|name| name == "sample")
        );
        run(
            &root,
            &[
                "-c",
                "user.name=GitVibe Test",
                "-c",
                "user.email=test@example.invalid",
                "tag",
                "-a",
                "v1",
                "-m",
                "Version one",
            ],
        )
        .unwrap();
        assert!(
            snapshot(&root)
                .unwrap()
                .tags
                .iter()
                .any(|tag| tag.name == "v1")
        );
        run(&root, &["remote", "remove", "sample"]).unwrap();
        assert!(
            !snapshot(&root)
                .unwrap()
                .remotes
                .iter()
                .any(|name| name == "sample")
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn stages_and_unstages_one_hunk_without_touching_other_edits() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("gitvibe-hunks-{}-{unique}", std::process::id()));
        let root = init(&root).unwrap();
        let file = root.join("notes.txt");
        let original = (0..30)
            .map(|i| format!("line {i:02}\n"))
            .collect::<String>();
        std::fs::write(&file, &original).unwrap();
        run(&root, &["add", "--", "notes.txt"]).unwrap();
        run(
            &root,
            &[
                "-c",
                "user.name=GitVibe Test",
                "-c",
                "user.email=test@example.invalid",
                "commit",
                "-m",
                "Base",
            ],
        )
        .unwrap();
        let edited = original
            .replace("line 01\n", "first edit\n")
            .replace("line 26\n", "second edit\n");
        std::fs::write(&file, edited).unwrap();
        let diff = run(&root, &["diff", "--no-color", "--", "notes.txt"]).unwrap();
        let hunks = diff_hunks(&diff);
        assert_eq!(hunks.len(), 2);
        apply_hunk(&root, &hunks[0].patch, false).unwrap();
        let staged = run(&root, &["diff", "--cached", "--", "notes.txt"]).unwrap();
        assert!(staged.contains("first edit"));
        assert!(!staged.contains("second edit"));
        let unstaged = run(&root, &["diff", "--", "notes.txt"]).unwrap();
        assert!(unstaged.contains("second edit"));
        assert!(!unstaged.contains("first edit"));
        let staged_hunk = diff_hunks(&staged);
        apply_hunk(&root, &staged_hunk[0].patch, true).unwrap();
        assert!(
            run(&root, &["diff", "--cached", "--", "notes.txt"])
                .unwrap()
                .is_empty()
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn stages_and_unstages_individual_lines_in_one_hunk() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = init(
            &std::env::temp_dir().join(format!("gitvibe-lines-{}-{unique}", std::process::id())),
        )
        .unwrap();
        let file = root.join("notes.txt");
        std::fs::write(&file, "alpha\nbeta\ngamma\n").unwrap();
        run(&root, &["add", "notes.txt"]).unwrap();
        run(
            &root,
            &[
                "-c",
                "user.name=GitVibe Test",
                "-c",
                "user.email=test@example.invalid",
                "commit",
                "-m",
                "Base",
            ],
        )
        .unwrap();
        std::fs::write(&file, "alpha\nnew one\nnew two\nbeta\ngamma\n").unwrap();
        let diff = run(&root, &["diff", "--no-color", "--", "notes.txt"]).unwrap();
        let hunk = diff_hunks(&diff).remove(0);
        let index = hunk
            .lines
            .iter()
            .position(|line| line == "+new one")
            .unwrap();
        apply_line(&root, &hunk.line_patch(index).unwrap(), false).unwrap();
        let staged = run(&root, &["diff", "--cached", "--", "notes.txt"]).unwrap();
        assert!(staged.contains("+new one"));
        assert!(!staged.contains("+new two"));
        let unstaged = run(&root, &["diff", "--", "notes.txt"]).unwrap();
        assert!(unstaged.contains("+new two"));
        let staged_hunk = diff_hunks(&staged).remove(0);
        let index = staged_hunk
            .lines
            .iter()
            .position(|line| line == "+new one")
            .unwrap();
        apply_line(&root, &staged_hunk.line_patch(index).unwrap(), true).unwrap();
        assert!(
            run(&root, &["diff", "--cached", "--", "notes.txt"])
                .unwrap()
                .is_empty()
        );
        std::fs::write(&file, "alpha\nBETA\ngamma\n").unwrap();
        let diff = run(&root, &["diff", "--no-color", "--", "notes.txt"]).unwrap();
        let hunk = diff_hunks(&diff).remove(0);
        let index = hunk.lines.iter().position(|line| line == "+BETA").unwrap();
        apply_line(&root, &hunk.line_patch(index).unwrap(), false).unwrap();
        let index_text = run(&root, &["show", ":notes.txt"]).unwrap();
        assert_eq!(index_text, "alpha\nBETA\nbeta\ngamma");
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn resolves_conflict_lines_and_rejects_external_edits() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = init(&std::env::temp_dir().join(format!(
            "gitvibe-conflict-lines-{}-{unique}",
            std::process::id()
        )))
        .unwrap();
        let file = root.join("notes.txt");
        let original = "before\n<<<<<<< HEAD\nours one\nours two\n=======\ntheirs one\n>>>>>>> feature\nafter\n";
        std::fs::write(&file, original).unwrap();
        let mut document = load_conflict(&root, "notes.txt").unwrap();
        if let ConflictPart::Block(block) = &mut document.parts[1] {
            block.selected_ours[1] = false;
            block.selected_theirs[0] = true;
        } else {
            panic!("expected conflict block");
        }
        assert_eq!(document.result(), "before\nours one\ntheirs one\nafter\n");
        std::fs::write(&file, "changed externally\n").unwrap();
        assert!(save_conflict(&root, &document).is_err());
        std::fs::write(&file, original).unwrap();
        save_conflict(&root, &document).unwrap();
        assert_eq!(
            run(&root, &["show", ":notes.txt"]).unwrap(),
            "before\nours one\ntheirs one\nafter"
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn searches_full_history_and_lists_file_revisions() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = init(
            &std::env::temp_dir().join(format!("gitvibe-search-{}-{unique}", std::process::id())),
        )
        .unwrap();
        let file = root.join("notes.txt");
        for i in 0..4 {
            std::fs::write(&file, format!("version {i}\n")).unwrap();
            run(&root, &["add", "notes.txt"]).unwrap();
            run(
                &root,
                &[
                    "-c",
                    "user.name=GitVibe Test",
                    "-c",
                    "user.email=test@example.invalid",
                    "commit",
                    "-m",
                    &format!("milestone {i}"),
                ],
            )
            .unwrap();
        }
        let (hits, more) = search_history(&root, "milestone 0").unwrap();
        assert_eq!(hits.len(), 1);
        assert!(!more);
        assert_eq!(file_history(&root, "notes.txt").unwrap().len(), 4);
        assert_eq!(
            commit_files(&root, &hits[0].id).unwrap()[0].path,
            "notes.txt"
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn deletes_only_selected_local_branch_and_tag() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("gitvibe-refs-{}-{unique}", std::process::id()));
        let root = init(&root).unwrap();
        std::fs::write(root.join("readme.txt"), "base\n").unwrap();
        run(&root, &["add", "--", "readme.txt"]).unwrap();
        run(
            &root,
            &[
                "-c",
                "user.name=GitVibe Test",
                "-c",
                "user.email=test@example.invalid",
                "commit",
                "-m",
                "Base",
            ],
        )
        .unwrap();
        run(&root, &["branch", "old-branch"]).unwrap();
        run(&root, &["tag", "old-tag"]).unwrap();
        run(&root, &["branch", "-d", "--", "old-branch"]).unwrap();
        run(&root, &["tag", "-d", "--", "old-tag"]).unwrap();
        let snapshot = snapshot(&root).unwrap();
        assert!(!snapshot.branches.iter().any(|r| r.name == "old-branch"));
        assert!(!snapshot.tags.iter().any(|r| r.name == "old-tag"));
        assert!(snapshot.branches.iter().any(|r| r.current));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn detects_conflict_and_resolves_chosen_side() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("gitvibe-conflict-{}-{unique}", std::process::id()));
        let root = init(&root).unwrap();
        run(&root, &["config", "user.name", "GitVibe Test"]).unwrap();
        run(&root, &["config", "user.email", "test@example.invalid"]).unwrap();
        let file = root.join("shared.txt");
        let commit = |message: &str| {
            run(&root, &["add", "--", "shared.txt"]).unwrap();
            run(
                &root,
                &[
                    "-c",
                    "user.name=GitVibe Test",
                    "-c",
                    "user.email=test@example.invalid",
                    "commit",
                    "-m",
                    message,
                ],
            )
            .unwrap();
        };
        std::fs::write(&file, "base\n").unwrap();
        commit("Base");
        let initial_branch = run(&root, &["branch", "--show-current"]).unwrap();
        run(&root, &["switch", "-c", "topic"]).unwrap();
        std::fs::write(&file, "topic change\n").unwrap();
        commit("Topic");
        run(&root, &["switch", &initial_branch]).unwrap();
        std::fs::write(&file, "current change\n").unwrap();
        commit("Current");
        let merge = run(&root, &["merge", "topic"]);
        assert!(merge.is_err(), "Expected a merge conflict, got {merge:?}");
        let conflicted = snapshot(&root).unwrap();
        assert!(
            conflicted.status.iter().any(FileStatus::conflicted),
            "Merge failed without an unmerged file: {merge:?}"
        );
        assert!(mark_conflict_resolved(&root, "shared.txt").is_err());
        choose_conflict_side(&root, "shared.txt", ConflictSide::Ours).unwrap();
        assert_eq!(
            std::fs::read_to_string(&file).unwrap().trim(),
            "current change"
        );
        let resolved = snapshot(&root).unwrap();
        assert!(!resolved.status.iter().any(FileStatus::conflicted));
        assert!(resolved.merge_in_progress);
        assert!(resolved.status.is_empty());
        run(&root, &["merge", "--abort"]).unwrap();
        assert!(!snapshot(&root).unwrap().merge_in_progress);
        assert!(run(&root, &["merge", "topic"]).is_err());
        choose_conflict_side(&root, "shared.txt", ConflictSide::Theirs).unwrap();
        assert_eq!(
            std::fs::read_to_string(&file).unwrap().trim(),
            "topic change"
        );
        let resolved = snapshot(&root).unwrap();
        assert!(!resolved.status.iter().any(FileStatus::conflicted));
        assert!(resolved.status.iter().any(FileStatus::staged));
        run(&root, &["merge", "--abort"]).unwrap();
        assert!(run(&root, &["merge", "topic"]).is_err());
        std::fs::write(&file, "combined change\n").unwrap();
        mark_conflict_resolved(&root, "shared.txt").unwrap();
        let resolved = snapshot(&root).unwrap();
        assert!(!resolved.status.iter().any(FileStatus::conflicted));
        assert!(resolved.status.iter().any(FileStatus::staged));
        std::fs::remove_dir_all(root).unwrap();
    }
}
