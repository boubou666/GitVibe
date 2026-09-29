use std::{
    path::{Path, PathBuf},
    sync::mpsc::{self, Receiver},
    thread,
};

use crate::git::{self, Snapshot};
use eframe::egui::{self, Color32, RichText, Stroke};

const BG: Color32 = Color32::from_rgb(10, 15, 24);
const PANEL: Color32 = Color32::from_rgb(17, 24, 36);
const PANEL_ALT: Color32 = Color32::from_rgb(25, 35, 50);
const ELEVATED: Color32 = Color32::from_rgb(31, 43, 59);
const BORDER: Color32 = Color32::from_rgb(43, 58, 76);
const TEXT: Color32 = Color32::from_rgb(232, 241, 249);
const MUTED: Color32 = Color32::from_rgb(137, 155, 176);
const ACCENT: Color32 = Color32::from_rgb(77, 225, 194);
const ORANGE: Color32 = Color32::from_rgb(255, 177, 96);
const RED: Color32 = Color32::from_rgb(255, 111, 134);
const VIOLET: Color32 = Color32::from_rgb(174, 145, 255);
const BLUE: Color32 = Color32::from_rgb(116, 172, 255);

#[derive(PartialEq, Clone, Copy)]
enum Page {
    History,
    Changes,
    Branches,
    Stashes,
    Console,
}

#[derive(Clone, Copy)]
enum CommitAction {
    CherryPick,
    Revert,
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

enum Job {
    Open(PathBuf),
    Init(PathBuf),
    Clone(String, PathBuf),
    Run(Vec<String>),
    Inspect(Vec<String>),
    InspectFile(String),
    InspectConflict(String),
    ApplyHunk(String, bool),
    ResolveConflict(String, git::ConflictSide),
    MarkResolved(String),
    Refresh,
}

enum Done {
    Loaded(Result<Snapshot, String>),
    Ran(Result<(String, Snapshot), String>),
    Inspected(Result<String, String>),
}

pub struct GitVibe {
    repo: Option<PathBuf>,
    recent_repos: Vec<PathBuf>,
    snapshot: Option<Snapshot>,
    page: Page,
    path_input: String,
    clone_url: String,
    clone_destination: String,
    branch_input: String,
    tag_input: String,
    commit_message: String,
    command_input: String,
    selected_commit: Option<String>,
    compare_base: Option<String>,
    history_limit: usize,
    selected_file: Option<String>,
    selected_file_staged: bool,
    detail: String,
    output: String,
    error: String,
    search: String,
    receiver: Option<Receiver<Done>>,
    pending: Option<Job>,
    busy: bool,
    show_clone: bool,
    confirm_discard: Option<String>,
    confirm_commit_action: Option<(CommitAction, String)>,
    confirm_delete_ref: Option<RefDeletion>,
    confirm_reset: Option<String>,
    confirm_conflict_side: Option<(String, git::ConflictSide)>,
    confirm_abort_merge: bool,
    reset_mode: ResetMode,
    reset_confirmation: String,
    committing: bool,
    hunk_action: bool,
    conflict_action: bool,
}

impl GitVibe {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let mut visuals = egui::Visuals::dark();
        visuals.override_text_color = Some(TEXT);
        visuals.weak_text_color = Some(MUTED);
        visuals.panel_fill = BG;
        visuals.window_fill = PANEL;
        visuals.window_stroke = Stroke::new(1.0, BORDER);
        visuals.window_corner_radius = egui::CornerRadius::same(12);
        visuals.extreme_bg_color = BG;
        visuals.text_edit_bg_color = Some(PANEL_ALT);
        visuals.code_bg_color = PANEL_ALT;
        visuals.widgets.inactive.bg_fill = PANEL_ALT;
        visuals.widgets.inactive.weak_bg_fill = PANEL_ALT;
        visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, BORDER);
        visuals.widgets.hovered.bg_fill = ELEVATED;
        visuals.widgets.hovered.weak_bg_fill = ELEVATED;
        visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, ACCENT);
        visuals.widgets.active.bg_fill = Color32::from_rgb(37, 77, 83);
        visuals.widgets.active.weak_bg_fill = Color32::from_rgb(37, 77, 83);
        visuals.widgets.active.bg_stroke = Stroke::new(1.0, ACCENT);
        for widget in [
            &mut visuals.widgets.inactive,
            &mut visuals.widgets.hovered,
            &mut visuals.widgets.active,
            &mut visuals.widgets.open,
        ] {
            widget.corner_radius = egui::CornerRadius::same(7);
        }
        visuals.selection.bg_fill = Color32::from_rgb(38, 91, 93);
        visuals.selection.stroke = Stroke::new(1.0, ACCENT);
        visuals.hyperlink_color = ACCENT;
        cc.egui_ctx.set_visuals(visuals);
        cc.egui_ctx.style_mut_of(egui::Theme::Dark, |s| {
            s.spacing.item_spacing = egui::vec2(9.0, 9.0);
            s.spacing.button_padding = egui::vec2(14.0, 8.0);
            s.spacing.interact_size.y = 32.0;
        });
        let recent_repos = cc
            .storage
            .and_then(|storage| eframe::get_value::<Vec<PathBuf>>(storage, "recent_repos"))
            .unwrap_or_default();
        let path = std::env::args_os()
            .nth(1)
            .map(PathBuf::from)
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
            recent_repos,
            snapshot: None,
            page: Page::History,
            path_input: path.to_string_lossy().into_owned(),
            clone_url: String::new(),
            clone_destination: String::new(),
            branch_input: String::new(),
            tag_input: String::new(),
            commit_message: String::new(),
            command_input: String::new(),
            selected_commit: None,
            compare_base: None,
            history_limit: 300,
            selected_file: None,
            selected_file_staged: false,
            detail: String::new(),
            output: String::new(),
            error: String::new(),
            search: String::new(),
            receiver: None,
            pending: None,
            busy: false,
            show_clone: false,
            confirm_discard: None,
            confirm_commit_action: None,
            confirm_delete_ref: None,
            confirm_reset: None,
            confirm_conflict_side: None,
            confirm_abort_merge: false,
            reset_mode: ResetMode::Mixed,
            reset_confirmation: String::new(),
            committing: false,
            hunk_action: false,
            conflict_action: false,
        };
        if git::discover(&path).is_ok() {
            app.pending = Some(Job::Open(path));
        }
        app
    }

    fn queue(&mut self, job: Job) {
        if !self.busy {
            if matches!(&job, Job::Open(_) | Job::Init(_) | Job::Clone(_, _)) {
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
                Job::Clone(url, path) => Done::Loaded(
                    git::clone_repo(&url, &path)
                        .and_then(|p| git::snapshot_with_limit(&p, history_limit)),
                ),
                Job::Refresh => Done::Loaded(
                    repo.ok_or("No repository selected".to_owned())
                        .and_then(|p| git::snapshot_with_limit(&p, history_limit)),
                ),
                Job::Run(args) => Done::Ran(
                    repo.ok_or("No repository selected".to_owned())
                        .and_then(|p| {
                            let output = git::run_owned(&p, &args)?;
                            let snapshot = git::snapshot_with_limit(&p, history_limit)?;
                            Ok((output, snapshot))
                        }),
                ),
                Job::Inspect(args) => Done::Inspected(
                    repo.ok_or("No repository selected".to_owned())
                        .and_then(|p| git::run_owned(&p, &args)),
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
                Job::ApplyHunk(patch, reverse) => Done::Ran(
                    repo.ok_or("No repository selected".to_owned())
                        .and_then(|p| {
                            git::apply_hunk(&p, &patch, reverse)?;
                            let snapshot = git::snapshot_with_limit(&p, history_limit)?;
                            Ok((
                                if reverse {
                                    "Hunk unstaged".to_owned()
                                } else {
                                    "Hunk staged".to_owned()
                                },
                                snapshot,
                            ))
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
        match done {
            Done::Loaded(result) => match result {
                Ok(snapshot) => {
                    if self.repo.as_ref() != Some(&snapshot.root) {
                        self.selected_commit = None;
                        self.selected_file = None;
                        self.detail.clear();
                        self.compare_base = None;
                    }
                    self.repo = Some(snapshot.root.clone());
                    self.path_input = snapshot.root.to_string_lossy().into_owned();
                    self.recent_repos.retain(|path| path != &snapshot.root);
                    self.recent_repos.insert(0, snapshot.root.clone());
                    self.recent_repos.truncate(8);
                    self.snapshot = Some(snapshot);
                    self.error.clear();
                }
                Err(error) => self.error = error,
            },
            Done::Ran(result) => match result {
                Ok((output, snapshot)) => {
                    self.output = output;
                    self.repo = Some(snapshot.root.clone());
                    self.snapshot = Some(snapshot);
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
                    }
                    self.conflict_action = false;
                }
                Err(error) => {
                    self.error = error;
                    self.committing = false;
                    self.hunk_action = false;
                    self.conflict_action = false;
                }
            },
            Done::Inspected(result) => match result {
                Ok(text) => {
                    self.detail = text;
                    self.error.clear();
                }
                Err(error) => self.error = error,
            },
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
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(
                    RichText::new("WORKSPACE  /  REPOSITORY")
                        .size(10.0)
                        .strong()
                        .color(MUTED),
                );
                ui.label(
                    RichText::new(repo_name.as_deref().unwrap_or("No repository open"))
                        .size(19.0)
                        .strong()
                        .color(TEXT),
                );
            });
            ui.add_space(14.0);
            if let Some(branch) = &branch {
                badge(ui, &format!("BRANCH  {branch}"), VIOLET);
            }
            if let Some(count) = changed {
                if count > 0 {
                    badge(ui, &format!("{count} changes"), ORANGE);
                } else {
                    badge(ui, "Working tree clean", ACCENT);
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_enabled_ui(self.repo.is_some() && !self.busy, |ui| {
                    if action(ui, "Refresh", false) {
                        self.queue(Job::Refresh);
                    }
                    if action(ui, "Fetch", false) {
                        self.git(&["fetch", "--all", "--prune"]);
                    }
                    if action(ui, "Pull", false) {
                        self.git(&["pull"]);
                    }
                    if action(ui, "Push", true) {
                        self.git(&["push"]);
                    }
                });
                if self.busy {
                    ui.spinner();
                }
            });
        });
    }

    fn sidebar(&mut self, ui: &mut egui::Ui) {
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            brand_icon(ui);
            ui.vertical(|ui| {
                ui.label(RichText::new("GITVIBE").size(18.0).strong().color(TEXT));
                ui.label(
                    RichText::new("GIT, WITH FLOW")
                        .size(9.0)
                        .strong()
                        .color(ACCENT),
                );
            });
        });
        ui.add_space(25.0);
        ui.label(RichText::new("EXPLORE").size(10.0).strong().color(MUTED));
        ui.add_space(3.0);
        let changed = self.snapshot.as_ref().map_or(0, |s| s.status.len());
        for (page, title) in [
            (Page::History, "Commit graph"),
            (Page::Changes, "Changes"),
            (Page::Branches, "Branches & tags"),
            (Page::Stashes, "Stashes"),
            (Page::Console, "Git console"),
        ] {
            let selected = self.page == page;
            let label = if page == Page::Changes && changed > 0 {
                format!("{title}  ({changed})")
            } else {
                title.to_owned()
            };
            let shown = egui::Frame::new()
                .fill(if selected {
                    ELEVATED
                } else {
                    Color32::TRANSPARENT
                })
                .corner_radius(egui::CornerRadius::same(7))
                .stroke(if selected {
                    Stroke::new(1.0, BORDER)
                } else {
                    Stroke::NONE
                })
                .inner_margin(egui::Margin::symmetric(11, 8))
                .show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    ui.horizontal(|ui| {
                        nav_icon(ui, page, if selected { ACCENT } else { MUTED });
                        ui.add_space(4.0);
                        ui.label(RichText::new(label).size(13.0).strong().color(if selected {
                            ACCENT
                        } else {
                            TEXT
                        }));
                    });
                });
            if ui
                .interact(
                    shown.response.rect,
                    egui::Id::new(("nav", title)),
                    egui::Sense::click(),
                )
                .clicked()
            {
                self.page = page;
            }
            ui.add_space(3.0);
        }
        ui.add_space(24.0);
        egui::Frame::new()
            .fill(PANEL_ALT)
            .corner_radius(egui::CornerRadius::same(10))
            .stroke(Stroke::new(1.0, BORDER))
            .inner_margin(egui::Margin::same(12))
            .show(ui, |ui| {
                ui.label(RichText::new("REPOSITORY").size(10.0).strong().color(MUTED));
                ui.add(
                    egui::TextEdit::singleline(&mut self.path_input)
                        .hint_text("Path to repository")
                        .desired_width(ui.available_width()),
                );
                ui.horizontal(|ui| {
                    if action(ui, "Open", true) {
                        self.queue(Job::Open(PathBuf::from(self.path_input.trim())));
                    }
                    if action(ui, "Browse...", false)
                        && let Some(path) = pick_folder(Some(Path::new(&self.path_input)))
                    {
                        self.path_input = path.to_string_lossy().into_owned();
                        self.queue(Job::Open(path));
                    }
                    if action(ui, "Initialize", false) {
                        self.queue(Job::Init(PathBuf::from(self.path_input.trim())));
                    }
                });
                if action(ui, "+  Clone repository", false) {
                    self.show_clone = true;
                }
            });
        if !self.recent_repos.is_empty() {
            ui.add_space(15.0);
            ui.label(
                RichText::new("RECENT REPOSITORIES")
                    .size(10.0)
                    .strong()
                    .color(MUTED),
            );
            for path in self.recent_repos.clone().into_iter().take(5) {
                ui.horizontal(|ui| {
                    let name = path
                        .file_name()
                        .unwrap_or(path.as_os_str())
                        .to_string_lossy();
                    if ui
                        .add(
                            egui::Button::new(RichText::new(name).size(11.5).color(TEXT))
                                .fill(Color32::TRANSPARENT)
                                .stroke(Stroke::NONE),
                        )
                        .on_hover_text(path.to_string_lossy())
                        .clicked()
                    {
                        self.queue(Job::Open(path.clone()));
                    }
                    if ui
                        .small_button("x")
                        .on_hover_text("Forget this repository")
                        .clicked()
                    {
                        self.recent_repos.retain(|recent| recent != &path);
                    }
                });
            }
        }
        ui.add_space(22.0);
        if let Some(snapshot) = &self.snapshot {
            let branches = snapshot.branches.clone();
            let remotes = snapshot.remotes.clone();
            ui.label(
                RichText::new("LOCAL BRANCHES")
                    .size(10.0)
                    .strong()
                    .color(MUTED),
            );
            ui.add_space(3.0);
            for branch in branches.iter().take(12) {
                let shown = ui.horizontal(|ui| {
                    let (rect, _) =
                        ui.allocate_exact_size(egui::vec2(12.0, 20.0), egui::Sense::hover());
                    ui.painter().circle_filled(
                        rect.center(),
                        4.0,
                        if branch.current { ACCENT } else { MUTED },
                    );
                    ui.label(
                        RichText::new(&branch.name)
                            .size(12.0)
                            .color(if branch.current { ACCENT } else { MUTED }),
                    );
                });
                if ui
                    .interact(
                        shown.response.rect,
                        egui::Id::new(("branch", &branch.name)),
                        egui::Sense::click(),
                    )
                    .clicked()
                {
                    self.page = Page::Branches;
                }
            }
            ui.add_space(15.0);
            ui.label(RichText::new("REMOTES").size(10.0).strong().color(MUTED));
            for remote in remotes {
                ui.label(
                    RichText::new(format!("remote  {remote}"))
                        .size(12.0)
                        .color(MUTED),
                );
            }
        }
    }

    fn history(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                section_title(
                    ui,
                    "YOUR REPOSITORY, IN MOTION",
                    "Commit graph",
                    "Trace branches, merges, and the story behind each change.",
                );
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut self.search)
                        .hint_text("Search commits")
                        .desired_width(205.0),
                );
            });
        });
        ui.add_space(15.0);
        let Some(snapshot) = self.snapshot.clone() else {
            self.empty(ui);
            return;
        };
        let changed = snapshot.status.len();
        let branch = snapshot.branch.clone();
        if changed > 0 {
            egui::Frame::new()
                .fill(ELEVATED)
                .corner_radius(egui::CornerRadius::same(10))
                .stroke(Stroke::new(1.0, BORDER))
                .inner_margin(egui::Margin::symmetric(16, 12))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let (dot, _) =
                            ui.allocate_exact_size(egui::vec2(18.0, 18.0), egui::Sense::hover());
                        ui.painter().circle_filled(dot.center(), 5.0, ORANGE);
                        ui.vertical(|ui| {
                            ui.label(
                                RichText::new("Work in progress")
                                    .size(14.0)
                                    .strong()
                                    .color(TEXT),
                            );
                            ui.label(
                                RichText::new(format!("{changed} changed files on {branch}"))
                                    .size(11.0)
                                    .color(MUTED),
                            );
                        });
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if action(ui, "Review changes", true) {
                                self.page = Page::Changes;
                            }
                        });
                    });
                });
            ui.add_space(14.0);
        }
        if snapshot.commits.is_empty() {
            egui::Frame::new()
                .fill(PANEL)
                .corner_radius(egui::CornerRadius::same(12))
                .inner_margin(egui::Margin::same(26))
                .show(ui, |ui| {
                    ui.label(
                        RichText::new("A fresh beginning")
                            .size(20.0)
                            .strong()
                            .color(TEXT),
                    );
                    ui.label(
                        RichText::new(
                            "Stage a file and make your first commit to start the graph.",
                        )
                        .color(MUTED),
                    );
                });
            return;
        }
        let refs = snapshot
            .branches
            .iter()
            .map(|r| (r.target.clone(), r.name.clone(), ACCENT))
            .chain(
                snapshot
                    .remote_branches
                    .iter()
                    .map(|r| (r.target.clone(), r.name.clone(), BLUE)),
            )
            .chain(
                snapshot
                    .tags
                    .iter()
                    .map(|r| (r.target.clone(), r.name.clone(), ORANGE)),
            )
            .collect::<Vec<_>>();
        let commits = snapshot.commits.clone();
        let search = self.search.to_lowercase();
        let max_lanes = commits.iter().map(|c| c.lane_count).max().unwrap_or(1);
        let graph_width = (115.0 + max_lanes.min(9) as f32 * 17.0).clamp(180.0, 280.0);
        let visible = commits
            .iter()
            .filter(|commit| {
                search.is_empty()
                    || format!("{} {} {}", commit.subject, commit.author, commit.short)
                        .to_lowercase()
                        .contains(&search)
            })
            .collect::<Vec<_>>();

        egui::Frame::new()
            .fill(ELEVATED)
            .inner_margin(egui::Margin::symmetric(10, 6))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.add_sized(
                        [graph_width - 10.0, 14.0],
                        egui::Label::new(
                            RichText::new("REFS / GRAPH")
                                .size(10.0)
                                .strong()
                                .color(MUTED),
                        ),
                    );
                    ui.label(
                        RichText::new("COMMIT MESSAGE")
                            .size(10.0)
                            .strong()
                            .color(MUTED),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            RichText::new(format!("{} visible", visible.len()))
                                .size(10.0)
                                .color(MUTED),
                        );
                    });
                });
            });
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
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
                        search.is_empty(),
                    );
                    if response.clicked() {
                        self.select_commit(&commit.id);
                    }
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
        self.selected_file = None;
        self.selected_file_staged = false;
        self.queue(Job::Inspect(vec![
            "show".into(),
            "--stat".into(),
            "--format=fuller".into(),
            id.into(),
        ]));
    }

    fn changes(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                section_title(
                    ui,
                    "MAKE THE NEXT MOVE",
                    "Working tree",
                    "Shape your next commit one file at a time.",
                );
            });
        });
        ui.add_space(16.0);
        let Some(snapshot) = self.snapshot.clone() else {
            self.empty(ui);
            return;
        };
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
                .fill(PANEL_ALT)
                .corner_radius(egui::CornerRadius::same(10))
                .stroke(Stroke::new(1.0, ORANGE))
                .inner_margin(egui::Margin::same(14))
                .show(ui, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        badge(ui, "MERGE IN PROGRESS", ORANGE);
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
                .fill(PANEL)
                .corner_radius(egui::CornerRadius::same(10))
                .stroke(Stroke::new(1.0, BORDER))
                .inner_margin(egui::Margin::same(22))
                .show(ui, |ui| {
                    ui.label(RichText::new("All clear").size(20.0).strong().color(ACCENT));
                    ui.label(
                        RichText::new(
                            "Your working tree is clean. Enjoy the calm before the next commit.",
                        )
                        .color(MUTED),
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
                            .color(RED),
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
                            .color(ORANGE),
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
                    ui.label(RichText::new("Nothing to stage").size(12.0).color(MUTED));
                }
                ui.add_space(16.0);
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(format!("STAGED  |  {}", staged.len()))
                            .size(11.0)
                            .strong()
                            .color(ACCENT),
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
                            .color(MUTED),
                    );
                }
            });
        ui.add_space(14.0);
        egui::Frame::new()
            .fill(PANEL_ALT)
            .corner_radius(egui::CornerRadius::same(10))
            .stroke(Stroke::new(1.0, BORDER))
            .inner_margin(egui::Margin::same(14))
            .show(ui, |ui| {
                ui.label(
                    RichText::new("NEW COMMIT")
                        .size(10.0)
                        .strong()
                        .color(ACCENT),
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
                        .color(MUTED),
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
                                    .color(BG),
                                )
                                .fill(ACCENT)
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
            .fill(if selected { ELEVATED } else { PANEL })
            .corner_radius(egui::CornerRadius::same(8))
            .stroke(Stroke::new(1.0, if selected { ACCENT } else { BORDER }))
            .inner_margin(egui::Margin::symmetric(10, 6))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    badge(ui, status, if staged { ACCENT } else { ORANGE });
                    if ui
                        .add(
                            egui::Button::new(RichText::new(&file.path).size(12.0).color(TEXT))
                                .fill(Color32::TRANSPARENT)
                                .stroke(Stroke::NONE),
                        )
                        .clicked()
                    {
                        self.selected_file = Some(file.path.clone());
                        self.selected_file_staged = staged;
                        self.selected_commit = None;
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
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if staged && action(ui, "Unstage", false) {
                            if self.snapshot.as_ref().is_some_and(|s| s.commits.is_empty()) {
                                self.git_owned(vec![
                                    "rm".into(),
                                    "--cached".into(),
                                    "--".into(),
                                    file.path.clone(),
                                ]);
                            } else {
                                self.git_owned(vec![
                                    "restore".into(),
                                    "--staged".into(),
                                    "--".into(),
                                    file.path.clone(),
                                ]);
                            }
                        }
                        if !staged && action(ui, "Stage", false) {
                            self.git_owned(vec!["add".into(), "--".into(), file.path.clone()]);
                        }
                        if !staged && file.index != '?' && action(ui, "Discard...", false) {
                            self.confirm_discard = Some(file.path.clone());
                        }
                    });
                });
            });
        ui.add_space(4.0);
    }

    fn conflict_row(&mut self, ui: &mut egui::Ui, file: &git::FileStatus) {
        egui::Frame::new()
            .fill(PANEL_ALT)
            .corner_radius(egui::CornerRadius::same(8))
            .stroke(Stroke::new(1.0, RED))
            .inner_margin(egui::Margin::symmetric(10, 7))
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    badge(ui, "CONFLICT", RED);
                    if ui
                        .add(
                            egui::Button::new(RichText::new(&file.path).size(12.0).color(TEXT))
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
        let tags = snapshot.tags.clone();
        egui::Frame::new()
            .fill(PANEL_ALT)
            .corner_radius(egui::CornerRadius::same(10))
            .stroke(Stroke::new(1.0, BORDER))
            .inner_margin(egui::Margin::same(14))
            .show(ui, |ui| {
                ui.label(
                    RichText::new("CREATE A BRANCH")
                        .size(10.0)
                        .strong()
                        .color(ACCENT),
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
                            egui::Button::new(RichText::new("Create & switch").strong().color(BG))
                                .fill(ACCENT)
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
                .color(MUTED),
        );
        ui.add_space(5.0);
        egui::ScrollArea::vertical()
            .max_height((ui.available_height() - 160.0).max(150.0))
            .show(ui, |ui| {
                for branch in branches {
                    egui::Frame::new()
                        .fill(PANEL)
                        .corner_radius(egui::CornerRadius::same(8))
                        .stroke(Stroke::new(1.0, BORDER))
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
                                    if branch.current { ACCENT } else { MUTED },
                                );
                                ui.vertical(|ui| {
                                    ui.label(
                                        RichText::new(&branch.name).size(13.0).strong().color(TEXT),
                                    );
                                    ui.label(
                                        RichText::new(&branch.target)
                                            .size(10.0)
                                            .monospace()
                                            .color(MUTED),
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
                                            badge(ui, "CURRENT", ACCENT);
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
                        .color(MUTED),
                );
                ui.horizontal(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut self.tag_input)
                            .hint_text("New tag name")
                            .desired_width(200.0),
                    );
                    if action(ui, "Create tag", false) && !self.tag_input.trim().is_empty() {
                        let name = self.tag_input.trim().to_owned();
                        self.git_owned(vec!["tag".into(), name]);
                        self.tag_input.clear();
                    }
                });
                for tag in tags {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(format!("tag  {}    {}", tag.name, tag.target))
                                .size(12.0)
                                .color(TEXT),
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
                        .color(BLUE),
                );
                for remote in remote_branches {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(&remote.name).size(12.0).color(TEXT));
                        ui.label(
                            RichText::new(&remote.target)
                                .size(10.0)
                                .monospace()
                                .color(MUTED),
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
            });
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
                .color(MUTED),
        );
        ui.add_space(5.0);
        if stashes.is_empty() {
            egui::Frame::new()
                .fill(PANEL)
                .corner_radius(egui::CornerRadius::same(10))
                .stroke(Stroke::new(1.0, BORDER))
                .inner_margin(egui::Margin::same(22))
                .show(ui, |ui| {
                    ui.label(
                        RichText::new("Nothing tucked away")
                            .size(18.0)
                            .strong()
                            .color(TEXT),
                    );
                    ui.label(
                        RichText::new("Create a stash when you need a clean slate.").color(MUTED),
                    );
                });
        }
        for stash in stashes {
            let name = stash.split_whitespace().next().unwrap_or("").to_owned();
            egui::Frame::new()
                .fill(PANEL)
                .corner_radius(egui::CornerRadius::same(8))
                .stroke(Stroke::new(1.0, BORDER))
                .inner_margin(egui::Margin::symmetric(12, 8))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        badge(ui, "STASH", VIOLET);
                        ui.label(RichText::new(&stash).size(12.0).color(TEXT));
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
        section_title(
            ui,
            "POWER WHEN YOU NEED IT",
            "Git console",
            "Direct access to Git for the workflows without a dedicated view yet.",
        );
        ui.add_space(18.0);
        egui::Frame::new()
            .fill(PANEL_ALT)
            .corner_radius(egui::CornerRadius::same(10))
            .stroke(Stroke::new(1.0, BORDER))
            .inner_margin(egui::Margin::same(14))
            .show(ui, |ui| {
                ui.label(RichText::new("COMMAND").size(10.0).strong().color(ACCENT));
                ui.horizontal(|ui| {
                    let edit = ui.add(
                        egui::TextEdit::singleline(&mut self.command_input)
                            .hint_text("git log --oneline -20")
                            .desired_width(ui.available_width() - 90.0),
                    );
                    let enter = edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                    if action(ui, "Run", true) || enter {
                        match git::split_command(&self.command_input) {
                            Ok(args) => {
                                self.git_owned(args);
                                self.command_input.clear();
                            }
                            Err(error) => self.error = error,
                        }
                    }
                });
                ui.label(
                    RichText::new("Arguments go straight to Git; no shell is involved.")
                        .size(11.0)
                        .color(MUTED),
                );
            });
        ui.add_space(20.0);
        ui.label(
            RichText::new("LAST OUTPUT")
                .size(10.0)
                .strong()
                .color(MUTED),
        );
        egui::Frame::new()
            .fill(PANEL)
            .corner_radius(egui::CornerRadius::same(10))
            .stroke(Stroke::new(1.0, BORDER))
            .inner_margin(egui::Margin::same(14))
            .show(ui, |ui| {
                egui::ScrollArea::both().show(ui, |ui| {
                    if self.output.is_empty() {
                        ui.label(
                            RichText::new("Run a command to see its output here.").color(MUTED),
                        );
                    } else {
                        ui.add(
                            egui::Label::new(
                                RichText::new(&self.output)
                                    .monospace()
                                    .size(11.0)
                                    .color(TEXT),
                            )
                            .selectable(true),
                        );
                    }
                });
            });
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
                    .color(TEXT),
            );
            ui.label(
                RichText::new("Open, initialize, or clone a repository from the sidebar to begin.")
                    .size(13.0)
                    .color(MUTED),
            );
        });
    }

    fn inspector(&mut self, ui: &mut egui::Ui) {
        ui.add_space(8.0);
        ui.label(RichText::new("DETAILS").size(10.0).strong().color(ACCENT));
        ui.label(RichText::new("Inspector").size(22.0).strong().color(TEXT));
        ui.add_space(10.0);
        if let Some(id) = &self.selected_commit {
            let commit = self
                .snapshot
                .as_ref()
                .and_then(|s| s.commits.iter().find(|c| &c.id == id))
                .cloned();
            if let Some(commit) = commit {
                egui::Frame::new()
                    .fill(ELEVATED)
                    .corner_radius(egui::CornerRadius::same(10))
                    .stroke(Stroke::new(1.0, BORDER))
                    .inner_margin(egui::Margin::same(14))
                    .show(ui, |ui| {
                        badge(ui, "COMMIT", VIOLET);
                        ui.add_space(7.0);
                        ui.label(
                            RichText::new(&commit.subject)
                                .size(16.0)
                                .strong()
                                .color(TEXT),
                        );
                        ui.label(
                            RichText::new(format!("{}  |  {}", commit.author, commit.date))
                                .size(11.0)
                                .color(MUTED),
                        );
                        ui.label(
                            RichText::new(&commit.short)
                                .size(11.0)
                                .monospace()
                                .color(ACCENT),
                        );
                        if self.compare_base.as_deref() == Some(&commit.id) {
                            ui.add_space(5.0);
                            badge(ui, "COMPARE BASE", ORANGE);
                        }
                    });
                ui.add_space(8.0);
                ui.add_enabled_ui(!self.busy, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        if action(ui, "Show patch", false) {
                            self.queue(Job::Inspect(vec![
                                "show".into(),
                                "--format=fuller".into(),
                                "--patch".into(),
                                "--stat".into(),
                                commit.id.clone(),
                            ]));
                        }
                        if let Some(parent) = commit.parents.first()
                            && action(ui, "Compare parent", false)
                        {
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
                    });
                });
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
                .fill(ELEVATED)
                .corner_radius(egui::CornerRadius::same(10))
                .stroke(Stroke::new(1.0, BORDER))
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
                            RED
                        } else if self.selected_file_staged {
                            ACCENT
                        } else {
                            ORANGE
                        },
                    );
                    ui.add_space(7.0);
                    ui.label(RichText::new(&path).size(15.0).strong().color(TEXT));
                });
            ui.add_space(8.0);
            ui.add_enabled_ui(tracked && !self.busy, |ui| {
                ui.horizontal_wrapped(|ui| {
                    if action(ui, "File history", false) {
                        self.queue(Job::Inspect(vec![
                            "log".into(),
                            "--follow".into(),
                            "--date=short".into(),
                            "--format=%h  %ad  %an  %s".into(),
                            "--".into(),
                            path.clone(),
                        ]));
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
        } else if let Some(snapshot) = &self.snapshot {
            egui::Frame::new()
                .fill(ELEVATED)
                .corner_radius(egui::CornerRadius::same(10))
                .stroke(Stroke::new(1.0, BORDER))
                .inner_margin(egui::Margin::same(16))
                .show(ui, |ui| {
                    ui.label(
                        RichText::new("REPOSITORY PULSE")
                            .size(10.0)
                            .strong()
                            .color(MUTED),
                    );
                    ui.add_space(7.0);
                    ui.label(
                        RichText::new(&snapshot.branch)
                            .size(18.0)
                            .strong()
                            .color(ACCENT),
                    );
                    ui.label(
                        RichText::new(format!(
                            "{} commits  |  {} changed files",
                            snapshot.commits.len(),
                            snapshot.status.len()
                        ))
                        .size(11.0)
                        .color(MUTED),
                    );
                    ui.label(
                        RichText::new(format!("{} remotes", snapshot.remotes.len()))
                            .size(11.0)
                            .color(MUTED),
                    );
                });
            ui.add_space(15.0);
            ui.label(
                RichText::new("Pick a commit or file to see its details here.")
                    .size(12.0)
                    .color(MUTED),
            );
        } else {
            ui.label(RichText::new("Open a repository to inspect its history.").color(MUTED));
        }
        if !self.detail.is_empty() {
            ui.add_space(17.0);
            ui.label(
                RichText::new("CHANGE DETAILS")
                    .size(10.0)
                    .strong()
                    .color(MUTED),
            );
            ui.add_space(5.0);
            let hunks = if self.selected_file.is_some() {
                git::diff_hunks(&self.detail)
            } else {
                Vec::new()
            };
            egui::ScrollArea::both().show(ui, |ui| {
                ui.style_mut().spacing.item_spacing.y = 2.0;
                if hunks.is_empty() {
                    for line in self.detail.lines().take(2500) {
                        diff_line(ui, line);
                    }
                } else {
                    for hunk in hunks {
                        egui::Frame::new()
                            .fill(PANEL_ALT)
                            .corner_radius(egui::CornerRadius::same(7))
                            .inner_margin(egui::Margin::symmetric(8, 5))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.label(
                                        RichText::new(&hunk.heading)
                                            .monospace()
                                            .size(10.5)
                                            .color(VIOLET),
                                    );
                                    if ui
                                        .add_enabled(
                                            !self.busy,
                                            egui::Button::new(if self.selected_file_staged {
                                                "Unstage hunk"
                                            } else {
                                                "Stage hunk"
                                            }),
                                        )
                                        .clicked()
                                    {
                                        self.hunk_action = true;
                                        self.queue(Job::ApplyHunk(
                                            hunk.patch.clone(),
                                            self.selected_file_staged,
                                        ));
                                    }
                                });
                            });
                        for line in hunk.lines.iter().take(400) {
                            diff_line(ui, line);
                        }
                        ui.add_space(9.0);
                    }
                }
            });
        }
    }

    fn dialogs(&mut self, ctx: &egui::Context) {
        if self.show_clone {
            let mut open = true;
            egui::Window::new("Clone repository")
                .open(&mut open)
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.label("Repository URL or local path");
                    ui.add(egui::TextEdit::singleline(&mut self.clone_url).desired_width(450.0));
                    ui.label("Destination folder");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.clone_destination)
                            .desired_width(450.0),
                    );
                    if action(ui, "Choose parent folder...", false)
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
                    if ui
                        .add_enabled(
                            !self.clone_url.trim().is_empty()
                                && !self.clone_destination.trim().is_empty(),
                            egui::Button::new("Clone"),
                        )
                        .clicked()
                    {
                        self.queue(Job::Clone(
                            self.clone_url.trim().to_owned(),
                            PathBuf::from(self.clone_destination.trim()),
                        ));
                        self.show_clone = false;
                    }
                });
            self.show_clone &= open;
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
                        if ui.add(egui::Button::new("Discard").fill(RED)).clicked() {
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
                        if ui.add(egui::Button::new(verb).fill(ACCENT)).clicked() {
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
                        if ui.add(egui::Button::new("Delete").fill(RED)).clicked() {
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
                            .add_enabled(enabled, egui::Button::new("Reset HEAD").fill(RED))
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
                            .add(egui::Button::new(format!("Use {label}")).fill(RED))
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
                        if ui.add(egui::Button::new("Abort merge").fill(RED)).clicked() {
                            self.git(&["merge", "--abort"]);
                            self.confirm_abort_merge = false;
                        }
                    });
                });
        }
    }
}

impl eframe::App for GitVibe {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, "recent_repos", &self.recent_repos);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.poll();
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
        egui::Panel::top("top")
            .frame(
                egui::Frame::new()
                    .fill(PANEL)
                    .stroke(Stroke::new(1.0, BORDER))
                    .inner_margin(egui::Margin::symmetric(22, 13)),
            )
            .show(ui, |ui| self.toolbar(ui));
        egui::Panel::bottom("status")
            .frame(
                egui::Frame::new()
                    .fill(PANEL)
                    .stroke(Stroke::new(1.0, BORDER))
                    .inner_margin(egui::Margin::symmetric(16, 7)),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    if self.error.is_empty() {
                        ui.label(
                            RichText::new(if self.busy { "Working..." } else { "Ready" })
                                .color(ACCENT),
                        );
                    } else {
                        ui.label(RichText::new(format!("Error: {}", self.error)).color(RED));
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            RichText::new(format!(
                                "GitVibe {}  |  Made for every desktop",
                                env!("CARGO_PKG_VERSION")
                            ))
                            .size(10.0)
                            .color(MUTED),
                        );
                    });
                });
            });
        egui::Panel::left("nav")
            .resizable(true)
            .default_size(252.0)
            .min_size(220.0)
            .frame(
                egui::Frame::new()
                    .fill(PANEL)
                    .stroke(Stroke::new(1.0, BORDER))
                    .inner_margin(egui::Margin::same(16)),
            )
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| self.sidebar(ui));
            });
        egui::Panel::right("inspector")
            .resizable(true)
            .default_size(350.0)
            .min_size(260.0)
            .frame(
                egui::Frame::new()
                    .fill(PANEL)
                    .stroke(Stroke::new(1.0, BORDER))
                    .inner_margin(egui::Margin::same(17)),
            )
            .show(ui, |ui| self.inspector(ui));
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(BG)
                    .inner_margin(egui::Margin::same(22)),
            )
            .show(ui, |ui| match self.page {
                Page::History => self.history(ui),
                Page::Changes => self.changes(ui),
                Page::Branches => self.branches(ui),
                Page::Stashes => self.stashes(ui),
                Page::Console => self.console(ui),
            });
        self.dialogs(&ctx);
        self.launch(&ctx);
    }
}

fn lane_color(lane: usize) -> Color32 {
    [ACCENT, ORANGE, VIOLET, BLUE, RED][lane % 5]
}

fn pick_folder(start: Option<&Path>) -> Option<PathBuf> {
    let mut dialog = rfd::FileDialog::new();
    if let Some(path) = start.filter(|path| path.is_dir()) {
        dialog = dialog.set_directory(path);
    }
    dialog.pick_folder()
}

fn diff_line(ui: &mut egui::Ui, line: &str) {
    let color = if line.starts_with("@@") || line.starts_with("diff --git") {
        VIOLET
    } else if line.starts_with('+') && !line.starts_with("+++") {
        ACCENT
    } else if line.starts_with('-') && !line.starts_with("---") {
        RED
    } else if line.starts_with("commit ")
        || line.starts_with("Author:")
        || line.starts_with("Date:")
    {
        BLUE
    } else {
        MUTED
    };
    ui.add(
        egui::Label::new(RichText::new(line).monospace().size(11.0).color(color)).selectable(true),
    );
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
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 29.0), egui::Sense::click());
    let painter = ui.painter();
    painter.rect_filled(
        rect,
        0.0,
        if selected {
            ELEVATED
        } else if row.is_multiple_of(2) {
            BG
        } else {
            PANEL
        },
    );
    painter.line_segment(
        [rect.left_bottom(), rect.right_bottom()],
        Stroke::new(0.5, BORDER),
    );
    if compare_base {
        painter.rect_filled(
            egui::Rect::from_min_size(rect.left_top(), egui::vec2(3.0, rect.height())),
            0.0,
            ORANGE,
        );
    }
    if response.hovered() && !selected {
        painter.rect_filled(
            egui::Rect::from_min_size(rect.left_top(), egui::vec2(2.0, rect.height())),
            0.0,
            ACCENT,
        );
    }

    let lane_x = |lane: usize| rect.left() + 115.0 + lane.min(9) as f32 * 17.0;
    let node_x = lane_x(commit.lane);
    let mid_y = rect.center().y;
    if show_edges {
        for &(from, to) in &commit.graph_edges {
            painter.line_segment(
                [
                    egui::pos2(lane_x(from), rect.top()),
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
    painter.circle_filled(egui::pos2(node_x, mid_y), 6.4, BG);
    painter.circle_filled(
        egui::pos2(node_x, mid_y),
        if commit.parents.len() > 1 { 5.6 } else { 4.6 },
        lane_color(commit.lane),
    );

    let names = refs
        .iter()
        .filter(|(target, _, _)| target == &commit.short)
        .collect::<Vec<_>>();
    if let Some((_, name, color)) = names.first() {
        let mut label = name.chars().take(13).collect::<String>();
        if names.len() > 1 {
            label.push_str(" +");
        }
        let width = (label.chars().count() as f32 * 5.9 + 14.0).min(104.0);
        let pill = egui::Rect::from_min_size(
            egui::pos2(rect.left() + 5.0, mid_y - 9.0),
            egui::vec2(width, 18.0),
        );
        painter.rect_filled(pill, 3.0, PANEL_ALT);
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
        Stroke::new(1.0, BORDER),
    );
    let available = (rect.width() - graph_width - 125.0).max(30.0);
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
        if selected { ACCENT } else { TEXT },
    );
    painter.text(
        egui::pos2(rect.right() - 8.0, mid_y),
        egui::Align2::RIGHT_CENTER,
        &commit.date,
        egui::FontId::proportional(10.5),
        MUTED,
    );
    response
}

fn action(ui: &mut egui::Ui, label: &str, prominent: bool) -> bool {
    ui.add(
        egui::Button::new(
            RichText::new(label)
                .size(12.5)
                .strong()
                .color(if prominent { BG } else { TEXT }),
        )
        .fill(if prominent { ACCENT } else { PANEL_ALT })
        .stroke(if prominent {
            Stroke::NONE
        } else {
            Stroke::new(1.0, BORDER)
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
    ui.label(RichText::new(eyebrow).size(11.0).strong().color(ACCENT));
    ui.label(RichText::new(title).size(25.0).strong().color(TEXT));
    ui.label(RichText::new(subtitle).size(12.0).color(MUTED));
}

fn brand_icon(ui: &mut egui::Ui) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(36.0, 36.0), egui::Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, 9.0, ACCENT);
    let left = rect.left() + 13.0;
    let right = rect.left() + 25.0;
    painter.line_segment(
        [
            egui::pos2(left, rect.top() + 8.0),
            egui::pos2(left, rect.bottom() - 8.0),
        ],
        Stroke::new(2.5, BG),
    );
    painter.line_segment(
        [
            egui::pos2(left, rect.center().y),
            egui::pos2(right, rect.center().y + 6.0),
        ],
        Stroke::new(2.5, BG),
    );
    for pos in [
        egui::pos2(left, rect.top() + 8.0),
        egui::pos2(left, rect.bottom() - 8.0),
        egui::pos2(right, rect.center().y + 6.0),
    ] {
        painter.circle_filled(pos, 3.5, BG);
    }
}

fn nav_icon(ui: &mut egui::Ui, page: Page, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(18.0, 18.0), egui::Sense::hover());
    let painter = ui.painter();
    let x = rect.left();
    let y = rect.top();
    let stroke = Stroke::new(1.7, color);
    match page {
        Page::History => {
            painter.line_segment(
                [egui::pos2(x + 9.0, y + 2.0), egui::pos2(x + 9.0, y + 16.0)],
                stroke,
            );
            for offset in [3.0, 9.0, 15.0] {
                painter.circle_filled(egui::pos2(x + 9.0, y + offset), 2.4, color);
            }
        }
        Page::Changes => {
            painter.rect_stroke(
                egui::Rect::from_min_size(egui::pos2(x + 4.0, y + 2.0), egui::vec2(10.0, 14.0)),
                2.0,
                stroke,
                egui::StrokeKind::Inside,
            );
            for offset in [6.0, 10.0, 14.0] {
                painter.line_segment(
                    [
                        egui::pos2(x + 7.0, y + offset),
                        egui::pos2(x + 12.0, y + offset),
                    ],
                    stroke,
                );
            }
        }
        Page::Branches => {
            painter.line_segment(
                [egui::pos2(x + 4.0, y + 3.0), egui::pos2(x + 4.0, y + 15.0)],
                stroke,
            );
            painter.line_segment(
                [egui::pos2(x + 4.0, y + 11.0), egui::pos2(x + 13.0, y + 5.0)],
                stroke,
            );
            for pos in [
                egui::pos2(x + 4.0, y + 3.0),
                egui::pos2(x + 4.0, y + 15.0),
                egui::pos2(x + 13.0, y + 5.0),
            ] {
                painter.circle_filled(pos, 2.3, color);
            }
        }
        Page::Stashes => {
            painter.rect_stroke(
                egui::Rect::from_min_size(egui::pos2(x + 2.0, y + 8.0), egui::vec2(14.0, 7.0)),
                2.0,
                stroke,
                egui::StrokeKind::Inside,
            );
            painter.line_segment(
                [egui::pos2(x + 4.0, y + 5.0), egui::pos2(x + 14.0, y + 5.0)],
                stroke,
            );
        }
        Page::Console => {
            painter.line_segment(
                [egui::pos2(x + 3.0, y + 4.0), egui::pos2(x + 8.0, y + 9.0)],
                stroke,
            );
            painter.line_segment(
                [egui::pos2(x + 8.0, y + 9.0), egui::pos2(x + 3.0, y + 14.0)],
                stroke,
            );
            painter.line_segment(
                [
                    egui::pos2(x + 10.0, y + 14.0),
                    egui::pos2(x + 16.0, y + 14.0),
                ],
                stroke,
            );
        }
    }
}
