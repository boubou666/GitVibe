use std::{
    path::{Path, PathBuf},
    process::Command,
    sync::mpsc::{self, Receiver},
    thread,
};

use crate::{
    git::{self, Snapshot},
    github, updates,
};
use eframe::egui::{self, Color32, RichText, Stroke};

const BG: Color32 = Color32::from_rgb(27, 29, 36);
const PANEL: Color32 = Color32::from_rgb(38, 41, 50);
const PANEL_ALT: Color32 = Color32::from_rgb(48, 52, 63);
const ELEVATED: Color32 = Color32::from_rgb(53, 70, 99);
const BORDER: Color32 = Color32::from_rgb(62, 68, 80);
const TEXT: Color32 = Color32::from_rgb(234, 238, 245);
const MUTED: Color32 = Color32::from_rgb(155, 166, 184);
const ACCENT: Color32 = Color32::from_rgb(61, 196, 230);
const ORANGE: Color32 = Color32::from_rgb(255, 177, 96);
const RED: Color32 = Color32::from_rgb(255, 111, 134);
const VIOLET: Color32 = Color32::from_rgb(174, 145, 255);
const BLUE: Color32 = Color32::from_rgb(116, 172, 255);

#[derive(Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
enum ThemeChoice {
    Aurora,
    Cosmic,
    Ember,
}

impl ThemeChoice {
    fn label(self) -> &'static str {
        match self {
            Self::Aurora => "Aurora",
            Self::Cosmic => "Cosmic",
            Self::Ember => "Ember",
        }
    }
}

#[derive(Clone, Copy)]
struct Palette {
    bg: Color32,
    panel: Color32,
    panel_alt: Color32,
    elevated: Color32,
    border: Color32,
    text: Color32,
    muted: Color32,
    accent: Color32,
    orange: Color32,
    red: Color32,
    violet: Color32,
    blue: Color32,
}

impl Palette {
    const fn for_choice(choice: ThemeChoice) -> Self {
        match choice {
            ThemeChoice::Aurora => Self {
                bg: BG,
                panel: PANEL,
                panel_alt: PANEL_ALT,
                elevated: ELEVATED,
                border: BORDER,
                text: TEXT,
                muted: MUTED,
                accent: ACCENT,
                orange: ORANGE,
                red: RED,
                violet: VIOLET,
                blue: BLUE,
            },
            ThemeChoice::Cosmic => Self {
                bg: Color32::from_rgb(13, 11, 26),
                panel: Color32::from_rgb(24, 20, 43),
                panel_alt: Color32::from_rgb(34, 28, 56),
                elevated: Color32::from_rgb(47, 37, 70),
                border: Color32::from_rgb(71, 58, 96),
                text: Color32::from_rgb(244, 235, 255),
                muted: Color32::from_rgb(169, 153, 193),
                accent: Color32::from_rgb(207, 139, 255),
                orange: Color32::from_rgb(255, 188, 111),
                red: Color32::from_rgb(255, 119, 167),
                violet: Color32::from_rgb(183, 156, 255),
                blue: Color32::from_rgb(126, 188, 255),
            },
            ThemeChoice::Ember => Self {
                bg: Color32::from_rgb(23, 16, 17),
                panel: Color32::from_rgb(34, 25, 28),
                panel_alt: Color32::from_rgb(49, 34, 37),
                elevated: Color32::from_rgb(64, 42, 44),
                border: Color32::from_rgb(91, 60, 61),
                text: Color32::from_rgb(255, 240, 225),
                muted: Color32::from_rgb(190, 156, 146),
                accent: Color32::from_rgb(255, 183, 105),
                orange: Color32::from_rgb(255, 199, 101),
                red: Color32::from_rgb(255, 114, 118),
                violet: Color32::from_rgb(204, 160, 255),
                blue: Color32::from_rgb(118, 194, 232),
            },
        }
    }
}

thread_local! {
    static PALETTE: std::cell::Cell<Palette> = const { std::cell::Cell::new(Palette::for_choice(ThemeChoice::Aurora)) };
}

fn color(f: impl FnOnce(Palette) -> Color32) -> Color32 {
    PALETTE.with(|palette| f(palette.get()))
}
fn bg() -> Color32 {
    color(|p| p.bg)
}
fn panel() -> Color32 {
    color(|p| p.panel)
}
fn panel_alt() -> Color32 {
    color(|p| p.panel_alt)
}
fn elevated() -> Color32 {
    color(|p| p.elevated)
}
fn border() -> Color32 {
    color(|p| p.border)
}
fn text() -> Color32 {
    color(|p| p.text)
}
fn muted() -> Color32 {
    color(|p| p.muted)
}
fn accent() -> Color32 {
    color(|p| p.accent)
}
fn orange() -> Color32 {
    color(|p| p.orange)
}
fn red() -> Color32 {
    color(|p| p.red)
}
fn violet() -> Color32 {
    color(|p| p.violet)
}
fn blue() -> Color32 {
    color(|p| p.blue)
}

#[derive(PartialEq, Clone, Copy)]
enum Page {
    History,
    Changes,
    CommitDiff,
    NewTab,
    Repositories,
    Changelog,
    Branches,
    Stashes,
    Worktrees,
    Submodules,
    Rebase,
    PullRequests,
    Console,
    Updates,
}

#[derive(Clone, Copy)]
enum PaletteAction {
    Show(Page),
    SearchHistory,
    Refresh,
    Fetch,
    Pull,
    Push,
    ToggleTerminal,
    StageSelected,
    UnstageSelected,
    StageAll,
    UnstageAll,
    UndoIndex,
    RedoIndex,
}

impl PaletteAction {
    fn needs_repository(self) -> bool {
        !matches!(
            self,
            Self::Show(Page::Repositories | Page::NewTab | Page::Changelog)
        )
    }

    fn needs_idle(self) -> bool {
        matches!(
            self,
            Self::Refresh
                | Self::Fetch
                | Self::Pull
                | Self::Push
                | Self::StageSelected
                | Self::UnstageSelected
                | Self::StageAll
                | Self::UnstageAll
                | Self::UndoIndex
                | Self::RedoIndex
        )
    }
}

const PALETTE_ACTIONS: &[(&str, PaletteAction)] = &[
    (
        "Open repository manager",
        PaletteAction::Show(Page::Repositories),
    ),
    ("New tab", PaletteAction::Show(Page::NewTab)),
    ("View history", PaletteAction::Show(Page::History)),
    ("View changes", PaletteAction::Show(Page::Changes)),
    ("Search commits", PaletteAction::SearchHistory),
    ("Branches and tags", PaletteAction::Show(Page::Branches)),
    ("Worktrees", PaletteAction::Show(Page::Worktrees)),
    ("Submodules", PaletteAction::Show(Page::Submodules)),
    ("Rebase", PaletteAction::Show(Page::Rebase)),
    ("Stashes", PaletteAction::Show(Page::Stashes)),
    ("Pull requests", PaletteAction::Show(Page::PullRequests)),
    ("Terminal", PaletteAction::ToggleTerminal),
    (
        "Stage selected file · Ctrl/Cmd+Shift+S",
        PaletteAction::StageSelected,
    ),
    (
        "Unstage selected file · Ctrl/Cmd+Shift+U",
        PaletteAction::UnstageSelected,
    ),
    ("Stage all changes", PaletteAction::StageAll),
    ("Unstage all changes", PaletteAction::UnstageAll),
    ("Undo staging change · Ctrl/Cmd+Z", PaletteAction::UndoIndex),
    (
        "Redo staging change · Ctrl/Cmd+Shift+Z",
        PaletteAction::RedoIndex,
    ),
    ("Refresh repository", PaletteAction::Refresh),
    ("Fetch all", PaletteAction::Fetch),
    ("Pull", PaletteAction::Pull),
    ("Push", PaletteAction::Push),
    ("Updates", PaletteAction::Show(Page::Updates)),
    ("Changelog", PaletteAction::Show(Page::Changelog)),
];

#[derive(Clone)]
enum UpdateState {
    Checking,
    UpToDate(String),
    Available(updates::Release),
    Downloading(updates::Release),
    Ready(updates::Release, PathBuf),
    Failed(String),
}

enum UpdateDone {
    Checked(Result<updates::Check, String>),
    Downloaded(updates::Release, Result<PathBuf, String>),
}

#[derive(Clone, Copy)]
enum CommitAction {
    CherryPick,
    Revert,
}

#[derive(Clone, Copy)]
enum RefAtKind {
    Branch,
    Tag,
}

#[derive(Clone)]
enum RefDeletion {
    Branch(String),
    Tag(String),
}

#[derive(Clone, Copy, PartialEq)]
enum ResetMode {
    Soft,
    Mixed,
    Hard,
}

#[derive(Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
enum PullMode {
    FetchAll,
    Merge,
    FastForwardOnly,
    Rebase,
}

impl PullMode {
    fn label(self) -> &'static str {
        match self {
            Self::FetchAll => "Fetch all",
            Self::Merge => "Pull (fast-forward if possible)",
            Self::FastForwardOnly => "Pull (fast-forward only)",
            Self::Rebase => "Pull (rebase)",
        }
    }
}

enum Job {
    Open(PathBuf),
    Init(PathBuf),
    Clone(String, PathBuf, bool, bool),
    Run(Vec<String>),
    UndoIndex(git::IndexChange, bool),
    SwitchBranch(String, bool, bool),
    Shell(String, PathBuf),
    LoadPullRequests,
    CreatePullRequest {
        head: String,
        base: String,
        title: String,
        body: String,
    },
    ExportPatch(Vec<String>, PathBuf),
    Inspect(Vec<String>),
    InspectCommit(String),
    InspectFile(String),
    InspectConflict(String),
    ApplyHunk(String, bool),
    DiscardHunk(String),
    ApplyLine(String, bool),
    SearchHistory(String),
    FileHistory(String),
    LoadConflict(String),
    SaveConflict(git::ConflictDocument),
    LoadInteractiveRebase(String),
    StartInteractiveRebase(git::RebasePlan),
    ResolveConflict(String, git::ConflictSide),
    MarkResolved(String),
    Refresh,
}

enum Done {
    Loaded(Result<Snapshot, String>),
    Ran(Result<(String, Snapshot), String>),
    RanFailed(String, Snapshot),
    IndexChanged(String, Snapshot, Option<git::IndexChange>),
    IndexMoved(String, Snapshot, git::IndexChange, bool),
    Shell(Result<String, String>),
    PullRequests(Result<Vec<github::PullRequest>, String>),
    PullRequestCreated(Result<String, String>),
    Exported(Result<String, String>),
    Inspected(Result<String, String>),
    CommitInspected(String, Result<(String, Vec<git::CommitFile>), String>),
    Searched(Result<(Vec<git::Commit>, bool), String>),
    FileHistory(String, Result<Vec<git::Commit>, String>),
    ConflictLoaded(Result<git::ConflictDocument, String>),
    InteractiveRebaseLoaded(Result<git::RebasePlan, String>),
}

fn finish_index_job<F>(
    repo: Option<PathBuf>,
    history_limit: usize,
    label: &str,
    operation: F,
) -> Done
where
    F: FnOnce(&Path) -> Result<String, String>,
{
    let Some(path) = repo else {
        return Done::Ran(Err("No repository selected".into()));
    };
    let result = git::with_index_history(&path, label, || operation(&path));
    match git::snapshot_with_limit(&path, history_limit) {
        Ok(snapshot) => match result {
            Ok((output, change)) => Done::IndexChanged(output, snapshot, change),
            Err(error) => Done::RanFailed(error, snapshot),
        },
        Err(error) => Done::Ran(Err(error)),
    }
}

pub struct GitVibe {
    repo: Option<PathBuf>,
    open_repo_tabs: Vec<PathBuf>,
    last_active_repo: Option<PathBuf>,
    changelog_open: bool,
    new_tab_open: bool,
    recent_repos: Vec<PathBuf>,
    pinned_repos: Vec<PathBuf>,
    repo_filter: String,
    branch_filter: String,
    theme_choice: ThemeChoice,
    pull_mode: PullMode,
    snapshot: Option<Snapshot>,
    page: Page,
    path_input: String,
    clone_url: String,
    clone_destination: String,
    clone_source: usize,
    clone_shallow: bool,
    clone_sparse: bool,
    worktree_path: String,
    worktree_branch: String,
    worktree_ref: String,
    submodule_url: String,
    submodule_path: String,
    rebase_onto: String,
    rebase_autostash: bool,
    interactive_plan: Option<git::RebasePlan>,
    confirm_interactive_rebase: bool,
    diff_wrap: bool,
    diff_split: bool,
    diff_hunk_focus: usize,
    diff_scroll_pending: bool,
    pr_base: String,
    pr_title: String,
    pr_body: String,
    pull_requests: Vec<github::PullRequest>,
    pr_loaded_for: Option<PathBuf>,
    branch_input: String,
    tag_input: String,
    tag_message: String,
    annotated_tag: bool,
    remote_name: String,
    remote_url: String,
    commit_message: String,
    command_input: String,
    terminal_output: String,
    terminal_cwd: Option<PathBuf>,
    command_history: Vec<String>,
    command_history_index: usize,
    terminal_open: bool,
    selected_commit: Option<String>,
    selected_commit_file: Option<String>,
    inline_branch_at: Option<String>,
    confirm_switch_branch: Option<(String, bool)>,
    commit_files: Vec<git::CommitFile>,
    commit_files_id: Option<String>,
    compare_base: Option<String>,
    history_limit: usize,
    selected_file: Option<String>,
    selected_file_staged: bool,
    detail: String,
    output: String,
    notice: String,
    error: String,
    search: String,
    palette_open: bool,
    palette_query: String,
    palette_index: usize,
    palette_focus: bool,
    graph_scroll_pending: bool,
    undo_stack: Vec<git::IndexChange>,
    redo_stack: Vec<git::IndexChange>,
    search_results: Vec<git::Commit>,
    search_more: bool,
    search_active: bool,
    file_history_path: Option<String>,
    file_history: Vec<git::Commit>,
    conflict_editor: Option<git::ConflictDocument>,
    receiver: Option<Receiver<Done>>,
    pending: Option<Job>,
    busy: bool,
    show_clone: bool,
    confirm_discard: Option<String>,
    confirm_discard_hunk: Option<String>,
    confirm_commit_action: Option<(CommitAction, String)>,
    confirm_checkout_commit: Option<String>,
    create_ref_at: Option<(RefAtKind, String)>,
    confirm_delete_ref: Option<RefDeletion>,
    confirm_remove_remote: Option<String>,
    edit_remote: Option<(String, String)>,
    confirm_reset: Option<String>,
    confirm_conflict_side: Option<(String, git::ConflictSide)>,
    confirm_abort_merge: bool,
    confirm_remove_worktree: Option<PathBuf>,
    confirm_rebase: Option<Vec<String>>,
    reset_mode: ResetMode,
    reset_confirmation: String,
    committing: bool,
    hunk_action: bool,
    conflict_action: bool,
    update_state: UpdateState,
    update_receiver: Option<Receiver<UpdateDone>>,
}

impl GitVibe {
    fn set_theme(&self, ctx: &egui::Context) {
        PALETTE.with(|palette| palette.set(Palette::for_choice(self.theme_choice)));
        ctx.set_theme(egui::Theme::Dark);
        let mut visuals = ctx.style_of(egui::Theme::Dark).visuals.clone();
        visuals.override_text_color = Some(text());
        visuals.weak_text_color = Some(muted());
        visuals.panel_fill = bg();
        visuals.window_fill = panel();
        visuals.window_stroke = Stroke::new(1.0, border());
        visuals.extreme_bg_color = bg();
        visuals.text_edit_bg_color = Some(panel_alt());
        visuals.code_bg_color = panel_alt();
        visuals.widgets.inactive.bg_fill = panel_alt();
        visuals.widgets.inactive.weak_bg_fill = panel_alt();
        visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, border());
        visuals.widgets.hovered.bg_fill = elevated();
        visuals.widgets.hovered.weak_bg_fill = elevated();
        visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, accent());
        visuals.widgets.active.bg_fill = elevated();
        visuals.widgets.active.weak_bg_fill = elevated();
        visuals.widgets.active.bg_stroke = Stroke::new(1.0, accent());
        visuals.selection.bg_fill = elevated();
        visuals.selection.stroke = Stroke::new(1.0, accent());
        visuals.hyperlink_color = accent();
        ctx.set_visuals_of(egui::Theme::Dark, visuals.clone());
        ctx.set_visuals_of(egui::Theme::Light, visuals);
    }

    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        cc.egui_ctx.set_zoom_factor(1.16);
        cc.egui_ctx.set_theme(egui::Theme::Dark);
        let theme_choice = cc
            .storage
            .and_then(|storage| eframe::get_value::<ThemeChoice>(storage, "theme_choice"))
            .unwrap_or(ThemeChoice::Aurora);
        let pull_mode = cc
            .storage
            .and_then(|storage| eframe::get_value::<PullMode>(storage, "pull_mode"))
            .unwrap_or(PullMode::Merge);
        PALETTE.with(|palette| palette.set(Palette::for_choice(theme_choice)));
        let mut visuals = egui::Visuals::dark();
        visuals.override_text_color = Some(text());
        visuals.weak_text_color = Some(muted());
        visuals.panel_fill = bg();
        visuals.window_fill = panel();
        visuals.window_stroke = Stroke::new(1.0, border());
        visuals.window_corner_radius = egui::CornerRadius::same(5);
        visuals.extreme_bg_color = bg();
        visuals.text_edit_bg_color = Some(panel_alt());
        visuals.code_bg_color = panel_alt();
        visuals.widgets.inactive.bg_fill = panel_alt();
        visuals.widgets.inactive.weak_bg_fill = panel_alt();
        visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, border());
        visuals.widgets.hovered.bg_fill = elevated();
        visuals.widgets.hovered.weak_bg_fill = elevated();
        visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, accent());
        visuals.widgets.active.bg_fill = elevated();
        visuals.widgets.active.weak_bg_fill = elevated();
        visuals.widgets.active.bg_stroke = Stroke::new(1.0, accent());
        for widget in [
            &mut visuals.widgets.inactive,
            &mut visuals.widgets.hovered,
            &mut visuals.widgets.active,
            &mut visuals.widgets.open,
        ] {
            widget.corner_radius = egui::CornerRadius::same(3);
        }
        visuals.selection.bg_fill = elevated();
        visuals.selection.stroke = Stroke::new(1.0, accent());
        visuals.hyperlink_color = accent();
        cc.egui_ctx
            .set_visuals_of(egui::Theme::Dark, visuals.clone());
        cc.egui_ctx.set_visuals_of(egui::Theme::Light, visuals);
        cc.egui_ctx.all_styles_mut(|s| {
            s.spacing.item_spacing = egui::vec2(5.0, 4.0);
            s.spacing.button_padding = egui::vec2(8.0, 4.0);
            s.spacing.interact_size.y = 26.0;
        });
        let recent_repos = cc
            .storage
            .and_then(|storage| eframe::get_value::<Vec<PathBuf>>(storage, "recent_repos"))
            .unwrap_or_default();
        let pinned_repos = cc
            .storage
            .and_then(|storage| eframe::get_value::<Vec<PathBuf>>(storage, "pinned_repos"))
            .unwrap_or_default();
        let open_repo_tabs = cc
            .storage
            .and_then(|storage| eframe::get_value::<Vec<PathBuf>>(storage, "open_repo_tabs"))
            .unwrap_or_default()
            .into_iter()
            .filter(|path: &PathBuf| path.exists())
            .collect::<Vec<_>>();
        let last_active_repo = cc
            .storage
            .and_then(|storage| eframe::get_value::<Option<PathBuf>>(storage, "last_active_repo"))
            .flatten()
            .filter(|path| path.exists());
        let changelog_open = cc
            .storage
            .and_then(|storage| eframe::get_value::<bool>(storage, "changelog_open"))
            .unwrap_or(true);
        let path = std::env::args_os()
            .nth(1)
            .map(PathBuf::from)
            .or_else(|| last_active_repo.clone())
            .or_else(|| open_repo_tabs.first().cloned())
            .or_else(|| {
                recent_repos
                    .iter()
                    .find(|path| git::discover(path).is_ok())
                    .cloned()
            })
            .or_else(|| std::env::current_dir().ok())
            .unwrap_or_default();
        let mut app = Self {
            repo: None,
            open_repo_tabs,
            last_active_repo,
            changelog_open,
            new_tab_open: false,
            recent_repos,
            pinned_repos,
            repo_filter: String::new(),
            branch_filter: String::new(),
            theme_choice,
            pull_mode,
            snapshot: None,
            page: Page::History,
            path_input: path.to_string_lossy().into_owned(),
            clone_url: String::new(),
            clone_destination: String::new(),
            clone_source: 0,
            clone_shallow: false,
            clone_sparse: false,
            worktree_path: String::new(),
            worktree_branch: String::new(),
            worktree_ref: "HEAD".to_owned(),
            submodule_url: String::new(),
            submodule_path: String::new(),
            rebase_onto: String::new(),
            rebase_autostash: false,
            interactive_plan: None,
            confirm_interactive_rebase: false,
            diff_wrap: false,
            diff_split: false,
            diff_hunk_focus: 0,
            diff_scroll_pending: false,
            pr_base: "main".to_owned(),
            pr_title: String::new(),
            pr_body: String::new(),
            pull_requests: Vec::new(),
            pr_loaded_for: None,
            branch_input: String::new(),
            tag_input: String::new(),
            tag_message: String::new(),
            annotated_tag: false,
            remote_name: String::new(),
            remote_url: String::new(),
            commit_message: String::new(),
            command_input: String::new(),
            terminal_output: String::new(),
            terminal_cwd: None,
            command_history: Vec::new(),
            command_history_index: 0,
            terminal_open: false,
            selected_commit: None,
            selected_commit_file: None,
            inline_branch_at: None,
            confirm_switch_branch: None,
            commit_files: Vec::new(),
            commit_files_id: None,
            compare_base: None,
            history_limit: 300,
            selected_file: None,
            selected_file_staged: false,
            detail: String::new(),
            output: String::new(),
            notice: String::new(),
            error: String::new(),
            search: String::new(),
            palette_open: false,
            palette_query: String::new(),
            palette_index: 0,
            palette_focus: false,
            graph_scroll_pending: false,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            search_results: Vec::new(),
            search_more: false,
            search_active: false,
            file_history_path: None,
            file_history: Vec::new(),
            conflict_editor: None,
            receiver: None,
            pending: None,
            busy: false,
            show_clone: false,
            confirm_discard: None,
            confirm_discard_hunk: None,
            confirm_commit_action: None,
            confirm_checkout_commit: None,
            create_ref_at: None,
            confirm_delete_ref: None,
            confirm_remove_remote: None,
            edit_remote: None,
            confirm_reset: None,
            confirm_conflict_side: None,
            confirm_abort_merge: false,
            confirm_remove_worktree: None,
            confirm_rebase: None,
            reset_mode: ResetMode::Mixed,
            reset_confirmation: String::new(),
            committing: false,
            hunk_action: false,
            conflict_action: false,
            update_state: UpdateState::Checking,
            update_receiver: None,
        };
        if git::discover(&path).is_ok() {
            app.pending = Some(Job::Open(path));
        } else {
            app.page = Page::Repositories;
        }
        app.check_updates(&cc.egui_ctx);
        app
    }

    fn check_updates(&mut self, ctx: &egui::Context) {
        let (sender, receiver) = mpsc::channel();
        self.update_receiver = Some(receiver);
        self.update_state = UpdateState::Checking;
        let ctx = ctx.clone();
        thread::spawn(move || {
            let _ = sender.send(UpdateDone::Checked(updates::check_latest()));
            ctx.request_repaint();
        });
    }

    fn download_update(&mut self, ctx: &egui::Context, release: updates::Release) {
        let (sender, receiver) = mpsc::channel();
        self.update_receiver = Some(receiver);
        self.update_state = UpdateState::Downloading(release.clone());
        let ctx = ctx.clone();
        thread::spawn(move || {
            let result = updates::download(&release);
            let _ = sender.send(UpdateDone::Downloaded(release, result));
            ctx.request_repaint();
        });
    }

    fn poll_updates(&mut self) {
        let Some(receiver) = &self.update_receiver else {
            return;
        };
        let Ok(done) = receiver.try_recv() else {
            return;
        };
        self.update_receiver = None;
        self.update_state = match done {
            UpdateDone::Checked(Ok(updates::Check::UpToDate(latest))) => {
                UpdateState::UpToDate(latest)
            }
            UpdateDone::Checked(Ok(updates::Check::Available(release))) => {
                UpdateState::Available(release)
            }
            UpdateDone::Checked(Err(error)) | UpdateDone::Downloaded(_, Err(error)) => {
                UpdateState::Failed(error)
            }
            UpdateDone::Downloaded(release, Ok(path)) => UpdateState::Ready(release, path),
        };
    }

    fn queue(&mut self, job: Job) {
        if !self.busy {
            self.notice.clear();
            if matches!(&job, Job::Open(_) | Job::Init(_) | Job::Clone(_, _, _, _)) {
                self.history_limit = 300;
            }
            self.pending = Some(job);
        }
    }
    fn git(&mut self, args: &[&str]) {
        self.queue(Job::Run(args.iter().map(|s| s.to_string()).collect()));
    }
    fn git_owned(&mut self, args: Vec<String>) {
        self.queue(Job::Run(args));
    }

    fn launch(&mut self, ctx: &egui::Context) {
        let Some(job) = self.pending.take() else {
            return;
        };
        let repo = self.repo.clone();
        let history_limit = self.history_limit;
        let (sender, receiver) = mpsc::channel();
        self.receiver = Some(receiver);
        self.busy = true;
        let ctx = ctx.clone();
        thread::spawn(move || {
            let result = match job {
                Job::Open(path) => Done::Loaded(git::snapshot_with_limit(&path, history_limit)),
                Job::Init(path) => Done::Loaded(
                    git::init(&path).and_then(|p| git::snapshot_with_limit(&p, history_limit)),
                ),
                Job::Clone(url, path, shallow, sparse) => Done::Loaded(
                    git::clone_repo_with_options(&url, &path, shallow, sparse)
                        .and_then(|p| git::snapshot_with_limit(&p, history_limit)),
                ),
                Job::LoadPullRequests => Done::PullRequests(
                    repo.ok_or("No repository selected".to_owned())
                        .and_then(|path| github::list(&path)),
                ),
                Job::CreatePullRequest { head, base, title, body } => Done::PullRequestCreated(
                    repo.ok_or("No repository selected".to_owned())
                        .and_then(|path| github::create(&path, &head, &base, &title, &body)),
                ),
                Job::ExportPatch(args, path) => Done::Exported(
                    repo.ok_or("No repository selected".to_owned())
                        .and_then(|repo| git::export_patch(&repo, &args, &path)),
                ),
                Job::LoadInteractiveRebase(target) => Done::InteractiveRebaseLoaded(
                    repo.ok_or("No repository selected".to_owned())
                        .and_then(|path| git::load_rebase_plan(&path, &target)),
                ),
                Job::StartInteractiveRebase(plan) => match repo {
                    Some(path) => {
                        let operation = git::start_interactive_rebase(&path, &plan);
                        match git::snapshot_with_limit(&path, history_limit) {
                            Ok(snapshot) => match operation {
                                Ok(output) => Done::Ran(Ok((output, snapshot))),
                                Err(error) => Done::RanFailed(error, snapshot),
                            },
                            Err(error) => Done::Ran(Err(error)),
                        }
                    }
                    None => Done::Ran(Err("No repository selected".to_owned())),
                },
                Job::Refresh => Done::Loaded(
                    repo.ok_or("No repository selected".to_owned())
                        .and_then(|p| git::snapshot_with_limit(&p, history_limit)),
                ),
                Job::Run(args) if git::is_index_mutation(&args) => {
                    let label = if args.first().is_some_and(|arg| arg == "add") {
                        "Stage changes"
                    } else {
                        "Unstage changes"
                    };
                    finish_index_job(repo, history_limit, label, |path| git::run_owned(path, &args))
                }
                Job::Run(args) => match repo {
                    Some(p) => {
                        let operation = git::run_owned(&p, &args);
                        match git::snapshot_with_limit(&p, history_limit) {
                            Ok(snapshot) => {
                                if !snapshot.rebase_in_progress {
                                    let _ = git::cleanup_interactive_rebase(&p);
                                }
                                match operation {
                                    Ok(output) => Done::Ran(Ok((output, snapshot))),
                                    Err(error) => Done::RanFailed(error, snapshot),
                                }
                            }
                            Err(error) => Done::Ran(Err(error)),
                        }
                    }
                    None => Done::Ran(Err("No repository selected".to_owned())),
                },
                Job::UndoIndex(change, redo) => match repo {
                    Some(path) => {
                        let operation = git::replay_index_change(&path, &change, redo);
                        match git::snapshot_with_limit(&path, history_limit) {
                            Ok(snapshot) => match operation {
                                Ok(output) => Done::IndexMoved(output, snapshot, change, redo),
                                Err(error) => Done::RanFailed(error, snapshot),
                            },
                            Err(error) => Done::Ran(Err(error)),
                        }
                    }
                    None => Done::Ran(Err("No repository selected".to_owned())),
                },
                Job::SwitchBranch(name, stash_first, remote) => Done::Ran(
                    repo.ok_or("No repository selected".to_owned()).and_then(|p| {
                        let mut output = String::new();
                        let previous_stash = git::run(&p, &["rev-parse", "-q", "--verify", "refs/stash"]).ok();
                        let mut created_stash = false;
                        if stash_first {
                            output.push_str(&git::run(&p, &["stash", "push", "-u", "-m", "GitVibe branch switch"])?);
                            output.push('\n');
                            created_stash = git::run(&p, &["rev-parse", "-q", "--verify", "refs/stash"]).ok() != previous_stash;
                        }
                        let command = if remote { vec!["switch", "--track", name.as_str()] } else { vec!["switch", name.as_str()] };
                        match git::run(&p, &command) {
                            Ok(switched) => output.push_str(&switched),
                            Err(error) => {
                                if created_stash {
                                    let restore = git::run(&p, &["stash", "pop"]);
                                    return Err(match restore {
                                        Ok(_) => format!("{error}\nYour changes were restored from the temporary stash."),
                                        Err(restore_error) => format!("{error}\nThe temporary stash was kept because restoring it failed: {restore_error}"),
                                    });
                                }
                                return Err(error);
                            }
                        }
                        let snapshot = git::snapshot_with_limit(&p, history_limit)?;
                        Ok((output, snapshot))
                    }),
                ),
                Job::Shell(command, cwd) => Done::Shell(run_shell_command(&cwd, &command)),
                Job::Inspect(args) => Done::Inspected(
                    repo.ok_or("No repository selected".to_owned())
                        .and_then(|p| git::run_owned(&p, &args)),
                ),
                Job::InspectCommit(id) => Done::CommitInspected(id.clone(),
                    repo.ok_or("No repository selected".to_owned()).and_then(|p| {
                        let detail = git::run(&p, &["show", "-s", "--format=%B", &id])?;
                        let files = git::commit_files(&p, &id)?;
                        Ok((detail, files))
                    }),
                ),
                Job::InspectFile(path) => Done::Inspected(
                    repo.ok_or("No repository selected".to_owned())
                        .and_then(|p| {
                            let bytes = std::fs::read(p.join(&path)).map_err(|e| e.to_string())?;
                            let preview =
                                String::from_utf8_lossy(&bytes[..bytes.len().min(64 * 1024)]);
                            Ok(format!("Untracked file: {path}\n\n{preview}"))
                        }),
                ),
                Job::InspectConflict(path) => Done::Inspected(
                    repo.ok_or("No repository selected".to_owned())
                        .and_then(|p| {
                            let file = p.join(&path);
                            if !file.exists() {
                                return Ok(format!(
                                    "Conflict in {path}\n\nThe file is absent in the working tree. Choose a side or mark its deletion resolved."
                                ));
                            }
                            let bytes = std::fs::read(file).map_err(|e| e.to_string())?;
                            let preview =
                                String::from_utf8_lossy(&bytes[..bytes.len().min(64 * 1024)]);
                            Ok(format!("Conflict in {path}\n\n{preview}"))
                        }),
                ),
                Job::ApplyHunk(patch, reverse) => {
                    let label = if reverse { "Unstage hunk" } else { "Stage hunk" };
                    finish_index_job(repo, history_limit, label, |path| {
                        git::apply_hunk(path, &patch, reverse)?;
                        Ok(label.to_owned())
                    })
                }
                Job::DiscardHunk(patch) => Done::Ran(
                    repo.ok_or("No repository selected".to_owned()).and_then(|path| {
                        git::discard_hunk(&path, &patch)?;
                        let snapshot = git::snapshot_with_limit(&path, history_limit)?;
                        Ok(("Hunk discarded".to_owned(), snapshot))
                    }),
                ),
                Job::ApplyLine(patch, reverse) => {
                    let label = if reverse { "Unstage line" } else { "Stage line" };
                    finish_index_job(repo, history_limit, label, |path| {
                        git::apply_line(path, &patch, reverse)?;
                        Ok(label.to_owned())
                    })
                }
                Job::SearchHistory(query) => Done::Searched(
                    repo.ok_or("No repository selected".to_owned())
                        .and_then(|p| git::search_history(&p, &query)),
                ),
                Job::FileHistory(path) => Done::FileHistory(path.clone(),
                    repo.ok_or("No repository selected".to_owned())
                        .and_then(|p| git::file_history(&p, &path)),
                ),
                Job::LoadConflict(path) => Done::ConflictLoaded(
                    repo.ok_or("No repository selected".to_owned())
                        .and_then(|p| git::load_conflict(&p, &path)),
                ),
                Job::SaveConflict(document) => Done::Ran(
                    repo.ok_or("No repository selected".to_owned()).and_then(|p| {
                        let output = git::save_conflict(&p, &document)?;
                        let snapshot = git::snapshot_with_limit(&p, history_limit)?;
                        Ok((output, snapshot))
                    }),
                ),
                Job::ResolveConflict(path, side) => Done::Ran(
                    repo.ok_or("No repository selected".to_owned())
                        .and_then(|p| {
                            let output = git::choose_conflict_side(&p, &path, side)?;
                            let snapshot = git::snapshot_with_limit(&p, history_limit)?;
                            Ok((output, snapshot))
                        }),
                ),
                Job::MarkResolved(path) => Done::Ran(
                    repo.ok_or("No repository selected".to_owned())
                        .and_then(|p| {
                            let output = git::mark_conflict_resolved(&p, &path)?;
                            let snapshot = git::snapshot_with_limit(&p, history_limit)?;
                            Ok((output, snapshot))
                        }),
                ),
            };
            let _ = sender.send(result);
            ctx.request_repaint();
        });
    }

    fn poll(&mut self) {
        let Some(receiver) = &self.receiver else {
            return;
        };
        let Ok(done) = receiver.try_recv() else {
            return;
        };
        self.receiver = None;
        self.busy = false;
        let mut keep_index_history = false;
        let done = match done {
            Done::IndexChanged(output, snapshot, change) => {
                if let Some(change) = change {
                    self.undo_stack.push(change);
                    if self.undo_stack.len() > 20 {
                        self.undo_stack.remove(0);
                    }
                    self.redo_stack.clear();
                    keep_index_history = true;
                } else {
                    self.undo_stack.clear();
                    self.redo_stack.clear();
                }
                Done::Ran(Ok((output, snapshot)))
            }
            Done::IndexMoved(output, snapshot, change, redo) => {
                if redo {
                    self.redo_stack.pop();
                    self.undo_stack.push(change);
                } else {
                    self.undo_stack.pop();
                    self.redo_stack.push(change);
                }
                keep_index_history = true;
                Done::Ran(Ok((output, snapshot)))
            }
            other => other,
        };
        match done {
            Done::Loaded(result) => match result {
                Ok(snapshot) => {
                    if self.repo.as_ref() != Some(&snapshot.root) {
                        self.undo_stack.clear();
                        self.redo_stack.clear();
                        self.terminal_cwd = Some(snapshot.root.clone());
                        self.terminal_output.clear();
                        self.selected_commit = None;
                        self.selected_file = None;
                        self.detail.clear();
                        self.compare_base = None;
                        self.search_results.clear();
                        self.search_active = false;
                        self.file_history.clear();
                        self.file_history_path = None;
                        self.conflict_editor = None;
                        self.commit_files.clear();
                        self.commit_files_id = None;
                        self.pr_loaded_for = None;
                        self.pull_requests.clear();
                        self.interactive_plan = None;
                    }
                    self.repo = Some(snapshot.root.clone());
                    if self.terminal_cwd.is_none() {
                        self.terminal_cwd = Some(snapshot.root.clone());
                    }
                    self.last_active_repo = Some(snapshot.root.clone());
                    self.path_input = snapshot.root.to_string_lossy().into_owned();
                    self.recent_repos.retain(|path| path != &snapshot.root);
                    self.recent_repos.insert(0, snapshot.root.clone());
                    self.recent_repos.truncate(30);
                    if !self.open_repo_tabs.contains(&snapshot.root) {
                        self.open_repo_tabs.push(snapshot.root.clone());
                    }
                    let first_commit = snapshot.commits.first().map(|commit| commit.id.clone());
                    self.snapshot = Some(snapshot);
                    self.interactive_plan = None;
                    self.error.clear();
                    if self.selected_commit.is_none()
                        && self.selected_file.is_none()
                        && let Some(id) = first_commit
                    {
                        self.select_commit(&id);
                    }
                }
                Err(error) => self.error = error,
            },
            Done::Ran(result) => match result {
                Ok((output, snapshot)) => {
                    if !keep_index_history {
                        self.undo_stack.clear();
                        self.redo_stack.clear();
                    }
                    let branch_changed = self
                        .snapshot
                        .as_ref()
                        .is_some_and(|current| current.branch != snapshot.branch);
                    let new_head = if branch_changed {
                        snapshot
                            .branches
                            .iter()
                            .find(|branch| branch.current)
                            .and_then(|branch| {
                                snapshot
                                    .commits
                                    .iter()
                                    .find(|commit| commit.short == branch.target)
                            })
                            .map(|commit| commit.id.clone())
                    } else {
                        None
                    };
                    self.output = output;
                    if !self.output.is_empty() && self.output.len() < 140 {
                        self.notice = self.output.clone();
                    }
                    self.repo = Some(snapshot.root.clone());
                    self.snapshot = Some(snapshot);
                    self.interactive_plan = None;
                    if let Some(id) = new_head {
                        self.select_commit(&id);
                    }
                    self.error.clear();
                    if self.committing {
                        self.commit_message.clear();
                    }
                    self.committing = false;
                    if self.hunk_action
                        && let Some(path) = self.selected_file.clone()
                    {
                        let still_present = self.snapshot.as_ref().is_some_and(|snapshot| {
                            snapshot.status.iter().any(|file| {
                                file.path == path
                                    && if self.selected_file_staged {
                                        file.staged()
                                    } else {
                                        file.unstaged()
                                    }
                            })
                        });
                        if still_present {
                            self.detail.clear();
                            let mut args = vec![
                                "diff".to_owned(),
                                "--no-ext-diff".to_owned(),
                                "--no-color".to_owned(),
                            ];
                            if self.selected_file_staged {
                                args.push("--cached".to_owned());
                            }
                            args.extend(["--".to_owned(), path]);
                            self.queue(Job::Inspect(args));
                        } else {
                            self.selected_file = None;
                            self.detail.clear();
                        }
                    }
                    self.hunk_action = false;
                    if self.conflict_action {
                        self.selected_file = None;
                        self.detail.clear();
                        self.conflict_editor = None;
                    }
                    self.conflict_action = false;
                }
                Err(error) => {
                    self.undo_stack.clear();
                    self.redo_stack.clear();
                    self.error = error;
                    self.committing = false;
                    self.hunk_action = false;
                    self.conflict_action = false;
                }
            },
            Done::RanFailed(error, snapshot) => {
                self.undo_stack.clear();
                self.redo_stack.clear();
                self.repo = Some(snapshot.root.clone());
                self.snapshot = Some(snapshot);
                self.interactive_plan = None;
                self.error = error;
                self.committing = false;
                self.hunk_action = false;
                self.conflict_action = false;
            }
            Done::Inspected(result) => match result {
                Ok(text) => {
                    self.detail = text;
                    self.error.clear();
                }
                Err(error) => self.error = error,
            },
            Done::Shell(result) => match result {
                Ok(output) => {
                    self.terminal_output.push_str(&output);
                    if !output.ends_with('\n') {
                        self.terminal_output.push('\n');
                    }
                    self.error.clear();
                }
                Err(error) => {
                    self.terminal_output.push_str(&format!("{error}\n"));
                    self.error = error;
                }
            },
            Done::PullRequests(result) => match result {
                Ok(requests) => {
                    self.pull_requests = requests;
                    self.error.clear();
                }
                Err(error) => self.error = error,
            },
            Done::InteractiveRebaseLoaded(result) => match result {
                Ok(plan) => {
                    self.interactive_plan = Some(plan);
                    self.error.clear();
                }
                Err(error) => {
                    self.interactive_plan = None;
                    self.error = error;
                }
            },
            Done::PullRequestCreated(result) => match result {
                Ok(url) => {
                    self.output = format!("Created pull request: {url}");
                    self.pr_title.clear();
                    self.pr_body.clear();
                    self.error.clear();
                    self.queue(Job::LoadPullRequests);
                    self.notice = format!("Created pull request: {url}");
                }
                Err(error) => self.error = error,
            },
            Done::Exported(result) => match result {
                Ok(message) => {
                    self.output = message;
                    self.notice = self.output.clone();
                    self.error.clear();
                }
                Err(error) => self.error = error,
            },
            Done::CommitInspected(id, result) => match result {
                Ok((detail, files)) => {
                    if self.selected_commit.as_deref() == Some(&id) {
                        self.detail = detail;
                        self.commit_files = files;
                        self.commit_files_id = Some(id);
                    }
                    self.error.clear();
                }
                Err(error) => self.error = error,
            },
            Done::Searched(result) => match result {
                Ok((hits, more)) => {
                    self.search_results = hits;
                    self.search_more = more;
                    self.search_active = true;
                    self.error.clear();
                }
                Err(error) => self.error = error,
            },
            Done::FileHistory(path, result) => match result {
                Ok(commits) => {
                    self.file_history_path = Some(path);
                    self.file_history = commits;
                    self.error.clear();
                }
                Err(error) => self.error = error,
            },
            Done::ConflictLoaded(result) => match result {
                Ok(document) => {
                    self.conflict_editor = Some(document);
                    self.error.clear();
                }
                Err(error) => self.error = error,
            },
            Done::IndexChanged(..) | Done::IndexMoved(..) => unreachable!(),
        }
    }

    fn toolbar(&mut self, ui: &mut egui::Ui) {
        let repo_name = self.snapshot.as_ref().map(|snapshot| {
            snapshot
                .root
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned()
        });
        let branch = self.snapshot.as_ref().map(|snapshot| {
            if snapshot.branch.is_empty() {
                "Detached HEAD".to_owned()
            } else {
                snapshot.branch.clone()
            }
        });
        let changed = self.snapshot.as_ref().map(|snapshot| snapshot.status.len());
        let local_branches = self
            .snapshot
            .as_ref()
            .map(|snapshot| snapshot.branches.clone())
            .unwrap_or_default();
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(RichText::new("repository").size(10.0).color(muted()));
                ui.menu_button(repo_name.as_deref().unwrap_or("No repository"), |ui| {
                    ui.set_min_width(230.0);
                    ui.add(
                        egui::TextEdit::singleline(&mut self.repo_filter)
                            .hint_text("Search repositories"),
                    );
                    ui.separator();
                    ui.label(RichText::new("RECENTLY OPENED").size(10.0).color(muted()));
                    let filter = self.repo_filter.to_lowercase();
                    for path in self
                        .recent_repos
                        .clone()
                        .into_iter()
                        .filter(|path| path.to_string_lossy().to_lowercase().contains(&filter))
                        .take(12)
                    {
                        let name = path
                            .file_name()
                            .unwrap_or(path.as_os_str())
                            .to_string_lossy();
                        if ui.button(name).clicked() {
                            self.queue(Job::Open(path));
                            self.page = Page::History;
                            ui.close();
                        }
                    }
                    ui.separator();
                    if ui.button("View all repositories").clicked() {
                        self.page = Page::Repositories;
                        ui.close();
                    }
                });
            });
            ui.add_space(9.0);
            ui.separator();
            if let Some(branch) = &branch {
                ui.vertical(|ui| {
                    ui.label(RichText::new("branch").size(10.0).color(muted()));
                    ui.menu_button(branch, |ui| {
                        ui.set_min_width(230.0);
                        ui.add(
                            egui::TextEdit::singleline(&mut self.branch_filter)
                                .hint_text("Search branches"),
                        );
                        ui.separator();
                        let filter = self.branch_filter.to_lowercase();
                        for candidate in local_branches
                            .iter()
                            .filter(|candidate| candidate.name.to_lowercase().contains(&filter))
                        {
                            if ui
                                .selectable_label(candidate.current, &candidate.name)
                                .clicked()
                            {
                                if !candidate.current {
                                    self.request_branch_switch(candidate.name.clone(), false);
                                }
                                ui.close();
                            }
                        }
                    });
                });
            }
            if let Some(count) = changed {
                ui.add_space(6.0);
                ui.label(
                    RichText::new(if count == 0 {
                        "Clean".to_owned()
                    } else {
                        format!("{count} changes")
                    })
                    .size(10.5)
                    .color(if count == 0 { accent() } else { orange() }),
                );
            }
            let toolbar_width = 665.0;
            ui.add_space(
                (ui.max_rect().center().x - ui.cursor().left() - toolbar_width / 2.0).max(8.0),
            );
            ui.horizontal(|ui| {
                ui.add_enabled_ui(self.repo.is_some() && !self.busy, |ui| {
                    let undo = ui
                        .add_enabled(!self.undo_stack.is_empty(), egui::Button::new("Undo"))
                        .on_hover_text(format!(
                            "Undo staging change only: {}",
                            self.undo_stack
                                .last()
                                .map_or("none", |change| change.label.as_str())
                        ));
                    if undo.clicked() {
                        self.undo_index(false);
                    }
                    let redo = ui
                        .add_enabled(!self.redo_stack.is_empty(), egui::Button::new("Redo"))
                        .on_hover_text(format!(
                            "Redo staging change only: {}",
                            self.redo_stack
                                .last()
                                .map_or("none", |change| change.label.as_str())
                        ));
                    if redo.clicked() {
                        self.undo_index(true);
                    }
                    if toolbar_action(ui, "Fetch") {
                        self.git(&["fetch", "--all", "--prune"]);
                    }
                    if toolbar_action(
                        ui,
                        if self.pull_mode == PullMode::FetchAll {
                            "Fetch"
                        } else {
                            "Pull"
                        },
                    ) {
                        self.run_pull_mode();
                    }
                    ui.menu_button("v", |ui| {
                        ui.label(
                            RichText::new("Default pull action")
                                .size(11.0)
                                .color(muted()),
                        );
                        ui.separator();
                        for mode in [
                            PullMode::FetchAll,
                            PullMode::Merge,
                            PullMode::FastForwardOnly,
                            PullMode::Rebase,
                        ] {
                            if ui
                                .radio_value(&mut self.pull_mode, mode, mode.label())
                                .clicked()
                            {
                                ui.close();
                            }
                        }
                    });
                    if toolbar_action(ui, "Push") {
                        self.git(&["push"]);
                    }
                    if toolbar_action(ui, "Branch") {
                        self.page = Page::Branches;
                    }
                    if toolbar_action(ui, "Stash") {
                        self.page = Page::Stashes;
                    }
                    if toolbar_action(ui, "Terminal") {
                        self.terminal_open = !self.terminal_open;
                    }
                    if toolbar_action(ui, "Refresh") {
                        self.queue(Job::Refresh);
                    }
                });
                if toolbar_action(ui, "Commands") {
                    self.open_palette();
                }
                if self.busy {
                    ui.spinner();
                }
            });
        });
    }

    fn run_pull_mode(&mut self) {
        match self.pull_mode {
            PullMode::FetchAll => self.git(&["fetch", "--all", "--prune"]),
            PullMode::Merge => self.git(&["pull", "--no-rebase"]),
            PullMode::FastForwardOnly => self.git(&["pull", "--ff-only"]),
            PullMode::Rebase => self.git(&["pull", "--rebase"]),
        }
    }

    fn undo_index(&mut self, redo: bool) {
        let change = if redo {
            self.redo_stack.last()
        } else {
            self.undo_stack.last()
        };
        if let Some(change) = change.cloned() {
            self.queue(Job::UndoIndex(change, redo));
        }
    }

    fn open_palette(&mut self) {
        self.palette_open = true;
        self.palette_query.clear();
        self.palette_index = 0;
        self.palette_focus = true;
    }

    fn run_palette_action(&mut self, action: PaletteAction, ctx: &egui::Context) {
        match action {
            PaletteAction::Show(Page::NewTab) => {
                self.new_tab_open = true;
                self.page = Page::NewTab;
            }
            PaletteAction::Show(Page::Changelog) => {
                self.changelog_open = true;
                self.page = Page::Changelog;
            }
            PaletteAction::Show(page) => self.page = page,
            PaletteAction::SearchHistory => {
                self.page = Page::History;
                ctx.memory_mut(|memory| memory.request_focus(egui::Id::new("history_search")));
            }
            PaletteAction::Refresh => self.queue(Job::Refresh),
            PaletteAction::Fetch => self.git(&["fetch", "--all", "--prune"]),
            PaletteAction::Pull => self.run_pull_mode(),
            PaletteAction::Push => self.git(&["push"]),
            PaletteAction::ToggleTerminal => self.terminal_open = !self.terminal_open,
            PaletteAction::StageSelected => {
                if let Some(path) = self.selected_file.clone() {
                    self.git_owned(vec!["add".into(), "--".into(), path]);
                    self.selected_file_staged = true;
                    self.detail.clear();
                }
            }
            PaletteAction::UnstageSelected => {
                if let Some(path) = self.selected_file.clone() {
                    if self
                        .snapshot
                        .as_ref()
                        .is_some_and(|snapshot| snapshot.commits.is_empty())
                    {
                        self.git_owned(vec!["rm".into(), "--cached".into(), "--".into(), path]);
                    } else {
                        self.git_owned(vec![
                            "restore".into(),
                            "--staged".into(),
                            "--".into(),
                            path,
                        ]);
                    }
                    self.selected_file_staged = false;
                    self.detail.clear();
                }
            }
            PaletteAction::StageAll => self.git(&["add", "-A"]),
            PaletteAction::UnstageAll => {
                if self
                    .snapshot
                    .as_ref()
                    .is_some_and(|snapshot| snapshot.commits.is_empty())
                {
                    self.git(&["rm", "-r", "--cached", "--", "."]);
                } else {
                    self.git(&["restore", "--staged", "."]);
                }
            }
            PaletteAction::UndoIndex => self.undo_index(false),
            PaletteAction::RedoIndex => self.undo_index(true),
        }
    }

    fn command_palette(&mut self, ctx: &egui::Context) {
        if !self.palette_open {
            return;
        }
        let mut close = ctx.input(|input| input.key_pressed(egui::Key::Escape));
        let mut chosen = None;
        egui::Window::new("Commands")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_TOP, [0.0, 82.0])
            .default_width(480.0)
            .show(ctx, |ui| {
                let search = ui.add(
                    egui::TextEdit::singleline(&mut self.palette_query)
                        .hint_text("Search commands")
                        .desired_width(460.0),
                );
                if self.palette_focus {
                    search.request_focus();
                    self.palette_focus = false;
                }
                if search.changed() {
                    self.palette_index = 0;
                }
                let query = self.palette_query.trim().to_lowercase();
                let matches = PALETTE_ACTIONS
                    .iter()
                    .copied()
                    .filter(|(label, action)| {
                        (!action.needs_repository() || self.repo.is_some())
                            && match action {
                                PaletteAction::StageSelected => {
                                    self.selected_file.is_some() && !self.selected_file_staged
                                }
                                PaletteAction::UnstageSelected => {
                                    self.selected_file.is_some() && self.selected_file_staged
                                }
                                PaletteAction::StageAll => {
                                    self.snapshot.as_ref().is_some_and(|snapshot| {
                                        snapshot.status.iter().any(git::FileStatus::unstaged)
                                    })
                                }
                                PaletteAction::UnstageAll => {
                                    self.snapshot.as_ref().is_some_and(|snapshot| {
                                        snapshot.status.iter().any(git::FileStatus::staged)
                                    })
                                }
                                PaletteAction::UndoIndex => !self.undo_stack.is_empty(),
                                PaletteAction::RedoIndex => !self.redo_stack.is_empty(),
                                _ => true,
                            }
                            && label.to_lowercase().contains(&query)
                    })
                    .collect::<Vec<_>>();
                if matches.is_empty() {
                    ui.label(RichText::new("No matching commands").color(muted()));
                    return;
                }
                self.palette_index = self.palette_index.min(matches.len() - 1);
                if ui.input(|input| input.key_pressed(egui::Key::ArrowDown)) {
                    self.palette_index = (self.palette_index + 1).min(matches.len() - 1);
                }
                if ui.input(|input| input.key_pressed(egui::Key::ArrowUp)) {
                    self.palette_index = self.palette_index.saturating_sub(1);
                }
                ui.separator();
                egui::ScrollArea::vertical()
                    .max_height(410.0)
                    .show(ui, |ui| {
                        for (index, (label, action)) in matches.iter().enumerate() {
                            let enabled = !action.needs_idle() || !self.busy;
                            if ui
                                .add_enabled(
                                    enabled,
                                    egui::Button::new(*label)
                                        .selected(index == self.palette_index)
                                        .min_size(egui::vec2(ui.available_width(), 25.0)),
                                )
                                .clicked()
                            {
                                chosen = Some(*action);
                            }
                        }
                    });
                if ui.input(|input| input.key_pressed(egui::Key::Enter)) {
                    let action = matches[self.palette_index].1;
                    if !action.needs_idle() || !self.busy {
                        chosen = Some(action);
                    }
                }
                ui.label(
                    RichText::new("↑ ↓ to choose · Enter to run · Esc to close")
                        .small()
                        .color(muted()),
                );
            });
        if let Some(action) = chosen {
            self.run_palette_action(action, ctx);
            close = true;
        }
        if close {
            self.palette_open = false;
        }
    }

    fn close_repository_tab(&mut self, path: &Path) {
        self.open_repo_tabs.retain(|tab| tab != path);
        if self.repo.as_deref() == Some(path) {
            if let Some(next) = self.open_repo_tabs.last().cloned() {
                self.last_active_repo = Some(next.clone());
                self.page = Page::History;
                self.queue(Job::Open(next));
            } else {
                self.last_active_repo = None;
                self.repo = None;
                self.snapshot = None;
                self.selected_commit = None;
                self.selected_file = None;
                self.detail.clear();
                self.page = Page::Repositories;
            }
        }
    }

    fn workspace_tabs(&mut self, ui: &mut egui::Ui) {
        ui.spacing_mut().item_spacing.x = 0.0;
        ui.spacing_mut().interact_size.y = 34.0;
        ui.horizontal(|ui| {
            let (open_launchpad, _) = workspace_tab(
                ui,
                egui::Id::new("launchpad_tab"),
                "Launchpad",
                self.page == Page::Repositories,
                false,
                124.0,
            );
            if open_launchpad {
                self.page = Page::Repositories;
            }

            let tabs = self.open_repo_tabs.clone();
            for path in tabs.iter().take(6) {
                let full_name = path
                    .file_name()
                    .unwrap_or(path.as_os_str())
                    .to_string_lossy();
                let mut label = full_name.chars().take(17).collect::<String>();
                if full_name.chars().count() > 17 {
                    label.push_str("...");
                }
                let selected = self.repo.as_deref() == Some(path.as_path())
                    && !matches!(
                        self.page,
                        Page::Repositories | Page::Changelog | Page::NewTab
                    );
                let (opened, closed) = workspace_tab(
                    ui,
                    egui::Id::new(("repo_tab", path)),
                    &label,
                    selected,
                    true,
                    150.0,
                );
                if closed && !self.busy {
                    self.close_repository_tab(path);
                } else if opened {
                    if self.repo.as_deref() != Some(path.as_path()) {
                        self.queue(Job::Open(path.clone()));
                    }
                    self.page = Page::History;
                }
            }
            if tabs.len() > 6 {
                ui.menu_button(format!("More ({})", tabs.len() - 6), |ui| {
                    for path in tabs.iter().skip(6) {
                        if ui
                            .button(
                                path.file_name()
                                    .unwrap_or(path.as_os_str())
                                    .to_string_lossy(),
                            )
                            .clicked()
                        {
                            self.queue(Job::Open(path.clone()));
                            self.page = Page::History;
                            ui.close();
                        }
                    }
                });
            }
            if self.changelog_open {
                let (opened, closed) = workspace_tab(
                    ui,
                    egui::Id::new("changelog_tab"),
                    "Changelog",
                    self.page == Page::Changelog,
                    true,
                    142.0,
                );
                if closed {
                    self.changelog_open = false;
                    if self.page == Page::Changelog {
                        self.page = if self.repo.is_some() {
                            Page::History
                        } else {
                            Page::Repositories
                        };
                    }
                } else if opened {
                    self.page = Page::Changelog;
                }
            }
            if self.new_tab_open {
                let (opened, closed) = workspace_tab(
                    ui,
                    egui::Id::new("new_tab"),
                    "New Tab",
                    self.page == Page::NewTab,
                    true,
                    118.0,
                );
                if closed {
                    self.new_tab_open = false;
                    if self.page == Page::NewTab {
                        self.page = if self.repo.is_some() {
                            Page::History
                        } else {
                            Page::Repositories
                        };
                    }
                } else if opened {
                    self.page = Page::NewTab;
                }
            }
            ui.add_space(5.0);
            if ui
                .add(egui::Button::new(RichText::new("+").size(17.0).color(text())).frame(false))
                .on_hover_text("New tab")
                .clicked()
            {
                self.new_tab_open = true;
                self.page = Page::NewTab;
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.menu_button(
                    RichText::new(format!("Theme: {}", self.theme_choice.label()))
                        .size(11.0)
                        .color(muted()),
                    |ui| {
                        let previous = self.theme_choice;
                        for theme in [ThemeChoice::Aurora, ThemeChoice::Cosmic, ThemeChoice::Ember]
                        {
                            ui.selectable_value(&mut self.theme_choice, theme, theme.label());
                        }
                        if previous != self.theme_choice {
                            self.set_theme(ui.ctx());
                        }
                    },
                );
            });
        });
    }
    fn focus_ref(&mut self, short: &str) {
        self.page = Page::History;
        let id = self.snapshot.as_ref().and_then(|snapshot| {
            snapshot
                .commits
                .iter()
                .find(|commit| commit.short == short || commit.id.starts_with(short))
                .map(|commit| commit.id.clone())
        });
        if let Some(id) = id {
            self.select_commit(&id);
        }
    }

    fn request_branch_switch(&mut self, name: String, remote: bool) {
        let (name, remote) = if remote {
            let local = name.split_once('/').map(|(_, local)| local.to_owned());
            if let Some(local) = local.filter(|local| {
                self.snapshot.as_ref().is_some_and(|snapshot| {
                    snapshot.branches.iter().any(|branch| &branch.name == local)
                })
            }) {
                (local, false)
            } else {
                (name, true)
            }
        } else {
            (name, false)
        };
        if self
            .snapshot
            .as_ref()
            .is_some_and(|snapshot| !snapshot.status.is_empty())
        {
            self.confirm_switch_branch = Some((name, remote));
        } else {
            self.queue(Job::SwitchBranch(name, false, remote));
        }
    }

    fn sidebar(&mut self, ui: &mut egui::Ui) {
        ui.style_mut().visuals.indent_has_left_vline = false;
        ui.spacing_mut().indent = 10.0;
        ui.spacing_mut().item_spacing.y = 2.0;
        let Some(snapshot) = self.snapshot.clone() else {
            ui.label(RichText::new("Opening repository...").color(muted()));
            return;
        };
        ui.horizontal(|ui| {
            if ui
                .add(
                    egui::Button::new(RichText::new("List").size(11.5).color(text()))
                        .fill(if self.page == Page::History {
                            elevated()
                        } else {
                            panel()
                        })
                        .stroke(Stroke::NONE),
                )
                .clicked()
            {
                self.page = Page::History;
            }
            if ui
                .add(
                    egui::Button::new(RichText::new("Changes").size(11.5).color(text()))
                        .fill(if self.page == Page::Changes {
                            elevated()
                        } else {
                            panel()
                        })
                        .stroke(Stroke::NONE),
                )
                .clicked()
            {
                self.page = Page::Changes;
            }
        });
        ui.add_space(7.0);
        ui.add(
            egui::TextEdit::singleline(&mut self.branch_filter)
                .hint_text("Filter branches")
                .desired_width(ui.available_width()),
        );
        let filter = self.branch_filter.trim().to_lowercase();
        let local = snapshot
            .branches
            .iter()
            .filter(|branch| branch.name.to_lowercase().contains(&filter))
            .cloned()
            .collect::<Vec<_>>();
        let remote = snapshot
            .remote_branches
            .iter()
            .filter(|branch| branch.name.to_lowercase().contains(&filter))
            .cloned()
            .collect::<Vec<_>>();
        ui.add_space(5.0);
        ui.label(
            RichText::new(format!("Viewing {}", local.len() + remote.len()))
                .size(10.5)
                .color(muted()),
        );
        ui.separator();

        let local_count = local.len();
        let mut folders = std::collections::BTreeMap::<String, Vec<git::Ref>>::new();
        let mut direct = Vec::new();
        for branch in local {
            if let Some((folder, _)) = branch.name.split_once('/') {
                folders.entry(folder.to_owned()).or_default().push(branch);
            } else {
                direct.push(branch);
            }
        }
        egui::CollapsingHeader::new(
            RichText::new(format!("LOCAL   {local_count}"))
                .size(10.5)
                .strong()
                .color(text()),
        )
        .id_salt("local_branches")
        .default_open(true)
        .show(ui, |ui| {
            for (folder, branches) in folders {
                egui::CollapsingHeader::new(RichText::new(&folder).size(11.5).color(muted()))
                    .id_salt(("local_folder", &folder))
                    .default_open(true)
                    .show(ui, |ui| {
                        for branch in branches {
                            let leaf = branch
                                .name
                                .strip_prefix(&format!("{folder}/"))
                                .unwrap_or(&branch.name);
                            let response = sidebar_branch_row(ui, leaf, branch.current, 0.0);
                            if response.double_clicked() && !branch.current {
                                self.request_branch_switch(branch.name.clone(), false);
                            } else if response.clicked() {
                                self.focus_ref(&branch.target);
                            }
                            response.context_menu(|ui| {
                                if ui.button("Create worktree from branch").clicked() {
                                    self.worktree_ref = branch.name.clone();
                                    self.page = Page::Worktrees;
                                    ui.close();
                                }
                            });
                        }
                    });
            }
            for branch in direct {
                let response = sidebar_branch_row(ui, &branch.name, branch.current, 0.0);
                if response.double_clicked() && !branch.current {
                    self.request_branch_switch(branch.name.clone(), false);
                } else if response.clicked() {
                    self.focus_ref(&branch.target);
                }
                response.context_menu(|ui| {
                    if ui.button("Create worktree from branch").clicked() {
                        self.worktree_ref = branch.name.clone();
                        self.page = Page::Worktrees;
                        ui.close();
                    }
                });
            }
        });

        ui.add_space(10.0);
        ui.separator();
        egui::CollapsingHeader::new(
            RichText::new(format!("REMOTE   {}", remote.len()))
                .size(10.5)
                .strong()
                .color(text()),
        )
        .id_salt("remote_branches")
        .default_open(true)
        .show(ui, |ui| {
            for remote_name in &snapshot.remotes {
                egui::CollapsingHeader::new(RichText::new(remote_name).size(11.5).color(muted()))
                    .id_salt(("remote_name", remote_name))
                    .default_open(true)
                    .show(ui, |ui| {
                        let prefix = format!("{remote_name}/");
                        let mut folders =
                            std::collections::BTreeMap::<String, Vec<git::Ref>>::new();
                        let mut direct = Vec::new();
                        for branch in remote
                            .iter()
                            .filter(|branch| branch.name.starts_with(&prefix))
                        {
                            let rest = branch.name.strip_prefix(&prefix).unwrap_or(&branch.name);
                            if let Some((folder, _)) = rest.split_once('/') {
                                folders
                                    .entry(folder.to_owned())
                                    .or_default()
                                    .push(branch.clone());
                            } else {
                                direct.push(branch.clone());
                            }
                        }
                        for (folder, branches) in folders {
                            egui::CollapsingHeader::new(
                                RichText::new(&folder).size(11.5).color(muted()),
                            )
                            .id_salt(("remote_folder", remote_name, &folder))
                            .default_open(true)
                            .show(ui, |ui| {
                                for branch in branches {
                                    let leaf = branch
                                        .name
                                        .strip_prefix(&format!("{prefix}{folder}/"))
                                        .unwrap_or(&branch.name);
                                    let response = sidebar_branch_row(ui, leaf, false, 0.0);
                                    if response.double_clicked() {
                                        self.request_branch_switch(branch.name.clone(), true);
                                    } else if response.clicked() {
                                        self.focus_ref(&branch.target);
                                    }
                                    response.context_menu(|ui| {
                                        if ui.button("Create worktree from branch").clicked() {
                                            self.worktree_ref = branch.name.clone();
                                            self.page = Page::Worktrees;
                                            ui.close();
                                        }
                                    });
                                }
                            });
                        }
                        for branch in direct {
                            let leaf = branch.name.strip_prefix(&prefix).unwrap_or(&branch.name);
                            let response = sidebar_branch_row(ui, leaf, false, 0.0);
                            if response.double_clicked() {
                                self.request_branch_switch(branch.name.clone(), true);
                            } else if response.clicked() {
                                self.focus_ref(&branch.target);
                            }
                            response.context_menu(|ui| {
                                if ui.button("Create worktree from branch").clicked() {
                                    self.worktree_ref = branch.name.clone();
                                    self.page = Page::Worktrees;
                                    ui.close();
                                }
                            });
                        }
                    });
            }
        });

        ui.add_space(10.0);
        ui.separator();
        if snapshot.worktrees.len() > 1 {
            egui::CollapsingHeader::new(
                RichText::new(format!("WORKTREES  {}", snapshot.worktrees.len()))
                    .size(10.5)
                    .strong()
                    .color(text()),
            )
            .id_salt("sidebar_worktrees")
            .default_open(true)
            .show(ui, |ui| {
                for worktree in &snapshot.worktrees {
                    let name = worktree.branch.as_deref().unwrap_or("Detached HEAD");
                    let changes = worktree
                        .dirty_count
                        .map(|count| format!(" · {count} changes"))
                        .unwrap_or_default();
                    let short_name = if name.chars().count() > 19 {
                        format!("{}…", name.chars().take(18).collect::<String>())
                    } else {
                        name.to_owned()
                    };
                    let label = format!(
                        "{}{}{}",
                        if worktree.current { "✓ " } else { "" },
                        short_name,
                        changes
                    );
                    if (filter.is_empty() || name.to_lowercase().contains(&filter))
                        && ui
                            .add_enabled(
                                !worktree.prunable,
                                egui::Button::new(RichText::new(label).size(11.0).color(
                                    if worktree.dirty_count.unwrap_or(0) > 0 {
                                        orange()
                                    } else {
                                        muted()
                                    },
                                ))
                                .fill(Color32::TRANSPARENT)
                                .stroke(Stroke::NONE),
                            )
                            .on_hover_text(worktree.path.display().to_string())
                            .clicked()
                    {
                        if worktree.current {
                            self.page = Page::Worktrees;
                        } else {
                            self.queue(Job::Open(worktree.path.clone()));
                        }
                    }
                }
            });
            ui.add_space(10.0);
            ui.separator();
        }
        egui::CollapsingHeader::new(
            RichText::new(format!("TAGS  {}", snapshot.tags.len()))
                .size(10.5)
                .strong()
                .color(muted()),
        )
        .default_open(false)
        .show(ui, |ui| {
            for tag in snapshot.tags.iter().take(20) {
                if (filter.is_empty() || tag.name.to_lowercase().contains(&filter))
                    && sidebar_branch_row(ui, &tag.name, false, 0.0).clicked()
                {
                    self.focus_ref(&tag.target);
                }
            }
        });
    }

    fn sidebar_footer(&mut self, ui: &mut egui::Ui) {
        ui.separator();
        for (page, title) in [
            (Page::Branches, "Branches & tags"),
            (Page::Stashes, "Stashes"),
            (Page::Worktrees, "Worktrees"),
            (Page::Submodules, "Submodules"),
            (Page::Rebase, "Rebase"),
            (Page::PullRequests, "Pull requests"),
            (Page::Console, "Terminal"),
            (Page::Updates, "Updates"),
        ] {
            let background = ui.painter().add(egui::Shape::Noop);
            let response = ui.add(
                egui::Button::new(RichText::new(title).size(11.5).color(muted()))
                    .fill(Color32::TRANSPARENT)
                    .stroke(Stroke::NONE)
                    .min_size(egui::vec2(ui.available_width(), 22.0)),
            );
            if response.hovered() {
                ui.painter().set(
                    background,
                    egui::Shape::rect_filled(response.rect, 3.0, elevated()),
                );
            }
            if response.clicked() {
                self.page = page;
            }
        }
    }
    fn history(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(RichText::new("Viewing history").size(11.0).color(muted()));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add_sized([58.0, 25.0], egui::Button::new("Clear"))
                    .clicked()
                {
                    self.search.clear();
                    self.search_active = false;
                    self.search_results.clear();
                }
                if ui
                    .add_enabled(
                        !self.busy && !self.search.trim().is_empty(),
                        egui::Button::new("Search").min_size(egui::vec2(66.0, 25.0)),
                    )
                    .clicked()
                {
                    self.queue(Job::SearchHistory(self.search.clone()));
                }
                let search_edit = ui.add_sized(
                    [250.0, 25.0],
                    egui::TextEdit::singleline(&mut self.search)
                        .id(egui::Id::new("history_search"))
                        .hint_text("Search commits, authors or SHA")
                        .desired_width(250.0),
                );
                if search_edit.changed() {
                    self.search_active = false;
                }
                if search_edit.has_focus()
                    && ui.input(|input| input.key_pressed(egui::Key::Enter))
                    && !self.search.trim().is_empty()
                {
                    self.queue(Job::SearchHistory(self.search.clone()));
                }
            });
        });
        ui.separator();
        let Some(snapshot) = self.snapshot.clone() else {
            self.empty(ui);
            return;
        };
        let changed = snapshot.status.len();
        if changed > 0 {
            egui::Frame::new()
                .fill(Color32::from_rgb(42, 59, 81))
                .inner_margin(egui::Margin::symmetric(8, 3))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("WIP").size(10.0).strong().color(orange()));
                        ui.label(
                            RichText::new(format!(
                                "{changed} changed files on {}",
                                snapshot.branch
                            ))
                            .size(11.0)
                            .color(text()),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.small_button("View changes").clicked() {
                                self.page = Page::Changes;
                            }
                        });
                    });
                });
        }
        for worktree in snapshot
            .worktrees
            .iter()
            .filter(|worktree| !worktree.current && worktree.dirty_count.unwrap_or(0) > 0)
        {
            egui::Frame::new()
                .fill(Color32::from_rgb(39, 50, 69))
                .inner_margin(egui::Margin::symmetric(8, 3))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("WIP").size(10.0).strong().color(orange()));
                        ui.label(
                            RichText::new(format!(
                                "{} changed files in {}",
                                worktree.dirty_count.unwrap_or(0),
                                worktree.branch.as_deref().unwrap_or("detached worktree")
                            ))
                            .size(11.0)
                            .color(text()),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.small_button("Open worktree").clicked() {
                                self.queue(Job::Open(worktree.path.clone()));
                            }
                        });
                    });
                });
        }
        if snapshot.commits.is_empty() {
            egui::Frame::new()
                .fill(panel())
                .corner_radius(egui::CornerRadius::same(12))
                .inner_margin(egui::Margin::same(26))
                .show(ui, |ui| {
                    ui.label(
                        RichText::new("A fresh beginning")
                            .size(20.0)
                            .strong()
                            .color(text()),
                    );
                    ui.label(
                        RichText::new(
                            "Stage a file and make your first commit to start the graph.",
                        )
                        .color(muted()),
                    );
                });
            return;
        }
        if self.search_active {
            ui.label(
                RichText::new(format!(
                    "{} matches across all refs{}",
                    self.search_results.len(),
                    if self.search_more {
                        " (first 300 shown)"
                    } else {
                        ""
                    }
                ))
                .size(11.0)
                .color(muted()),
            );
            ui.add_space(8.0);
            let hits = self.search_results.clone();
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.style_mut().spacing.item_spacing.y = 0.0;
                    for (row, commit) in hits.iter().enumerate() {
                        let response = paint_commit_row(
                            ui,
                            commit,
                            &[],
                            row,
                            CommitRowStyle {
                                selected: self.selected_commit.as_deref() == Some(&commit.id),
                                compare_base: self.compare_base.as_deref() == Some(&commit.id),
                            },
                            130.0,
                            false,
                        );
                        if response.clicked() {
                            self.select_commit(&commit.id);
                        }
                        if self.graph_scroll_pending
                            && self.selected_commit.as_deref() == Some(&commit.id)
                        {
                            ui.scroll_to_rect(response.rect, Some(egui::Align::Center));
                            self.graph_scroll_pending = false;
                        }
                        response.context_menu(|ui| self.commit_context_menu(ui, commit));
                        self.inline_branch_editor(ui, commit, response.rect);
                        response.on_hover_text(format!(
                            "{}\n{} | {} | {}",
                            commit.subject, commit.short, commit.author, commit.date
                        ));
                    }
                });
            return;
        }
        let refs = snapshot
            .branches
            .iter()
            .map(|r| (r.target.clone(), r.name.clone(), accent()))
            .chain(
                snapshot
                    .remote_branches
                    .iter()
                    .map(|r| (r.target.clone(), r.name.clone(), blue())),
            )
            .chain(
                snapshot
                    .tags
                    .iter()
                    .map(|r| (r.target.clone(), r.name.clone(), orange())),
            )
            .collect::<Vec<_>>();
        let commits = snapshot.commits.clone();
        let max_lanes = commits.iter().map(|c| c.lane_count).max().unwrap_or(1);
        let graph_width = 154.0 + max_lanes as f32 * 16.0;
        let visible = commits.iter().collect::<Vec<_>>();

        egui::Frame::new()
            .fill(panel_alt())
            .inner_margin(egui::Margin::symmetric(0, 4))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.add_sized(
                        [144.0, 14.0],
                        egui::Label::new(
                            RichText::new("BRANCH / TAG")
                                .size(10.0)
                                .strong()
                                .color(muted()),
                        ),
                    );
                    ui.add_sized(
                        [graph_width - 148.0, 14.0],
                        egui::Label::new(RichText::new("GRAPH").size(10.0).strong().color(muted())),
                    );
                    ui.label(
                        RichText::new("COMMIT MESSAGE")
                            .size(10.0)
                            .strong()
                            .color(muted()),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            RichText::new(format!("{} visible", visible.len()))
                                .size(10.0)
                                .color(muted()),
                        );
                    });
                });
            });
        egui::ScrollArea::both()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.set_min_width(graph_width + 500.0);
                ui.style_mut().spacing.item_spacing.y = 0.0;
                for (row, commit) in visible.iter().enumerate() {
                    let selected = self.selected_commit.as_deref() == Some(&commit.id);
                    let compare_base = self.compare_base.as_deref() == Some(&commit.id);
                    let response = paint_commit_row(
                        ui,
                        commit,
                        &refs,
                        row,
                        CommitRowStyle {
                            selected,
                            compare_base,
                        },
                        graph_width,
                        true,
                    );
                    if response.clicked() {
                        self.select_commit(&commit.id);
                    }
                    if self.graph_scroll_pending
                        && self.selected_commit.as_deref() == Some(&commit.id)
                    {
                        ui.scroll_to_rect(response.rect, Some(egui::Align::Center));
                        self.graph_scroll_pending = false;
                    }
                    response.context_menu(|ui| self.commit_context_menu(ui, commit));
                    self.inline_branch_editor(ui, commit, response.rect);
                    response.on_hover_text(format!(
                        "{}\n{} | {} | {}",
                        commit.subject, commit.short, commit.author, commit.date
                    ));
                }
                if snapshot.has_more_commits {
                    ui.add_space(12.0);
                    if ui
                        .add_enabled(!self.busy, egui::Button::new("Load 300 more commits"))
                        .clicked()
                    {
                        self.history_limit = self.history_limit.saturating_add(300);
                        self.queue(Job::Refresh);
                    }
                }
            });
    }

    fn select_commit(&mut self, id: &str) {
        self.selected_commit = Some(id.to_owned());
        self.selected_commit_file = None;
        self.selected_file = None;
        self.selected_file_staged = false;
        self.detail.clear();
        self.commit_files.clear();
        self.commit_files_id = None;
        self.queue(Job::InspectCommit(id.to_owned()));
    }

    fn inline_branch_editor(&mut self, ui: &mut egui::Ui, commit: &git::Commit, row: egui::Rect) {
        if self.inline_branch_at.as_deref() != Some(&commit.id) {
            return;
        }
        let mut create = false;
        let mut cancel = false;
        egui::Area::new(egui::Id::new(("inline_branch", &commit.id)))
            .order(egui::Order::Foreground)
            .fixed_pos(egui::pos2(row.left() + 5.0, row.top() - 2.0))
            .show(ui.ctx(), |ui| {
                egui::Frame::new()
                    .fill(panel_alt())
                    .stroke(Stroke::new(1.0, accent()))
                    .inner_margin(egui::Margin::same(3))
                    .show(ui, |ui| {
                        let edit = ui.add(
                            egui::TextEdit::singleline(&mut self.branch_input)
                                .hint_text("Enter branch name")
                                .desired_width(132.0),
                        );
                        edit.request_focus();
                        create = ui.input(|input| input.key_pressed(egui::Key::Enter));
                        cancel = ui.input(|input| input.key_pressed(egui::Key::Escape));
                    });
            });
        if cancel {
            self.inline_branch_at = None;
        } else if create && !self.branch_input.trim().is_empty() {
            self.git_owned(vec![
                "branch".into(),
                self.branch_input.trim().to_owned(),
                commit.id.clone(),
            ]);
            self.inline_branch_at = None;
        }
    }

    fn commit_context_menu(&mut self, ui: &mut egui::Ui, commit: &git::Commit) {
        ui.set_min_width(245.0);
        let id = commit.id.clone();
        if ui.button("Show commit diff").clicked() {
            self.selected_commit = Some(id.clone());
            self.selected_commit_file = None;
            self.page = Page::CommitDiff;
            self.detail.clear();
            self.queue(Job::Inspect(vec![
                "show".into(),
                "--format=".into(),
                "--patch".into(),
                "--no-color".into(),
                id.clone(),
            ]));
            ui.close();
        }
        if ui.button("Compare with working tree").clicked() {
            self.selected_commit = Some(id.clone());
            self.page = Page::CommitDiff;
            self.queue(Job::Inspect(vec!["diff".into(), id.clone()]));
            ui.close();
        }
        if commit.parents.len() < 2 && ui.button("Export commit patch...").clicked() {
            if let Some(destination) = rfd::FileDialog::new()
                .set_file_name(format!("{}.patch", commit.short))
                .save_file()
            {
                self.queue(Job::ExportPatch(
                    vec![
                        "format-patch".into(),
                        "-1".into(),
                        "--stdout".into(),
                        "--binary".into(),
                        id.clone(),
                    ],
                    destination,
                ));
            }
            ui.close();
        }
        ui.separator();
        if ui.button("Checkout this commit...").clicked() {
            self.confirm_checkout_commit = Some(id.clone());
            ui.close();
        }
        if ui.button("Create branch here...").clicked() {
            self.branch_input.clear();
            self.inline_branch_at = Some(id.clone());
            ui.close();
        }
        if ui.button("Create worktree from this commit...").clicked() {
            self.worktree_ref = id.clone();
            self.worktree_path.clear();
            self.worktree_branch.clear();
            self.page = Page::Worktrees;
            ui.close();
        }
        if ui.button("Create tag here...").clicked() {
            self.tag_input.clear();
            self.create_ref_at = Some((RefAtKind::Tag, id.clone()));
            ui.close();
        }
        if ui.button("Reset current branch here...").clicked() {
            self.confirm_reset = Some(id.clone());
            self.reset_mode = ResetMode::Mixed;
            self.reset_confirmation.clear();
            ui.close();
        }
        ui.separator();
        if commit.parents.len() < 2 {
            if ui.button("Cherry-pick commit...").clicked() {
                self.confirm_commit_action = Some((CommitAction::CherryPick, id.clone()));
                ui.close();
            }
            if ui.button("Revert commit...").clicked() {
                self.confirm_commit_action = Some((CommitAction::Revert, id.clone()));
                ui.close();
            }
        }
        if self.compare_base.as_deref() == Some(&id) {
            if ui.button("Clear compare base").clicked() {
                self.compare_base = None;
                ui.close();
            }
        } else {
            if ui.button("Set compare base").clicked() {
                self.compare_base = Some(id.clone());
                ui.close();
            }
            if let Some(base) = self.compare_base.clone()
                && ui.button("Compare with base").clicked()
            {
                self.selected_commit = Some(id.clone());
                self.page = Page::CommitDiff;
                self.queue(Job::Inspect(vec![
                    "diff".into(),
                    "--no-color".into(),
                    base,
                    id.clone(),
                ]));
                ui.close();
            }
        }
        ui.separator();
        if ui.button("Copy commit SHA").clicked() {
            ui.ctx().copy_text(id);
            ui.close();
        }
    }

    fn commit_diff_view(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if ui.button("Back to graph").clicked() {
                self.page = Page::History;
            }
            if let Some(path) = &self.selected_commit_file {
                ui.label(RichText::new(path).size(15.0).strong().color(text()));
            }
        });
        if let Some(id) = &self.selected_commit {
            let commit = self
                .snapshot
                .as_ref()
                .and_then(|s| s.commits.iter().find(|c| &c.id == id));
            if let Some(commit) = commit {
                ui.add_space(8.0);
                ui.label(
                    RichText::new(&commit.subject)
                        .size(18.0)
                        .strong()
                        .color(text()),
                );
                ui.label(
                    RichText::new(format!(
                        "{}  ·  {}  ·  {}",
                        commit.author, commit.date, commit.short
                    ))
                    .size(11.0)
                    .color(muted()),
                );
            }
        }
        ui.add_space(8.0);
        ui.separator();
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.diff_split, false, "Unified");
            ui.selectable_value(&mut self.diff_split, true, "Side by side");
        });
        if self.detail.is_empty() {
            ui.label(RichText::new("Loading commit diff...").color(muted()));
        } else {
            let mut sections = Vec::<Vec<&str>>::new();
            for line in self.detail.lines() {
                if line.starts_with("diff --git ") {
                    sections.push(Vec::new());
                }
                if let Some(section) = sections.last_mut() {
                    section.push(line);
                }
            }
            if sections.is_empty() {
                ui.label(
                    RichText::new(&self.detail)
                        .monospace()
                        .size(12.0)
                        .color(muted()),
                );
                return;
            }
            egui::ScrollArea::both()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.set_min_width(820.0);
                    for (index, section) in sections.into_iter().enumerate() {
                        let path = section
                            .iter()
                            .find_map(|line| line.strip_prefix("+++ b/"))
                            .or_else(|| section.iter().find_map(|line| line.strip_prefix("--- a/")))
                            .unwrap_or("Changed file");
                        let patch = section.join("\n");
                        let hunks = git::diff_hunks(&patch);
                        egui::CollapsingHeader::new(
                            RichText::new(path).size(13.0).strong().color(text()),
                        )
                        .id_salt(("commit_patch", index, path))
                        .default_open(index == 0 || self.selected_commit_file.is_some())
                        .show(ui, |ui| {
                            if hunks.is_empty() {
                                for line in section.iter().skip(1) {
                                    diff_line(ui, line);
                                }
                            }
                            for hunk in hunks {
                                ui.add_space(9.0);
                                egui::Frame::new()
                                    .fill(panel_alt())
                                    .inner_margin(egui::Margin::symmetric(8, 4))
                                    .show(ui, |ui| {
                                        ui.label(
                                            RichText::new(&hunk.heading)
                                                .monospace()
                                                .size(11.0)
                                                .color(violet()),
                                        );
                                    });
                                if self.diff_split {
                                    for row in hunk.split_rows() {
                                        split_diff_row(ui, &row);
                                    }
                                } else {
                                    let numbers = hunk.line_numbers();
                                    for (i, line) in hunk.lines.iter().enumerate() {
                                        let (old, new) = numbers[i];
                                        diff_line_numbered(ui, line, old, new, None, false);
                                    }
                                }
                            }
                        });
                    }
                });
        }
    }

    fn changes(&mut self, ui: &mut egui::Ui) {
        let Some(snapshot) = self.snapshot.clone() else {
            self.empty(ui);
            return;
        };
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(format!(
                    "{} file changes on {}",
                    snapshot.status.len(),
                    snapshot.branch
                ))
                .size(11.0)
                .strong()
                .color(text()),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add_enabled(!self.busy, egui::Button::new("Apply patch..."))
                    .clicked()
                    && let Some(path) = rfd::FileDialog::new()
                        .add_filter("Git patch", &["patch", "diff"])
                        .pick_file()
                {
                    self.git_owned(vec![
                        "apply".into(),
                        "--".into(),
                        path.to_string_lossy().into_owned(),
                    ]);
                }
            });
        });
        ui.separator();
        let unstaged: Vec<_> = snapshot
            .status
            .iter()
            .filter(|f| f.unstaged() && !f.conflicted())
            .cloned()
            .collect();
        let staged: Vec<_> = snapshot
            .status
            .iter()
            .filter(|f| f.staged() && !f.conflicted())
            .cloned()
            .collect();
        let conflicts: Vec<_> = snapshot
            .status
            .iter()
            .filter(|f| f.conflicted())
            .cloned()
            .collect();
        if snapshot.merge_in_progress {
            egui::Frame::new()
                .fill(panel_alt())
                .corner_radius(egui::CornerRadius::same(10))
                .stroke(Stroke::new(1.0, orange()))
                .inner_margin(egui::Margin::same(14))
                .show(ui, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        badge(ui, "MERGE IN PROGRESS", orange());
                        ui.label("Resolve every conflict, then create the merge commit.");
                        if action(ui, "Abort merge...", false) {
                            self.confirm_abort_merge = true;
                        }
                    });
                });
            ui.add_space(10.0);
        }
        if snapshot.status.is_empty() && !snapshot.merge_in_progress {
            egui::Frame::new()
                .fill(panel())
                .corner_radius(egui::CornerRadius::same(10))
                .stroke(Stroke::new(1.0, border()))
                .inner_margin(egui::Margin::same(22))
                .show(ui, |ui| {
                    ui.label(
                        RichText::new("All clear")
                            .size(20.0)
                            .strong()
                            .color(accent()),
                    );
                    ui.label(
                        RichText::new(
                            "Your working tree is clean. Enjoy the calm before the next commit.",
                        )
                        .color(muted()),
                    );
                });
        }
        let file_area_height = (ui.available_height() - 190.0).max(140.0);
        egui::ScrollArea::vertical()
            .max_height(file_area_height)
            .show(ui, |ui| {
                if !conflicts.is_empty() {
                    ui.label(
                        RichText::new(format!("CONFLICTS  |  {}", conflicts.len()))
                            .size(11.0)
                            .strong()
                            .color(red()),
                    );
                    ui.add_space(4.0);
                    for file in &conflicts {
                        self.conflict_row(ui, file);
                    }
                    ui.add_space(16.0);
                }
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(format!("UNSTAGED  |  {}", unstaged.len()))
                            .size(11.0)
                            .strong()
                            .color(orange()),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_enabled_ui(conflicts.is_empty(), |ui| {
                            if action(ui, "Stage all", false) {
                                self.git(&["add", "-A"]);
                            }
                        });
                    });
                });
                ui.add_space(4.0);
                for file in &unstaged {
                    self.file_row(ui, file, false);
                }
                if unstaged.is_empty() {
                    ui.label(RichText::new("Nothing to stage").size(12.0).color(muted()));
                }
                ui.add_space(16.0);
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(format!("STAGED  |  {}", staged.len()))
                            .size(11.0)
                            .strong()
                            .color(accent()),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if conflicts.is_empty() && action(ui, "Unstage all", false) {
                            if snapshot.commits.is_empty() {
                                self.git(&["rm", "-r", "--cached", "--", "."]);
                            } else {
                                self.git(&["restore", "--staged", "."]);
                            }
                        }
                    });
                });
                ui.add_space(4.0);
                for file in &staged {
                    self.file_row(ui, file, true);
                }
                if staged.is_empty() {
                    ui.label(
                        RichText::new("Stage files to prepare a commit")
                            .size(12.0)
                            .color(muted()),
                    );
                }
            });
        ui.add_space(7.0);
        egui::Frame::new()
            .fill(panel_alt())
            .inner_margin(egui::Margin::same(8))
            .show(ui, |ui| {
                ui.label(
                    RichText::new("NEW COMMIT")
                        .size(10.0)
                        .strong()
                        .color(accent()),
                );
                ui.add(
                    egui::TextEdit::multiline(&mut self.commit_message)
                        .hint_text("What changed? Write a clear commit message...")
                        .desired_width(ui.available_width())
                        .desired_rows(2),
                );
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(if snapshot.merge_in_progress && staged.is_empty() {
                            "Merge is ready to complete".to_owned()
                        } else {
                            format!("{} staged files", staged.len())
                        })
                        .size(11.0)
                        .color(muted()),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .add_enabled(
                                !self.commit_message.trim().is_empty()
                                    && (!staged.is_empty() || snapshot.merge_in_progress)
                                    && conflicts.is_empty(),
                                egui::Button::new(
                                    RichText::new(if snapshot.merge_in_progress {
                                        "Complete merge"
                                    } else {
                                        "Commit changes"
                                    })
                                    .strong()
                                    .color(bg()),
                                )
                                .fill(accent())
                                .stroke(Stroke::NONE),
                            )
                            .clicked()
                        {
                            self.committing = true;
                            self.git_owned(vec![
                                "commit".into(),
                                "-m".into(),
                                self.commit_message.clone(),
                            ]);
                        }
                    });
                });
            });
    }

    fn file_row(&mut self, ui: &mut egui::Ui, file: &git::FileStatus, staged: bool) {
        let selected = self.selected_file.as_deref() == Some(&file.path)
            && self.selected_file_staged == staged;
        let status = if file.index == '?' {
            "NEW"
        } else if staged {
            "STAGED"
        } else {
            "EDITED"
        };
        egui::Frame::new()
            .fill(if selected { elevated() } else { panel() })
            .inner_margin(egui::Margin::symmetric(6, 1))
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(if status == "NEW" { "+" } else { "M" })
                            .size(11.0)
                            .strong()
                            .color(if staged { accent() } else { orange() }),
                    );
                    if ui
                        .add(
                            egui::Button::new(RichText::new(&file.path).size(12.0).color(text()))
                                .fill(Color32::TRANSPARENT)
                                .stroke(Stroke::NONE),
                        )
                        .clicked()
                    {
                        self.selected_file = Some(file.path.clone());
                        self.selected_file_staged = staged;
                        self.diff_hunk_focus = 0;
                        self.diff_scroll_pending = true;
                        self.selected_commit = None;
                        self.detail.clear();
                        if file.index == '?' {
                            self.queue(Job::InspectFile(file.path.clone()));
                        } else {
                            let mut args = vec![
                                "diff".to_owned(),
                                "--no-ext-diff".to_owned(),
                                "--no-color".to_owned(),
                            ];
                            if staged {
                                args.push("--cached".to_owned());
                            }
                            args.extend(["--".to_owned(), file.path.clone()]);
                            self.queue(Job::Inspect(args));
                        }
                    }
                });
            });
        ui.add_space(1.0);
    }

    fn conflict_row(&mut self, ui: &mut egui::Ui, file: &git::FileStatus) {
        egui::Frame::new()
            .fill(panel_alt())
            .corner_radius(egui::CornerRadius::same(8))
            .stroke(Stroke::new(1.0, red()))
            .inner_margin(egui::Margin::symmetric(10, 7))
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    badge(ui, "CONFLICT", red());
                    if ui
                        .add(
                            egui::Button::new(RichText::new(&file.path).size(12.0).color(text()))
                                .fill(Color32::TRANSPARENT)
                                .stroke(Stroke::NONE),
                        )
                        .clicked()
                    {
                        self.selected_file = Some(file.path.clone());
                        self.selected_file_staged = false;
                        self.selected_commit = None;
                        self.queue(Job::InspectConflict(file.path.clone()));
                    }
                    ui.add_enabled_ui(!self.busy, |ui| {
                        if action(ui, "Edit lines", true) {
                            self.queue(Job::LoadConflict(file.path.clone()));
                        }
                        if action(ui, "Use ours...", false) {
                            self.confirm_conflict_side =
                                Some((file.path.clone(), git::ConflictSide::Ours));
                        }
                        if action(ui, "Use theirs...", false) {
                            self.confirm_conflict_side =
                                Some((file.path.clone(), git::ConflictSide::Theirs));
                        }
                        if action(ui, "Mark resolved", false) {
                            self.conflict_action = true;
                            self.queue(Job::MarkResolved(file.path.clone()));
                        }
                    });
                });
            });
        ui.add_space(4.0);
    }

    fn branches(&mut self, ui: &mut egui::Ui) {
        section_title(
            ui,
            "NAVIGATE YOUR IDEAS",
            "Branches & tags",
            "Keep experiments moving without losing your place.",
        );
        ui.add_space(18.0);
        let Some(snapshot) = &self.snapshot else {
            self.empty(ui);
            return;
        };
        let branches = snapshot.branches.clone();
        let local_names = branches.iter().map(|r| r.name.clone()).collect::<Vec<_>>();
        let remote_branches = snapshot.remote_branches.clone();
        let remotes = snapshot.remotes.clone();
        let remote_urls = snapshot.remote_urls.clone();
        let tags = snapshot.tags.clone();
        egui::Frame::new()
            .fill(panel_alt())
            .corner_radius(egui::CornerRadius::same(10))
            .stroke(Stroke::new(1.0, border()))
            .inner_margin(egui::Margin::same(14))
            .show(ui, |ui| {
                ui.label(
                    RichText::new("CREATE A BRANCH")
                        .size(10.0)
                        .strong()
                        .color(accent()),
                );
                ui.horizontal(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut self.branch_input)
                            .hint_text("feature/your-idea")
                            .desired_width(250.0),
                    );
                    if ui
                        .add_enabled(
                            !self.branch_input.trim().is_empty(),
                            egui::Button::new(
                                RichText::new("Create & switch").strong().color(bg()),
                            )
                            .fill(accent())
                            .stroke(Stroke::NONE),
                        )
                        .clicked()
                    {
                        let name = self.branch_input.trim().to_owned();
                        self.git_owned(vec!["switch".into(), "-c".into(), name]);
                        self.branch_input.clear();
                    }
                });
            });
        ui.add_space(20.0);
        ui.label(
            RichText::new(format!("LOCAL BRANCHES  |  {}", branches.len()))
                .size(11.0)
                .strong()
                .color(muted()),
        );
        ui.add_space(5.0);
        egui::ScrollArea::vertical()
            .max_height((ui.available_height() - 160.0).max(150.0))
            .show(ui, |ui| {
                for branch in branches {
                    egui::Frame::new()
                        .fill(panel())
                        .corner_radius(egui::CornerRadius::same(8))
                        .stroke(Stroke::new(1.0, border()))
                        .inner_margin(egui::Margin::symmetric(12, 6))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                let (dot, _) = ui.allocate_exact_size(
                                    egui::vec2(11.0, 16.0),
                                    egui::Sense::hover(),
                                );
                                ui.painter().circle_filled(
                                    dot.center(),
                                    3.5,
                                    if branch.current { accent() } else { muted() },
                                );
                                ui.vertical(|ui| {
                                    ui.label(
                                        RichText::new(&branch.name)
                                            .size(13.0)
                                            .strong()
                                            .color(text()),
                                    );
                                    ui.label(
                                        RichText::new(&branch.target)
                                            .size(10.0)
                                            .monospace()
                                            .color(muted()),
                                    );
                                });
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if !branch.current && action(ui, "Switch", true) {
                                            self.git_owned(vec![
                                                "switch".into(),
                                                branch.name.clone(),
                                            ]);
                                        }
                                        if !branch.current && action(ui, "Merge", false) {
                                            self.git_owned(vec![
                                                "merge".into(),
                                                branch.name.clone(),
                                            ]);
                                        }
                                        if !branch.current && action(ui, "Delete...", false) {
                                            self.confirm_delete_ref =
                                                Some(RefDeletion::Branch(branch.name.clone()));
                                        }
                                        if branch.current {
                                            badge(ui, "CURRENT", accent());
                                        }
                                    },
                                );
                            });
                        });
                    ui.add_space(4.0);
                }
                ui.add_space(14.0);
                ui.label(
                    RichText::new(format!("TAGS  |  {}", tags.len()))
                        .size(11.0)
                        .strong()
                        .color(muted()),
                );
                ui.horizontal(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut self.tag_input)
                            .hint_text("New tag name")
                            .desired_width(200.0),
                    );
                    if ui
                        .add_enabled(
                            !self.busy
                                && !self.tag_input.trim().is_empty()
                                && (!self.annotated_tag || !self.tag_message.trim().is_empty()),
                            egui::Button::new("Create tag"),
                        )
                        .clicked()
                    {
                        let name = self.tag_input.trim().to_owned();
                        let args = if self.annotated_tag {
                            vec![
                                "tag".into(),
                                "-a".into(),
                                name,
                                "-m".into(),
                                self.tag_message.trim().to_owned(),
                            ]
                        } else {
                            vec!["tag".into(), name]
                        };
                        self.git_owned(args);
                        self.tag_input.clear();
                        self.tag_message.clear();
                    }
                });
                ui.checkbox(&mut self.annotated_tag, "Annotated tag");
                if self.annotated_tag {
                    ui.add(
                        egui::TextEdit::singleline(&mut self.tag_message)
                            .hint_text("Tag message")
                            .desired_width(300.0),
                    );
                }
                for tag in tags {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(format!("tag  {}    {}", tag.name, tag.target))
                                .size(12.0)
                                .color(text()),
                        );
                        if action(ui, "Delete...", false) {
                            self.confirm_delete_ref = Some(RefDeletion::Tag(tag.name.clone()));
                        }
                    });
                }
                ui.add_space(15.0);
                ui.label(
                    RichText::new(format!("REMOTE BRANCHES  |  {}", remote_branches.len()))
                        .size(11.0)
                        .strong()
                        .color(blue()),
                );
                for remote in remote_branches {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(&remote.name).size(12.0).color(text()));
                        ui.label(
                            RichText::new(&remote.target)
                                .size(10.0)
                                .monospace()
                                .color(muted()),
                        );
                        let local = remote.name.split_once('/').map(|(_, name)| name);
                        let already_local = local.is_some_and(|name| {
                            local_names.iter().any(|local_name| local_name == name)
                        });
                        if ui
                            .add_enabled(!already_local && !self.busy, egui::Button::new("Track"))
                            .clicked()
                        {
                            self.git_owned(vec![
                                "switch".into(),
                                "--track".into(),
                                remote.name.clone(),
                            ]);
                        }
                    });
                }
                ui.add_space(15.0);
                ui.separator();
                ui.label(
                    RichText::new(format!("REMOTES  |  {}", remotes.len()))
                        .size(11.0)
                        .strong()
                        .color(muted()),
                );
                ui.horizontal(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut self.remote_name)
                            .hint_text("Name")
                            .desired_width(110.0),
                    );
                    ui.add(
                        egui::TextEdit::singleline(&mut self.remote_url)
                            .hint_text("Repository URL")
                            .desired_width(300.0),
                    );
                    if ui
                        .add_enabled(
                            !self.busy
                                && !self.remote_name.trim().is_empty()
                                && !self.remote_url.trim().is_empty(),
                            egui::Button::new("Add remote"),
                        )
                        .clicked()
                    {
                        self.git_owned(vec![
                            "remote".into(),
                            "add".into(),
                            self.remote_name.trim().to_owned(),
                            self.remote_url.trim().to_owned(),
                        ]);
                        self.remote_name.clear();
                        self.remote_url.clear();
                    }
                });
                for remote in remotes {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(&remote).color(text()));
                        let url = remote_urls.get(&remote).cloned().unwrap_or_default();
                        ui.add_sized(
                            [280.0, 22.0],
                            egui::Label::new(RichText::new(&url).small().color(muted())).truncate(),
                        )
                        .on_hover_text(&url);
                        if ui
                            .add_enabled(!self.busy, egui::Button::new("Edit URL..."))
                            .clicked()
                        {
                            self.edit_remote = Some((remote.clone(), url));
                        }
                        if ui
                            .add_enabled(!self.busy, egui::Button::new("Remove..."))
                            .clicked()
                        {
                            self.confirm_remove_remote = Some(remote);
                        }
                    });
                }
            });
    }

    fn worktree_page(&mut self, ui: &mut egui::Ui) {
        section_title(
            ui,
            "PARALLEL CHECKOUTS",
            "Worktrees",
            "Keep another branch in its own folder.",
        );
        ui.add_space(14.0);
        let Some(snapshot) = &self.snapshot else {
            self.empty(ui);
            return;
        };
        let current_root = snapshot.root.clone();
        let worktrees = snapshot.worktrees.clone();
        egui::Frame::new()
            .fill(panel_alt())
            .stroke(Stroke::new(1.0, border()))
            .inner_margin(egui::Margin::same(12))
            .show(ui, |ui| {
                ui.label(
                    RichText::new("ADD WORKTREE")
                        .small()
                        .strong()
                        .color(accent()),
                );
                ui.horizontal(|ui| {
                    ui.label("Folder");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.worktree_path).desired_width(380.0),
                    );
                    if action(ui, "Browse parent", false)
                        && let Some(parent) = pick_folder(current_root.parent())
                    {
                        let name = self
                            .worktree_branch
                            .rsplit('/')
                            .next()
                            .filter(|s| !s.is_empty())
                            .unwrap_or("worktree");
                        self.worktree_path = parent.join(name).to_string_lossy().into_owned();
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("New branch (optional)");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.worktree_branch).desired_width(280.0),
                    );
                });
                ui.horizontal(|ui| {
                    ui.label("Starting commit or branch");
                    ui.add(egui::TextEdit::singleline(&mut self.worktree_ref).desired_width(280.0));
                });
                ui.label(
                    RichText::new(
                        "Leave branch blank for a detached checkout at the chosen revision.",
                    )
                    .small()
                    .color(muted()),
                );
                let enabled = !self.busy
                    && !self.worktree_path.trim().is_empty()
                    && !self.worktree_ref.trim().is_empty();
                if ui
                    .add_enabled(enabled, egui::Button::new("Add worktree"))
                    .clicked()
                {
                    let mut args = vec!["worktree".into(), "add".into()];
                    if self.worktree_branch.trim().is_empty() {
                        args.push("--detach".into());
                    } else {
                        args.extend(["-b".into(), self.worktree_branch.trim().to_owned()]);
                    }
                    args.push(self.worktree_path.trim().to_owned());
                    args.push(self.worktree_ref.trim().to_owned());
                    self.git_owned(args);
                }
            });
        ui.add_space(18.0);
        ui.label(
            RichText::new(format!("WORKTREES  |  {}", worktrees.len()))
                .small()
                .strong()
                .color(muted()),
        );
        ui.add_space(5.0);
        egui::ScrollArea::vertical().show(ui, |ui| {
            for worktree in worktrees {
                let is_current = worktree.current;
                egui::Frame::new()
                    .fill(panel())
                    .stroke(Stroke::new(1.0, border()))
                    .inner_margin(egui::Margin::symmetric(12, 8))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.vertical(|ui| {
                                ui.label(
                                    RichText::new(worktree.path.display().to_string()).strong(),
                                );
                                let branch = worktree.branch.as_deref().unwrap_or("Detached HEAD");
                                ui.label(
                                    RichText::new(format!(
                                        "{}  ·  {}",
                                        branch,
                                        &worktree.head[..worktree.head.len().min(8)]
                                    ))
                                    .small()
                                    .color(muted()),
                                );
                                if worktree.prunable {
                                    ui.label(
                                        RichText::new("Missing folder; metadata can be pruned")
                                            .small()
                                            .color(orange()),
                                    );
                                }
                                if let Some(count) = worktree.dirty_count.filter(|count| *count > 0)
                                {
                                    ui.label(
                                        RichText::new(format!(
                                            "{count} file changes in this worktree"
                                        ))
                                        .small()
                                        .color(orange()),
                                    );
                                }
                            });
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.add_enabled_ui(!self.busy, |ui| {
                                        if !is_current
                                            && !worktree.prunable
                                            && action(ui, "Open", true)
                                        {
                                            self.queue(Job::Open(worktree.path.clone()));
                                        }
                                        if !is_current
                                            && !worktree.prunable
                                            && action(ui, "Remove...", false)
                                        {
                                            self.confirm_remove_worktree =
                                                Some(worktree.path.clone());
                                        }
                                        if !is_current
                                            && !worktree.prunable
                                            && action(
                                                ui,
                                                if worktree.locked { "Unlock" } else { "Lock" },
                                                false,
                                            )
                                        {
                                            self.git_owned(vec![
                                                "worktree".into(),
                                                if worktree.locked { "unlock" } else { "lock" }
                                                    .into(),
                                                worktree.path.to_string_lossy().into_owned(),
                                            ]);
                                        }
                                    });
                                    if is_current {
                                        badge(ui, "CURRENT", accent());
                                    }
                                },
                            );
                        });
                    });
                ui.add_space(5.0);
            }
            if ui
                .add_enabled(!self.busy, egui::Button::new("Prune missing worktrees"))
                .clicked()
            {
                self.git(&["worktree", "prune"]);
            }
        });
    }

    fn submodule_page(&mut self, ui: &mut egui::Ui) {
        section_title(
            ui,
            "NESTED REPOSITORIES",
            "Submodules",
            "Initialize and update repositories tracked inside this project.",
        );
        ui.add_space(14.0);
        let Some(snapshot) = &self.snapshot else {
            self.empty(ui);
            return;
        };
        let submodules = snapshot.submodules.clone();
        let root = snapshot.root.clone();
        egui::Frame::new()
            .fill(panel_alt())
            .stroke(Stroke::new(1.0, border()))
            .inner_margin(egui::Margin::same(12))
            .show(ui, |ui| {
                ui.label(
                    RichText::new("ADD SUBMODULE")
                        .small()
                        .strong()
                        .color(accent()),
                );
                ui.horizontal(|ui| {
                    ui.label("URL");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.submodule_url).desired_width(420.0),
                    );
                });
                ui.horizontal(|ui| {
                    ui.label("Path");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.submodule_path).desired_width(280.0),
                    );
                });
                if ui
                    .add_enabled(
                        !self.busy
                            && !self.submodule_url.trim().is_empty()
                            && !self.submodule_path.trim().is_empty(),
                        egui::Button::new("Add submodule"),
                    )
                    .clicked()
                {
                    self.git_owned(vec![
                        "submodule".into(),
                        "add".into(),
                        "--".into(),
                        self.submodule_url.trim().to_owned(),
                        self.submodule_path.trim().to_owned(),
                    ]);
                }
            });
        ui.add_space(15.0);
        ui.add_enabled_ui(!self.busy, |ui| {
            ui.horizontal(|ui| {
                if action(ui, "Initialize / update all", true) {
                    self.git(&["submodule", "update", "--init", "--recursive"]);
                }
                if action(ui, "Sync URLs", false) {
                    self.git(&["submodule", "sync", "--recursive"]);
                }
            });
        });
        ui.add_space(16.0);
        ui.label(
            RichText::new(format!("SUBMODULES  |  {}", submodules.len()))
                .small()
                .strong()
                .color(muted()),
        );
        ui.add_space(5.0);
        egui::ScrollArea::vertical().show(ui, |ui| {
            for submodule in submodules {
                egui::Frame::new()
                    .fill(panel())
                    .stroke(Stroke::new(1.0, border()))
                    .inner_margin(egui::Margin::symmetric(12, 8))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.vertical(|ui| {
                                ui.label(
                                    RichText::new(format!(
                                        "{}  ·  {}",
                                        submodule.path, submodule.name
                                    ))
                                    .strong(),
                                );
                                ui.label(RichText::new(&submodule.url).small().color(muted()));
                                ui.label(
                                    RichText::new(if submodule.initialized {
                                        "Initialized"
                                    } else {
                                        "Not initialized"
                                    })
                                    .small()
                                    .color(
                                        if submodule.initialized {
                                            accent()
                                        } else {
                                            orange()
                                        },
                                    ),
                                );
                            });
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.add_enabled_ui(!self.busy, |ui| {
                                        if submodule.initialized && action(ui, "Open", true) {
                                            self.queue(Job::Open(root.join(&submodule.path)));
                                        }
                                        if action(ui, "Update", false) {
                                            self.git_owned(vec![
                                                "submodule".into(),
                                                "update".into(),
                                                "--init".into(),
                                                "--".into(),
                                                submodule.path.clone(),
                                            ]);
                                        }
                                    });
                                },
                            );
                        });
                    });
                ui.add_space(5.0);
            }
        });
    }

    fn pull_request_page(&mut self, ui: &mut egui::Ui) {
        section_title(
            ui,
            "COLLABORATE ON GITHUB",
            "Pull requests",
            "Review and open pull requests using your GitHub CLI login.",
        );
        ui.add_space(14.0);
        let Some(snapshot) = &self.snapshot else {
            self.empty(ui);
            return;
        };
        let root = snapshot.root.clone();
        let branch = snapshot.branch.clone();
        let bases = snapshot
            .branches
            .iter()
            .filter(|reference| reference.name != branch)
            .map(|reference| reference.name.clone())
            .collect::<Vec<_>>();
        if self.pr_loaded_for.as_ref() != Some(&root) && !self.busy {
            self.pr_loaded_for = Some(root);
            self.queue(Job::LoadPullRequests);
        }
        egui::Frame::new().fill(panel_alt()).stroke(Stroke::new(1.0, border())).inner_margin(egui::Margin::same(14)).show(ui, |ui| {
            ui.label(RichText::new("OPEN A PULL REQUEST").small().strong().color(accent()));
            ui.add_space(5.0);
            ui.label(format!("From current branch: {branch}"));
            ui.horizontal(|ui| {
                ui.label("Base branch");
                ui.add(egui::TextEdit::singleline(&mut self.pr_base).desired_width(220.0));
                egui::ComboBox::from_id_salt("pr_base_picker").selected_text("Choose branch").show_ui(ui, |ui| {
                    for base in bases {
                        if ui.selectable_label(false, &base).clicked() { self.pr_base = base; }
                    }
                });
            });
            ui.horizontal(|ui| {
                ui.label("Title");
                ui.add(egui::TextEdit::singleline(&mut self.pr_title).desired_width(430.0));
            });
            ui.label("Description");
            ui.add(egui::TextEdit::multiline(&mut self.pr_body).desired_rows(4).desired_width(520.0));
            let enabled = !self.busy && !branch.is_empty() && !self.pr_base.trim().is_empty() && !self.pr_title.trim().is_empty();
            if ui.add_enabled(enabled, egui::Button::new("Create pull request")).clicked() {
                self.queue(Job::CreatePullRequest {
                    head: branch,
                    base: self.pr_base.trim().to_owned(),
                    title: self.pr_title.trim().to_owned(),
                    body: self.pr_body.clone(),
                });
            }
            ui.label(RichText::new("Requires the GitHub CLI (`gh`) and an existing login. Push the branch before creating a pull request.").small().color(muted()));
        });
        ui.add_space(14.0);
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(format!(
                    "RECENT PULL REQUESTS  |  {}",
                    self.pull_requests.len()
                ))
                .small()
                .strong()
                .color(muted()),
            );
            if ui
                .add_enabled(!self.busy, egui::Button::new("Refresh"))
                .clicked()
            {
                self.queue(Job::LoadPullRequests);
            }
        });
        if !self.error.is_empty() {
            ui.label(RichText::new(&self.error).color(red()));
        }
        let requests = self.pull_requests.clone();
        egui::ScrollArea::vertical().show(ui, |ui| {
            for request in requests {
                egui::Frame::new()
                    .fill(panel())
                    .stroke(Stroke::new(1.0, border()))
                    .inner_margin(egui::Margin::symmetric(12, 8))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            badge(
                                ui,
                                &request.state,
                                if request.state == "OPEN" {
                                    accent()
                                } else {
                                    muted()
                                },
                            );
                            ui.hyperlink_to(
                                format!("#{}  {}", request.number, request.title),
                                &request.url,
                            );
                        });
                        ui.label(
                            RichText::new(format!("{} → {}", request.head, request.base))
                                .small()
                                .color(muted()),
                        );
                    });
                ui.add_space(5.0);
            }
        });
    }

    fn rebase_page(&mut self, ui: &mut egui::Ui) {
        section_title(
            ui,
            "REWRITE BRANCH HISTORY",
            "Rebase",
            "Replay the current branch onto another branch or commit.",
        );
        ui.add_space(14.0);
        let Some(snapshot) = &self.snapshot else {
            self.empty(ui);
            return;
        };
        let branch = snapshot.branch.clone();
        let dirty = !snapshot.status.is_empty();
        let in_progress = snapshot.rebase_in_progress;
        let conflicts = snapshot
            .status
            .iter()
            .filter(|file| file.conflicted())
            .map(|file| file.path.clone())
            .collect::<Vec<_>>();
        let targets = snapshot
            .branches
            .iter()
            .chain(&snapshot.remote_branches)
            .filter(|reference| reference.name != branch)
            .map(|reference| reference.name.clone())
            .collect::<Vec<_>>();
        if in_progress {
            egui::Frame::new()
                .fill(panel_alt())
                .stroke(Stroke::new(1.0, orange()))
                .inner_margin(egui::Margin::same(14))
                .show(ui, |ui| {
                    badge(ui, "REBASE IN PROGRESS", orange());
                    if !conflicts.is_empty() {
                        ui.label("Resolve and stage these files before continuing:");
                        for path in &conflicts {
                            ui.label(RichText::new(path).color(orange()));
                        }
                        if action(ui, "View changes", false) {
                            self.page = Page::Changes;
                        }
                    } else {
                        ui.label("Continue to apply the next commit, or stop the rebase.");
                    }
                    ui.add_enabled_ui(!self.busy, |ui| {
                        ui.horizontal(|ui| {
                            if action(ui, "Continue", true) {
                                self.git(&["-c", "core.editor=true", "rebase", "--continue"]);
                            }
                            if action(ui, "Skip commit...", false) {
                                self.confirm_rebase = Some(vec!["rebase".into(), "--skip".into()]);
                            }
                            if action(ui, "Abort...", false) {
                                self.confirm_rebase = Some(vec!["rebase".into(), "--abort".into()]);
                            }
                        });
                    });
                });
            return;
        }
        egui::Frame::new()
            .fill(panel_alt())
            .stroke(Stroke::new(1.0, border()))
            .inner_margin(egui::Margin::same(14))
            .show(ui, |ui| {
                ui.label(format!("Current branch: {branch}"));
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.label("Rebase onto");
                    if ui
                        .add(
                            egui::TextEdit::singleline(&mut self.rebase_onto)
                                .hint_text("Branch, tag, or commit SHA")
                                .desired_width(330.0),
                        )
                        .changed()
                    {
                        self.interactive_plan = None;
                    }
                    egui::ComboBox::from_id_salt("rebase_target_picker")
                        .selected_text("Choose branch")
                        .show_ui(ui, |ui| {
                            for target in targets {
                                if ui.selectable_label(false, &target).clicked() {
                                    self.rebase_onto = target;
                                    self.interactive_plan = None;
                                }
                            }
                        });
                });
                ui.checkbox(
                    &mut self.rebase_autostash,
                    "Temporarily stash and restore local changes",
                );
                if dirty && !self.rebase_autostash {
                    ui.label(
                        RichText::new(
                            "The working tree has changes. Enable autostash or commit them first.",
                        )
                        .color(orange()),
                    );
                }
                ui.add_space(10.0);
                let enabled = !self.busy
                    && !branch.is_empty()
                    && !self.rebase_onto.trim().is_empty()
                    && (!dirty || self.rebase_autostash);
                if ui
                    .add_enabled(enabled, egui::Button::new("Review rebase..."))
                    .clicked()
                {
                    let mut args = vec!["-c".into(), "core.editor=true".into(), "rebase".into()];
                    if self.rebase_autostash {
                        args.push("--autostash".into());
                    }
                    args.push(self.rebase_onto.trim().to_owned());
                    self.confirm_rebase = Some(args);
                }
            });
        ui.add_space(14.0);
        ui.separator();
        ui.heading("Interactive rebase");
        ui.label(
            "Review each commit before replaying it. Reorder, squash, reword, or drop commits.",
        );
        ui.add_space(6.0);
        if ui
            .add_enabled(
                !self.busy && !dirty && !branch.is_empty() && !self.rebase_onto.trim().is_empty(),
                egui::Button::new("Load commits to edit"),
            )
            .clicked()
        {
            self.interactive_plan = None;
            self.queue(Job::LoadInteractiveRebase(
                self.rebase_onto.trim().to_owned(),
            ));
        }
        if dirty {
            ui.label(
                RichText::new("Commit or stash local changes before interactive rebase.")
                    .color(orange()),
            );
        }
        if let Some(plan) = self.interactive_plan.as_mut() {
            ui.add_space(8.0);
            ui.label(format!("{} commits onto {}", plan.steps.len(), plan.target));
            let mut move_to = None;
            egui::ScrollArea::vertical()
                .max_height(390.0)
                .show(ui, |ui| {
                    let count = plan.steps.len();
                    for index in 0..count {
                        let step = &mut plan.steps[index];
                        ui.horizontal(|ui| {
                            ui.monospace(&step.id[..7]);
                            ui.label(&step.subject);
                            egui::ComboBox::from_id_salt(("rebase_step", &step.id))
                                .selected_text(step.action.label())
                                .show_ui(ui, |ui| {
                                    for action in [
                                        git::RebaseAction::Pick,
                                        git::RebaseAction::Squash,
                                        git::RebaseAction::Reword,
                                        git::RebaseAction::Drop,
                                    ] {
                                        ui.selectable_value(
                                            &mut step.action,
                                            action,
                                            action.label(),
                                        );
                                    }
                                });
                            if ui.add_enabled(index > 0, egui::Button::new("↑")).clicked() {
                                move_to = Some((index, index - 1));
                            }
                            if ui
                                .add_enabled(index + 1 < count, egui::Button::new("↓"))
                                .clicked()
                            {
                                move_to = Some((index, index + 1));
                            }
                        });
                        if step.action == git::RebaseAction::Reword {
                            ui.add(
                                egui::TextEdit::multiline(&mut step.message)
                                    .desired_width(f32::INFINITY)
                                    .desired_rows(3),
                            );
                        }
                        ui.separator();
                    }
                });
            if let Some((from, to)) = move_to {
                plan.steps.swap(from, to);
            }
            let kept = plan
                .steps
                .iter()
                .filter(|step| step.action != git::RebaseAction::Drop)
                .count();
            let first_squash = plan
                .steps
                .iter()
                .find(|step| step.action != git::RebaseAction::Drop)
                .is_some_and(|step| step.action == git::RebaseAction::Squash);
            let blank_message = plan.steps.iter().any(|step| {
                step.action == git::RebaseAction::Reword && step.message.trim().is_empty()
            });
            if first_squash {
                ui.label(
                    RichText::new("The first kept commit cannot be squashed.").color(orange()),
                );
            }
            if blank_message {
                ui.label(RichText::new("Reworded commits need a message.").color(orange()));
            }
            if ui
                .add_enabled(
                    !self.busy && !dirty && kept > 0 && !first_squash && !blank_message,
                    egui::Button::new("Review interactive rebase..."),
                )
                .clicked()
            {
                self.confirm_interactive_rebase = true;
            }
        }
        ui.add_space(10.0);
        ui.label(
            RichText::new(
                "Rebase changes commit IDs. Git keeps the previous tip in the reflog for recovery.",
            )
            .small()
            .color(muted()),
        );
    }

    fn stashes(&mut self, ui: &mut egui::Ui) {
        section_title(
            ui,
            "SAVE YOUR FLOW",
            "Stashes",
            "Set work aside and come back when the moment is right.",
        );
        ui.add_space(18.0);
        let Some(snapshot) = &self.snapshot else {
            self.empty(ui);
            return;
        };
        let stashes = snapshot.stashes.clone();
        if action(ui, "+  Stash current changes", true) {
            self.git(&["stash", "push", "-u"]);
        }
        ui.add_space(20.0);
        ui.label(
            RichText::new(format!("SAVED WORK  |  {}", stashes.len()))
                .size(11.0)
                .strong()
                .color(muted()),
        );
        ui.add_space(5.0);
        if stashes.is_empty() {
            egui::Frame::new()
                .fill(panel())
                .corner_radius(egui::CornerRadius::same(10))
                .stroke(Stroke::new(1.0, border()))
                .inner_margin(egui::Margin::same(22))
                .show(ui, |ui| {
                    ui.label(
                        RichText::new("Nothing tucked away")
                            .size(18.0)
                            .strong()
                            .color(text()),
                    );
                    ui.label(
                        RichText::new("Create a stash when you need a clean slate.").color(muted()),
                    );
                });
        }
        for stash in stashes {
            let name = stash.split_whitespace().next().unwrap_or("").to_owned();
            egui::Frame::new()
                .fill(panel())
                .corner_radius(egui::CornerRadius::same(8))
                .stroke(Stroke::new(1.0, border()))
                .inner_margin(egui::Margin::symmetric(12, 8))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        badge(ui, "STASH", violet());
                        ui.label(RichText::new(&stash).size(12.0).color(text()));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if action(ui, "Apply", true) {
                                self.git_owned(vec!["stash".into(), "apply".into(), name.clone()]);
                            }
                            if action(ui, "Pop", false) {
                                self.git_owned(vec!["stash".into(), "pop".into(), name.clone()]);
                            }
                            if action(ui, "Show", false) {
                                self.queue(Job::Inspect(vec![
                                    "stash".into(),
                                    "show".into(),
                                    "-p".into(),
                                    name.clone(),
                                ]));
                            }
                        });
                    });
                });
            ui.add_space(5.0);
        }
    }

    fn console(&mut self, ui: &mut egui::Ui) {
        self.console_dock(ui);
    }

    fn console_dock(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(RichText::new("Terminal").size(11.0).strong().color(text()));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .small_button("×")
                    .on_hover_text("Close console")
                    .clicked()
                {
                    self.terminal_open = false;
                }
                if ui.small_button("Clear").clicked() {
                    self.terminal_output.clear();
                }
            });
        });
        ui.separator();
        egui::ScrollArea::vertical()
            .stick_to_bottom(true)
            .max_height((ui.available_height() - 35.0).max(50.0))
            .show(ui, |ui| {
                if self.terminal_output.is_empty() {
                    ui.label(
                        RichText::new("Run a shell command in this repository.")
                            .size(11.0)
                            .color(muted()),
                    );
                } else {
                    ui.add(
                        egui::Label::new(
                            RichText::new(&self.terminal_output)
                                .monospace()
                                .size(11.0)
                                .color(text()),
                        )
                        .selectable(true),
                    );
                }
            });
        ui.horizontal(|ui| {
            let cwd = self.terminal_cwd.as_ref().or(self.repo.as_ref());
            let prompt = cwd
                .map(|path| path.display().to_string())
                .unwrap_or_default();
            ui.label(
                RichText::new(if cfg!(windows) {
                    format!("PS {prompt}>")
                } else {
                    format!("{prompt} $")
                })
                .monospace()
                .size(11.0)
                .color(accent()),
            );
            let edit = ui.add(
                egui::TextEdit::singleline(&mut self.command_input)
                    .hint_text("Enter a shell command")
                    .desired_width((ui.available_width() - 56.0).max(80.0)),
            );
            if edit.has_focus()
                && ui.input(|i| i.key_pressed(egui::Key::ArrowUp))
                && self.command_history_index > 0
            {
                self.command_history_index -= 1;
                self.command_input = self.command_history[self.command_history_index].clone();
            }
            if edit.has_focus() && ui.input(|i| i.key_pressed(egui::Key::ArrowDown)) {
                if self.command_history_index + 1 < self.command_history.len() {
                    self.command_history_index += 1;
                    self.command_input = self.command_history[self.command_history_index].clone();
                } else {
                    self.command_history_index = self.command_history.len();
                    self.command_input.clear();
                }
            }
            let enter = edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if ui.button("Run").clicked() || enter {
                self.run_terminal_input();
            }
        });
    }

    fn run_terminal_input(&mut self) {
        let command = self.command_input.trim().to_owned();
        if command.is_empty() {
            return;
        }
        let Some(cwd) = self.terminal_cwd.clone().or_else(|| self.repo.clone()) else {
            return;
        };
        self.terminal_output
            .push_str(&format!("{}> {command}\n", cwd.display()));
        self.command_history.push(command.clone());
        self.command_history_index = self.command_history.len();
        self.command_input.clear();
        if command.eq_ignore_ascii_case("clear") || command.eq_ignore_ascii_case("cls") {
            self.terminal_output.clear();
            return;
        }
        let change_directory = command
            .strip_prefix("cd ")
            .or_else(|| command.strip_prefix("Set-Location "));
        if let Some(target) = change_directory {
            let target = target.trim().trim_matches(['\'', '"']);
            let path = PathBuf::from(target);
            let path = if path.is_absolute() {
                path
            } else {
                cwd.join(path)
            };
            if path.is_dir() {
                self.terminal_cwd = Some(path.canonicalize().unwrap_or(path));
            } else {
                self.terminal_output.push_str("Directory does not exist.\n");
            }
            return;
        }
        self.queue(Job::Shell(command, cwd));
    }

    fn empty(&self, ui: &mut egui::Ui) {
        ui.vertical_centered(|ui| {
            ui.add_space(90.0);
            brand_icon(ui);
            ui.add_space(8.0);
            ui.label(
                RichText::new("Make your next move.")
                    .size(25.0)
                    .strong()
                    .color(text()),
            );
            ui.label(
                RichText::new("Open, initialize, or clone a repository from the sidebar to begin.")
                    .size(13.0)
                    .color(muted()),
            );
        });
    }

    fn updates_page(&mut self, ui: &mut egui::Ui) {
        section_title(
            ui,
            "STAY IN FLOW",
            "Updates",
            "Get the latest GitVibe release from GitHub.",
        );
        ui.add_space(16.0);
        egui::Frame::new()
            .fill(panel())
            .corner_radius(egui::CornerRadius::same(10))
            .stroke(Stroke::new(1.0, border()))
            .inner_margin(egui::Margin::same(18))
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    badge(ui, &format!("INSTALLED  v{}", env!("CARGO_PKG_VERSION")), accent());
                    if !matches!(
                        self.update_state,
                        UpdateState::Checking | UpdateState::Downloading(_)
                    ) && action(ui, "Check for updates", false)
                    {
                        self.check_updates(ui.ctx());
                    }
                });
                ui.add_space(14.0);
                match self.update_state.clone() {
                    UpdateState::Checking => {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label("Checking GitHub Releases...");
                        });
                    }
                    UpdateState::UpToDate(latest) => {
                        ui.label(
                            RichText::new("You're up to date.").size(17.0).strong().color(accent()),
                        );
                        ui.label(format!("Latest published release: {latest}"));
                    }
                    UpdateState::Available(release) => {
                        ui.label(
                            RichText::new(format!("{} is ready", release.version))
                                .size(19.0)
                                .strong()
                                .color(orange()),
                        );
                        ui.add_space(6.0);
                        if action(
                            ui,
                            if cfg!(target_os = "windows") {
                                "Download installer"
                            } else {
                                "Download update"
                            },
                            true,
                        ) {
                            self.download_update(ui.ctx(), release.clone());
                        }
                        ui.hyperlink_to("View release on GitHub", &release.page_url);
                        if !release.notes.is_empty() {
                            ui.add_space(10.0);
                            ui.label(RichText::new("RELEASE NOTES").size(10.0).strong().color(muted()));
                            egui::ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
                                ui.label(&release.notes);
                            });
                        }
                    }
                    UpdateState::Downloading(release) => {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label(format!("Downloading and verifying {}...", release.version));
                        });
                    }
                    UpdateState::Ready(release, path) => {
                        ui.label(
                            RichText::new(format!("{} downloaded and verified", release.version))
                                .size(17.0)
                                .strong()
                                .color(accent()),
                        );
                        ui.label(RichText::new(path.display().to_string()).monospace().color(muted()));
                        ui.add_space(8.0);
                        if cfg!(target_os = "windows") {
                            ui.label("The installer will open and GitVibe will close. It installs or updates your per-user copy. If you run the portable zip, launch the installed copy from the Start Menu afterward.");
                        } else {
                            ui.label("Open the archive, then replace your installed GitVibe app with the new version.");
                        }
                        if action(
                            ui,
                            if cfg!(target_os = "windows") {
                                "Install update"
                            } else {
                                "Open downloaded archive"
                            },
                            true,
                        ) {
                            match updates::open_download(&path) {
                                Ok(()) if cfg!(target_os = "windows") => {
                                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                                }
                                Ok(()) => {}
                                Err(error) => self.update_state = UpdateState::Failed(error),
                            }
                        }
                    }
                    UpdateState::Failed(error) => {
                        ui.label(RichText::new("Update check failed").size(17.0).color(red()));
                        ui.label(error);
                        ui.hyperlink_to(
                            "Open GitVibe releases",
                            "https://github.com/boubou666/GitVibe/releases",
                        );
                    }
                }
            });
    }

    fn change_diff_view(&mut self, ui: &mut egui::Ui) {
        let Some(path) = self.selected_file.clone() else {
            ui.vertical_centered(|ui| {
                ui.add_space(70.0);
                ui.label(
                    RichText::new("Review a file")
                        .size(24.0)
                        .strong()
                        .color(text()),
                );
                ui.label(
                    RichText::new("Choose a changed file in the right panel to inspect its diff.")
                        .size(12.0)
                        .color(muted()),
                );
            });
            return;
        };
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(&path).size(16.0).strong().color(text()));
            badge(
                ui,
                if self.selected_file_staged {
                    "STAGED"
                } else {
                    "UNSTAGED"
                },
                if self.selected_file_staged {
                    accent()
                } else {
                    orange()
                },
            );
        });
        ui.add_space(10.0);
        ui.horizontal_wrapped(|ui| {
            if self.selected_file_staged {
                if ui
                    .add_enabled(!self.busy, egui::Button::new("Unstage file"))
                    .clicked()
                {
                    if self
                        .snapshot
                        .as_ref()
                        .is_some_and(|snapshot| snapshot.commits.is_empty())
                    {
                        self.git_owned(vec![
                            "rm".into(),
                            "--cached".into(),
                            "--".into(),
                            path.clone(),
                        ]);
                    } else {
                        self.git_owned(vec![
                            "restore".into(),
                            "--staged".into(),
                            "--".into(),
                            path.clone(),
                        ]);
                    }
                    self.selected_file_staged = false;
                }
            } else {
                if ui
                    .add_enabled(!self.busy, egui::Button::new("Stage file"))
                    .clicked()
                {
                    self.git_owned(vec!["add".into(), "--".into(), path.clone()]);
                    self.selected_file_staged = true;
                }
                let tracked = self.snapshot.as_ref().is_some_and(|snapshot| {
                    snapshot
                        .status
                        .iter()
                        .any(|file| file.path == path && file.index != '?')
                });
                if tracked
                    && ui
                        .add_enabled(!self.busy, egui::Button::new("Discard file..."))
                        .clicked()
                {
                    self.confirm_discard = Some(path.clone());
                }
            }
            ui.separator();
            if ui
                .add_enabled(!self.busy, egui::Button::new("Diff view"))
                .clicked()
            {
                let mut args = vec!["diff".into(), "--no-ext-diff".into(), "--no-color".into()];
                if self.selected_file_staged {
                    args.push("--cached".into());
                }
                args.extend(["--".into(), path.clone()]);
                self.queue(Job::Inspect(args));
            }
            if ui
                .add_enabled(!self.busy, egui::Button::new("File view"))
                .clicked()
            {
                if self.selected_file_staged {
                    self.queue(Job::Inspect(vec!["show".into(), format!(":{path}")]));
                } else {
                    self.queue(Job::InspectFile(path.clone()));
                }
            }
            if ui
                .add_enabled(!self.busy, egui::Button::new("File history"))
                .clicked()
            {
                self.queue(Job::FileHistory(path.clone()));
            }
            if ui
                .add_enabled(!self.busy, egui::Button::new("Blame"))
                .clicked()
            {
                self.queue(Job::Inspect(vec![
                    "blame".into(),
                    "--".into(),
                    path.clone(),
                ]));
            }
            if ui
                .add_enabled(!self.busy, egui::Button::new("Export patch..."))
                .clicked()
                && let Some(destination) = rfd::FileDialog::new()
                    .set_file_name("changes.patch")
                    .save_file()
            {
                let mut args = vec![
                    "diff".into(),
                    "--binary".into(),
                    "--no-ext-diff".into(),
                    "--no-color".into(),
                ];
                if self.selected_file_staged {
                    args.push("--cached".into());
                }
                args.extend(["--".into(), path.clone()]);
                self.queue(Job::ExportPatch(args, destination));
            }
        });
        let hunk_count = git::diff_hunks(&self.detail).len();
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.diff_split, false, "Unified");
            ui.selectable_value(&mut self.diff_split, true, "Side by side");
            ui.add_enabled_ui(!self.diff_split, |ui| {
                ui.checkbox(&mut self.diff_wrap, "Wrap lines");
            });
            if hunk_count > 0 {
                ui.separator();
                if ui
                    .add_enabled(
                        self.diff_hunk_focus > 0,
                        egui::Button::new("↑ Previous change"),
                    )
                    .clicked()
                {
                    self.diff_hunk_focus -= 1;
                    self.diff_scroll_pending = true;
                }
                ui.label(format!(
                    "{} / {}",
                    self.diff_hunk_focus.min(hunk_count - 1) + 1,
                    hunk_count
                ));
                if ui
                    .add_enabled(
                        self.diff_hunk_focus + 1 < hunk_count,
                        egui::Button::new("Next change ↓"),
                    )
                    .clicked()
                {
                    self.diff_hunk_focus += 1;
                    self.diff_scroll_pending = true;
                }
            }
        });
        ui.add_space(12.0);
        if self.file_history_path.as_deref() == Some(&path) {
            ui.label(
                RichText::new(format!(
                    "FILE HISTORY · {} commits",
                    self.file_history.len()
                ))
                .size(11.0)
                .strong()
                .color(violet()),
            );
            let commits = self.file_history.clone();
            egui::ScrollArea::vertical()
                .max_height(160.0)
                .show(ui, |ui| {
                    for commit in commits {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(&commit.short).monospace().color(accent()));
                            if ui.link(&commit.subject).clicked() {
                                self.queue(Job::Inspect(vec![
                                    "show".into(),
                                    "--format=fuller".into(),
                                    "--patch".into(),
                                    commit.id.clone(),
                                    "--".into(),
                                    path.clone(),
                                ]));
                            }
                            ui.label(RichText::new(&commit.date).size(10.0).color(muted()));
                        });
                    }
                });
            ui.add_space(10.0);
        }
        if self.detail.is_empty() {
            ui.label(
                RichText::new("Select Diff view or File view to inspect this file.").color(muted()),
            );
        } else {
            self.detail_view(ui);
        }
    }

    fn inspector(&mut self, ui: &mut egui::Ui) {
        if self.page == Page::Changes {
            self.changes(ui);
            return;
        }
        ui.label(
            RichText::new("COMMIT DETAILS")
                .size(10.0)
                .strong()
                .color(muted()),
        );
        ui.separator();
        if let Some(id) = &self.selected_commit {
            let commit = self
                .snapshot
                .as_ref()
                .and_then(|s| s.commits.iter().find(|c| &c.id == id))
                .or_else(|| self.search_results.iter().find(|c| &c.id == id))
                .cloned();
            if let Some(commit) = commit {
                let change_count = self
                    .snapshot
                    .as_ref()
                    .map_or(0, |snapshot| snapshot.status.len());
                if change_count > 0 {
                    egui::Frame::new()
                        .fill(elevated())
                        .inner_margin(egui::Margin::symmetric(9, 6))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new(format!(
                                        "{change_count} file changes in working directory"
                                    ))
                                    .size(11.0)
                                    .color(text()),
                                );
                                if ui.small_button("View changes").clicked() {
                                    self.page = Page::Changes;
                                }
                            });
                        });
                    ui.add_space(7.0);
                }
                ui.label(
                    RichText::new(format!("commit: {}", commit.short))
                        .size(10.5)
                        .monospace()
                        .color(accent()),
                );
                ui.add_space(5.0);
                egui::Frame::new()
                    .fill(bg())
                    .stroke(Stroke::new(1.0, border()))
                    .inner_margin(egui::Margin::same(10))
                    .show(ui, |ui| {
                        ui.set_min_height(85.0);
                        ui.label(RichText::new(&commit.subject).size(16.0).color(text()));
                        if self.compare_base.as_deref() == Some(&commit.id) {
                            ui.add_space(5.0);
                            badge(ui, "COMPARE BASE", orange());
                        }
                    });
                ui.add_space(9.0);
                ui.horizontal(|ui| {
                    let initials = commit
                        .author
                        .split_whitespace()
                        .filter_map(|part| part.chars().next())
                        .take(2)
                        .collect::<String>()
                        .to_uppercase();
                    egui::Frame::new()
                        .fill(violet())
                        .inner_margin(egui::Margin::same(7))
                        .show(ui, |ui| {
                            ui.label(RichText::new(initials).size(12.0).strong().color(bg()));
                        });
                    ui.vertical(|ui| {
                        ui.label(RichText::new(&commit.author).size(12.0).color(text()));
                        ui.label(
                            RichText::new(format!("authored {}", commit.date))
                                .size(10.5)
                                .color(muted()),
                        );
                    });
                    if let Some(parent) = commit.parents.first() {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(
                                RichText::new(format!(
                                    "parent: {}",
                                    &parent[..parent.len().min(7)]
                                ))
                                .size(10.0)
                                .color(muted()),
                            );
                        });
                    }
                });
                ui.add_space(8.0);
                ui.add_enabled_ui(!self.busy, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        if action(ui, "Show patch", false) {
                            self.selected_commit_file = None;
                            self.page = Page::CommitDiff;
                            self.detail.clear();
                            self.queue(Job::Inspect(vec![
                                "show".into(),
                                "--format=".into(),
                                "--patch".into(),
                                "--no-color".into(),
                                commit.id.clone(),
                            ]));
                        }
                        if let Some(parent) = commit.parents.first()
                            && action(ui, "Compare parent", false)
                        {
                            self.page = Page::CommitDiff;
                            self.queue(Job::Inspect(vec![
                                "diff".into(),
                                "--no-color".into(),
                                parent.clone(),
                                commit.id.clone(),
                            ]));
                        }
                        if commit.parents.len() < 2 {
                            if action(ui, "Cherry-pick...", false) {
                                self.confirm_commit_action =
                                    Some((CommitAction::CherryPick, commit.id.clone()));
                            }
                            if action(ui, "Revert...", false) {
                                self.confirm_commit_action =
                                    Some((CommitAction::Revert, commit.id.clone()));
                            }
                        }
                        if self.compare_base.as_deref() == Some(&commit.id) {
                            if action(ui, "Clear base", false) {
                                self.compare_base = None;
                            }
                        } else {
                            if action(ui, "Set compare base", false) {
                                self.compare_base = Some(commit.id.clone());
                            }
                            if let Some(base) = self.compare_base.clone()
                                && action(ui, "Compare with base", false)
                            {
                                self.page = Page::CommitDiff;
                                self.queue(Job::Inspect(vec![
                                    "diff".into(),
                                    "--no-color".into(),
                                    base,
                                    commit.id.clone(),
                                ]));
                            }
                        }
                        if action(ui, "Reset HEAD...", false) {
                            self.confirm_reset = Some(commit.id.clone());
                            self.reset_mode = ResetMode::Mixed;
                            self.reset_confirmation.clear();
                        }
                        if action(ui, "New branch...", false) {
                            self.branch_input.clear();
                            self.create_ref_at = Some((RefAtKind::Branch, commit.id.clone()));
                        }
                    });
                });
                if self.commit_files_id.as_deref() == Some(&commit.id) {
                    ui.add_space(14.0);
                    ui.label(
                        RichText::new(format!("CHANGED FILES  |  {}", self.commit_files.len()))
                            .size(10.0)
                            .strong()
                            .color(muted()),
                    );
                    ui.add_space(5.0);
                    let files = self.commit_files.clone();
                    for file in files {
                        let selected = self.selected_commit_file.as_deref() == Some(&file.path);
                        egui::Frame::new()
                            .fill(if selected {
                                elevated()
                            } else {
                                Color32::TRANSPARENT
                            })
                            .inner_margin(egui::Margin::symmetric(6, 2))
                            .show(ui, |ui| {
                                ui.set_min_width(ui.available_width());
                                ui.horizontal(|ui| {
                                    let status_color = match file.status.as_str() {
                                        "A" => accent(),
                                        "D" => red(),
                                        _ => orange(),
                                    };
                                    ui.label(
                                        RichText::new(&file.status)
                                            .monospace()
                                            .strong()
                                            .color(status_color),
                                    );
                                    if ui
                                        .add(
                                            egui::Button::new(
                                                RichText::new(&file.path).size(11.0).color(text()),
                                            )
                                            .fill(Color32::TRANSPARENT)
                                            .stroke(Stroke::NONE),
                                        )
                                        .clicked()
                                    {
                                        self.selected_commit_file = Some(file.path.clone());
                                        self.page = Page::CommitDiff;
                                        self.detail.clear();
                                        self.queue(Job::Inspect(vec![
                                            "show".into(),
                                            "--format=".into(),
                                            "--patch".into(),
                                            "--no-color".into(),
                                            commit.id.clone(),
                                            "--".into(),
                                            file.path.clone(),
                                        ]));
                                    }
                                });
                            });
                    }
                }
            }
        } else if let Some(path) = &self.selected_file {
            let path = path.clone();
            let file_state = self
                .snapshot
                .as_ref()
                .and_then(|snapshot| snapshot.status.iter().find(|file| file.path == path));
            let conflicted = file_state.is_some_and(git::FileStatus::conflicted);
            let tracked = file_state.is_some_and(|file| file.index != '?' && !file.conflicted());
            egui::Frame::new()
                .fill(elevated())
                .corner_radius(egui::CornerRadius::same(10))
                .stroke(Stroke::new(1.0, border()))
                .inner_margin(egui::Margin::same(14))
                .show(ui, |ui| {
                    badge(
                        ui,
                        if conflicted {
                            "CONFLICT"
                        } else if self.selected_file_staged {
                            "STAGED"
                        } else {
                            "WORKING TREE"
                        },
                        if conflicted {
                            red()
                        } else if self.selected_file_staged {
                            accent()
                        } else {
                            orange()
                        },
                    );
                    ui.add_space(7.0);
                    ui.label(RichText::new(&path).size(15.0).strong().color(text()));
                });
            ui.add_space(8.0);
            ui.add_enabled_ui(tracked && !self.busy, |ui| {
                ui.horizontal_wrapped(|ui| {
                    if action(ui, "File history", false) {
                        self.queue(Job::FileHistory(path.clone()));
                    }
                    if action(ui, "Blame", false) {
                        self.queue(Job::Inspect(vec![
                            "blame".into(),
                            "--".into(),
                            path.clone(),
                        ]));
                    }
                });
            });
            if self.file_history_path.as_deref() == Some(&path) {
                ui.add_space(12.0);
                ui.label(
                    RichText::new(format!(
                        "FILE HISTORY  |  {} commits",
                        self.file_history.len()
                    ))
                    .size(10.5)
                    .strong()
                    .color(violet()),
                );
                let commits = self.file_history.clone();
                egui::ScrollArea::vertical()
                    .max_height(220.0)
                    .show(ui, |ui| {
                        for commit in &commits {
                            egui::Frame::new()
                                .fill(panel_alt())
                                .inner_margin(egui::Margin::symmetric(8, 6))
                                .show(ui, |ui| {
                                    ui.label(
                                        RichText::new(&commit.subject).size(11.5).color(text()),
                                    );
                                    ui.label(
                                        RichText::new(format!(
                                            "{}  ·  {}  ·  {}",
                                            commit.short, commit.author, commit.date
                                        ))
                                        .size(10.0)
                                        .color(muted()),
                                    );
                                    ui.horizontal(|ui| {
                                        if ui.small_button("Diff").clicked() {
                                            self.queue(Job::Inspect(vec![
                                                "show".into(),
                                                "--format=fuller".into(),
                                                "--patch".into(),
                                                commit.id.clone(),
                                                "--".into(),
                                                path.clone(),
                                            ]));
                                        }
                                        if ui.small_button("View file").clicked() {
                                            self.queue(Job::Inspect(vec![
                                                "show".into(),
                                                format!("{}:{path}", commit.id),
                                            ]));
                                        }
                                    });
                                });
                            ui.add_space(3.0);
                        }
                    });
            }
        } else if let Some(snapshot) = &self.snapshot {
            egui::Frame::new()
                .fill(elevated())
                .corner_radius(egui::CornerRadius::same(10))
                .stroke(Stroke::new(1.0, border()))
                .inner_margin(egui::Margin::same(16))
                .show(ui, |ui| {
                    ui.label(
                        RichText::new("REPOSITORY PULSE")
                            .size(10.0)
                            .strong()
                            .color(muted()),
                    );
                    ui.add_space(7.0);
                    ui.label(
                        RichText::new(&snapshot.branch)
                            .size(18.0)
                            .strong()
                            .color(accent()),
                    );
                    ui.label(
                        RichText::new(format!(
                            "{} commits  |  {} changed files",
                            snapshot.commits.len(),
                            snapshot.status.len()
                        ))
                        .size(11.0)
                        .color(muted()),
                    );
                    ui.label(
                        RichText::new(format!("{} remotes", snapshot.remotes.len()))
                            .size(11.0)
                            .color(muted()),
                    );
                });
            ui.add_space(15.0);
            ui.label(
                RichText::new("Pick a commit or file to see its details here.")
                    .size(12.0)
                    .color(muted()),
            );
        } else {
            ui.label(RichText::new("Open a repository to inspect its history.").color(muted()));
        }
        if self.page != Page::Changes && self.selected_commit.is_none() {
            self.detail_view(ui);
        }
    }

    fn detail_view(&mut self, ui: &mut egui::Ui) {
        if !self.detail.is_empty() {
            ui.add_space(17.0);
            ui.label(
                RichText::new(if self.selected_commit.is_some() {
                    "MESSAGE / PATCH"
                } else {
                    "FILE CONTENT / DIFF"
                })
                .size(10.0)
                .strong()
                .color(muted()),
            );
            ui.add_space(5.0);
            let hunks = if self.selected_file.is_some() {
                git::diff_hunks(&self.detail)
            } else {
                Vec::new()
            };
            let scroll = if self.diff_wrap && !self.diff_split {
                egui::ScrollArea::vertical()
            } else {
                egui::ScrollArea::both()
            };
            scroll.show(ui, |ui| {
                ui.style_mut().spacing.item_spacing.y = 2.0;
                if self.diff_split {
                    ui.set_min_width(840.0);
                }
                if hunks.is_empty() {
                    for line in self.detail.lines().take(2500) {
                        diff_line(ui, line);
                    }
                } else {
                    for (hunk_index, hunk) in hunks.into_iter().enumerate() {
                        let heading = egui::Frame::new()
                            .fill(panel_alt())
                            .corner_radius(egui::CornerRadius::same(7))
                            .inner_margin(egui::Margin::symmetric(8, 5))
                            .show(ui, |ui| {
                                ui.set_min_width(ui.available_width());
                                ui.horizontal(|ui| {
                                    ui.label(
                                        RichText::new(&hunk.heading)
                                            .monospace()
                                            .size(10.5)
                                            .color(violet()),
                                    );
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            if !self.selected_file_staged
                                                && ui
                                                    .add_enabled(
                                                        !self.busy,
                                                        egui::Button::new("Discard hunk..."),
                                                    )
                                                    .clicked()
                                            {
                                                self.confirm_discard_hunk =
                                                    Some(hunk.patch.clone());
                                            }
                                            if ui
                                                .add_enabled(
                                                    !self.busy,
                                                    egui::Button::new(
                                                        if self.selected_file_staged {
                                                            "Unstage hunk"
                                                        } else {
                                                            "Stage hunk"
                                                        },
                                                    ),
                                                )
                                                .clicked()
                                            {
                                                self.hunk_action = true;
                                                self.queue(Job::ApplyHunk(
                                                    hunk.patch.clone(),
                                                    self.selected_file_staged,
                                                ));
                                            }
                                        },
                                    );
                                });
                            });
                        if self.diff_scroll_pending && hunk_index == self.diff_hunk_focus {
                            ui.scroll_to_rect(heading.response.rect, Some(egui::Align::Center));
                            self.diff_scroll_pending = false;
                        }
                        if self.diff_split {
                            for row in hunk.split_rows().into_iter().take(400) {
                                split_diff_row(ui, &row);
                            }
                        } else {
                            let numbers = hunk.line_numbers();
                            for (index, line) in hunk.lines.iter().enumerate().take(400) {
                                let counterpart = if line.starts_with('+') {
                                    index
                                        .checked_sub(1)
                                        .and_then(|i| hunk.lines.get(i))
                                        .filter(|other| other.starts_with('-'))
                                        .map(String::as_str)
                                } else if line.starts_with('-') {
                                    hunk.lines
                                        .get(index + 1)
                                        .filter(|other| other.starts_with('+'))
                                        .map(String::as_str)
                                } else {
                                    None
                                };
                                let row_y = ui.cursor().top();
                                let row_hovered =
                                    ui.ctx().pointer_hover_pos().is_some_and(|pointer| {
                                        pointer.y >= row_y && pointer.y < row_y + 24.0
                                    });
                                ui.horizontal(|ui| {
                                    if let Some(patch) = hunk.line_patch(index) {
                                        if row_hovered
                                            && ui
                                                .add_enabled(
                                                    !self.busy,
                                                    egui::Button::new(
                                                        if self.selected_file_staged {
                                                            "Unstage"
                                                        } else {
                                                            "Stage"
                                                        },
                                                    )
                                                    .small(),
                                                )
                                                .clicked()
                                        {
                                            self.hunk_action = true;
                                            self.queue(Job::ApplyLine(
                                                patch,
                                                self.selected_file_staged,
                                            ));
                                        } else if !row_hovered {
                                            ui.add_space(51.0);
                                        }
                                    } else {
                                        ui.add_space(51.0);
                                    }
                                    let (old, new) = numbers[index];
                                    diff_line_numbered(
                                        ui,
                                        line,
                                        old,
                                        new,
                                        counterpart,
                                        self.diff_wrap,
                                    );
                                });
                            }
                        }
                        ui.add_space(9.0);
                    }
                }
            });
        }
    }

    fn repository_row(&mut self, ui: &mut egui::Ui, path: &Path) {
        let path = path.to_path_buf();
        let label = path
            .file_name()
            .unwrap_or(path.as_os_str())
            .to_string_lossy();
        let pinned = self.pinned_repos.contains(&path);
        let open = self.open_repo_tabs.contains(&path);
        let exists = path.exists();
        egui::Frame::new()
            .fill(panel())
            .inner_margin(egui::Margin::symmetric(13, 8))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(
                            RichText::new(label.as_ref())
                                .size(13.0)
                                .strong()
                                .color(if exists { text() } else { red() }),
                        );
                        ui.label(
                            RichText::new(path.to_string_lossy())
                                .size(10.0)
                                .color(muted()),
                        );
                    });
                    if self.repo.as_ref() == Some(&path)
                        && let Some(snapshot) = &self.snapshot
                    {
                        ui.add_space(18.0);
                        badge(ui, &snapshot.branch, violet());
                        if !snapshot.status.is_empty() {
                            badge(ui, &format!("{} changes", snapshot.status.len()), orange());
                        }
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .add_sized([62.0, 27.0], egui::Button::new("Forget"))
                            .clicked()
                        {
                            self.recent_repos.retain(|p| p != &path);
                            self.pinned_repos.retain(|p| p != &path);
                        }
                        if ui
                            .add_sized(
                                [56.0, 27.0],
                                egui::Button::new(if pinned { "Unpin" } else { "Pin" }),
                            )
                            .clicked()
                        {
                            if pinned {
                                self.pinned_repos.retain(|p| p != &path);
                            } else {
                                self.pinned_repos.push(path.clone());
                            }
                        }
                        if open
                            && ui
                                .add_enabled(
                                    !self.busy,
                                    egui::Button::new("Close tab").min_size(egui::vec2(77.0, 27.0)),
                                )
                                .clicked()
                        {
                            self.close_repository_tab(&path);
                        }
                        if ui
                            .add_enabled(
                                exists && !self.busy,
                                egui::Button::new("Open").min_size(egui::vec2(58.0, 27.0)),
                            )
                            .clicked()
                        {
                            if self.repo.as_ref() != Some(&path) {
                                self.queue(Job::Open(path.clone()));
                            }
                            self.page = Page::History;
                        }
                    });
                });
            });
        ui.add_space(3.0);
    }

    fn repositories(&mut self, ui: &mut egui::Ui) {
        ui.label(
            RichText::new("Repository Management")
                .size(22.0)
                .strong()
                .color(text()),
        );
        ui.add_space(12.0);
        ui.horizontal_wrapped(|ui| {
            if action(ui, "Browse...", true)
                && let Some(path) = pick_folder(self.repo.as_deref())
            {
                self.queue(Job::Open(path));
                self.page = Page::History;
            }
            if action(ui, "Clone", false) {
                self.show_clone = true;
            }
            if action(ui, "Initialize", false) {
                self.queue(Job::Init(PathBuf::from(self.path_input.trim())));
                self.page = Page::History;
            }
            ui.add(
                egui::TextEdit::singleline(&mut self.path_input)
                    .hint_text("Repository path for Open or Initialize")
                    .desired_width(330.0),
            );
            if action(ui, "Open path", false) {
                self.queue(Job::Open(PathBuf::from(self.path_input.trim())));
                self.page = Page::History;
            }
        });
        ui.add_space(12.0);
        ui.add(
            egui::TextEdit::singleline(&mut self.repo_filter)
                .hint_text("Search repositories")
                .desired_width(ui.available_width()),
        );
        ui.add_space(16.0);
        let groups = [
            ("OPEN REPOSITORIES", self.open_repo_tabs.clone()),
            ("FAVORITES", self.pinned_repos.clone()),
            ("RECENT REPOSITORIES", self.recent_repos.clone()),
        ];
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for (title, paths) in groups {
                    let filtered = paths
                        .into_iter()
                        .filter(|path| {
                            path.to_string_lossy()
                                .to_lowercase()
                                .contains(&self.repo_filter.to_lowercase())
                        })
                        .collect::<Vec<_>>();
                    egui::Frame::new()
                        .fill(panel_alt())
                        .inner_margin(egui::Margin::symmetric(12, 7))
                        .show(ui, |ui| {
                            ui.set_min_width(ui.available_width());
                            ui.label(
                                RichText::new(format!("{title}  {}", filtered.len()))
                                    .size(10.5)
                                    .strong()
                                    .color(muted()),
                            );
                        });
                    ui.add_space(4.0);
                    if filtered.is_empty() {
                        ui.label(
                            RichText::new("No repositories to show")
                                .size(11.0)
                                .italics()
                                .color(muted()),
                        );
                    }
                    for path in filtered {
                        self.repository_row(ui, &path);
                    }
                    ui.add_space(15.0);
                }
                if ui.small_button("Forget missing repositories").clicked() {
                    self.recent_repos.retain(|path| path.exists());
                    self.pinned_repos.retain(|path| path.exists());
                }
            });
    }

    fn new_tab(&mut self, ui: &mut egui::Ui) {
        ui.add_space(26.0);
        ui.horizontal(|ui| {
            ui.add_space(48.0);
            let left_width = (ui.available_width() * 0.57).max(340.0);
            ui.vertical(|ui| {
                ui.set_width(left_width);
                ui.label(RichText::new("Repositories").size(22.0).color(text()));
                ui.add_space(16.0);
                ui.horizontal(|ui| {
                    if ui.button("Open").clicked()
                        && let Some(path) = pick_folder(self.repo.as_deref())
                    {
                        self.queue(Job::Open(path));
                        self.page = Page::History;
                    }
                    if ui.button("Clone").clicked() {
                        self.show_clone = true;
                    }
                    if ui.button("Create").clicked() {
                        self.page = Page::Repositories;
                    }
                });
                ui.add_space(22.0);
                ui.label(RichText::new("Recent").size(12.0).color(muted()));
                ui.add_space(5.0);
                for path in self.recent_repos.clone().into_iter().take(12) {
                    let name = path
                        .file_name()
                        .unwrap_or(path.as_os_str())
                        .to_string_lossy();
                    ui.horizontal(|ui| {
                        if ui.link(name).clicked() {
                            self.queue(Job::Open(path.clone()));
                            self.page = Page::History;
                        }
                        ui.label(
                            RichText::new(path.display().to_string())
                                .size(11.0)
                                .color(muted()),
                        );
                    });
                }
            });
            ui.separator();
            ui.add_space(18.0);
            ui.vertical(|ui| {
                ui.label(RichText::new("Get started").size(20.0).color(text()));
                ui.add_space(14.0);
                ui.label(
                    RichText::new(
                        "Open an existing repository, clone from a remote, or create one locally.",
                    )
                    .size(12.0)
                    .color(muted()),
                );
                ui.add_space(22.0);
                ui.label(RichText::new("Resources").size(12.0).color(text()));
                if ui.link("Release notes").clicked() {
                    self.changelog_open = true;
                    self.page = Page::Changelog;
                }
                ui.hyperlink_to(
                    "Documentation",
                    "https://github.com/boubou666/GitVibe#readme",
                );
            });
        });
    }

    fn changelog(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new("Changelog").size(23.0).strong().color(text()));
        ui.label(
            RichText::new("What changed in GitVibe")
                .size(12.0)
                .color(muted()),
        );
        ui.add_space(18.0);
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let mut in_releases = false;
                for line in include_str!("../CHANGELOG.md")
                    .lines()
                    .take_while(|line| !line.starts_with("[Unreleased]:"))
                {
                    if let Some(title) = line.strip_prefix("## ") {
                        in_releases = true;
                        ui.add_space(14.0);
                        egui::Frame::new()
                            .fill(panel_alt())
                            .inner_margin(egui::Margin::symmetric(12, 8))
                            .show(ui, |ui| {
                                ui.set_min_width(ui.available_width());
                                ui.label(RichText::new(title).size(16.0).strong().color(accent()));
                            });
                    } else if !in_releases {
                        continue;
                    } else if let Some(title) = line.strip_prefix("### ") {
                        ui.add_space(8.0);
                        ui.label(RichText::new(title).size(12.0).strong().color(violet()));
                    } else if let Some(item) = line.strip_prefix("- ") {
                        ui.label(RichText::new(format!("•  {item}")).size(11.5).color(text()));
                    } else if !line.is_empty() && !line.starts_with('#') {
                        ui.label(RichText::new(line).size(11.0).color(muted()));
                    }
                }
            });
    }

    fn conflict_dialog(&mut self, ctx: &egui::Context) {
        let mut open = true;
        let mut save = None;
        if let Some(document) = self.conflict_editor.as_mut() {
            egui::Window::new(format!("Resolve lines · {}", document.path))
                .open(&mut open).resizable(true).default_size([900.0, 630.0])
                .show(ctx, |ui| {
                    ui.label(RichText::new("Choose individual lines from each side, or edit the result for each block.")
                        .size(12.0).color(muted()));
                    ui.add_space(8.0);
                    egui::ScrollArea::vertical().max_height(530.0).show(ui, |ui| {
                        let mut block_number = 0;
                        for part in &mut document.parts {
                            match part {
                                git::ConflictPart::Plain(context) => {
                                    let lines = context.lines().count();
                                    if lines > 0 { ui.label(RichText::new(format!("{lines} unchanged lines"))
                                        .size(10.0).color(muted())); }
                                }
                                git::ConflictPart::Block(block) => {
                                    block_number += 1;
                                    egui::Frame::new().fill(panel_alt()).stroke(Stroke::new(1.0, border()))
                                        .inner_margin(egui::Margin::same(10)).show(ui, |ui| {
                                            ui.label(RichText::new(format!("CONFLICT {block_number}"))
                                                .strong().color(violet()));
                                            ui.horizontal(|ui| {
                                                if ui.small_button("All ours").clicked() {
                                                    block.selected_ours.fill(true); block.selected_theirs.fill(false);
                                                    block.manual_result = None;
                                                }
                                                if ui.small_button("All theirs").clicked() {
                                                    block.selected_ours.fill(false); block.selected_theirs.fill(true);
                                                    block.manual_result = None;
                                                }
                                                if ui.small_button("Both").clicked() {
                                                    block.selected_ours.fill(true); block.selected_theirs.fill(true);
                                                    block.manual_result = None;
                                                }
                                                if ui.small_button("Neither").clicked() {
                                                    block.selected_ours.fill(false); block.selected_theirs.fill(false);
                                                    block.manual_result = None;
                                                }
                                            });
                                            ui.columns(2, |columns| {
                                                columns[0].label(RichText::new("OURS").strong().color(accent()));
                                                for (line, checked) in block.ours.iter().zip(&mut block.selected_ours) {
                                                    if columns[0].checkbox(checked, RichText::new(line.trim_end()).monospace().size(11.0)).changed() {
                                                        block.manual_result = None;
                                                    }
                                                }
                                                columns[1].label(RichText::new("THEIRS").strong().color(orange()));
                                                for (line, checked) in block.theirs.iter().zip(&mut block.selected_theirs) {
                                                    if columns[1].checkbox(checked, RichText::new(line.trim_end()).monospace().size(11.0)).changed() {
                                                        block.manual_result = None;
                                                    }
                                                }
                                            });
                                            ui.label(RichText::new("RESULT · editable").size(10.0).strong().color(muted()));
                                            let mut result = block.manual_result.clone().unwrap_or_else(|| {
                                                let mut text = String::new();
                                                for (line, checked) in block.ours.iter().zip(&block.selected_ours) {
                                                    if *checked { text.push_str(line); }
                                                }
                                                for (line, checked) in block.theirs.iter().zip(&block.selected_theirs) {
                                                    if *checked { text.push_str(line); }
                                                }
                                                text
                                            });
                                            if ui.add(egui::TextEdit::multiline(&mut result)
                                                .desired_width(ui.available_width()).desired_rows(3).code_editor()).changed() {
                                                block.manual_result = Some(result);
                                            }
                                        });
                                    ui.add_space(8.0);
                                }
                            }
                        }
                    });
                    ui.add_space(8.0);
                    if ui.add_enabled(!self.busy, egui::Button::new("Save result and stage file")
                        .fill(accent())).clicked() { save = Some(document.clone()); }
                });
        }
        if !open {
            self.conflict_editor = None;
        }
        if let Some(document) = save {
            self.conflict_action = true;
            self.queue(Job::SaveConflict(document));
        }
    }

    fn dialogs(&mut self, ctx: &egui::Context) {
        self.conflict_dialog(ctx);
        if let Some((name, remote)) = self.confirm_switch_branch.clone() {
            egui::Window::new("Switch branch with local changes")
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.label(format!("Switch to {name}?"));
                    ui.label("Git will refuse the switch if your changes would be overwritten.");
                    ui.horizontal(|ui| {
                        if ui.button("Cancel").clicked() {
                            self.confirm_switch_branch = None;
                        }
                        if ui.button("Keep changes and switch").clicked() {
                            self.queue(Job::SwitchBranch(name.clone(), false, remote));
                            self.confirm_switch_branch = None;
                        }
                        if ui.button("Stash changes and switch").clicked() {
                            self.queue(Job::SwitchBranch(name.clone(), true, remote));
                            self.confirm_switch_branch = None;
                        }
                    });
                });
        }
        if let Some(id) = self.confirm_checkout_commit.clone() {
            egui::Window::new("Checkout commit?")
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.label(format!(
                        "Switch to commit {} with a detached HEAD?",
                        &id[..id.len().min(12)]
                    ));
                    ui.label("Git will protect working changes that cannot be carried across.");
                    ui.horizontal(|ui| {
                        if ui.button("Cancel").clicked() {
                            self.confirm_checkout_commit = None;
                        }
                        if ui
                            .add_enabled(!self.busy, egui::Button::new("Checkout commit"))
                            .clicked()
                        {
                            self.git_owned(vec!["switch".into(), "--detach".into(), id]);
                            self.confirm_checkout_commit = None;
                            self.page = Page::History;
                        }
                    });
                });
        }
        if let Some((kind, id)) = self.create_ref_at.clone() {
            let (title, label) = match kind {
                RefAtKind::Branch => ("Create branch here", "Branch name"),
                RefAtKind::Tag => ("Create tag here", "Tag name"),
            };
            egui::Window::new(title)
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.label(format!("At commit {}", &id[..id.len().min(12)]));
                    ui.label(label);
                    let name = match kind {
                        RefAtKind::Branch => &mut self.branch_input,
                        RefAtKind::Tag => &mut self.tag_input,
                    };
                    ui.add(egui::TextEdit::singleline(name).desired_width(300.0));
                    let value = name.trim().to_owned();
                    ui.horizontal(|ui| {
                        if ui.button("Cancel").clicked() {
                            self.create_ref_at = None;
                        }
                        if ui
                            .add_enabled(
                                !self.busy && !value.is_empty(),
                                egui::Button::new("Create"),
                            )
                            .clicked()
                        {
                            let command = match kind {
                                RefAtKind::Branch => "branch",
                                RefAtKind::Tag => "tag",
                            };
                            self.git_owned(vec![command.into(), value, id]);
                            self.create_ref_at = None;
                        }
                    });
                });
        }
        if self.show_clone {
            let mut open = true;
            egui::Window::new("Clone a Repository")
                .open(&mut open)
                .collapsible(false)
                .resizable(true)
                .default_width(880.0)
                .default_height(500.0)
                .show(ctx, |ui| {
                    const SOURCES: [(&str, &str); 8] = [
                        ("Clone with URL", "https://example.com/owner/repository.git"),
                        ("GitHub.com", "https://github.com/owner/repository.git"),
                        ("GitHub Enterprise", "https://github.example.com/owner/repository.git"),
                        ("GitLab.com", "https://gitlab.com/owner/repository.git"),
                        ("GitLab Self-Managed", "https://gitlab.example.com/owner/repository.git"),
                        ("Bitbucket.org", "https://bitbucket.org/owner/repository.git"),
                        ("Bitbucket Server", "https://bitbucket.example.com/scm/project/repository.git"),
                        ("Azure DevOps", "https://dev.azure.com/organization/project/_git/repository"),
                    ];
                    ui.set_min_size(egui::vec2(850.0, 470.0));
                    ui.horizontal_top(|ui| {
                        egui::Frame::new().fill(panel_alt()).inner_margin(10.0).show(ui, |ui| {
                            ui.set_width(205.0);
                            for (index, (label, _)) in SOURCES.iter().enumerate() {
                                if ui.selectable_label(self.clone_source == index, *label).clicked() {
                                    self.clone_source = index;
                                }
                            }
                        });
                        ui.add_space(10.0);
                        ui.vertical(|ui| {
                            ui.set_min_width(600.0);
                            ui.heading("Clone a Repo");
                            ui.add_space(16.0);
                            ui.horizontal(|ui| {
                                ui.add_sized([135.0, 26.0], egui::Label::new("Where to clone to"));
                                ui.add_sized(
                                    [405.0, 26.0],
                                    egui::TextEdit::singleline(&mut self.clone_destination)
                                        .hint_text("Destination folder"),
                                );
                                if ui.button("Browse").clicked()
                                    && let Some(parent) = pick_folder(
                                        Path::new(&self.clone_destination)
                                            .parent()
                                            .filter(|path| path.is_dir()),
                                    )
                                {
                                    let name = git::default_clone_directory(&self.clone_url)
                                        .unwrap_or_else(|| "repository".to_owned());
                                    self.clone_destination = parent.join(name).to_string_lossy().into_owned();
                                }
                            });
                            ui.add_space(8.0);
                            ui.horizontal(|ui| {
                                ui.add_sized([135.0, 26.0], egui::Label::new("URL or local path"));
                                ui.add_sized(
                                    [490.0, 26.0],
                                    egui::TextEdit::singleline(&mut self.clone_url)
                                        .hint_text(SOURCES[self.clone_source].1),
                                );
                            });
                            ui.add_space(10.0);
                            ui.horizontal(|ui| {
                                ui.add_space(135.0);
                                ui.checkbox(&mut self.clone_shallow, "Shallow clone")
                                    .on_hover_text("Download only the latest commit (--depth=1)");
                            });
                            ui.horizontal(|ui| {
                                ui.add_space(135.0);
                                ui.checkbox(&mut self.clone_sparse, "Sparse checkout")
                                    .on_hover_text("Start with only root files checked out (--sparse)");
                            });
                            ui.add_space(8.0);
                            ui.label(RichText::new("Git uses your existing credentials and credential helper for private repositories.").color(muted()).small());
                            ui.add_space(18.0);
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui
                                    .add_enabled(
                                        !self.clone_url.trim().is_empty()
                                            && !self.clone_destination.trim().is_empty(),
                                        egui::Button::new("Clone the repo!").fill(elevated()),
                                    )
                                    .clicked()
                                {
                                    self.queue(Job::Clone(
                                        self.clone_url.trim().to_owned(),
                                        PathBuf::from(self.clone_destination.trim()),
                                        self.clone_shallow,
                                        self.clone_sparse,
                                    ));
                                    self.show_clone = false;
                                }
                            });
                        });
                    });
                });
            self.show_clone &= open;
        }
        if let Some(patch) = self.confirm_discard_hunk.clone() {
            egui::Window::new("Discard this hunk?")
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.label("Discard the selected unstaged lines? This cannot be undone.");
                    ui.horizontal(|ui| {
                        if ui.button("Cancel").clicked() {
                            self.confirm_discard_hunk = None;
                        }
                        if ui
                            .add(egui::Button::new("Discard hunk").fill(red()))
                            .clicked()
                        {
                            self.hunk_action = true;
                            self.queue(Job::DiscardHunk(patch));
                            self.confirm_discard_hunk = None;
                        }
                    });
                });
        }
        if let Some(path) = self.confirm_discard.clone() {
            egui::Window::new("Discard changes?")
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.label(format!(
                        "Discard unstaged edits to {path}? This cannot be undone."
                    ));
                    ui.horizontal(|ui| {
                        if ui.button("Cancel").clicked() {
                            self.confirm_discard = None;
                        }
                        if ui.add(egui::Button::new("Discard").fill(red())).clicked() {
                            self.git_owned(vec![
                                "restore".into(),
                                "--worktree".into(),
                                "--".into(),
                                path,
                            ]);
                            self.confirm_discard = None;
                        }
                    });
                });
        }
        if let Some((action_kind, id)) = self.confirm_commit_action.clone() {
            let verb = match action_kind {
                CommitAction::CherryPick => "Cherry-pick",
                CommitAction::Revert => "Revert",
            };
            egui::Window::new(format!("{verb} commit?"))
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.label(format!(
                        "{verb} commit {} on the current branch?",
                        &id[..id.len().min(12)]
                    ));
                    ui.label("If Git reports conflicts, resolve them before continuing.");
                    ui.horizontal(|ui| {
                        if ui.button("Cancel").clicked() {
                            self.confirm_commit_action = None;
                        }
                        if ui.add(egui::Button::new(verb).fill(accent())).clicked() {
                            let args = match action_kind {
                                CommitAction::CherryPick => vec!["cherry-pick".into(), id],
                                CommitAction::Revert => {
                                    vec!["revert".into(), "--no-edit".into(), id]
                                }
                            };
                            self.git_owned(args);
                            self.confirm_commit_action = None;
                        }
                    });
                });
        }
        if let Some(deletion) = self.confirm_delete_ref.clone() {
            let (kind, name) = match &deletion {
                RefDeletion::Branch(name) => ("branch", name),
                RefDeletion::Tag(name) => ("tag", name),
            };
            egui::Window::new(format!("Delete {kind}?"))
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.label(format!("Delete local {kind} '{name}'?"));
                    ui.label(match &deletion {
                        RefDeletion::Branch(_) => {
                            "Git will refuse to delete a branch with unmerged commits."
                        }
                        RefDeletion::Tag(_) => "This removes the local tag only.",
                    });
                    ui.horizontal(|ui| {
                        if ui.button("Cancel").clicked() {
                            self.confirm_delete_ref = None;
                        }
                        if ui.add(egui::Button::new("Delete").fill(red())).clicked() {
                            let args = match &deletion {
                                RefDeletion::Branch(_) => {
                                    vec!["branch".into(), "-d".into(), "--".into(), name.clone()]
                                }
                                RefDeletion::Tag(_) => {
                                    vec!["tag".into(), "-d".into(), "--".into(), name.clone()]
                                }
                            };
                            self.git_owned(args);
                            self.confirm_delete_ref = None;
                        }
                    });
                });
        }
        if let Some(id) = self.confirm_reset.clone() {
            egui::Window::new("Reset HEAD to commit?")
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.label(format!("Target commit: {}", &id[..id.len().min(12)]));
                    ui.radio_value(&mut self.reset_mode, ResetMode::Soft, "Soft");
                    ui.radio_value(&mut self.reset_mode, ResetMode::Mixed, "Mixed");
                    ui.radio_value(&mut self.reset_mode, ResetMode::Hard, "Hard");
                    ui.label(match self.reset_mode {
                        ResetMode::Soft => "Moves HEAD; keeps staged and working changes.",
                        ResetMode::Mixed => "Moves HEAD and unstages changes; keeps working files.",
                        ResetMode::Hard => {
                            "Moves HEAD and discards tracked staged and working changes."
                        }
                    });
                    if self.reset_mode == ResetMode::Hard {
                        ui.label("Type RESET to confirm the hard reset:");
                        ui.add(egui::TextEdit::singleline(&mut self.reset_confirmation));
                    }
                    ui.horizontal(|ui| {
                        if ui.button("Cancel").clicked() {
                            self.confirm_reset = None;
                            self.reset_confirmation.clear();
                        }
                        let enabled = !self.busy
                            && (self.reset_mode != ResetMode::Hard
                                || self.reset_confirmation == "RESET");
                        if ui
                            .add_enabled(enabled, egui::Button::new("Reset HEAD").fill(red()))
                            .clicked()
                        {
                            let mode = match self.reset_mode {
                                ResetMode::Soft => "--soft",
                                ResetMode::Mixed => "--mixed",
                                ResetMode::Hard => "--hard",
                            };
                            self.git_owned(vec!["reset".into(), mode.into(), id]);
                            self.selected_commit = None;
                            self.detail.clear();
                            self.confirm_reset = None;
                            self.reset_confirmation.clear();
                        }
                    });
                });
        }
        if let Some((path, side)) = self.confirm_conflict_side.clone() {
            let label = match side {
                git::ConflictSide::Ours => "ours",
                git::ConflictSide::Theirs => "theirs",
            };
            egui::Window::new(format!("Use {label} for this file?"))
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.label(format!("Replace {path} with Git's {label} version?"));
                    ui.label("This overwrites the conflict file and stages the chosen version.");
                    ui.label("During a rebase, Git's ours/theirs labels may feel reversed.");
                    ui.horizontal(|ui| {
                        if ui.button("Cancel").clicked() {
                            self.confirm_conflict_side = None;
                        }
                        if ui
                            .add(egui::Button::new(format!("Use {label}")).fill(red()))
                            .clicked()
                        {
                            self.conflict_action = true;
                            self.queue(Job::ResolveConflict(path, side));
                            self.confirm_conflict_side = None;
                        }
                    });
                });
        }
        if self.confirm_abort_merge {
            egui::Window::new("Abort this merge?")
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.label("Return to the state before this merge began?");
                    ui.label("Conflict edits made during the merge may be lost.");
                    ui.horizontal(|ui| {
                        if ui.button("Cancel").clicked() {
                            self.confirm_abort_merge = false;
                        }
                        if ui
                            .add(egui::Button::new("Abort merge").fill(red()))
                            .clicked()
                        {
                            self.git(&["merge", "--abort"]);
                            self.confirm_abort_merge = false;
                        }
                    });
                });
        }
        if let Some(path) = self.confirm_remove_worktree.clone() {
            egui::Window::new("Remove worktree?")
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.label(path.display().to_string());
                    ui.label("Git refuses removal when the worktree has uncommitted changes or is locked.");
                    ui.horizontal(|ui| {
                        if ui.button("Cancel").clicked() {
                            self.confirm_remove_worktree = None;
                        }
                        if ui.add(egui::Button::new("Remove worktree").fill(red())).clicked() {
                            self.git_owned(vec!["worktree".into(), "remove".into(), path.to_string_lossy().into_owned()]);
                            self.confirm_remove_worktree = None;
                        }
                    });
                });
        }
        if let Some((name, mut url)) = self.edit_remote.clone() {
            let mut cancel = false;
            let mut save = false;
            egui::Window::new(format!("Edit {name} URL"))
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.label("Fetch and push URL");
                    ui.add(egui::TextEdit::singleline(&mut url).desired_width(480.0));
                    ui.horizontal(|ui| {
                        cancel = ui.button("Cancel").clicked();
                        save = ui
                            .add_enabled(
                                !self.busy && !url.trim().is_empty() && !url.contains(['\r', '\n']),
                                egui::Button::new("Save URL"),
                            )
                            .clicked();
                    });
                });
            if save {
                self.git_owned(vec![
                    "remote".into(),
                    "set-url".into(),
                    name,
                    url.trim().to_owned(),
                ]);
                self.edit_remote = None;
            } else if cancel {
                self.edit_remote = None;
            } else {
                self.edit_remote = Some((name, url));
            }
        }
        if let Some(remote) = self.confirm_remove_remote.clone() {
            egui::Window::new("Remove remote?")
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.label(format!(
                        "Remove remote '{remote}' and its local tracking refs?"
                    ));
                    ui.label("Commits and the remote repository itself are not deleted.");
                    ui.horizontal(|ui| {
                        if ui.button("Cancel").clicked() {
                            self.confirm_remove_remote = None;
                        }
                        if ui
                            .add(egui::Button::new("Remove remote").fill(red()))
                            .clicked()
                        {
                            self.git_owned(vec!["remote".into(), "remove".into(), remote]);
                            self.confirm_remove_remote = None;
                        }
                    });
                });
        }
        if self.confirm_interactive_rebase {
            egui::Window::new("Start interactive rebase?")
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    if let Some(plan) = &self.interactive_plan {
                        ui.label(format!(
                            "Replay {} commits onto {}?",
                            plan.steps.len(),
                            plan.target
                        ));
                        ui.label(
                            "This rewrites commit IDs. The old branch tip remains in the reflog.",
                        );
                    }
                    ui.horizontal(|ui| {
                        if ui.button("Cancel").clicked() {
                            self.confirm_interactive_rebase = false;
                        }
                        if ui
                            .add(egui::Button::new("Start rebase").fill(orange()))
                            .clicked()
                        {
                            if let Some(plan) = self.interactive_plan.take() {
                                self.queue(Job::StartInteractiveRebase(plan));
                            }
                            self.confirm_interactive_rebase = false;
                        }
                    });
                });
        }
        if let Some(args) = self.confirm_rebase.clone() {
            let action_name = if args.iter().any(|arg| arg == "--abort") {
                "Abort rebase"
            } else if args.iter().any(|arg| arg == "--skip") {
                "Skip current commit"
            } else {
                "Start rebase"
            };
            egui::Window::new(format!("{action_name}?"))
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.label(match action_name {
                        "Abort rebase" => "Return to the branch state before this rebase began.",
                        "Skip current commit" => "Omit this commit from the rebased branch.",
                        _ => "Replay commits onto the selected target. Commit IDs will change.",
                    });
                    if action_name == "Start rebase" {
                        ui.label(format!("Target: {}", args.last().unwrap_or(&String::new())));
                    }
                    ui.horizontal(|ui| {
                        if ui.button("Cancel").clicked() {
                            self.confirm_rebase = None;
                        }
                        if ui
                            .add(egui::Button::new(action_name).fill(orange()))
                            .clicked()
                        {
                            self.git_owned(args);
                            self.confirm_rebase = None;
                        }
                    });
                });
        }
    }
}

impl eframe::App for GitVibe {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, "recent_repos", &self.recent_repos);
        eframe::set_value(storage, "pinned_repos", &self.pinned_repos);
        eframe::set_value(storage, "open_repo_tabs", &self.open_repo_tabs);
        eframe::set_value(storage, "last_active_repo", &self.last_active_repo);
        eframe::set_value(storage, "changelog_open", &self.changelog_open);
        eframe::set_value(storage, "theme_choice", &self.theme_choice);
        eframe::set_value(storage, "pull_mode", &self.pull_mode);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.poll();
        self.poll_updates();
        if !self.palette_open
            && ctx.input(|input| input.modifiers.command && input.key_pressed(egui::Key::F))
        {
            self.page = Page::History;
            ctx.memory_mut(|memory| memory.request_focus(egui::Id::new("history_search")));
        }
        if ctx.input(|input| input.modifiers.command && input.key_pressed(egui::Key::K)) {
            self.open_palette();
        }
        if !self.palette_open && !self.busy && !ctx.text_edit_focused() {
            let index_shortcut = ctx.input(|input| {
                (input.modifiers.command && input.key_pressed(egui::Key::Z))
                    .then_some(input.modifiers.shift)
            });
            if let Some(redo) = index_shortcut {
                self.undo_index(redo);
            }
        }
        if self.page == Page::Changes
            && !self.palette_open
            && !self.busy
            && ctx.input(|input| {
                input.modifiers.command && input.modifiers.shift && input.key_pressed(egui::Key::S)
            })
            && self.selected_file.is_some()
            && !self.selected_file_staged
        {
            self.run_palette_action(PaletteAction::StageSelected, &ctx);
        }
        if self.page == Page::Changes
            && !self.palette_open
            && !self.busy
            && ctx.input(|input| {
                input.modifiers.command && input.modifiers.shift && input.key_pressed(egui::Key::U)
            })
            && self.selected_file.is_some()
            && self.selected_file_staged
        {
            self.run_palette_action(PaletteAction::UnstageSelected, &ctx);
        }
        if ctx.input(|input| input.modifiers.command && input.key_pressed(egui::Key::Tab))
            && !self.palette_open
            && !self.busy
            && self.open_repo_tabs.len() > 1
        {
            let current = self
                .repo
                .as_ref()
                .and_then(|repo| self.open_repo_tabs.iter().position(|tab| tab == repo))
                .unwrap_or(0);
            let backwards = ctx.input(|input| input.modifiers.shift);
            let next = if backwards {
                (current + self.open_repo_tabs.len() - 1) % self.open_repo_tabs.len()
            } else {
                (current + 1) % self.open_repo_tabs.len()
            };
            self.page = Page::History;
            self.queue(Job::Open(self.open_repo_tabs[next].clone()));
        }
        if self.page == Page::History
            && !self.palette_open
            && !self.busy
            && !ctx.text_edit_focused()
        {
            let direction = ctx.input(|input| {
                if input.key_pressed(egui::Key::ArrowDown) {
                    1_i32
                } else if input.key_pressed(egui::Key::ArrowUp) {
                    -1_i32
                } else {
                    0
                }
            });
            if direction != 0 {
                let commits = if self.search_active {
                    Some(&self.search_results)
                } else {
                    self.snapshot.as_ref().map(|snapshot| &snapshot.commits)
                };
                if let Some(commits) = commits.filter(|commits| !commits.is_empty()) {
                    let current = self
                        .selected_commit
                        .as_ref()
                        .and_then(|id| commits.iter().position(|commit| &commit.id == id))
                        .unwrap_or(0);
                    let next = if direction > 0 {
                        (current + 1).min(commits.len() - 1)
                    } else {
                        current.saturating_sub(1)
                    };
                    let id = commits[next].id.clone();
                    self.select_commit(&id);
                    self.graph_scroll_pending = true;
                }
            }
        }
        if ctx.input(|input| input.key_pressed(egui::Key::F5)) && !self.busy && self.repo.is_some()
        {
            self.queue(Job::Refresh);
        }
        let dropped = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .filter_map(|f| f.path.clone())
                .next()
        });
        if let Some(path) = dropped {
            self.queue(Job::Open(path));
        }
        egui::Panel::top("workspace_tabs")
            .frame(
                egui::Frame::new()
                    .fill(panel_alt())
                    .inner_margin(egui::Margin::symmetric(8, 3)),
            )
            .show(ui, |ui| self.workspace_tabs(ui));
        let repository_view = !matches!(
            self.page,
            Page::Repositories | Page::Changelog | Page::NewTab
        );
        if repository_view {
            egui::Panel::top("top")
                .frame(
                    egui::Frame::new()
                        .fill(panel())
                        .stroke(Stroke::new(1.0, border()))
                        .inner_margin(egui::Margin::symmetric(10, 5)),
                )
                .show(ui, |ui| self.toolbar(ui));
        }
        egui::Panel::bottom("status")
            .frame(
                egui::Frame::new()
                    .fill(panel())
                    .stroke(Stroke::new(1.0, border()))
                    .inner_margin(egui::Margin::symmetric(9, 3)),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    if self.error.is_empty() {
                        ui.label(
                            RichText::new(if self.busy {
                                "Working..."
                            } else if self.notice.is_empty() {
                                "Ready"
                            } else {
                                &self.notice
                            })
                            .color(accent()),
                        );
                    } else {
                        ui.label(RichText::new(format!("Error: {}", self.error)).color(red()));
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            RichText::new(format!(
                                "GitVibe {}  |  Made for every desktop",
                                env!("CARGO_PKG_VERSION")
                            ))
                            .size(10.0)
                            .color(muted()),
                        );
                    });
                });
            });
        if repository_view {
            egui::Panel::left("nav_compact_v3")
                .resizable(true)
                .default_size(162.0)
                .min_size(155.0)
                .frame(
                    egui::Frame::new()
                        .fill(panel())
                        .stroke(Stroke::new(1.0, border()))
                        .inner_margin(egui::Margin::same(8)),
                )
                .show(ui, |ui| {
                    let body_height = (ui.available_height() - 220.0).max(100.0);
                    egui::ScrollArea::vertical()
                        .max_height(body_height)
                        .auto_shrink([false, false])
                        .show(ui, |ui| self.sidebar(ui));
                    self.sidebar_footer(ui);
                });
        }
        if repository_view {
            egui::Panel::right("inspector_compact")
                .resizable(true)
                .default_size(340.0)
                .min_size(260.0)
                .frame(
                    egui::Frame::new()
                        .fill(panel())
                        .stroke(Stroke::new(1.0, border()))
                        .inner_margin(egui::Margin::same(10)),
                )
                .show(ui, |ui| {
                    if self.page == Page::Changes {
                        self.inspector(ui);
                    } else {
                        egui::ScrollArea::vertical()
                            .auto_shrink([false, false])
                            .show(ui, |ui| self.inspector(ui));
                    }
                });
        }
        if repository_view && self.terminal_open {
            egui::Panel::bottom("git_console_dock")
                .resizable(true)
                .default_size(180.0)
                .min_size(115.0)
                .frame(
                    egui::Frame::new()
                        .fill(bg())
                        .stroke(Stroke::new(1.0, border()))
                        .inner_margin(egui::Margin::symmetric(9, 5)),
                )
                .show(ui, |ui| self.console_dock(ui));
        }
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(bg())
                    .inner_margin(egui::Margin::same(if self.page == Page::History {
                        0
                    } else {
                        10
                    })),
            )
            .show(ui, |ui| match self.page {
                Page::History => self.history(ui),
                Page::Changes => self.change_diff_view(ui),
                Page::CommitDiff => self.commit_diff_view(ui),
                Page::NewTab => self.new_tab(ui),
                Page::Repositories => self.repositories(ui),
                Page::Changelog => self.changelog(ui),
                Page::Branches => self.branches(ui),
                Page::Stashes => self.stashes(ui),
                Page::Worktrees => self.worktree_page(ui),
                Page::Submodules => self.submodule_page(ui),
                Page::Rebase => self.rebase_page(ui),
                Page::PullRequests => self.pull_request_page(ui),
                Page::Console => self.console(ui),
                Page::Updates => self.updates_page(ui),
            });
        self.command_palette(&ctx);
        self.dialogs(&ctx);
        self.launch(&ctx);
    }
}

fn lane_color(lane: usize) -> Color32 {
    [accent(), orange(), violet(), blue(), red()][lane % 5]
}

fn pick_folder(start: Option<&Path>) -> Option<PathBuf> {
    let mut dialog = rfd::FileDialog::new();
    if let Some(path) = start.filter(|path| path.is_dir()) {
        dialog = dialog.set_directory(path);
    }
    dialog.pick_folder()
}

fn run_shell_command(cwd: &Path, script: &str) -> Result<String, String> {
    let mut command = if cfg!(windows) {
        let mut command = Command::new("powershell.exe");
        command.args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            script,
        ]);
        command
    } else {
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_owned());
        let mut command = Command::new(shell);
        command.args(["-lc", script]);
        command
    };
    command.current_dir(cwd);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000);
    }
    let output = command.output().map_err(|error| error.to_string())?;
    let mut combined = String::from_utf8_lossy(&output.stdout).into_owned();
    combined.push_str(&String::from_utf8_lossy(&output.stderr));
    if !output.status.success() {
        combined.push_str(&format!("\nExited with {}\n", output.status));
    }
    Ok(combined)
}

fn diff_line(ui: &mut egui::Ui, line: &str) {
    let color = if line.starts_with("@@") || line.starts_with("diff --git") {
        violet()
    } else if line.starts_with('+') && !line.starts_with("+++") {
        accent()
    } else if line.starts_with('-') && !line.starts_with("---") {
        red()
    } else if line.starts_with("commit ")
        || line.starts_with("Author:")
        || line.starts_with("Date:")
    {
        blue()
    } else {
        muted()
    };
    ui.add(
        egui::Label::new(RichText::new(line).monospace().size(11.0).color(color)).selectable(true),
    );
}

fn split_diff_row(ui: &mut egui::Ui, row: &git::SplitDiffRow) {
    ui.columns(2, |columns| {
        if let Some(old) = &row.old {
            let counterpart = if old.line.starts_with('-') {
                row.new.as_ref().map(|side| side.line.as_str())
            } else {
                None
            };
            diff_line_numbered(
                &mut columns[0],
                &old.line,
                Some(old.number),
                None,
                counterpart,
                false,
            );
        } else {
            columns[0].add_space(18.0);
        }
        if let Some(new) = &row.new {
            let counterpart = if new.line.starts_with('+') {
                row.old.as_ref().map(|side| side.line.as_str())
            } else {
                None
            };
            diff_line_numbered(
                &mut columns[1],
                &new.line,
                None,
                Some(new.number),
                counterpart,
                false,
            );
        } else {
            columns[1].add_space(18.0);
        }
    });
}

fn diff_line_numbered(
    ui: &mut egui::Ui,
    line: &str,
    old: Option<usize>,
    new: Option<usize>,
    other: Option<&str>,
    wrap: bool,
) {
    let added = line.starts_with('+');
    let removed = line.starts_with('-');
    let color = if added {
        accent()
    } else if removed {
        red()
    } else {
        text()
    };
    let tint = if added {
        Color32::from_rgba_premultiplied(45, 102, 82, 95)
    } else if removed {
        Color32::from_rgba_premultiplied(121, 50, 66, 95)
    } else {
        Color32::TRANSPARENT
    };
    egui::Frame::new()
        .fill(tint)
        .inner_margin(egui::Margin::symmetric(3, 1))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                ui.add_sized(
                    [33.0, 16.0],
                    egui::Label::new(
                        RichText::new(old.map_or(String::new(), |n| n.to_string()))
                            .monospace()
                            .size(10.0)
                            .color(muted()),
                    ),
                );
                ui.add_sized(
                    [33.0, 16.0],
                    egui::Label::new(
                        RichText::new(new.map_or(String::new(), |n| n.to_string()))
                            .monospace()
                            .size(10.0)
                            .color(muted()),
                    ),
                );
                let mut job = egui::text::LayoutJob::default();
                let content = line.strip_prefix(['+', '-', ' ']).unwrap_or(line);
                job.append(
                    &line[..line.len() - content.len()],
                    0.0,
                    egui::TextFormat {
                        font_id: egui::FontId::monospace(11.0),
                        color,
                        ..Default::default()
                    },
                );
                if let Some(other) = other {
                    let other = other.strip_prefix(['+', '-']).unwrap_or(other);
                    let a: Vec<char> = content.chars().collect();
                    let b: Vec<char> = other.chars().collect();
                    let prefix = a.iter().zip(&b).take_while(|(a, b)| a == b).count();
                    let suffix = a[prefix..]
                        .iter()
                        .rev()
                        .zip(b[prefix..].iter().rev())
                        .take_while(|(a, b)| a == b)
                        .count();
                    let start = a[..prefix].iter().collect::<String>();
                    let middle = a[prefix..a.len() - suffix].iter().collect::<String>();
                    let end = a[a.len() - suffix..].iter().collect::<String>();
                    for (part, highlight) in [(start, false), (middle, true), (end, false)] {
                        append_code_highlight(
                            &mut job,
                            &part,
                            color,
                            if highlight {
                                elevated()
                            } else {
                                Color32::TRANSPARENT
                            },
                        );
                    }
                } else {
                    append_code_highlight(&mut job, content, color, Color32::TRANSPARENT);
                }
                ui.add(egui::Label::new(job).selectable(true).wrap_mode(if wrap {
                    egui::TextWrapMode::Wrap
                } else {
                    egui::TextWrapMode::Extend
                }));
            });
        });
}

fn append_code_highlight(
    job: &mut egui::text::LayoutJob,
    content: &str,
    base: Color32,
    background: Color32,
) {
    let chars = content.chars().collect::<Vec<_>>();
    let mut i = 0;
    while i < chars.len() {
        let start = i;
        let syntax_color = if chars[i] == '"' || chars[i] == '\'' {
            let quote = chars[i];
            i += 1;
            while i < chars.len() {
                if chars[i] == '\\' {
                    i = (i + 2).min(chars.len());
                    continue;
                }
                i += 1;
                if chars[i - 1] == quote {
                    break;
                }
            }
            orange()
        } else if chars[i] == '/' && chars.get(i + 1) == Some(&'/') {
            i = chars.len();
            muted()
        } else if chars[i].is_ascii_alphanumeric() || chars[i] == '_' {
            while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            let token = chars[start..i].iter().collect::<String>();
            if matches!(
                token.as_str(),
                "fn" | "let"
                    | "mut"
                    | "pub"
                    | "struct"
                    | "enum"
                    | "impl"
                    | "use"
                    | "mod"
                    | "match"
                    | "if"
                    | "else"
                    | "for"
                    | "while"
                    | "return"
                    | "const"
                    | "static"
                    | "async"
                    | "await"
                    | "class"
                    | "def"
                    | "function"
                    | "import"
                    | "from"
                    | "export"
                    | "new"
                    | "var"
                    | "interface"
                    | "type"
            ) {
                blue()
            } else if token
                .chars()
                .next()
                .is_some_and(|first| first.is_ascii_digit())
            {
                orange()
            } else if token
                .chars()
                .next()
                .is_some_and(|first| first.is_ascii_uppercase())
            {
                violet()
            } else {
                base
            }
        } else {
            i += 1;
            base
        };
        let token = chars[start..i].iter().collect::<String>();
        job.append(
            &token,
            0.0,
            egui::TextFormat {
                font_id: egui::FontId::monospace(11.0),
                color: syntax_color,
                background,
                ..Default::default()
            },
        );
    }
}

struct CommitRowStyle {
    selected: bool,
    compare_base: bool,
}

fn paint_commit_row(
    ui: &mut egui::Ui,
    commit: &git::Commit,
    refs: &[(String, String, Color32)],
    row: usize,
    style: CommitRowStyle,
    graph_width: f32,
    show_edges: bool,
) -> egui::Response {
    let CommitRowStyle {
        selected,
        compare_base,
    } = style;
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(ui.available_width().max(graph_width + 500.0), 27.0),
        egui::Sense::click(),
    );
    let painter = ui.painter();
    painter.rect_filled(
        rect,
        0.0,
        if selected {
            elevated()
        } else if row.is_multiple_of(2) {
            bg()
        } else {
            panel()
        },
    );
    painter.line_segment(
        [rect.left_bottom(), rect.right_bottom()],
        Stroke::new(0.5, border()),
    );
    if compare_base {
        painter.rect_filled(
            egui::Rect::from_min_size(rect.left_top(), egui::vec2(3.0, rect.height())),
            0.0,
            orange(),
        );
    }
    if response.hovered() && !selected {
        painter.rect_filled(
            egui::Rect::from_min_size(rect.left_top(), egui::vec2(2.0, rect.height())),
            0.0,
            accent(),
        );
    }

    let lane_x = |lane: usize| rect.left() + 148.0 + lane as f32 * 16.0;
    let node_x = lane_x(commit.lane);
    let mid_y = rect.center().y;
    if show_edges {
        for &(from, to) in &commit.graph_edges {
            painter.line_segment(
                [
                    egui::pos2(lane_x(from), rect.top()),
                    egui::pos2(lane_x(from), mid_y),
                ],
                Stroke::new(2.4, lane_color(from)),
            );
            painter.line_segment(
                [
                    egui::pos2(lane_x(from), mid_y),
                    egui::pos2(lane_x(to), rect.bottom()),
                ],
                Stroke::new(2.4, lane_color(from)),
            );
        }
        painter.line_segment(
            [egui::pos2(node_x, rect.top()), egui::pos2(node_x, mid_y)],
            Stroke::new(2.4, lane_color(commit.lane)),
        );
        for &to in &commit.parent_edges {
            painter.line_segment(
                [
                    egui::pos2(node_x, mid_y),
                    egui::pos2(lane_x(to), rect.bottom()),
                ],
                Stroke::new(2.4, lane_color(to)),
            );
        }
    }
    let avatar_colors = [accent(), violet(), orange(), blue(), red()];
    let author_hash = commit.author.bytes().fold(0usize, |acc, byte| {
        acc.wrapping_mul(31).wrapping_add(byte as usize)
    });
    let node = egui::pos2(node_x, mid_y);
    painter.circle_filled(node, 9.4, lane_color(commit.lane));
    painter.circle_filled(node, 7.6, avatar_colors[author_hash % avatar_colors.len()]);
    let initials = commit
        .author
        .split_whitespace()
        .filter_map(|part| part.chars().next())
        .take(2)
        .collect::<String>()
        .to_uppercase();
    painter.text(
        node,
        egui::Align2::CENTER_CENTER,
        initials,
        egui::FontId::proportional(7.5),
        bg(),
    );

    let names = refs
        .iter()
        .filter(|(target, _, _)| target == &commit.short)
        .collect::<Vec<_>>();
    if let Some((_, name, color)) = names.first() {
        let mut label = name.chars().take(19).collect::<String>();
        if names.len() > 1 {
            label.push_str(" +");
        }
        let width = (label.chars().count() as f32 * 5.9 + 14.0).min(137.0);
        let pill = egui::Rect::from_min_size(
            egui::pos2(rect.left() + 5.0, mid_y - 9.0),
            egui::vec2(width, 18.0),
        );
        painter.rect_filled(pill, 3.0, panel_alt());
        painter.rect_stroke(
            pill,
            3.0,
            Stroke::new(1.0, *color),
            egui::StrokeKind::Inside,
        );
        painter.text(
            egui::pos2(pill.left() + 6.0, mid_y),
            egui::Align2::LEFT_CENTER,
            label,
            egui::FontId::proportional(10.0),
            *color,
        );
    }

    let divider_x = rect.left() + graph_width;
    painter.line_segment(
        [
            egui::pos2(divider_x, rect.top()),
            egui::pos2(divider_x, rect.bottom()),
        ],
        Stroke::new(1.0, border()),
    );
    let available = (rect.width() - graph_width - 215.0).max(30.0);
    let max_chars = (available / 6.6).floor() as usize;
    let mut subject = commit.subject.chars().take(max_chars).collect::<String>();
    if commit.subject.chars().count() > max_chars {
        subject.push_str("...");
    }
    painter.text(
        egui::pos2(divider_x + 11.0, mid_y),
        egui::Align2::LEFT_CENTER,
        subject,
        egui::FontId::proportional(12.0),
        if selected { accent() } else { text() },
    );
    painter.text(
        egui::pos2(rect.right() - 8.0, mid_y),
        egui::Align2::RIGHT_CENTER,
        format!("{} · {}", commit.author, commit.date),
        egui::FontId::proportional(10.5),
        muted(),
    );
    response
}

fn workspace_tab(
    ui: &mut egui::Ui,
    id: egui::Id,
    title: &str,
    selected: bool,
    closeable: bool,
    width: f32,
) -> (bool, bool) {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, 34.0), egui::Sense::click());
    let fill = if selected { bg() } else { panel_alt() };
    ui.painter().rect_filled(rect, 0.0, fill);
    ui.painter().line_segment(
        [rect.right_top(), rect.right_bottom()],
        Stroke::new(1.0, border()),
    );
    if selected {
        ui.painter().rect_filled(
            egui::Rect::from_min_size(
                egui::pos2(rect.left(), rect.bottom() - 2.0),
                egui::vec2(rect.width(), 2.0),
            ),
            0.0,
            accent(),
        );
    }
    ui.painter().text(
        egui::pos2(rect.left() + 12.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        title,
        egui::FontId::proportional(12.0),
        text(),
    );
    let mut close_clicked = false;
    if closeable {
        let close_rect = egui::Rect::from_center_size(
            egui::pos2(rect.right() - 15.0, rect.center().y),
            egui::vec2(18.0, 18.0),
        );
        let close_response = ui
            .interact(close_rect, id.with("close"), egui::Sense::click())
            .on_hover_text("Close tab");
        if close_response.hovered() {
            ui.painter().rect_filled(close_rect, 3.0, elevated());
        }
        let c = close_rect.center();
        for (from, to) in [
            (
                egui::pos2(c.x - 3.0, c.y - 3.0),
                egui::pos2(c.x + 3.0, c.y + 3.0),
            ),
            (
                egui::pos2(c.x + 3.0, c.y - 3.0),
                egui::pos2(c.x - 3.0, c.y + 3.0),
            ),
        ] {
            ui.painter()
                .line_segment([from, to], Stroke::new(1.4, muted()));
        }
        close_clicked = close_response.clicked();
    }
    (response.clicked() && !close_clicked, close_clicked)
}

fn sidebar_branch_row(
    ui: &mut egui::Ui,
    label: &str,
    selected: bool,
    depth: f32,
) -> egui::Response {
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 25.0), egui::Sense::click());
    if selected {
        ui.painter()
            .rect_filled(rect, 0.0, Color32::from_rgb(48, 87, 66));
        let box_rect = egui::Rect::from_center_size(
            egui::pos2(rect.left() + 10.0, rect.center().y),
            egui::vec2(10.0, 10.0),
        );
        ui.painter()
            .rect_filled(box_rect, 1.0, Color32::from_rgb(83, 177, 105));
        ui.painter().line_segment(
            [
                egui::pos2(box_rect.left() + 2.0, box_rect.center().y),
                egui::pos2(box_rect.left() + 4.2, box_rect.bottom() - 2.0),
            ],
            Stroke::new(1.3, text()),
        );
        ui.painter().line_segment(
            [
                egui::pos2(box_rect.left() + 4.2, box_rect.bottom() - 2.0),
                egui::pos2(box_rect.right() - 1.7, box_rect.top() + 2.0),
            ],
            Stroke::new(1.3, text()),
        );
    } else if response.hovered() {
        ui.painter().rect_filled(rect, 0.0, panel_alt());
    }
    let x = rect.left() + 14.0 + depth;
    ui.painter().line_segment(
        [
            egui::pos2(x, rect.center().y - 4.0),
            egui::pos2(x, rect.center().y + 3.0),
        ],
        Stroke::new(1.0, if selected { text() } else { muted() }),
    );
    ui.painter().circle_filled(
        egui::pos2(x, rect.center().y - 4.0),
        1.8,
        if selected { text() } else { muted() },
    );
    ui.painter().circle_filled(
        egui::pos2(x + 4.0, rect.center().y + 1.0),
        1.8,
        if selected { text() } else { muted() },
    );
    ui.painter().line_segment(
        [
            egui::pos2(x, rect.center().y + 1.0),
            egui::pos2(x + 4.0, rect.center().y + 1.0),
        ],
        Stroke::new(1.0, if selected { text() } else { muted() }),
    );
    let max_chars = ((rect.width() - depth - 32.0) / 6.1).floor().max(5.0) as usize;
    let mut display = label.chars().take(max_chars).collect::<String>();
    if label.chars().count() > max_chars {
        display.truncate(display.len().saturating_sub(3));
        display.push_str("...");
    }
    ui.painter().text(
        egui::pos2(x + 13.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        display,
        egui::FontId::proportional(11.5),
        if selected { text() } else { muted() },
    );
    response
}

fn toolbar_action(ui: &mut egui::Ui, label: &str) -> bool {
    let background = ui.painter().add(egui::Shape::Noop);
    let response = ui.add(
        egui::Button::new(RichText::new(label).size(11.5).color(text()))
            .fill(Color32::TRANSPARENT)
            .stroke(Stroke::NONE),
    );
    if response.hovered() {
        ui.painter().set(
            background,
            egui::Shape::rect_filled(response.rect, 3.0, elevated()),
        );
    }
    response.clicked()
}

fn action(ui: &mut egui::Ui, label: &str, prominent: bool) -> bool {
    ui.add(
        egui::Button::new(
            RichText::new(label)
                .size(12.5)
                .strong()
                .color(if prominent { bg() } else { text() }),
        )
        .fill(if prominent { accent() } else { panel_alt() })
        .stroke(if prominent {
            Stroke::NONE
        } else {
            Stroke::new(1.0, border())
        }),
    )
    .clicked()
}

fn badge(ui: &mut egui::Ui, text: &str, color: Color32) {
    egui::Frame::new()
        .fill(Color32::from_rgba_unmultiplied(
            color.r(),
            color.g(),
            color.b(),
            30,
        ))
        .corner_radius(egui::CornerRadius::same(6))
        .inner_margin(egui::Margin::symmetric(7, 2))
        .show(ui, |ui| {
            ui.label(RichText::new(text).size(11.0).strong().color(color));
        });
}

fn section_title(ui: &mut egui::Ui, eyebrow: &str, title: &str, subtitle: &str) {
    ui.label(RichText::new(eyebrow).size(11.0).strong().color(accent()));
    ui.label(RichText::new(title).size(25.0).strong().color(text()));
    ui.label(RichText::new(subtitle).size(12.0).color(muted()));
}

fn brand_icon(ui: &mut egui::Ui) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(36.0, 36.0), egui::Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, 9.0, accent());
    let left = rect.left() + 13.0;
    let right = rect.left() + 25.0;
    painter.line_segment(
        [
            egui::pos2(left, rect.top() + 8.0),
            egui::pos2(left, rect.bottom() - 8.0),
        ],
        Stroke::new(2.5, bg()),
    );
    painter.line_segment(
        [
            egui::pos2(left, rect.center().y),
            egui::pos2(right, rect.center().y + 6.0),
        ],
        Stroke::new(2.5, bg()),
    );
    for pos in [
        egui::pos2(left, rect.top() + 8.0),
        egui::pos2(left, rect.bottom() - 8.0),
        egui::pos2(right, rect.center().y + 6.0),
    ] {
        painter.circle_filled(pos, 3.5, bg());
    }
}
