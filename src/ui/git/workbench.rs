use super::*;

impl GitUi {
    pub(super) fn bind_workbench(self: &Rc<Self>) {
        let weak = Rc::downgrade(self);
        self.filter.connect_search_changed(move |_| {
            if let Some(me) = weak.upgrade() { me.filter_files(); }
        });
        for index in 0..2 {
            let weak = Rc::downgrade(self);
            self.stage_selected[index].connect_clicked(move |_| {
                if let Some(me) = weak.upgrade() { me.transfer(index, false); }
            });
            let weak = Rc::downgrade(self);
            self.stage_all[index].connect_clicked(move |_| {
                if let Some(me) = weak.upgrade() { me.transfer(index, true); }
            });
            let weak = Rc::downgrade(self);
            self.groups[index].connect_selected_rows_changed(move |list| {
                let Some(me) = weak.upgrade() else { return };
                if me.rendering.get() { return; }
                me.update_actions();
                let row = list.focus_child().and_downcast::<gtk::ListBoxRow>()
                    .filter(|row| row.is_selected()).or_else(|| list.selected_rows().last().cloned());
                let file = row.and_then(|row| me.files.borrow()[index].get(row.index() as usize).cloned());
                if let Some(file) = file { me.show_diff(&file, index == 1); }
            });
            let keys = gtk::EventControllerKey::new();
            keys.set_propagation_phase(gtk::PropagationPhase::Capture);
            let weak = Rc::downgrade(self);
            keys.connect_key_pressed(move |_, key, _, modifiers| {
                if modifiers.intersects(gtk::gdk::ModifierType::CONTROL_MASK | gtk::gdk::ModifierType::ALT_MASK | gtk::gdk::ModifierType::SHIFT_MASK) {
                    return glib::Propagation::Proceed;
                }
                if matches!(key, gtk::gdk::Key::space | gtk::gdk::Key::Return | gtk::gdk::Key::KP_Enter) {
                    if let Some(me) = weak.upgrade() { me.transfer(index, false); }
                    return glib::Propagation::Stop;
                }
                glib::Propagation::Proceed
            });
            self.groups[index].add_controller(keys);
        }
        let keys = gtk::EventControllerKey::new();
        let weak = Rc::downgrade(self);
        keys.connect_key_pressed(move |_, key, _, modifiers| {
            let Some(me) = weak.upgrade() else { return glib::Propagation::Proceed };
            if modifiers.contains(gtk::gdk::ModifierType::CONTROL_MASK) {
                if matches!(key, gtk::gdk::Key::f | gtk::gdk::Key::F) {
                    me.focus_filter();
                    return glib::Propagation::Stop;
                }
                if key == gtk::gdk::Key::Return && me.stack.visible_child_name().as_deref() == Some("changes") {
                    me.commit();
                    return glib::Propagation::Stop;
                }
            }
            glib::Propagation::Proceed
        });
        self.root.add_controller(keys);
        self.update_actions();
    }

    pub(super) fn selected_files(&self, index: usize) -> Vec<GitFile> {
        self.groups[index].selected_rows().iter().filter(|row| row.get_visible())
            .filter_map(|row| self.files.borrow()[index].get(row.index() as usize).cloned()).collect()
    }

    pub(super) fn transfer(self: &Rc<Self>, index: usize, all: bool) {
        if self.busy.get() || self.opening.get() { return; }
        let files = if all { self.shown_files(index) } else { self.selected_files(index) };
        if files.is_empty() { return; }
        *self.destination.borrow_mut() = Some((files[0].path.clone(), index == 0));
        self.perform(if index == 0 { "Staging…" } else { "Unstaging…" }, move |repo| {
            if index == 0 { git::stage_files(repo, &files) } else { git::unstage_files(repo, &files) }
        });
        self.update_actions();
    }

    pub(super) fn update_actions(&self) {
        let filtered = !self.filter.text().is_empty();
        for index in 0..2 {
            let count = self.selected_files(index).len();
            self.stage_selected[index].set_label(&format!("{}{}", if index == 0 { "Stage" } else { "Unstage" },
                if count > 0 { format!(" ({count})") } else { String::new() }));
            self.stage_selected[index].set_sensitive(!self.busy.get() && !self.opening.get() && count > 0);
            self.stage_all[index].set_label(match (index, filtered) {
                (0, true) => "Stage shown", (1, true) => "Unstage shown",
                (0, false) => "Stage all", _ => "Unstage all",
            });
            self.stage_all[index].set_tooltip_text(Some(if filtered {
                "Move only files shown by the filter. Hidden files stay unchanged."
            } else { "Move every file in this group." }));
            self.stage_all[index].set_sensitive(!self.busy.get() && !self.opening.get() && !self.shown_files(index).is_empty());
        }
        self.update_commit();
    }

    fn shown_files(&self, index: usize) -> Vec<GitFile> {
        self.files.borrow()[index].iter().enumerate()
            .filter(|(position, _)| self.groups[index].row_at_index(*position as i32).is_some_and(|row| row.get_visible()))
            .map(|(_, file)| file.clone()).collect()
    }

    pub(super) fn update_commit(&self) {
        let snapshot = self.snapshot.borrow();
        let count = snapshot.as_ref().map(|s| s.status.staged_count()).unwrap_or(0);
        let conflicts = snapshot.as_ref().is_some_and(|s| s.status.files.iter().any(GitFile::conflicted));
        let message = !self.commit_entry.text().trim().is_empty();
        self.commit_button.set_sensitive(!self.busy.get() && !self.opening.get() && count > 0 && !conflicts && message);
        let hint = if self.busy.get() || self.opening.get() { "Wait for the current Git operation.".into() }
            else if conflicts { "Resolve and stage the conflicts before committing.".into() }
            else if count == 0 { "Stage the files you want to include in this commit.".into() }
            else if !message { "Describe the staged changes to enable Commit.".into() }
            else { format!("Commit {count} staged file{}. Other unstaged edits stay unchanged. Ctrl+Enter.", if count == 1 { "" } else { "s" }) };
        self.commit_button.set_tooltip_text(Some(&hint));
    }

    pub(super) fn render_files(self: &Rc<Self>, files: &[GitFile]) {
        let selections = [self.selected_files(0), self.selected_files(1)];
        let focused: [Option<String>; 2] = std::array::from_fn(|index|
            self.groups[index].focus_child().and_downcast::<gtk::ListBoxRow>()
                .and_then(|row| self.files.borrow()[index].get(row.index() as usize).map(|file| file.path.clone())));
        self.rendering.set(true);
        for index in 0..2 {
            let scroll = self.groups[index].parent().and_downcast::<gtk::Viewport>()
                .and_then(|viewport| viewport.parent()).and_downcast::<gtk::ScrolledWindow>();
            let offset = scroll.as_ref().map(|s| s.vadjustment().value()).unwrap_or(0.0);
            while let Some(row) = self.groups[index].row_at_index(0) { self.groups[index].remove(&row); }
            let group: Vec<_> = files.iter().filter(|file| if index == 1 { file.staged() }
                else { file.untracked() || file.unstaged() || file.conflicted() }).cloned().collect();
            *self.files.borrow_mut().get_mut(index).unwrap() = group.clone();
            for file in &group {
                let row = self.file_row(file, index == 1);
                self.groups[index].append(&row);
                if selections[index].iter().any(|selected| selected.path == file.path) { self.groups[index].select_row(Some(&row)); }
            }
            if let Some(scroll) = scroll {
                glib::idle_add_local_once(move || scroll.vadjustment().set_value(offset));
            }
        }
        self.filter_files();
        let destination = self.destination.borrow_mut().take();
        let preview = destination.clone().or_else(|| self.selected.borrow().clone());
        let mut chosen = None;
        if let Some((path, staged)) = preview {
            let index = usize::from(staged);
            chosen = self.files.borrow()[index].iter().position(|f| f.path == path)
                .map(|position| (index, position));
        }
        if chosen.is_none() {
            for index in 0..2 {
                let visible = self.files.borrow()[index].iter().enumerate().find_map(|(i, _)|
                    self.groups[index].row_at_index(i as i32).filter(|row| row.get_visible()).map(|_| (index, i)));
                if visible.is_some() { chosen = visible; break; }
            }
        }
        if let Some((index, position)) = chosen {
            if let Some(row) = self.groups[index].row_at_index(position as i32) {
                if row.get_visible() {
                    self.groups[index].select_row(Some(&row));
                    if destination.is_some() && focused.iter().any(Option::is_some) { row.grab_focus(); }
                    let file = self.files.borrow()[index][position].clone();
                    self.show_diff(&file, index == 1);
                }
            }
        } else {
            *self.selected.borrow_mut() = None;
            self.clear_diff();
        }
        if destination.is_none() {
            for (index, path) in focused.iter().enumerate() {
                if let Some(position) = path.as_ref().and_then(|path| self.files.borrow()[index].iter().position(|file| &file.path == path)) {
                    if let Some(row) = self.groups[index].row_at_index(position as i32) { row.grab_focus(); }
                }
            }
        }
        self.rendering.set(false);
        self.update_actions();
    }

    fn filter_files(self: &Rc<Self>) {
        let query = self.filter.text().to_lowercase();
        for index in 0..2 {
            let mut visible = 0;
            for (position, file) in self.files.borrow()[index].iter().enumerate() {
                if let Some(row) = self.groups[index].row_at_index(position as i32) {
                    let matches = file.path.to_lowercase().contains(&query)
                        || file.old_path.as_ref().is_some_and(|path| path.to_lowercase().contains(&query));
                    row.set_visible(matches);
                    visible += usize::from(matches);
                }
            }
            let name = if index == 0 { "Unstaged" } else { "Staged" };
            let count = self.files.borrow()[index].len();
            self.group_titles[index].set_text(&if query.is_empty() { format!("{name} · {count}") }
                else { format!("{name} · {visible} of {count}") });
            self.group_empty[index].set_text(if !query.is_empty() { "No matching files" }
                else if index == 0 { "No unstaged changes" } else { "No files staged" });
        }
        self.update_actions();
        if !self.rendering.get() {
            let current = self.selected.borrow().clone();
            let visible: Vec<(usize, usize, GitFile)> = (0..2).flat_map(|index|
                self.files.borrow()[index].iter().enumerate()
                    .filter(|(i, _)| self.groups[index].row_at_index(*i as i32).is_some_and(|row| row.get_visible()))
                    .map(|(position, file)| (index, position, file.clone())).collect::<Vec<_>>()).collect();
            let selected = visible.iter().find(|(index, _, file)| current.as_ref() == Some(&(file.path.clone(), *index == 1)))
                .or_else(|| visible.first());
            if let Some((index, position, file)) = selected {
                self.rendering.set(true);
                self.groups[*index].select_row(self.groups[*index].row_at_index(*position as i32).as_ref());
                self.rendering.set(false);
                self.show_diff(file, *index == 1);
                self.update_actions();
            } else {
                *self.selected.borrow_mut() = None;
                self.clear_diff();
            }
        }
    }

    pub(in crate::ui) async fn qa_workbench(self: &Rc<Self>) -> serde_json::Value {
        let initial = self.qa_state();
        self.groups[0].unselect_all();
        for (position, file) in self.files.borrow()[0].iter().enumerate() {
            if matches!(file.path.as_str(), "partial.txt" | "a -> b.txt") {
                self.groups[0].select_row(self.groups[0].row_at_index(position as i32).as_ref());
            }
        }
        let multi = self.qa_state();
        let stage_key = emit_list_key(&self.groups[0], gtk::gdk::Key::space);
        glib::timeout_future(Duration::from_millis(450)).await;
        let staged = self.qa_state();
        self.refresh();
        glib::timeout_future(Duration::from_millis(250)).await;
        let polled = self.qa_state();
        self.groups[1].unselect_all();
        for (position, file) in self.files.borrow()[1].iter().enumerate() {
            if matches!(file.path.as_str(), "partial.txt" | "a -> b.txt") {
                self.groups[1].select_row(self.groups[1].row_at_index(position as i32).as_ref());
            }
        }
        let unstage_key = emit_list_key(&self.groups[1], gtk::gdk::Key::Return);
        glib::timeout_future(Duration::from_millis(400)).await;
        let unstaged = self.qa_state();
        self.filter.set_text("partial");
        glib::timeout_future(Duration::from_millis(250)).await;
        let filtered = self.qa_state();
        self.filter.set_text("no-such-file-qa-fixture");
        glib::timeout_future(Duration::from_millis(250)).await;
        let empty = self.qa_state();
        self.filter.set_text("");
        glib::timeout_future(Duration::from_millis(250)).await;
        json!({"initial":initial,"multi":multi,"staged":staged,"polled":polled,"unstaged":unstaged,"filtered":filtered,
            "cleared":self.qa_state(),"empty":empty,"stage_key":stage_key,"unstage_key":unstage_key})
    }

    pub(in crate::ui) async fn qa_history(self: &Rc<Self>) -> serde_json::Value {
        let before = git::snapshot(&self.repo.borrow()).unwrap();
        self.stack.set_visible_child_name("history");
        glib::timeout_future(Duration::from_millis(500)).await;
        let initial = self.history.qa_state();
        self.history.qa_select_last();
        glib::timeout_future(Duration::from_millis(400)).await;
        let selected = self.history.qa_state();
        self.history.qa_filter("no-such-commit-qa-fixture");
        glib::timeout_future(Duration::from_millis(250)).await;
        let empty = self.history.qa_state();
        self.history.qa_filter(selected["selected"].as_str().unwrap());
        glib::timeout_future(Duration::from_millis(350)).await;
        let filtered = self.history.qa_state();
        self.history.qa_filter("");
        glib::timeout_future(Duration::from_millis(250)).await;
        let after = git::snapshot(&self.repo.borrow()).unwrap();
        json!({"initial":initial,"selected":selected,"empty":empty,"filtered":filtered,"read_only":before == after})
    }

    pub(in crate::ui) async fn qa_ux(self: &Rc<Self>) -> serde_json::Value {
        let initial = self.qa_state();
        self.commit_entry.set_text("   ");
        let blank_blocked = !self.commit_button.is_sensitive();
        self.commit_entry.set_text("Describe the change");
        let ready = self.commit_button.is_sensitive();
        self.commit_entry.set_text("");
        let cleared_blocked = !self.commit_button.is_sensitive();
        self.filter.set_text("b.txt");
        glib::timeout_future(Duration::from_millis(250)).await;
        let bulk_label = self.stage_all[0].label().unwrap().to_string();
        self.stage_all[0].emit_clicked();
        glib::timeout_future(Duration::from_millis(400)).await;
        let staged_shown = self.qa_state();
        self.stage_all[1].emit_clicked();
        glib::timeout_future(Duration::from_millis(400)).await;
        let unstaged_shown = self.qa_state();
        let partial_index = git::run(&self.repo.borrow(), &["show".into(), ":partial.txt".into()]).unwrap();
        let partial_worktree = std::fs::read_to_string(self.repo.borrow().join("partial.txt")).unwrap();
        self.filter.set_text("no-such-file-ux-fixture");
        glib::timeout_future(Duration::from_millis(250)).await;
        let no_matches = self.selected.borrow().is_none();
        let mut child = self.diff_view.first_child();
        let mut clear = None;
        while let Some(widget) = child {
            child = widget.next_sibling();
            if let Ok(button) = widget.downcast::<gtk::Button>() {
                if button.label().as_deref() == Some("Clear filter") { clear = Some(button); }
            }
        }
        clear.unwrap().emit_clicked();
        glib::timeout_future(Duration::from_millis(250)).await;
        let restored = self.qa_state();
        self.more.popup();
        glib::timeout_future(Duration::from_millis(200)).await;
        let menu_open = self.more.popover().is_some_and(|p| p.is_mapped());
        self.more.popdown();
        json!({"initial":initial,"blank_blocked":blank_blocked,"ready":ready,"cleared_blocked":cleared_blocked,
            "bulk_label":bulk_label,"staged_shown":staged_shown,"unstaged_shown":unstaged_shown,
            "partial_index":partial_index,"partial_worktree":partial_worktree,
            "no_matches":no_matches,"restored":restored,"menu_open":menu_open})
    }

    pub(in crate::ui) async fn qa_clean(self: &Rc<Self>) -> serde_json::Value {
        let initial = self.qa_state();
        let mut child = self.diff_view.first_child();
        let mut action = None;
        let mut message = String::new();
        while let Some(widget) = child {
            child = widget.next_sibling();
            if let Some(label) = widget.downcast_ref::<gtk::Label>() { message.push_str(&label.text()); }
            if let Ok(button) = widget.downcast::<gtk::Button>() {
                if button.label().as_deref() == Some("View history") { action = Some(button); }
            }
        }
        action.unwrap().emit_clicked();
        glib::timeout_future(Duration::from_millis(400)).await;
        let history = self.history.qa_state();
        self.stack.set_visible_child_name("changes");
        json!({"initial":initial,"message":message,"history":history})
    }
}

fn emit_list_key(list: &gtk::ListBox, key: gtk::gdk::Key) -> bool {
    let controllers = list.observe_controllers();
    let controller = (0..controllers.n_items()).find_map(|i| controllers.item(i)
        .and_downcast::<gtk::EventControllerKey>()
        .filter(|controller| controller.propagation_phase() == gtk::PropagationPhase::Capture)).unwrap();
    controller.emit_by_name::<bool>("key-pressed", &[&key, &0_u32, &gtk::gdk::ModifierType::empty()])
}
