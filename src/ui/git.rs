
use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;

use gtk::prelude::*;

use crate::git::{self, GitFile, GitStatus};
use crate::ui::dialog;

pub struct GitUi {
    pub root: gtk::Box,
    repo: RefCell<PathBuf>,
    branch: gtk::Label,
    meta: gtk::Label,
    status: gtk::Label,
    list: gtk::Box,
    empty: gtk::Label,
    diff_title: gtk::Label,
    diff_view: gtk::TextView,
    commit_entry: gtk::Entry,
    commit_button: gtk::Button,
    push_button: gtk::Button,
    refresh_button: gtk::Button,
    stage_all_button: gtk::Button,
    busy: Cell<bool>,
    diff_seq: Cell<u64>,
    selected: RefCell<Option<String>>,
    files: RefCell<Vec<GitFile>>,
    count: Cell<usize>,
    on_count: RefCell<Option<Box<dyn Fn(usize)>>>,
}

impl GitUi {
    pub fn build(repo: PathBuf) -> Rc<Self> {
        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        root.add_css_class("git-page");
        root.set_hexpand(true);
        root.set_vexpand(true);

        let inner = gtk::Box::new(gtk::Orientation::Vertical, 12);
        inner.add_css_class("git-inner");
        inner.set_hexpand(true);
        inner.set_vexpand(true);

        let title = gtk::Label::new(Some("Git"));
        title.add_css_class("panel-title");
        title.set_xalign(0.0);

        let branch = gtk::Label::new(Some("—"));
        branch.add_css_class("git-branch");
        branch.set_xalign(0.0);

        let meta = gtk::Label::new(None);
        meta.add_css_class("git-meta");
        meta.set_xalign(0.0);
        meta.set_ellipsize(gtk::pango::EllipsizeMode::Middle);

        let heading = gtk::Box::new(gtk::Orientation::Vertical, 4);
        heading.append(&title);
        let branch_line = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        branch_line.append(&branch);
        branch_line.append(&meta);
        heading.append(&branch_line);
        inner.append(&heading);

        let refresh_button = gtk::Button::with_label("update");
        refresh_button.add_css_class("git-btn");
        let stage_all_button = gtk::Button::with_label("Internship all");
        stage_all_button.add_css_class("git-btn");
        stage_all_button.set_tooltip_text(Some("git add -A (prepare everything that appears in the list)"));
        stage_all_button.set_sensitive(false);
        let push_button = gtk::Button::with_label("send");
        push_button.add_css_class("git-btn");
        push_button.set_sensitive(false);
        let toolbar = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        toolbar.set_halign(gtk::Align::End);
        toolbar.append(&stage_all_button);
        toolbar.append(&refresh_button);
        toolbar.append(&push_button);
        inner.append(&toolbar);

        let status = gtk::Label::new(None);
        status.add_css_class("git-status");
        status.set_xalign(0.0);
        status.set_wrap(true);
        status.set_visible(false);
        inner.append(&status);

        let list = gtk::Box::new(gtk::Orientation::Vertical, 0);
        list.add_css_class("git-list");
        let empty = gtk::Label::new(Some("clean working tree"));
        empty.add_css_class("git-empty");
        empty.set_xalign(0.0);
        empty.set_wrap(true);
        empty.set_max_width_chars(64);
        empty.set_visible(false);
        list.append(&empty);

        let list_scroll = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vscrollbar_policy(gtk::PolicyType::Automatic)
            .child(&list)
            .build();
        list_scroll.add_css_class("git-list-scroll");
        list_scroll.set_min_content_width(300);

        let diff_title = gtk::Label::new(Some("DIFFERENCE"));
        diff_title.add_css_class("git-diff-title");
        diff_title.set_xalign(0.0);
        diff_title.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
        let diff_view = gtk::TextView::new();
        diff_view.add_css_class("git-diff");
        diff_view.set_editable(false);
        diff_view.set_monospace(true);
        diff_view.set_wrap_mode(gtk::WrapMode::None);
        diff_view.set_left_margin(8);
        diff_view.set_right_margin(8);
        diff_view.set_top_margin(6);
        diff_view.set_bottom_margin(6);
        diff_view.buffer().set_text("select a file to view the diff");
        let diff_scroll = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Automatic)
            .vscrollbar_policy(gtk::PolicyType::Automatic)
            .child(&diff_view)
            .build();
        diff_scroll.add_css_class("git-diff-scroll");
        diff_scroll.set_vexpand(true);
        diff_view.set_vexpand(true);

        let diff_box = gtk::Box::new(gtk::Orientation::Vertical, 6);
        diff_box.set_margin_start(12);
        diff_box.set_hexpand(true);
        diff_box.set_vexpand(true);
        diff_box.append(&diff_title);
        diff_box.append(&diff_scroll);

        let split = gtk::Paned::new(gtk::Orientation::Horizontal);
        split.add_css_class("git-split");
        split.set_position(340);
        split.set_wide_handle(false);
        split.set_resize_start_child(false);
        split.set_shrink_start_child(false);
        split.set_start_child(Some(&list_scroll));
        split.set_end_child(Some(&diff_box));
        split.set_vexpand(true);
        inner.append(&split);

        let commit_entry = gtk::Entry::new();
        commit_entry.add_css_class("git-commit-entry");
        commit_entry.set_placeholder_text(Some("confirmation message (only what is prepared)"));
        commit_entry.set_hexpand(true);
        let commit_button = gtk::Button::with_label("confirmar");
        commit_button.add_css_class("git-btn");
        commit_button.add_css_class("accent");
        let commit_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        commit_row.append(&commit_entry);
        commit_row.append(&commit_button);
        inner.append(&commit_row);

        root.append(&inner);

        let this = Rc::new(Self {
            root,
            repo: RefCell::new(repo),
            branch,
            meta,
            status,
            list,
            empty,
            diff_title,
            diff_view,
            commit_entry,
            commit_button,
            push_button,
            refresh_button,
            stage_all_button,
            busy: Cell::new(false),
            diff_seq: Cell::new(0),
            selected: RefCell::new(None),
            files: RefCell::new(Vec::new()),
            count: Cell::new(0),
            on_count: RefCell::new(None),
        });
        this.wire();
        this.refresh();
        this
    }

    fn wire(self: &Rc<Self>) {
        {
            let me = self.clone();
            self.refresh_button.connect_clicked(move |_| me.refresh());
        }
        {
            let me = self.clone();
            self.stage_all_button
                .connect_clicked(move |_| me.run_action(git::add_all_args()));
        }
        {
            let me = self.clone();
            self.push_button.connect_clicked(move |_| me.push());
        }
        {
            let me = self.clone();
            self.commit_button.connect_clicked(move |_| me.commit());
        }
        {
            let me = self.clone();
            self.commit_entry.connect_activate(move |_| me.commit());
        }
    }

    pub fn set_on_count(&self, callback: impl Fn(usize) + 'static) {
        *self.on_count.borrow_mut() = Some(Box::new(callback));
        self.notify_count();
    }

    fn notify_count(&self) {
        if let Some(callback) = self.on_count.borrow().as_ref() {
            callback(self.count.get());
        }
    }

    pub fn set_repo(&self, repo: PathBuf) {
        if *self.repo.borrow() != repo {
            *self.repo.borrow_mut() = repo;
            *self.selected.borrow_mut() = None;
        }
    }

    fn set_status(&self, text: &str, error: bool) {
        self.status.set_text(text);
        if error {
            self.status.remove_css_class("ok");
            self.status.add_css_class("error");
        } else {
            self.status.remove_css_class("error");
            self.status.add_css_class("ok");
        }
        self.status.set_visible(!text.trim().is_empty());
    }

    pub fn refresh(self: &Rc<Self>) {
        if self.busy.get() {
            return;
        }
        self.busy.set(true);
        self.refresh_button.set_sensitive(false);
        self.set_status("I'm on git...", false);
        let repo = self.repo.borrow().clone();
        let args = git::status_args().into_iter().map(str::to_string).collect();
        let me = self.clone();
        run_git(repo, args, move |result| {
            me.busy.set(false);
            me.refresh_button.set_sensitive(true);
            match result {
                Ok(text) => {
                    let status = git::parse_status(&text);
                    me.render(&status);
                    me.set_status("", false);
                }
                Err(err) => {
                    me.branch.set_text("—");
                    me.meta.set_text("workspace is not a git repository");
                    me.push_button.set_sensitive(false);
                    me.stage_all_button.set_sensitive(false);
                    me.commit_button.set_sensitive(false);
                    me.count.set(0);
                    me.notify_count();
                    me.files.borrow_mut().clear();
                    me.clear_list();
                    me.empty.set_text(
                        "no git repository in this workspace — open a git project or change the workspace in settings",
                    );
                    me.empty.set_visible(true);
                    me.set_status(&err, true);
                }
            }
        });
    }

    fn render(self: &Rc<Self>, status: &GitStatus) {
        self.branch.set_text(&status.branch);
        let mut meta = Vec::new();
        if !status.upstream.is_empty() {
            meta.push(format!("↑{} ↓{}", status.ahead, status.behind));
        }
        meta.push(format!("{} file(s)", status.changed_count()));
        if status.staged_count() > 0 {
            meta.push(format!("{} preparados", status.staged_count()));
        }
        self.meta.set_text(&meta.join(" · "));
        self.push_button.set_sensitive(!status.upstream.is_empty() && status.ahead > 0);
        self.push_button.set_tooltip_text(Some(if status.upstream.is_empty() {
            "no upstream configured; use git push -u in the terminal"
        } else if status.ahead == 0 {
            "Nothing to send"
        } else {
            "git push"
        }));
        self.commit_button.set_sensitive(status.staged_count() > 0);
        self.stage_all_button.set_sensitive(!status.clean());
        *self.files.borrow_mut() = status.files.clone();
        self.count.set(status.files.len());
        self.notify_count();
        self.clear_list();
        self.empty.set_text("clean working tree");
        self.empty.set_visible(status.clean());
        if status.clean() {
            return;
        }
        for file in &status.files {
            self.list.append(&self.file_row(file));
        }
    }

    fn clear_list(&self) {
        let mut child = self.list.first_child();
        while let Some(widget) = child {
            let next = widget.next_sibling();
            if widget != self.empty {
                self.list.remove(&widget);
            }
            child = next;
        }
    }

    fn file_row(self: &Rc<Self>, file: &GitFile) -> gtk::Box {
        let row = gtk::Box::new(gtk::Orientation::Vertical, 5);
        row.add_css_class("git-file");

        let top = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let chip = gtk::Label::new(Some(file.label()));
        chip.add_css_class("git-chip");
        chip.add_css_class(match file.label() {
            "staged" => "staged",
            "new" => "new",
            "partial" => "partial",
            "removed" => "deleted",
            "renamed" => "renamed",
            _ => "modified",
        });
        top.append(&chip);

        let path = gtk::Label::new(Some(&file.path));
        path.add_css_class("git-file-path");
        path.set_xalign(0.0);
        path.set_hexpand(true);
        path.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
        path.set_tooltip_text(Some(
            file.old_path
                .as_ref()
                .map(|old| format!("{old} → {}", file.path))
                .unwrap_or_else(|| file.path.clone())
                .as_str(),
        ));
        top.append(&path);
        row.append(&top);

        let actions = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        actions.set_halign(gtk::Align::Start);

        let diff = gtk::Button::with_label("difference");
        diff.add_css_class("git-mini");
        actions.append(&diff);
        {
            let me = self.clone();
            let file = file.clone();
            diff.connect_clicked(move |_| me.show_diff(&file));
        }

        let action = if file.staged() && !file.unstaged() {
            Some(("retirar", false))
        } else if file.untracked() || file.unstaged() {
            Some(("stage", true))
        } else {
            None
        };
        if let Some((label, is_stage)) = action {
            let button = gtk::Button::with_label(label);
            button.add_css_class("git-mini");
            actions.append(&button);
            let me = self.clone();
            let file = file.clone();
            button.connect_clicked(move |_| {
                let args = if is_stage {
                    git::add_args(&file)
                } else {
                    git::unstage_args(&file)
                };
                me.run_action(args);
            });
        }

        if file.unstaged() || file.untracked() {
            let discard = gtk::Button::with_label("descartar");
            discard.add_css_class("git-mini");
            discard.add_css_class("danger");
            actions.append(&discard);
            let me = self.clone();
            let file = file.clone();
            discard.connect_clicked(move |_| {
                let Some(window) = me
                    .root
                    .root()
                    .and_then(|root| root.downcast::<gtk::Window>().ok())
                else {
                    return;
                };
                let detail = if file.untracked() {
                    format!("{} (new file: will be deleted)", file.path)
                } else {
                    format!("{} (loses the unstaged change)", file.path)
                };
                let me = me.clone();
                let file = file.clone();
                dialog::confirm(
                    &window,
                    "Does the roll out change?",
                    &format!("{detail}\n\ncannot be undone."),
                    "descartar",
                    move || {
                        let args = git::discard_args(&file);
                        me.run_action(args);
                    },
                );
            });
        }
        row.append(&actions);
        row
    }

    pub fn preview_first(self: &Rc<Self>) {
        let file = self.files.borrow().first().cloned();
        if let Some(file) = file {
            self.show_diff(&file);
        }
    }

    fn show_diff(self: &Rc<Self>, file: &GitFile) {
        let staged = !file.unstaged() && file.staged();
        *self.selected.borrow_mut() = Some(file.path.clone());
        let seq = self.diff_seq.get() + 1;
        self.diff_seq.set(seq);
        self.diff_title.set_text(&format!(
            "DIFF · {}{}",
            file.path,
            if staged { "(ready)" } else { "" }
        ));
        self.diff_view
            .buffer()
            .set_text("I'm reading diff...");
        let repo = self.repo.borrow().clone();
        let args = git::diff_args(file, staged);
        let me = self.clone();
        run_git(repo, args, move |result| {
            if me.diff_seq.get() != seq {
                return;
            }
            let buffer = me.diff_view.buffer();
            match result {
                Ok(text) if text.trim().is_empty() => {
                    buffer.set_text("no diff (empty file, binary or metadata only)");
                }
                Ok(text) => {
                    if std::env::var("PI_DESKTOP_DEBUG_GIT").is_ok() {
                        eprintln!("git diff: {} bytes, {:?}", text.len(), text.chars().take(80).collect::<String>());
                    }
                    let clipped = if text.chars().count() > 200_000 {
                        format!(
                            "{}…\n\n[diff truncated at 200k characters]",
                            text.chars().take(200_000).collect::<String>()
                        )
                    } else {
                        text
                    };
                    buffer.set_text(&clipped);
                    let mut start = buffer.start_iter();
                    me.diff_view.scroll_to_iter(&mut start, 0.0, false, 0.0, 0.0);
                }
                Err(err) => buffer.set_text(&format!("diff failure: {err}")),
            }
        });
    }

    fn run_action(self: &Rc<Self>, args: Vec<String>) {
        if self.busy.get() {
            return;
        }
        self.busy.set(true);
        self.set_status("Apply...", false);
        let repo = self.repo.borrow().clone();
        let me = self.clone();
        run_git(repo, args, move |result| {
            me.busy.set(false);
            match result {
                Ok(_) => me.refresh(),
                Err(err) => me.set_status(&err, true),
            }
        });
    }

    fn commit(self: &Rc<Self>) {
        if self.busy.get() {
            return;
        }
        let message = self.commit_entry.text().trim().to_string();
        if message.is_empty() {
            self.set_status("write the commit message", true);
            return;
        }
        self.busy.set(true);
        self.set_status("I'm committing...", false);
        let repo = self.repo.borrow().clone();
        let me = self.clone();
        let args = git::commit_args(&message);
        run_git(repo, args, move |result| {
            me.busy.set(false);
            match result {
                Ok(output) => {
                    me.commit_entry.set_text("");
                    me.set_status(
                        output.lines().next().unwrap_or("commit made").trim(),
                        false,
                    );
                    me.refresh();
                }
                Err(err) => me.set_status(&err, true),
            }
        });
    }

    fn push(self: &Rc<Self>) {
        if self.busy.get() {
            return;
        }
        self.busy.set(true);
        self.set_status("Sending...", false);
        let repo = self.repo.borrow().clone();
        let me = self.clone();
        run_git(repo, git::push_args(), move |result| {
            me.busy.set(false);
            match result {
                Ok(output) => {
                    let text = output.trim();
                    me.set_status(
                        if text.is_empty() { "push done" } else { text },
                        false,
                    );
                    me.refresh();
                }
                Err(err) => me.set_status(&err, true),
            }
        });
    }
}

fn run_git(repo: PathBuf, args: Vec<String>, done: impl FnOnce(Result<String, String>) + 'static) {
    let (tx, rx) = async_channel::bounded::<Result<String, String>>(1);
    std::thread::spawn(move || {
        let _ = tx.send_blocking(git::run(&repo, &args));
    });
    glib::spawn_future_local(async move {
        if let Ok(result) = rx.recv().await {
            done(result);
        }
    });
}
