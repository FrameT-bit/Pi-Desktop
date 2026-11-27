use super::*;
use crate::git_history::{self as history, Change, Commit, History};

pub struct HistoryUi {
    pub root: gtk::Box,
    repo: RefCell<PathBuf>,
    filter: gtk::SearchEntry,
    commits: gtk::ListBox,
    details: gtk::Label,
    files: gtk::ListBox,
    diff: gtk::Box,
    diff_title: gtk::Label,
    status: gtk::Label,
    more: gtk::Button,
    state: RefCell<Option<History>>,
    changes: RefCell<Vec<Change>>,
    selected: RefCell<Option<String>>,
    limit: Cell<usize>,
    loading: Cell<bool>,
    rendering: Cell<bool>,
    generation: Cell<u64>,
    detail_seq: Cell<u64>,
    diff_seq: Cell<u64>,
}

fn scroll(child: &impl IsA<gtk::Widget>) -> gtk::ScrolledWindow {
    gtk::ScrolledWindow::builder().child(child)
        .hscrollbar_policy(gtk::PolicyType::Automatic)
        .vscrollbar_policy(gtk::PolicyType::Automatic)
        .hexpand(true).vexpand(true).build()
}

impl HistoryUi {
    pub fn build(repo: PathBuf) -> Rc<Self> {
        let root = gtk::Box::new(gtk::Orientation::Vertical, 8);
        root.add_css_class("git-history");
        let filter = gtk::SearchEntry::new();
        filter.set_placeholder_text(Some("Filter loaded commits · message, author, hash or ref"));
        filter.add_css_class("git-filter");
        root.append(&filter);
        let status = label("", "git-status");
        root.append(&status);
        let commits = gtk::ListBox::new();
        commits.set_selection_mode(gtk::SelectionMode::Single);
        commits.add_css_class("git-file-list");
        commits.set_placeholder(Some(&label("No commits to show", "git-empty")));
        let top = gtk::Box::new(gtk::Orientation::Vertical, 0);
        top.append(&scroll(&commits));
        let more = button("Load more commits");
        more.set_visible(false);
        top.append(&more);
        let details = label("", "git-history-message");
        details.set_wrap_mode(gtk::pango::WrapMode::WordChar);
        details.set_width_chars(1);
        details.set_selectable(true);
        let message_scroll = scroll(&details);
        message_scroll.set_vexpand(false);
        message_scroll.set_max_content_height(96);
        message_scroll.set_propagate_natural_height(true);
        let files = gtk::ListBox::new();
        files.set_selection_mode(gtk::SelectionMode::Single);
        files.add_css_class("git-file-list");
        files.set_placeholder(Some(&label("No changed files", "git-empty")));
        let diff_title = label("Commit diff · read-only", "git-diff-title");
        let diff = gtk::Box::new(gtk::Orientation::Vertical, 0);
        diff.add_css_class("diff-body");
        let diff_box = gtk::Box::new(gtk::Orientation::Vertical, 8);
        diff_box.append(&diff_title);
        let diff_scroll = scroll(&diff);
        diff_scroll.add_css_class("git-diff-scroll");
        diff_box.append(&diff_scroll);
        let split = gtk::Paned::new(gtk::Orientation::Horizontal);
        split.set_position(300);
        split.set_shrink_start_child(true);
        split.set_shrink_end_child(true);
        split.set_start_child(Some(&scroll(&files)));
        split.set_end_child(Some(&diff_box));
        let bottom = gtk::Box::new(gtk::Orientation::Vertical, 8);
        bottom.append(&message_scroll);
        bottom.append(&split);
        let vertical = gtk::Paned::new(gtk::Orientation::Vertical);
        // Three 44px commit rows leave room for patch content, including at 125% font scale.
        vertical.set_position(132);
        vertical.set_shrink_start_child(true);
        vertical.set_shrink_end_child(true);
        vertical.set_start_child(Some(&top));
        vertical.set_end_child(Some(&bottom));
        vertical.set_vexpand(true);
        root.append(&vertical);
        let weak_root = root.downgrade();
        glib::timeout_add_local(Duration::from_millis(120), move || {
            let Some(root) = weak_root.upgrade() else { return glib::ControlFlow::Break };
            if root.is_mapped() {
                let orientation = if root.width() < 700 { gtk::Orientation::Vertical } else { gtk::Orientation::Horizontal };
                if split.orientation() != orientation {
                    split.set_orientation(orientation);
                    split.set_position(if orientation == gtk::Orientation::Vertical { 96 } else { 300 });
                }
            }
            glib::ControlFlow::Continue
        });
        let me = Rc::new(Self {
            root, repo: RefCell::new(repo), filter, commits, details, files, diff, diff_title,
            status, more, state: RefCell::new(None), changes: RefCell::new(Vec::new()),
            selected: RefCell::new(None), limit: Cell::new(history::PAGE_SIZE), loading: Cell::new(false),
            rendering: Cell::new(false), generation: Cell::new(0), detail_seq: Cell::new(0), diff_seq: Cell::new(0),
        });
        let weak = Rc::downgrade(&me);
        me.more.connect_clicked(move |_| {
            if let Some(me) = weak.upgrade() {
                me.limit.set((me.limit.get() + history::PAGE_SIZE).min(history::MAX_COMMITS));
                me.refresh();
            }
        });
        let weak = Rc::downgrade(&me);
        me.filter.connect_search_changed(move |_| { if let Some(me) = weak.upgrade() { me.apply_filter(); } });
        let weak = Rc::downgrade(&me);
        me.commits.connect_row_selected(move |_, row| {
            let Some(me) = weak.upgrade() else { return };
            if me.rendering.get() { return; }
            let commit = row.and_then(|row| me.state.borrow().as_ref()?.commits.get(row.index() as usize).cloned());
            if let Some(commit) = commit { me.select(&commit); }
        });
        let weak = Rc::downgrade(&me);
        me.files.connect_row_selected(move |_, row| {
            let Some(me) = weak.upgrade() else { return };
            if me.rendering.get() { return; }
            let change = row.and_then(|row| me.changes.borrow().get(row.index() as usize).cloned());
            if let Some(change) = change { me.show_diff(&change); }
        });
        me
    }

    pub fn focus_filter(&self) { self.filter.grab_focus(); }

    pub fn set_repo(&self, repo: PathBuf) {
        *self.repo.borrow_mut() = repo;
        self.generation.set(self.generation.get() + 1);
        self.detail_seq.set(self.detail_seq.get() + 1);
        self.diff_seq.set(self.diff_seq.get() + 1);
        self.loading.set(false);
        self.rendering.set(true);
        *self.state.borrow_mut() = None;
        *self.selected.borrow_mut() = None;
        self.limit.set(history::PAGE_SIZE);
        self.filter.set_text("");
        self.commits.remove_all();
        self.files.remove_all();
        self.details.set_text("");
        clear_box(&self.diff);
        self.status.set_text("");
        self.more.set_visible(false);
        self.rendering.set(false);
    }

    pub fn refresh(self: &Rc<Self>) {
        if self.loading.replace(true) { return; }
        let generation = self.generation.get();
        let limit = self.limit.get();
        let me = self.clone();
        task(self.repo.borrow().clone(), move |repo| history::read(repo, limit), move |result| {
            if generation != me.generation.get() { return; }
            me.loading.set(false);
            match result {
                Ok(state) => {
                    me.status.remove_css_class("error");
                    me.status.set_text(&format!("{} commits · all refs · read-only{}", state.commits.len(),
                        if state.commits.len() == history::MAX_COMMITS { " · limit reached" } else { "" }));
                    me.more.set_visible(state.commits.len() == limit && limit < history::MAX_COMMITS);
                    if me.state.borrow().as_ref() == Some(&state) { me.apply_filter(); return; }
                    let selected = me.selected.borrow().clone();
                    let graph = history::graph(&state.commits);
                    let graph_width = graph.iter().map(|r| r.lanes).max().unwrap_or(1);
                    let scroll = me.commits.parent().and_downcast::<gtk::Viewport>()
                        .and_then(|viewport| viewport.parent()).and_downcast::<gtk::ScrolledWindow>();
                    let offset = scroll.as_ref().map(|s| s.vadjustment().value()).unwrap_or(0.0);
                    me.rendering.set(true);
                    me.commits.remove_all();
                    for (commit, graph) in state.commits.iter().zip(graph) {
                        me.commits.append(&commit_row(commit, graph, graph_width, state.head.as_ref() == Some(&commit.oid)));
                    }
                    let position = selected.as_ref().and_then(|oid| state.commits.iter().position(|c| &c.oid == oid)).unwrap_or(0);
                    *me.state.borrow_mut() = Some(state.clone());
                    me.apply_filter();
                    let row = me.commits.row_at_index(position as i32).filter(|r| r.get_visible())
                        .or_else(|| (0..state.commits.len()).find_map(|i| me.commits.row_at_index(i as i32).filter(|r| r.get_visible())));
                    if let Some(row) = &row { me.commits.select_row(Some(row)); }
                    me.rendering.set(false);
                    if let Some(scroll) = scroll {
                        glib::idle_add_local_once(move || scroll.vadjustment().set_value(offset));
                    }
                    if let Some(commit) = row.and_then(|row| state.commits.get(row.index() as usize)) { me.select(commit); }
                    else { me.clear_details(); }
                }
                Err(error) => {
                    me.status.set_text(&error);
                    me.status.add_css_class("error");
                    me.more.set_visible(false);
                    me.rendering.set(true);
                    me.commits.remove_all();
                    *me.state.borrow_mut() = None;
                    me.rendering.set(false);
                    me.clear_details();
                }
            }
        });
    }

    fn apply_filter(self: &Rc<Self>) {
        let query = self.filter.text().to_lowercase();
        let state = self.state.borrow();
        let Some(state) = state.as_ref() else { return };
        for (i, commit) in state.commits.iter().enumerate() {
            if let Some(row) = self.commits.row_at_index(i as i32) {
                row.set_visible(format!("{} {} {} {}", commit.oid, commit.refs, commit.message, commit.author).to_lowercase().contains(&query));
                // Filtering breaks graph adjacency; never draw fictional parent links.
                if let Some(graph) = row.child().and_then(|child| child.first_child()) {
                    graph.set_visible(query.is_empty());
                }
            }
        }
        let shown = (0..state.commits.len()).filter(|i|
            self.commits.row_at_index(*i as i32).is_some_and(|row| row.get_visible())).count();
        self.status.set_text(&if query.is_empty() {
            format!("{} commits · all refs · read-only{}", state.commits.len(),
                if state.commits.len() == history::MAX_COMMITS { " · limit reached" } else { "" })
        } else { format!("{shown} of {} loaded commits · graph hidden while filtered", state.commits.len()) });
        if !self.rendering.get() {
            let row = self.commits.selected_row().filter(|row| row.get_visible())
                .or_else(|| (0..state.commits.len()).find_map(|i| self.commits.row_at_index(i as i32).filter(|r| r.get_visible())));
            if let Some(row) = row {
                self.rendering.set(true);
                self.commits.select_row(Some(&row));
                self.rendering.set(false);
                // GTK may retain the row while an empty filter clears our
                // preview; selecting that same row does not emit a new signal.
                if let Some(commit) = state.commits.get(row.index() as usize) { self.select(commit); }
            } else { self.clear_details(); }
        }
    }

    fn clear_details(&self) {
        self.detail_seq.set(self.detail_seq.get() + 1);
        self.diff_seq.set(self.diff_seq.get() + 1);
        *self.selected.borrow_mut() = None;
        self.details.set_text("");
        self.files.remove_all();
        self.changes.borrow_mut().clear();
        clear_box(&self.diff);
    }

    fn select(self: &Rc<Self>, commit: &Commit) {
        if self.selected.borrow().as_ref() == Some(&commit.oid) { return; }
        *self.selected.borrow_mut() = Some(commit.oid.clone());
        self.details.set_text(&format!("{}\n{} · {}{}\n{}", commit.oid, commit.author, commit.date,
            if commit.parents.len() > 1 { " · merge diff against first parent" } else { "" }, commit.message));
        self.rendering.set(true);
        self.files.remove_all();
        self.rendering.set(false);
        self.changes.borrow_mut().clear();
        clear_box(&self.diff);
        self.diff_seq.set(self.diff_seq.get() + 1);
        let seq = self.detail_seq.get() + 1;
        self.detail_seq.set(seq);
        let oid = commit.oid.clone();
        let me = self.clone();
        task(self.repo.borrow().clone(), move |repo| history::changes(repo, &oid), move |result| {
            if seq != me.detail_seq.get() { return; }
            match result {
                Ok(changes) => {
                    me.rendering.set(true);
                    for change in &changes {
                        let row = label(&format!("{}  {}", change.kind, change.path), "git-file-path");
                        row.set_wrap(false);
                        row.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
                        row.set_tooltip_text(Some(&change.old_path.as_ref().map(|old| format!("{old} → {}", change.path)).unwrap_or_else(|| change.path.clone())));
                        me.files.append(&row);
                    }
                    *me.changes.borrow_mut() = changes.clone();
                    me.rendering.set(false);
                    if let Some(row) = me.files.row_at_index(0) { me.files.select_row(Some(&row)); }
                    if changes.is_empty() { me.diff.append(&label("This commit has no file changes", "git-empty")); }
                }
                Err(error) => me.diff.append(&label(&error, "git-status")),
            }
        });
    }

    fn show_diff(self: &Rc<Self>, change: &Change) {
        let Some(oid) = self.selected.borrow().clone() else { return };
        self.diff_title.set_text(&format!("{} · {}", &oid[..8], change.path));
        let seq = self.diff_seq.get() + 1;
        self.diff_seq.set(seq);
        let change = change.clone();
        let me = self.clone();
        task(self.repo.borrow().clone(), move |repo| history::diff(repo, &oid, &change), move |result| {
            if seq != me.diff_seq.get() { return; }
            clear_box(&me.diff);
            match result {
                Ok(text) => {
                    let diff = crate::diff::parse(&text);
                    if diff.is_empty() { me.diff.append(&label("No textual diff (empty, binary or metadata only)", "git-empty")); }
                    else { chat::append_diff_rows(&me.diff, &diff, false); }
                }
                Err(error) => me.diff.append(&label(&error, "git-status")),
            }
        });
    }

    pub(super) fn qa_state(&self) -> serde_json::Value {
        let viewport = self.diff.parent();
        let mut child = self.diff.first_child();
        let mut visible_changes = 0;
        while let Some(row) = child {
            child = row.next_sibling();
            if row.has_css_class("add") || row.has_css_class("del") {
                if let Some(viewport) = &viewport {
                    if row.compute_bounds(viewport).is_some_and(|rect|
                        rect.y() >= 0.0 && rect.y() + rect.height() <= viewport.height() as f32) {
                        visible_changes += 1;
                    }
                }
            }
        }
        json!({"commits":self.state.borrow().as_ref().map(|s| s.commits.len()),
            "selected":self.selected.borrow().clone(), "files":self.changes.borrow().len(),
            "message":self.details.text().as_str(), "loading":self.loading.get(),
            "visible":self.state.borrow().as_ref().map(|s| (0..s.commits.len())
                .filter(|i| self.commits.row_at_index(*i as i32).is_some_and(|row| row.get_visible())).count()),
            "filter":self.filter.text().as_str(),"visible_changes":visible_changes})
    }

    pub(super) fn qa_filter(&self, query: &str) { self.filter.set_text(query); }

    pub(super) fn qa_select_last(&self) {
        let count = self.state.borrow().as_ref().map(|s| s.commits.len()).unwrap_or(0);
        if count > 0 { self.commits.select_row(self.commits.row_at_index(count as i32 - 1).as_ref()); }
    }
}

fn commit_row(commit: &Commit, graph: history::GraphRow, lanes: usize, head: bool) -> gtk::ListBoxRow {
    let row = gtk::ListBoxRow::new();
    row.add_css_class("git-history-row");
    let content = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let drawing = gtk::DrawingArea::new();
    // Graph geometry uses the shared 12/24 spacing scale; row height stays a CSS token.
    drawing.set_content_width((lanes * 12 + 24) as i32);
    drawing.set_draw_func(move |widget, context, _, height| {
        let color = widget.color();
        context.set_source_rgba(color.red() as f64, color.green() as f64, color.blue() as f64, 0.8);
        context.set_line_width(2.0);
        let x = |column: usize| (column * 12 + 12) as f64;
        let mid = height as f64 / 2.0;
        if graph.incoming {
            context.move_to(x(graph.column), 0.0);
            context.line_to(x(graph.column), mid);
        }
        for (from, to) in &graph.continuations {
            context.move_to(x(*from), 0.0);
            context.line_to(x(*from), mid);
            context.line_to(x(*to), height as f64);
        }
        for parent in &graph.parents {
            context.move_to(x(graph.column), mid);
            context.line_to(x(*parent), height as f64);
        }
        if head { context.set_line_width(3.0); }
        let _ = context.stroke();
        context.arc(x(graph.column), mid, 4.0, 0.0, std::f64::consts::TAU);
        let _ = context.fill();
    });
    content.append(&drawing);
    let hash = label(&commit.short, "git-meta");
    hash.set_wrap(false);
    content.append(&hash);
    let subject = label(&format!("{}{}", if commit.refs.is_empty() { String::new() } else { format!("[{}] ", commit.refs) }, commit.subject), "git-file-path");
    subject.set_wrap(false);
    subject.set_ellipsize(gtk::pango::EllipsizeMode::End);
    subject.set_hexpand(true);
    content.append(&subject);
    let author = label(&commit.author, "git-meta");
    author.set_wrap(false);
    author.set_ellipsize(gtk::pango::EllipsizeMode::End);
    author.set_max_width_chars(16);
    content.append(&author);
    row.set_tooltip_text(Some(&format!("{}\n{} · {}\n{}", commit.oid, commit.author, commit.date, commit.message)));
    row.set_child(Some(&content));
    row
}
