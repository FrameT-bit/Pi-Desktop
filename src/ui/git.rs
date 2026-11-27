use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{Duration, Instant};

use gtk::prelude::*;
use serde_json::json;

use crate::extension_ui::{Request, Response};
use crate::git::{self, GitFile, Snapshot};
use crate::ui::{chat, dialog};

mod workbench;
mod history;
mod repository;

pub struct GitUi {
    pub root: gtk::Box,
    repo: RefCell<PathBuf>,
    workspace: RefCell<PathBuf>,
    pinned: Cell<bool>,
    opening: Cell<bool>,
    open_seq: Cell<u64>,
    open_button: gtk::Button,
    workspace_button: gtk::Button,
    repo_label: gtk::Label,
    repo_controls: gtk::FlowBox,
    tabs: gtk::StackSwitcher,
    drafts: RefCell<std::collections::HashMap<PathBuf, String>>,
    branch: gtk::DropDown,
    more: gtk::MenuButton,
    syncing_branch: Cell<bool>,
    meta: gtk::Label,
    status: gtk::Label,
    groups: [gtk::ListBox; 2],
    group_titles: [gtk::Label; 2],
    group_empty: [gtk::Label; 2],
    stage_selected: [gtk::Button; 2],
    stage_all: [gtk::Button; 2],
    files: RefCell<[Vec<GitFile>; 2]>,
    filter: gtk::SearchEntry,
    rendering: Cell<bool>,
    destination: RefCell<Option<(String, bool)>>,
    stack: gtk::Stack,
    history: Rc<history::HistoryUi>,
    diff_title: gtk::Label,
    diff_view: gtk::Box,
    commit_entry: gtk::Entry,
    commit_button: gtk::Button,
    push_button: gtk::Button,
    operation_row: gtk::Box,
    continue_button: gtk::Button,
    abort_button: gtk::Button,
    busy: Cell<bool>,
    polling: Cell<bool>,
    refresh_pending: Cell<bool>,
    last_poll: Cell<Instant>,
    generation: Cell<u64>,
    diff_seq: Cell<u64>,
    diff_text: RefCell<Option<String>>,
    split: gtk::Paned,
    selected: RefCell<Option<(String, bool)>>,
    snapshot: RefCell<Option<Snapshot>>,
    count: Cell<usize>,
    on_count: RefCell<Option<Box<dyn Fn(usize)>>>,
}

fn button(label: &str) -> gtk::Button {
    let button = gtk::Button::with_label(label);
    button.add_css_class("git-btn");
    button
}

fn label(text: &str, class: &str) -> gtk::Label {
    let label = gtk::Label::new(Some(text));
    label.add_css_class(class);
    label.set_xalign(0.0);
    label.set_wrap(true);
    label
}

fn sync_summary(snapshot: &Snapshot) -> String {
    let status = &snapshot.status;
    if let Some(operation) = snapshot.operation {
        let count = status.files.iter().filter(|file| file.conflicted()).count();
        return format!("{} in progress{}", if operation == "rebase" { "Rebase" } else { "Merge" },
            if count == 0 { " · ready to continue".into() }
            else { format!(" · {count} conflict{} to resolve", if count == 1 { "" } else { "s" }) });
    }
    if !snapshot.has_head { return "No commits yet · stage files to make the first commit".into(); }
    if status.detached { return "Detached HEAD · switch to a branch before publishing".into(); }
    if status.upstream.is_empty() { return "Local branch · not published".into(); }
    if status.ahead == 0 && status.behind == 0 { return format!("Up to date with {}", status.upstream); }
    let mut parts = vec![status.upstream.clone()];
    if status.behind > 0 { parts.push(format!("{} to pull", status.behind)); }
    if status.ahead > 0 { parts.push(format!("{} to push", status.ahead)); }
    parts.join(" · ")
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
        let open_button = button("Open repository…");
        open_button.set_tooltip_text(Some("Choose a local Git repository without changing the chat workspace"));
        let workspace_button = button("Use workspace");
        workspace_button.set_tooltip_text(Some("Follow the chat workspace again"));
        workspace_button.set_visible(false);
        let repository_actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        repository_actions.append(&open_button);
        repository_actions.append(&workspace_button);
        let repo_label = label("", "git-meta");
        repo_label.set_wrap(false);
        repo_label.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
        repo_label.set_width_chars(1);
        repo_label.set_hexpand(true);
        let repository_header = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        repository_header.append(&repo_label);
        repository_header.append(&repository_actions);
        inner.append(&repository_header);

        let branch = gtk::DropDown::from_strings(&[]);
        branch.add_css_class("git-branches");
        branch.set_hexpand(true);
        let create = button("New branch");
        let heading = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        heading.append(&branch);
        inner.append(&heading);
        let meta = label("", "git-meta");
        inner.append(&meta);

        let refresh = button("Refresh");
        let fetch = button("Fetch");
        let pull = button("Pull");
        let push_button = button("Push");
        fetch.set_tooltip_text(Some("Check the remote for new commits without changing your files"));
        pull.set_tooltip_text(Some("Bring remote commits into this branch; fast-forward only"));
        let more = gtk::MenuButton::new();
        more.add_css_class("git-more");
        more.set_child(Some(&gtk::Label::new(Some("More…"))));
        more.set_tooltip_text(Some("New branch and manual refresh"));
        let menu = gtk::Box::new(gtk::Orientation::Vertical, 0);
        menu.append(&create);
        menu.append(&refresh);
        let popover = gtk::Popover::new();
        popover.set_child(Some(&menu));
        more.set_popover(Some(&popover));
        for action in [&create, &refresh] {
            let weak = popover.downgrade();
            action.connect_clicked(move |_| { if let Some(popover) = weak.upgrade() { popover.popdown(); } });
        }
        let toolbar = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        for button in [&fetch, &pull, &push_button] { toolbar.append(button); }
        toolbar.append(&more);
        let controls = gtk::FlowBox::new();
        controls.set_selection_mode(gtk::SelectionMode::None);
        controls.set_max_children_per_line(2);
        controls.set_min_children_per_line(1);
        inner.remove(&heading);
        controls.insert(&heading, -1);
        controls.insert(&toolbar, -1);
        inner.insert_child_after(&controls, Some(&repository_header));
        let status = label("", "git-status");
        status.set_visible(false);
        inner.append(&status);

        let operation_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let abort_button = button("Abort merge");
        let continue_button = button("Continue");
        operation_row.append(&abort_button);
        operation_row.append(&continue_button);
        operation_row.set_visible(false);
        inner.append(&operation_row);

        let list = gtk::Box::new(gtk::Orientation::Vertical, 8);
        list.add_css_class("git-list");
        let filter = gtk::SearchEntry::new();
        filter.set_placeholder_text(Some("Filter files"));
        filter.add_css_class("git-filter");
        list.append(&filter);
        let groups = [gtk::ListBox::new(), gtk::ListBox::new()];
        let group_titles = [label("Unstaged", "git-diff-title"), label("Staged", "git-diff-title")];
        group_titles[0].set_tooltip_text(Some("Working-tree edits not included in the next commit"));
        group_titles[1].set_tooltip_text(Some("Only these staged edits will be included in the next commit"));
        let group_empty = [label("No unstaged changes", "git-empty"), label("No files staged", "git-empty")];
        let stage_selected = [button("Stage"), button("Unstage")];
        let stage_all = [button("Stage all"), button("Unstage all")];
        for index in 0..2 {
            let heading = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            group_titles[index].set_hexpand(true);
            group_titles[index].set_wrap(false);
            group_titles[index].set_ellipsize(gtk::pango::EllipsizeMode::End);
            group_titles[index].set_width_chars(1);
            stage_selected[index].set_tooltip_text(Some("Move selected files · Ctrl/Shift to select · Space or Enter to move"));
            heading.append(&group_titles[index]);
            heading.append(&stage_selected[index]);
            heading.append(&stage_all[index]);
            list.append(&heading);
            groups[index].set_selection_mode(gtk::SelectionMode::Multiple);
            groups[index].add_css_class("git-file-list");
            groups[index].set_placeholder(Some(&group_empty[index]));
            let scroll = gtk::ScrolledWindow::builder()
                .hscrollbar_policy(gtk::PolicyType::Never)
                .vscrollbar_policy(gtk::PolicyType::Automatic)
                .child(&groups[index]).vexpand(true).build();
            scroll.add_css_class("git-list-scroll");
            list.append(&scroll);
        }
        let diff_title = label("DIFF", "git-diff-title");
        diff_title.set_wrap(false);
        diff_title.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
        diff_title.set_width_chars(1);
        let diff_view = gtk::Box::new(gtk::Orientation::Vertical, 0);
        diff_view.add_css_class("diff-body");
        let diff_scroll = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Automatic)
            .vscrollbar_policy(gtk::PolicyType::Automatic)
            .child(&diff_view).vexpand(true).hexpand(true).build();
        diff_scroll.add_css_class("git-diff-scroll");
        let diff_box = gtk::Box::new(gtk::Orientation::Vertical, 8);
        diff_box.set_hexpand(true);
        diff_box.set_vexpand(true);
        diff_box.append(&diff_title);
        diff_box.append(&diff_scroll);
        let split = gtk::Paned::new(gtk::Orientation::Horizontal);
        split.add_css_class("git-split");
        split.set_position(340);
        split.set_shrink_start_child(true);
        split.set_start_child(Some(&list));
        split.set_end_child(Some(&diff_box));
        split.set_vexpand(true);
        let weak_split = split.downgrade();
        let weak_root = root.downgrade();
        let weak_repository_header = repository_header.downgrade();
        glib::timeout_add_local(Duration::from_millis(120), move || {
            if let (Some(split), Some(root)) = (weak_split.upgrade(), weak_root.upgrade()) {
                if !root.is_mapped() || root.width() <= 1 { return glib::ControlFlow::Continue; }
                let orientation = if root.width() < 700 { gtk::Orientation::Vertical } else { gtk::Orientation::Horizontal };
                if let Some(header) = weak_repository_header.upgrade() { header.set_orientation(orientation); }
                if split.orientation() != orientation {
                    split.set_orientation(orientation);
                    split.set_position(if orientation == gtk::Orientation::Vertical { 240 } else { 340 });
                }
                glib::ControlFlow::Continue
            } else { glib::ControlFlow::Break }
        });
        let changes = gtk::Box::new(gtk::Orientation::Vertical, 8);
        changes.append(&split);
        let history = history::HistoryUi::build(repo.clone());
        let stack = gtk::Stack::new();
        stack.set_hhomogeneous(false);
        stack.set_vhomogeneous(false);
        stack.set_vexpand(true);
        stack.add_titled(&changes, Some("changes"), "Changes");
        stack.add_titled(&history.root, Some("history"), "History");
        let tabs = gtk::StackSwitcher::new();
        tabs.set_stack(Some(&stack));
        tabs.add_css_class("git-tabs");
        tabs.set_halign(gtk::Align::Start);
        inner.append(&tabs);
        inner.append(&stack);

        let commit_entry = gtk::Entry::new();
        commit_entry.add_css_class("git-commit-entry");
        commit_entry.set_placeholder_text(Some("Describe the staged changes"));
        commit_entry.set_hexpand(true);
        let commit_button = button("Commit");
        commit_button.add_css_class("accent");
        commit_button.set_sensitive(false);
        let commit_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        commit_row.append(&commit_entry);
        commit_row.append(&commit_button);
        changes.append(&commit_row);
        root.append(&inner);
        let this = Rc::new(Self {
            root, workspace: RefCell::new(repo.clone()), repo: RefCell::new(repo),
            pinned: Cell::new(false), opening: Cell::new(false), open_seq: Cell::new(0),
            open_button, workspace_button, repo_label, repo_controls: controls, tabs,
            drafts: RefCell::new(std::collections::HashMap::new()),
            branch, more, syncing_branch: Cell::new(false), meta, status,
            groups, group_titles, group_empty, stage_selected, stage_all, filter,
            files: RefCell::new([Vec::new(), Vec::new()]), rendering: Cell::new(false),
            destination: RefCell::new(None), stack, history,
            diff_title, diff_view, commit_entry, commit_button, push_button,
            operation_row, continue_button, abort_button, busy: Cell::new(false),
            polling: Cell::new(false), refresh_pending: Cell::new(false),
            last_poll: Cell::new(Instant::now()), generation: Cell::new(0),
            diff_seq: Cell::new(0), diff_text: RefCell::new(None), split, selected: RefCell::new(None), snapshot: RefCell::new(None),
            count: Cell::new(0), on_count: RefCell::new(None),
        });
        this.bind_workbench();
        this.bind_repository();
        let weak = Rc::downgrade(&this);
        this.stack.connect_visible_child_name_notify(move |stack| {
            if let Some(me) = weak.upgrade() {
                if stack.visible_child_name().as_deref() == Some("history") { me.history.refresh(); }
            }
        });
        let weak = Rc::downgrade(&this);
        refresh.connect_clicked(move |_| { if let Some(me) = weak.upgrade() { me.refresh(); } });
        let weak = Rc::downgrade(&this);
        fetch.connect_clicked(move |_| { if let Some(me) = weak.upgrade() { me.run_action(vec!["fetch".into()], "Fetching…"); } });
        let weak = Rc::downgrade(&this);
        pull.connect_clicked(move |_| { if let Some(me) = weak.upgrade() { me.run_action(vec!["pull".into(), "--ff-only".into()], "Pulling…"); } });
        let weak = Rc::downgrade(&this);
        this.push_button.connect_clicked(move |_| { if let Some(me) = weak.upgrade() { me.push(); } });
        let weak = Rc::downgrade(&this);
        this.commit_button.connect_clicked(move |_| { if let Some(me) = weak.upgrade() { me.commit(); } });
        let weak = Rc::downgrade(&this);
        this.commit_entry.connect_activate(move |_| { if let Some(me) = weak.upgrade() { me.commit(); } });
        let weak = Rc::downgrade(&this);
        this.commit_entry.connect_changed(move |_| { if let Some(me) = weak.upgrade() { me.update_commit(); } });
        let weak = Rc::downgrade(&this);
        create.connect_clicked(move |_| {
            let Some(me) = weak.upgrade() else { return };
            let Some(window) = me.window() else { return };
            let request = Request::parse(&json!({"type":"extension_ui_request","method":"input","title":"New branch","placeholder":"Branch name"})).unwrap();
            dialog::show(&window, &request, move |response| {
                if let Response::Value(name) = response { me.switch_branch(&name, true); }
            });
        });
        let weak = Rc::downgrade(&this);
        this.branch.connect_selected_notify(move |dropdown| {
            let Some(me) = weak.upgrade() else { return };
            if me.syncing_branch.get() { return; }
            let branch = dropdown.selected_item().and_downcast::<gtk::StringObject>().map(|item| item.string().to_string());
            if let Some(branch) = branch {
                // Restoring the model can queue a notify after syncing_branch
                // resets. Never interpret the already-selected branch as a new action.
                if me.busy.get() || me.snapshot.borrow().as_ref().is_some_and(|s| s.status.branch == branch) { return; }
                me.switch_branch(&branch, false);
                // Do not replace a DropDown model while its selection signal
                // is still being dispatched inside GTK.
                glib::idle_add_local_once(move || me.restore_branch());
            }
        });
        let weak = Rc::downgrade(&this);
        this.abort_button.connect_clicked(move |_| {
            let Some(me) = weak.upgrade() else { return };
            let operation = me.snapshot.borrow().as_ref().and_then(|s| s.operation);
            if let Some(operation) = operation {
                let args = vec![operation.into(), "--abort".into()];
                me.confirm("Abort operation?", "Uncommitted conflict resolutions may be lost.", "Abort", move |me| me.run_action(args.clone(), "Aborting…"));
            }
        });
        let weak = Rc::downgrade(&this);
        this.continue_button.connect_clicked(move |_| {
            let Some(me) = weak.upgrade() else { return };
            let operation = me.snapshot.borrow().as_ref().and_then(|s| s.operation);
            if let Some(operation) = operation {
                me.run_action(vec![operation.into(), "--continue".into()], "Continuing…");
            }
        });
        let weak = Rc::downgrade(&this);
        glib::timeout_add_seconds_local(1, move || {
            let Some(me) = weak.upgrade() else { return glib::ControlFlow::Break };
            let seconds = if me.root.is_mapped() { 3 } else { 15 };
            if me.last_poll.get().elapsed() >= Duration::from_secs(seconds) { me.refresh(); }
            glib::ControlFlow::Continue
        });
        this.clear_diff();
        this.refresh();
        this
    }

    fn window(&self) -> Option<gtk::Window> { self.root.root().and_downcast::<gtk::Window>() }

    pub(super) fn focus_filter(&self) {
        if self.stack.visible_child_name().as_deref() == Some("history") { self.history.focus_filter(); }
        else { self.filter.grab_focus(); }
    }

    fn confirm(self: &Rc<Self>, title: &str, text: &str, action: &str, done: impl Fn(Rc<Self>) + 'static) {
        if let Some(window) = self.window() {
            let me = self.clone();
            dialog::confirm(&window, title, text, action, move || done(me.clone()));
        }
    }

    pub fn set_on_count(&self, callback: impl Fn(usize) + 'static) {
        *self.on_count.borrow_mut() = Some(Box::new(callback));
        self.notify_count();
    }

    fn notify_count(&self) {
        if let Some(callback) = self.on_count.borrow().as_ref() { callback(self.count.get()); }
    }

    fn set_status(&self, text: &str, error: bool) {
        self.status.set_text(text);
        self.status.set_tooltip_text(None);
        self.status.remove_css_class(if error { "ok" } else { "error" });
        self.status.add_css_class(if error { "error" } else { "ok" });
        self.status.set_visible(!text.is_empty());
    }

    pub fn refresh(self: &Rc<Self>) {
        if self.busy.get() || self.polling.replace(true) {
            self.refresh_pending.set(true);
            return;
        }
        self.refresh_pending.set(false);
        self.last_poll.set(Instant::now());
        let generation = self.generation.get();
        let me = self.clone();
        task(self.repo.borrow().clone(), |folder| {
            let repo = git::repository_root(folder)?;
            git::snapshot(&repo).map(|snapshot| (repo, snapshot))
        }, move |result| {
            me.polling.set(false);
            if generation == me.generation.get() && !me.busy.get() {
                match result {
                    Ok((repo, snapshot)) => {
                        if *me.repo.borrow() != repo {
                            if me.commit_entry.text().is_empty() {
                                if let Some(draft) = me.drafts.borrow().get(&repo) { me.commit_entry.set_text(draft); }
                            }
                            *me.repo.borrow_mut() = repo.clone();
                            me.history.set_repo(repo);
                        }
                        let changed = me.snapshot.borrow().as_ref() != Some(&snapshot);
                        *me.snapshot.borrow_mut() = Some(snapshot.clone());
                        if changed { me.render(&snapshot); }
                        else {
                            let selected = me.selected.borrow().clone();
                            if let Some((path, staged)) = selected {
                                if let Some(file) = snapshot.status.files.iter().find(|file| file.path == path) {
                                    me.show_diff(file, staged);
                                }
                            }
                        }
                    }
                    Err(_) => {
                        *me.snapshot.borrow_mut() = None;
                        me.meta.set_text("No Git repository in this folder. Choose Open repository… to select another folder.");
                        me.render_files(&[]);
                        me.commit_button.set_sensitive(false);
                        me.push_button.set_sensitive(false);
                        me.operation_row.set_visible(false);
                        me.count.set(0);
                        me.notify_count();
                        me.clear_diff();
                    }
                }
                me.update_repository_controls();
            } else { me.refresh_pending.set(true); }
            if me.refresh_pending.get() && !me.busy.get() { me.refresh(); }
            if me.stack.visible_child_name().as_deref() == Some("history") { me.history.refresh(); }
        });
    }

    fn restore_branch(&self) {
        let snapshot = self.snapshot.borrow();
        let Some(snapshot) = snapshot.as_ref() else { return };
        let mut branches = snapshot.branches.clone();
        if !branches.contains(&snapshot.status.branch) { branches.insert(0, snapshot.status.branch.clone()); }
        self.syncing_branch.set(true);
        let current: Vec<String> = self.branch.model().map(|model| (0..model.n_items())
            .filter_map(|index| model.item(index).and_downcast::<gtk::StringObject>().map(|item| item.string().to_string()))
            .collect()).unwrap_or_default();
        if current != branches {
            self.branch.set_model(Some(&gtk::StringList::new(&branches.iter().map(String::as_str).collect::<Vec<_>>())));
        }
        self.branch.set_selected(branches.iter().position(|b| *b == snapshot.status.branch).unwrap_or(0) as u32);
        self.syncing_branch.set(false);
    }

    fn render(self: &Rc<Self>, snapshot: &Snapshot) {
        let status = &snapshot.status;
        self.restore_branch();
        self.meta.set_text(&sync_summary(snapshot));
        let publish = status.upstream.is_empty();
        self.push_button.set_label(if publish { "Publish" } else { "Push" });
        self.push_button.set_sensitive(snapshot.has_head && !status.detached && (publish || status.ahead > 0));
        self.push_button.set_tooltip_text(Some(if publish { "Publish this branch to origin and set its upstream" } else { "git push" }));
        let conflicts = status.files.iter().any(GitFile::conflicted);
        self.update_commit();
        self.operation_row.set_visible(snapshot.operation.is_some());
        self.abort_button.set_label(if snapshot.operation == Some("rebase") { "Abort rebase" } else { "Abort merge" });
        self.continue_button.set_sensitive(!conflicts);
        self.count.set(status.changed_count());
        self.notify_count();
        self.render_files(&status.files);
        let selected = self.selected.borrow().clone();
        if let Some((path, staged)) = selected {
            let file = status.files.iter().find(|file| file.path == path && if staged { file.staged() } else { file.untracked() || file.unstaged() || file.conflicted() });
            if let Some(file) = file { self.show_diff(file, staged); }
            else { *self.selected.borrow_mut() = None; self.clear_diff(); }
        }
    }

    fn file_row(self: &Rc<Self>, file: &GitFile, staged: bool) -> gtk::ListBoxRow {
        let row = gtk::ListBoxRow::new();
        row.add_css_class("git-file");
        let top = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let partial = file.staged() && file.unstaged();
        let kind = if partial { if staged { "staged" } else { "changed" } } else { file.label() };
        let chip = label(kind, "git-chip");
        chip.add_css_class(if file.conflicted() { "error" } else { kind });
        if partial {
            row.set_tooltip_text(Some(if staged {
                "This file also has unstaged edits. This row previews only the staged version."
            } else { "This file also has staged edits. This row previews only the unstaged version." }));
        }
        top.append(&chip);
        let path = label(&file.path, "git-file-path");
        path.set_hexpand(true);
        path.set_wrap(false);
        path.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
        path.set_tooltip_text(Some(&file.path));
        top.append(&path);
        row.set_child(Some(&top));
        let actions = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let popover = gtk::Popover::new();
        popover.set_child(Some(&actions));
        popover.set_parent(&row);
        for name in if staged { vec!["Unstage", "Open file"] } else if file.conflicted() {
            vec!["Stage", "Open file"]
        } else { vec!["Stage", "Open file", "Discard", "Destroy"] } {
            let action = gtk::Button::with_label(name);
            action.add_css_class("git-mini");
            let weak = Rc::downgrade(self);
            let file = file.clone();
            let weak_popover = popover.downgrade();
            action.connect_clicked(move |_| {
                let Some(me) = weak.upgrade() else { return };
                if let Some(popover) = weak_popover.upgrade() { popover.popdown(); }
                if me.busy.get() { return; }
                match name {
                    "Stage" => me.run_action(git::add_args(&file), "Staging…"),
                    "Unstage" => me.unstage(Some(file.clone())),
                    "Discard" => {
                        let file = file.clone();
                        me.perform("Saving to stash…", move |repo| git::stash_discard(repo, &file));
                    }
                    "Destroy" => {
                        let file = file.clone();
                        me.confirm("Destroy this change?", &format!("{}\nUnstaged content will be permanently lost. Use Discard to save it in a stash instead.", file.path),
                            "Destroy", move |me| me.run_action(git::discard_args(&file), "Destroying…"));
                    }
                    "Open file" => {
                        let uri = gtk::gio::File::for_path(me.repo.borrow().join(&file.path)).uri();
                        gtk::gio::AppInfo::launch_default_for_uri_async(&uri, None::<&gtk::gio::AppLaunchContext>, None::<&gtk::gio::Cancellable>, move |result| {
                            if let Err(err) = result { me.set_status(&err.to_string(), true); }
                        });
                    }
                    _ => {}
                }
            });
            actions.append(&action);
        }
        let click = gtk::GestureClick::new();
        click.set_button(3);
        let weak = popover.downgrade();
        click.connect_pressed(move |gesture, _, x, y| {
            if let Some(popover) = weak.upgrade() {
                popover.set_pointing_to(Some(&gtk::gdk::Rectangle::new(x as i32, y as i32, 1, 1)));
                popover.popup();
                gesture.set_state(gtk::EventSequenceState::Claimed);
            }
        });
        row.add_controller(click);
        let keys = gtk::EventControllerKey::new();
        let weak = popover.downgrade();
        keys.connect_key_pressed(move |_, key, _, modifiers| {
            if key == gtk::gdk::Key::Menu || (key == gtk::gdk::Key::F10 && modifiers.contains(gtk::gdk::ModifierType::SHIFT_MASK)) {
                if let Some(popover) = weak.upgrade() { popover.popup(); }
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
        row.add_controller(keys);
        row.connect_destroy(move |_| popover.unparent());
        row
    }

    pub fn preview_first(self: &Rc<Self>) {
        let file = self.snapshot.borrow().as_ref().and_then(|s| s.status.files.first()).cloned();
        if let Some(file) = file { self.show_diff(&file, !file.unstaged() && file.staged()); }
    }

    fn clear_diff(self: &Rc<Self>) {
        *self.diff_text.borrow_mut() = None;
        self.diff_seq.set(self.diff_seq.get() + 1);
        self.diff_title.set_text("Diff");
        clear_box(&self.diff_view);
        let query = !self.filter.text().is_empty();
        let clean = self.snapshot.borrow().as_ref().is_some_and(|s| s.status.clean());
        let text = if query { "No files match this filter." }
            else if clean { "Working tree clean. No changes to commit." }
            else { "Select a file to review its changes." };
        self.diff_view.append(&label(text, "git-empty"));
        if query || clean {
            let action = button(if query { "Clear filter" } else { "View history" });
            action.set_halign(gtk::Align::Start);
            let weak = Rc::downgrade(self);
            action.connect_clicked(move |_| {
                if let Some(me) = weak.upgrade() {
                    if query { me.filter.set_text(""); me.filter.grab_focus(); }
                    else { me.stack.set_visible_child_name("history"); }
                }
            });
            self.diff_view.append(&action);
        }
    }

    fn show_diff(self: &Rc<Self>, file: &GitFile, staged: bool) {
        *self.selected.borrow_mut() = Some((file.path.clone(), staged));
        let seq = self.diff_seq.get() + 1;
        self.diff_seq.set(seq);
        self.diff_title.set_text(&format!("{} · {}", if staged { "Staged" } else { "Unstaged" }, file.path));
        let args = git::diff_args(file, staged);
        let me = self.clone();
        task(self.repo.borrow().clone(), move |repo| git::run(repo, &args), move |result| {
            if me.diff_seq.get() != seq { return; }
            if let Ok(text) = &result {
                if me.diff_text.borrow().as_ref() == Some(text) { return; }
                *me.diff_text.borrow_mut() = Some(text.clone());
            }
            clear_box(&me.diff_view);
            match result {
                Ok(text) if !text.is_empty() => {
                    let diff = crate::diff::parse(&text);
                    if diff.is_empty() {
                        me.diff_view.append(&label("No textual diff (binary or metadata only)", "git-empty"));
                    } else {
                        chat::append_diff_rows(&me.diff_view, &diff, false);
                    }
                }
                Ok(_) => me.diff_view.append(&label("No textual diff (empty file, binary or metadata only)", "git-empty")),
                Err(err) => me.diff_view.append(&label(&err, "git-status")),
            }
        });
    }

    fn unstage(self: &Rc<Self>, file: Option<GitFile>) {
        self.perform("Unstaging…", move |repo| git::unstage(repo, file.as_ref()));
    }

    fn perform(self: &Rc<Self>, progress: &str, work: impl FnOnce(&std::path::Path) -> Result<String, String> + Send + 'static) {
        if self.opening.get() || self.busy.replace(true) { return; }
        self.update_repository_controls();
        self.branch.set_sensitive(false);
        self.generation.set(self.generation.get() + 1);
        let generation = self.generation.get();
        self.set_status(progress, false);
        let success = match progress {
            "Staging…" => Some("Files staged."), "Unstaging…" => Some("Files unstaged."),
            "Committing…" => Some("Commit created."), "Fetching…" => Some("Remote checked."),
            "Pulling…" => Some("Branch updated."), "Pushing…" => Some("Push completed."),
            "Switching branch…" => Some("Branch changed."), "Aborting…" => Some("Operation aborted."),
            "Continuing…" => Some("Operation continued."), "Destroying…" => Some("Change deleted."),
            _ => None,
        };
        let me = self.clone();
        task(self.repo.borrow().clone(), work, move |result| {
            me.busy.set(false);
            me.branch.set_sensitive(true);
            me.update_repository_controls();
            me.update_actions();
            if result.is_err() { *me.destination.borrow_mut() = None; }
            if generation == me.generation.get() {
                match result {
                    Ok(output) => {
                        me.set_status(success.unwrap_or_else(|| if output.trim().is_empty() { "Done" } else { output.trim() }), false);
                        if success.is_some() && !output.trim().is_empty() { me.status.set_tooltip_text(Some(output.trim())); }
                    }
                    Err(err) => me.set_status(&if err.contains("fast-forward") || err.contains("diverg") {
                        format!("{err}\nBranches have diverged. Choose a merge or rebase strategy before pulling; no merge was started.")
                    } else { err }, true),
                }
            }
            me.refresh();
        });
    }

    fn run_action(self: &Rc<Self>, args: Vec<String>, progress: &str) {
        self.perform(progress, move |repo| git::run(repo, &args));
    }

    fn commit(self: &Rc<Self>) {
        if self.busy.get() || self.opening.get() || !self.snapshot.borrow().as_ref().is_some_and(|s|
            s.status.staged_count() > 0 && !s.status.files.iter().any(GitFile::conflicted)) { return; }
        let message = self.commit_entry.text().trim().to_string();
        if message.is_empty() { self.set_status("Write a commit message", true); return; }
        let entry = self.commit_entry.clone();
        let args = git::commit_args(&message);
        // Clear only after success, on the main loop.
        let me = self.clone();
        self.perform("Committing…", move |repo| {
            let status = git::snapshot(repo)?;
            if status.status.staged_count() == 0 { return Err("Nothing staged; no commit was run.".into()); }
            git::run(repo, &args)
        });
        let generation = me.generation.get();
        let weak = Rc::downgrade(&me);
        glib::timeout_add_local(Duration::from_millis(80), move || {
            let Some(me) = weak.upgrade() else { return glib::ControlFlow::Break };
            if me.generation.get() != generation { return glib::ControlFlow::Break; }
            if me.busy.get() { return glib::ControlFlow::Continue; }
            if !me.status.has_css_class("error") { entry.set_text(""); }
            glib::ControlFlow::Break
        });
    }

    fn push(self: &Rc<Self>) {
        let args = {
            let snapshot = self.snapshot.borrow();
            let Some(s) = snapshot.as_ref() else { return };
            if s.status.detached { return; }
            if s.status.upstream.is_empty() { git::publish_args(&s.status.branch) } else { git::push_args() }
        };
        self.run_action(args, "Pushing…");
    }

    fn switch_branch(self: &Rc<Self>, branch: &str, create: bool) {
        let args = match git::switch_args(branch, create) {
            Ok(args) => args, Err(err) => { self.set_status(&err, true); return; }
        };
        if self.snapshot.borrow().as_ref().is_some_and(|s| !s.status.clean()) {
            self.confirm("Switch branch with changes?", "Git will keep compatible changes and refuse to overwrite conflicting files.", "Switch",
                move |me| me.run_action(args.clone(), "Switching branch…"));
        } else { self.run_action(args, "Switching branch…"); }
    }

    pub(super) fn qa_state(&self) -> serde_json::Value {
        let snapshot = self.snapshot.borrow();
        json!({"branch":snapshot.as_ref().map(|s| &s.status.branch),
            "repository":self.repo.borrow().to_string_lossy(), "workspace":self.workspace.borrow().to_string_lossy(),
            "pinned":self.pinned.get(), "opening":self.opening.get(), "workbench_visible":self.stack.is_visible(),
            "split":[self.split.width(),self.split.height(),self.split.position()],
            "vertical":self.split.orientation() == gtk::Orientation::Vertical,
            "count":self.count.get(), "staged":snapshot.as_ref().map(|s| s.status.staged_count()),
            "conflicts":snapshot.as_ref().map(|s| s.status.files.iter().filter(|f| f.conflicted()).count()),
            "operation":snapshot.as_ref().and_then(|s| s.operation),
            "visible_files":(0..2).map(|index| self.files.borrow()[index].iter().enumerate()
                .filter(|(i, _)| self.groups[index].row_at_index(*i as i32).is_some_and(|r| r.get_visible()))
                .map(|(_, f)| f.path.clone()).collect::<Vec<_>>()).collect::<Vec<_>>(),
            "selected_files":[self.selected_files(0).iter().map(|f| f.path.clone()).collect::<Vec<_>>(),
                self.selected_files(1).iter().map(|f| f.path.clone()).collect::<Vec<_>>()],
            "history":self.history.qa_state(),
            "summary":self.meta.text().as_str(),
            "group_titles":[self.group_titles[0].text().as_str(), self.group_titles[1].text().as_str()],
            "commit_enabled":self.commit_button.is_sensitive(),
            "selected":self.selected.borrow().clone(), "busy":self.busy.get(), "status":self.status.text().as_str()})
    }
}

#[cfg(test)]
mod ux_tests {
    use super::*;

    #[test]
    fn summaries_explain_sync_without_repeating_zero_counters() {
        let mut snapshot = Snapshot::default();
        assert!(sync_summary(&snapshot).contains("No commits yet"));
        snapshot.has_head = true;
        assert_eq!(sync_summary(&snapshot), "Local branch · not published");
        snapshot.status.upstream = "origin/main".into();
        assert_eq!(sync_summary(&snapshot), "Up to date with origin/main");
        snapshot.status.ahead = 2;
        snapshot.status.behind = 3;
        assert_eq!(sync_summary(&snapshot), "origin/main · 3 to pull · 2 to push");
        snapshot.status.detached = true;
        assert!(sync_summary(&snapshot).starts_with("Detached HEAD"));
    }

    #[test]
    fn summaries_explain_conflicts_and_when_to_continue() {
        let mut snapshot = Snapshot::default();
        snapshot.operation = Some("merge");
        snapshot.status.files.push(GitFile { path:"conflict".into(), index:'U', worktree:'U', old_path:None });
        assert_eq!(sync_summary(&snapshot), "Merge in progress · 1 conflict to resolve");
        snapshot.status.files.clear();
        snapshot.operation = Some("rebase");
        assert_eq!(sync_summary(&snapshot), "Rebase in progress · ready to continue");
    }
}

fn clear_box(container: &gtk::Box) {
    while let Some(child) = container.first_child() { container.remove(&child); }
}

fn task<T: Send + 'static>(repo: PathBuf, work: impl FnOnce(&std::path::Path) -> T + Send + 'static, done: impl FnOnce(T) + 'static) {
    let (tx, rx) = async_channel::bounded(1);
    std::thread::spawn(move || { let _ = tx.send_blocking(work(&repo)); });
    glib::spawn_future_local(async move { if let Ok(result) = rx.recv().await { done(result); } });
}
