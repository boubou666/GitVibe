use std::{
    path::PathBuf,
    sync::mpsc::{self, Receiver},
    thread,
};

use crate::git::{self, Snapshot};
use eframe::egui::{self, Color32, RichText, Stroke};

const BG: Color32 = Color32::from_rgb(15, 20, 29);
const PANEL: Color32 = Color32::from_rgb(23, 30, 41);
const PANEL_ALT: Color32 = Color32::from_rgb(29, 38, 51);
const TEXT: Color32 = Color32::from_rgb(222, 231, 242);
const MUTED: Color32 = Color32::from_rgb(126, 144, 164);
const ACCENT: Color32 = Color32::from_rgb(79, 190, 180);
const ORANGE: Color32 = Color32::from_rgb(245, 165, 94);
const RED: Color32 = Color32::from_rgb(232, 104, 116);

#[derive(PartialEq, Clone, Copy)]
enum Page {
    History,
    Changes,
    Branches,
    Stashes,
    Console,
}

enum Job {
    Open(PathBuf),
    Init(PathBuf),
    Clone(String, PathBuf),
    Run(Vec<String>),
    Inspect(Vec<String>),
    InspectFile(String),
    Refresh,
}

enum Done {
    Loaded(Result<Snapshot, String>),
    Ran(Result<(String, Snapshot), String>),
    Inspected(Result<String, String>),
}

pub struct GitVibe {
    repo: Option<PathBuf>,
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
    selected_file: Option<String>,
    detail: String,
    output: String,
    error: String,
    search: String,
    receiver: Option<Receiver<Done>>,
    pending: Option<Job>,
    busy: bool,
    show_clone: bool,
    confirm_discard: Option<String>,
    committing: bool,
}

impl GitVibe {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let mut visuals = egui::Visuals::dark();
        visuals.panel_fill = BG;
        visuals.window_fill = PANEL;
        visuals.extreme_bg_color = PANEL_ALT;
        visuals.widgets.inactive.bg_fill = PANEL_ALT;
        visuals.widgets.hovered.bg_fill = Color32::from_rgb(43, 59, 74);
        visuals.widgets.active.bg_fill = Color32::from_rgb(49, 85, 91);
        visuals.selection.bg_fill = Color32::from_rgb(40, 109, 110);
        cc.egui_ctx.set_visuals(visuals);
        cc.egui_ctx.style_mut_of(egui::Theme::Dark, |s| {
            s.spacing.item_spacing = egui::vec2(9.0, 9.0);
            s.spacing.button_padding = egui::vec2(12.0, 7.0);
        });
        let path = std::env::args()
            .nth(1)
            .or_else(|| {
                std::env::current_dir()
                    .ok()
                    .map(|p| p.to_string_lossy().to_string())
            })
            .unwrap_or_default();
        let mut app = Self {
            repo: None,
            snapshot: None,
            page: Page::History,
            path_input: path.clone(),
            clone_url: String::new(),
            clone_destination: String::new(),
            branch_input: String::new(),
            tag_input: String::new(),
            commit_message: String::new(),
            command_input: String::new(),
            selected_commit: None,
            selected_file: None,
            detail: String::new(),
            output: String::new(),
            error: String::new(),
            search: String::new(),
            receiver: None,
            pending: None,
            busy: false,
            show_clone: false,
            confirm_discard: None,
            committing: false,
        };
        if git::discover(&PathBuf::from(&path)).is_ok() {
            app.pending = Some(Job::Open(PathBuf::from(path)));
        }
        app
    }

    fn queue(&mut self, job: Job) {
        if !self.busy {
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
        let (sender, receiver) = mpsc::channel();
        self.receiver = Some(receiver);
        self.busy = true;
        let ctx = ctx.clone();
        thread::spawn(move || {
            let result = match job {
                Job::Open(path) => Done::Loaded(git::snapshot(&path)),
                Job::Init(path) => Done::Loaded(git::init(&path).and_then(|p| git::snapshot(&p))),
                Job::Clone(url, path) => {
                    Done::Loaded(git::clone_repo(&url, &path).and_then(|p| git::snapshot(&p)))
                }
                Job::Refresh => Done::Loaded(
                    repo.ok_or("No repository selected".to_owned())
                        .and_then(|p| git::snapshot(&p)),
                ),
                Job::Run(args) => Done::Ran(
                    repo.ok_or("No repository selected".to_owned())
                        .and_then(|p| {
                            let output = git::run_owned(&p, &args)?;
                            let snapshot = git::snapshot(&p)?;
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
                    self.repo = Some(snapshot.root.clone());
                    self.path_input = snapshot.root.to_string_lossy().into_owned();
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
                }
                Err(error) => {
                    self.error = error;
                    self.committing = false;
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
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("◆  GitVibe")
                    .size(23.0)
                    .strong()
                    .color(ACCENT),
            );
            ui.add_space(24.0);
            if let Some(snapshot) = &self.snapshot {
                ui.label(
                    RichText::new(
                        snapshot
                            .root
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy(),
                    )
                    .strong()
                    .color(TEXT),
                );
                ui.label(
                    RichText::new(format!(
                        "⌁ {}",
                        if snapshot.branch.is_empty() {
                            "detached HEAD"
                        } else {
                            &snapshot.branch
                        }
                    ))
                    .color(ACCENT),
                );
            } else {
                ui.label(RichText::new("Open a repository to begin").color(MUTED));
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_enabled_ui(self.repo.is_some() && !self.busy, |ui| {
                    if ui.button("↻ Refresh").clicked() {
                        self.queue(Job::Refresh);
                    }
                    if ui.button("↓ Fetch").clicked() {
                        self.git(&["fetch", "--all", "--prune"]);
                    }
                    if ui.button("↓ Pull").clicked() {
                        self.git(&["pull"]);
                    }
                    if ui.button("↑ Push").clicked() {
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
        ui.add_space(10.0);
        for (page, icon, title) in [
            (Page::History, "◉", "History"),
            (Page::Changes, "◧", "Changes"),
            (Page::Branches, "⑂", "Branches & tags"),
            (Page::Stashes, "▤", "Stashes"),
            (Page::Console, "❯", "Git console"),
        ] {
            let selected = self.page == page;
            if ui
                .selectable_label(
                    selected,
                    RichText::new(format!("{icon}   {title}")).size(15.0),
                )
                .clicked()
            {
                self.page = page;
            }
        }
        ui.separator();
        ui.label(RichText::new("REPOSITORY").small().strong().color(MUTED));
        ui.add(egui::TextEdit::singleline(&mut self.path_input).hint_text("Path to repository"));
        ui.horizontal(|ui| {
            if ui.button("Open").clicked() {
                self.queue(Job::Open(PathBuf::from(self.path_input.trim())));
            }
            if ui.button("Init here").clicked() {
                self.queue(Job::Init(PathBuf::from(self.path_input.trim())));
            }
        });
        if ui.button("Clone repository…").clicked() {
            self.show_clone = true;
        }
        ui.add_space(12.0);
        if let Some(snapshot) = &self.snapshot {
            ui.label(
                RichText::new("LOCAL BRANCHES")
                    .small()
                    .strong()
                    .color(MUTED),
            );
            for branch in snapshot.branches.iter().take(18) {
                let color = if branch.current { ACCENT } else { TEXT };
                if ui
                    .selectable_label(
                        false,
                        RichText::new(format!(
                            "{} {}",
                            if branch.current { "●" } else { "○" },
                            branch.name
                        ))
                        .color(color),
                    )
                    .clicked()
                {
                    self.page = Page::Branches;
                }
            }
            ui.add_space(10.0);
            ui.label(RichText::new("REMOTES").small().strong().color(MUTED));
            for remote in &snapshot.remotes {
                ui.label(RichText::new(format!("◈ {remote}")).color(TEXT));
            }
        }
    }

    fn history(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.heading("Commit history");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut self.search)
                        .hint_text("Search commits")
                        .desired_width(230.0),
                );
            });
        });
        ui.separator();
        let Some(snapshot) = &self.snapshot else {
            self.empty(ui);
            return;
        };
        if snapshot.commits.is_empty() {
            ui.label("No commits yet. Stage files and create your first commit.");
            return;
        }
        let refs = snapshot
            .branches
            .iter()
            .chain(snapshot.tags.iter())
            .map(|r| (r.target.clone(), r.name.clone()))
            .collect::<Vec<_>>();
        let commits = snapshot.commits.clone();
        let search = self.search.to_lowercase();
        egui::ScrollArea::vertical().show(ui, |ui| {
            for commit in commits.iter().filter(|c| {
                search.is_empty()
                    || format!("{} {} {}", c.subject, c.author, c.short)
                        .to_lowercase()
                        .contains(&search)
            }) {
                let selected = self.selected_commit.as_deref() == Some(&commit.id);
                let response = ui.allocate_ui_with_layout(
                    egui::vec2(ui.available_width(), 46.0),
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| {
                        let (rect, response) =
                            ui.allocate_exact_size(egui::vec2(114.0, 42.0), egui::Sense::click());
                        let painter = ui.painter();
                        for lane in 0..commit.lane_count.min(5) {
                            let x = rect.left() + 16.0 + lane as f32 * 19.0;
                            painter.line_segment(
                                [egui::pos2(x, rect.top()), egui::pos2(x, rect.bottom())],
                                Stroke::new(1.5, lane_color(lane)),
                            );
                        }
                        let x = rect.left() + 16.0 + commit.lane.min(5) as f32 * 19.0;
                        painter.circle_filled(
                            egui::pos2(x, rect.center().y),
                            if selected { 7.0 } else { 5.5 },
                            lane_color(commit.lane),
                        );
                        if response.clicked() {
                            self.select_commit(&commit.id);
                        }
                        ui.vertical(|ui| {
                            ui.horizontal(|ui| {
                                let title = RichText::new(&commit.subject)
                                    .strong()
                                    .color(if selected { ACCENT } else { TEXT });
                                if ui.selectable_label(selected, title).clicked() {
                                    self.select_commit(&commit.id);
                                }
                                for (_, name) in
                                    refs.iter().filter(|(id, _)| commit.short == *id).take(3)
                                {
                                    ui.label(
                                        RichText::new(name)
                                            .small()
                                            .color(ACCENT)
                                            .background_color(PANEL_ALT),
                                    );
                                }
                            });
                            ui.label(
                                RichText::new(format!(
                                    "{}  ·  {}  ·  {}",
                                    commit.short, commit.author, commit.date
                                ))
                                .small()
                                .color(MUTED),
                            );
                        });
                    },
                );
                if response.response.hovered() {
                    ui.painter().rect_stroke(
                        response.response.rect,
                        4.0,
                        Stroke::new(1.0, PANEL_ALT),
                        egui::StrokeKind::Inside,
                    );
                }
            }
        });
    }

    fn select_commit(&mut self, id: &str) {
        self.selected_commit = Some(id.to_owned());
        self.selected_file = None;
        self.queue(Job::Inspect(vec![
            "show".into(),
            "--stat".into(),
            "--format=fuller".into(),
            id.into(),
        ]));
    }

    fn changes(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.heading("Working tree");
            if let Some(snapshot) = &self.snapshot {
                ui.label(
                    RichText::new(format!("{} changed files", snapshot.status.len())).color(MUTED),
                );
            }
        });
        ui.separator();
        let Some(snapshot) = &self.snapshot else {
            self.empty(ui);
            return;
        };
        let files = snapshot.status.clone();
        if files.is_empty() {
            ui.label(RichText::new("✓  Working tree clean").color(ACCENT));
        }
        egui::ScrollArea::vertical().show(ui, |ui| {
            for file in files {
                ui.horizontal(|ui| {
                    let staged = file.staged();
                    ui.label(
                        RichText::new(format!("{}{}", file.index, file.worktree))
                            .monospace()
                            .color(if staged { ACCENT } else { ORANGE }),
                    );
                    if ui
                        .selectable_label(
                            self.selected_file.as_deref() == Some(&file.path),
                            &file.path,
                        )
                        .clicked()
                    {
                        self.selected_file = Some(file.path.clone());
                        self.selected_commit = None;
                        if file.index == '?' {
                            self.queue(Job::InspectFile(file.path.clone()));
                        } else {
                            let args = if staged {
                                vec!["diff", "--cached", "--", &file.path]
                            } else {
                                vec!["diff", "--", &file.path]
                            };
                            self.queue(Job::Inspect(args.iter().map(|s| s.to_string()).collect()));
                        }
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if file.unstaged() && ui.small_button("Stage").clicked() {
                            self.git_owned(vec!["add".into(), "--".into(), file.path.clone()]);
                        }
                        if staged && ui.small_button("Unstage").clicked() {
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
                        if file.unstaged()
                            && file.index != '?'
                            && ui.small_button("Discard…").clicked()
                        {
                            self.confirm_discard = Some(file.path.clone());
                        }
                    });
                });
                ui.separator();
            }
        });
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if ui.button("Stage all").clicked() {
                self.git(&["add", "-A"]);
            }
            if ui.button("Unstage all").clicked() {
                if self.snapshot.as_ref().is_some_and(|s| s.commits.is_empty()) {
                    self.git(&["rm", "-r", "--cached", "--", "."]);
                } else {
                    self.git(&["restore", "--staged", "."]);
                }
            }
        });
        ui.add(
            egui::TextEdit::multiline(&mut self.commit_message)
                .hint_text("Commit message")
                .desired_rows(3),
        );
        if ui
            .add_enabled(
                !self.commit_message.trim().is_empty(),
                egui::Button::new("Commit staged changes").fill(Color32::from_rgb(35, 105, 102)),
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
    }

    fn branches(&mut self, ui: &mut egui::Ui) {
        ui.heading("Branches & tags");
        ui.separator();
        let Some(snapshot) = &self.snapshot else {
            self.empty(ui);
            return;
        };
        let branches = snapshot.branches.clone();
        let tags = snapshot.tags.clone();
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(&mut self.branch_input).hint_text("New branch name"));
            if ui
                .add_enabled(
                    !self.branch_input.trim().is_empty(),
                    egui::Button::new("Create & switch"),
                )
                .clicked()
            {
                let name = self.branch_input.trim().to_owned();
                self.git_owned(vec!["switch".into(), "-c".into(), name]);
                self.branch_input.clear();
            }
        });
        ui.add_space(12.0);
        ui.label(RichText::new("LOCAL BRANCHES").small().color(MUTED));
        for branch in branches {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(if branch.current { "●" } else { "○" })
                        .color(if branch.current { ACCENT } else { MUTED }),
                );
                ui.label(RichText::new(&branch.name).strong().color(TEXT));
                ui.label(
                    RichText::new(&branch.target)
                        .monospace()
                        .small()
                        .color(MUTED),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if !branch.current && ui.small_button("Switch").clicked() {
                        self.git_owned(vec!["switch".into(), branch.name.clone()]);
                    }
                    if !branch.current && ui.small_button("Merge into current").clicked() {
                        self.git_owned(vec!["merge".into(), branch.name.clone()]);
                    }
                });
            });
            ui.separator();
        }
        ui.add_space(12.0);
        ui.label(RichText::new("TAGS").small().color(MUTED));
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(&mut self.tag_input).hint_text("New tag name"));
            if ui
                .add_enabled(
                    !self.tag_input.trim().is_empty(),
                    egui::Button::new("Create tag"),
                )
                .clicked()
            {
                let name = self.tag_input.trim().to_owned();
                self.git_owned(vec!["tag".into(), name]);
                self.tag_input.clear();
            }
        });
        for tag in tags {
            ui.label(RichText::new(format!("◇ {}   {}", tag.name, tag.target)).color(TEXT));
        }
    }

    fn stashes(&mut self, ui: &mut egui::Ui) {
        ui.heading("Stashes");
        ui.separator();
        let Some(snapshot) = &self.snapshot else {
            self.empty(ui);
            return;
        };
        let stashes = snapshot.stashes.clone();
        if ui.button("Stash current changes").clicked() {
            self.git(&["stash", "push", "-u"]);
        }
        ui.add_space(12.0);
        if stashes.is_empty() {
            ui.label(RichText::new("No stashes").color(MUTED));
        }
        for stash in stashes {
            let name = stash.split_whitespace().next().unwrap_or("").to_owned();
            ui.horizontal(|ui| {
                ui.label(&stash);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button("Apply").clicked() {
                        self.git_owned(vec!["stash".into(), "apply".into(), name.clone()]);
                    }
                    if ui.small_button("Pop").clicked() {
                        self.git_owned(vec!["stash".into(), "pop".into(), name.clone()]);
                    }
                    if ui.small_button("Show").clicked() {
                        self.queue(Job::Inspect(vec![
                            "stash".into(),
                            "show".into(),
                            "-p".into(),
                            name,
                        ]));
                    }
                });
            });
            ui.separator();
        }
    }

    fn console(&mut self, ui: &mut egui::Ui) {
        ui.heading("Git console");
        ui.label(RichText::new("Run any Git command in the selected repository. Arguments are passed directly to Git, without a shell.").color(MUTED));
        ui.separator();
        ui.horizontal(|ui| {
            let edit = ui.add(
                egui::TextEdit::singleline(&mut self.command_input)
                    .hint_text("git log --oneline -20")
                    .desired_width(ui.available_width() - 90.0),
            );
            let enter = edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if ui.button("Run").clicked() || enter {
                match git::split_command(&self.command_input) {
                    Ok(args) => {
                        self.git_owned(args);
                        self.command_input.clear();
                    }
                    Err(error) => self.error = error,
                }
            }
        });
        ui.add_space(12.0);
        ui.label(RichText::new("LAST OUTPUT").small().color(MUTED));
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.add(
                egui::TextEdit::multiline(&mut self.output)
                    .font(egui::TextStyle::Monospace)
                    .desired_width(f32::INFINITY)
                    .desired_rows(20)
                    .interactive(false),
            );
        });
    }

    fn empty(&self, ui: &mut egui::Ui) {
        ui.vertical_centered(|ui| {
            ui.add_space(100.0);
            ui.label(RichText::new("◇").size(42.0).color(ACCENT));
            ui.heading("Your repository, at a glance");
            ui.label(
                RichText::new("Open, initialize, or clone a repository from the sidebar.")
                    .color(MUTED),
            );
        });
    }

    fn inspector(&mut self, ui: &mut egui::Ui) {
        ui.add_space(8.0);
        ui.label(RichText::new("INSPECTOR").small().strong().color(MUTED));
        if let Some(id) = &self.selected_commit {
            ui.label(
                RichText::new(format!("Commit {}", &id[..id.len().min(10)]))
                    .strong()
                    .color(ACCENT),
            );
        }
        if let Some(path) = &self.selected_file {
            ui.label(RichText::new(path).strong().color(ACCENT));
        }
        ui.separator();
        if self.detail.is_empty() {
            ui.label(
                RichText::new("Select a commit, changed file, or stash to inspect it.")
                    .color(MUTED),
            );
        } else {
            egui::ScrollArea::both().show(ui, |ui| {
                ui.add(
                    egui::TextEdit::multiline(&mut self.detail)
                        .font(egui::TextStyle::Monospace)
                        .desired_width(f32::INFINITY)
                        .interactive(false),
                );
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
    }
}

impl eframe::App for GitVibe {
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
                    .inner_margin(egui::Margin::symmetric(18, 14)),
            )
            .show(ui, |ui| self.toolbar(ui));
        egui::Panel::bottom("status")
            .frame(
                egui::Frame::new()
                    .fill(PANEL)
                    .inner_margin(egui::Margin::symmetric(14, 6)),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    if self.error.is_empty() {
                        ui.label(
                            RichText::new(if self.busy { "Working…" } else { "Ready" })
                                .color(ACCENT),
                        );
                    } else {
                        ui.label(RichText::new(format!("⚠ {}", self.error)).color(RED));
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(RichText::new("GitVibe 0.1.0").small().color(MUTED));
                    });
                });
            });
        egui::Panel::left("nav")
            .resizable(true)
            .default_size(235.0)
            .min_size(190.0)
            .frame(
                egui::Frame::new()
                    .fill(PANEL)
                    .inner_margin(egui::Margin::same(14)),
            )
            .show(ui, |ui| self.sidebar(ui));
        egui::Panel::right("inspector")
            .resizable(true)
            .default_size(360.0)
            .min_size(220.0)
            .frame(
                egui::Frame::new()
                    .fill(PANEL)
                    .inner_margin(egui::Margin::same(14)),
            )
            .show(ui, |ui| self.inspector(ui));
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(BG)
                    .inner_margin(egui::Margin::same(18)),
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
    [
        ACCENT,
        ORANGE,
        Color32::from_rgb(145, 151, 245),
        RED,
        Color32::from_rgb(196, 131, 220),
    ][lane % 5]
}
