use std::{
    io::Write,
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
    let mut command = Command::new("git");
    command.arg("--no-pager").arg("-C").arg(repo).arg("apply");
    if reverse {
        command.arg("--reverse");
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

pub fn clone_repo(url: &str, destination: &Path) -> Result<PathBuf, String> {
    let destination_arg = destination.to_string_lossy().into_owned();
    let mut args = vec!["clone"];
    if Path::new(url).exists() {
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
    })
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
    let mut commits = Vec::new();
    for record in text.split('\x1e') {
        let fields: Vec<&str> = record.trim().split('\x1f').collect();
        if fields.len() != 6 {
            continue;
        }
        commits.push(Commit {
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
        });
    }
    let has_more = commits.len() > limit;
    commits.truncate(limit);
    assign_lanes(&mut commits);
    Ok((commits, has_more))
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

pub fn split_command(input: &str) -> Result<Vec<String>, String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut quote = None;
    let mut started = false;
    let mut escaped = false;
    for ch in input.chars() {
        if escaped {
            current.push(ch);
            escaped = false;
            started = true;
            continue;
        }
        match ch {
            '\\' if quote == Some('"') => {
                escaped = true;
            }
            '\'' | '"' if quote == Some(ch) => {
                quote = None;
            }
            '\'' | '"' if quote.is_none() => {
                quote = Some(ch);
                started = true;
            }
            c if c.is_whitespace() && quote.is_none() => {
                if started {
                    args.push(std::mem::take(&mut current));
                    started = false;
                }
            }
            _ => {
                current.push(ch);
                started = true;
            }
        }
    }
    if escaped || quote.is_some() {
        return Err("Unclosed quote or escape".into());
    }
    if started {
        args.push(current);
    }
    if args
        .first()
        .is_some_and(|first| first.eq_ignore_ascii_case("git"))
    {
        args.remove(0);
    }
    if args.is_empty() {
        return Err("Enter a Git command".into());
    }
    Ok(args)
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
    fn splits_quoted_git_command_without_shell() {
        assert_eq!(
            split_command("git commit -m 'hello world'").unwrap(),
            ["commit", "-m", "hello world"]
        );
        assert_eq!(
            split_command("git add -- \"a file.txt\"").unwrap(),
            ["add", "--", "a file.txt"]
        );
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
        let cloned = clone_repo(&root.to_string_lossy(), &clone_path).unwrap();
        let clone_snapshot = snapshot(&cloned).unwrap();
        assert_eq!(clone_snapshot.commits.len(), 2);
        assert!(!clone_snapshot.remote_branches.is_empty());
        std::fs::remove_dir_all(cloned).unwrap();
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
}
