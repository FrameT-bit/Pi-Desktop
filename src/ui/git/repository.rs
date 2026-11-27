use super::*;

impl GitUi {
    pub(super) fn bind_repository(self: &Rc<Self>) {
        let weak = Rc::downgrade(self);
        self.open_button.connect_clicked(move |_| {
            if let Some(me) = weak.upgrade() { me.pick_repository(); }
        });
        let weak = Rc::downgrade(self);
        self.workspace_button.connect_clicked(move |_| {
            if let Some(me) = weak.upgrade() { me.use_workspace(); }
        });
        self.update_repository_controls();
    }

    pub fn set_workspace(self: &Rc<Self>, workspace: PathBuf) {
        if *self.workspace.borrow() == workspace { return; }
        *self.workspace.borrow_mut() = workspace.clone();
        if !self.pinned.get() { self.apply_repository(workspace, false); }
    }

    fn use_workspace(self: &Rc<Self>) {
        if self.busy.get() || self.opening.get() { return; }
        let workspace = self.workspace.borrow().clone();
        self.apply_repository(workspace, false);
    }

    fn apply_repository(self: &Rc<Self>, repo: PathBuf, pinned: bool) {
        self.drafts.borrow_mut().insert(self.repo.borrow().clone(), self.commit_entry.text().to_string());
        self.pinned.set(pinned);
        *self.repo.borrow_mut() = repo.clone();
        self.generation.set(self.generation.get() + 1);
        *self.selected.borrow_mut() = None;
        *self.destination.borrow_mut() = None;
        *self.snapshot.borrow_mut() = None;
        self.history.set_repo(repo.clone());
        self.render_files(&[]);
        self.filter.set_text("");
        let draft = self.drafts.borrow().get(&repo).cloned().unwrap_or_default();
        self.commit_entry.set_text(&draft);
        self.set_status("", false);
        self.meta.set_text("Loading repository…");
        self.clear_diff();
        self.count.set(0);
        self.notify_count();
        self.update_repository_controls();
        self.refresh();
    }

    pub(super) fn update_repository_controls(&self) {
        let valid = self.snapshot.borrow().is_some();
        let available = !self.busy.get() && !self.opening.get();
        self.open_button.set_sensitive(available);
        self.workspace_button.set_sensitive(available);
        self.workspace_button.set_visible(self.pinned.get());
        self.repo_controls.set_visible(valid);
        self.repo_controls.set_sensitive(valid && available);
        self.tabs.set_visible(valid);
        self.stack.set_visible(valid);
        self.stack.set_sensitive(available);
        self.operation_row.set_sensitive(available);
        if !valid {
            self.operation_row.set_visible(false);
            self.commit_button.set_sensitive(false);
            self.push_button.set_sensitive(false);
        }
        let path = self.repo.borrow().to_string_lossy().to_string();
        self.repo_label.set_text(&format!("{} · {path}", if self.pinned.get() { "Repository" } else { "Workspace" }));
        self.repo_label.set_tooltip_text(Some(&path));
        if valid { self.open_button.remove_css_class("accent"); }
        else { self.open_button.add_css_class("accent"); }
        self.update_actions();
    }

    fn pick_repository(self: &Rc<Self>) {
        if self.busy.get() || self.opening.replace(true) { return; }
        self.update_repository_controls();
        let dialog = gtk::FileDialog::builder().title("Open Git repository")
            .accept_label("Open repository").modal(true).build();
        dialog.set_initial_folder(Some(&gtk::gio::File::for_path(&*self.repo.borrow())));
        let me = self.clone();
        // Headless fixtures inject the chooser's result, not a second selection
        // path; normal launches always use the native folder picker.
        if std::env::var_os("PI_DESKTOP_QA_REPORT").is_some() {
            if let Some(folder) = std::env::var_os("PI_DESKTOP_QA_PICK_REPOSITORY") {
                me.folder_chosen(Ok(gtk::gio::File::for_path(folder)));
                return;
            }
        }
        dialog.select_folder(self.window().as_ref(), None::<&gtk::gio::Cancellable>,
            move |result| me.folder_chosen(result));
    }

    fn folder_chosen(self: &Rc<Self>, result: Result<gtk::gio::File, glib::Error>) {
        match result {
            Ok(file) => {
                let Some(folder) = file.path() else {
                    self.opening.set(false);
                    self.update_repository_controls();
                    self.set_status("Choose a local repository folder.", true);
                    return;
                };
                self.open_repository(folder);
            }
            Err(error) => {
                self.opening.set(false);
                self.update_repository_controls();
                if !error.matches(gtk::DialogError::Dismissed) && !error.matches(gtk::DialogError::Cancelled) {
                    self.set_status(&error.to_string(), true);
                }
            }
        }
    }

    fn open_repository(self: &Rc<Self>, folder: PathBuf) {
        if self.busy.get() {
            self.opening.set(false);
            self.update_repository_controls();
            return;
        }
        self.opening.set(true);
        self.set_status("Opening repository…", false);
        let seq = self.open_seq.get() + 1;
        self.open_seq.set(seq);
        self.update_repository_controls();
        let me = self.clone();
        task(folder, git::repository_root, move |result| {
            if seq != me.open_seq.get() { return; }
            me.opening.set(false);
            match result {
                Ok(repo) => me.apply_repository(repo, true),
                Err(error) => {
                    me.set_status(&format!("Could not open repository: {error}\nChoose the repository folder or a folder inside it."), true);
                    me.update_repository_controls();
                }
            }
        });
    }

    pub(in crate::ui) async fn qa_repository(self: &Rc<Self>) -> serde_json::Value {
        let workspace = self.workspace.borrow().clone();
        let initial = self.qa_state();
        self.open_button.emit_clicked();
        glib::timeout_future(Duration::from_millis(450)).await;
        let opened = self.qa_state();
        self.commit_entry.set_text("Draft for external repository");
        let alternate = workspace.join("alternate");
        std::fs::create_dir_all(&alternate).unwrap();
        self.set_workspace(alternate);
        let independent = self.qa_state();
        self.set_workspace(workspace.clone());
        self.folder_chosen(Err(glib::Error::new(gtk::DialogError::Dismissed, "Fixture cancelled")));
        let cancelled = self.qa_state();
        self.open_repository(workspace.clone());
        glib::timeout_future(Duration::from_millis(300)).await;
        let invalid = self.qa_state();
        self.busy.set(true);
        self.update_repository_controls();
        self.open_button.emit_clicked();
        self.workspace_button.emit_clicked();
        let during_operation = self.qa_state();
        self.busy.set(false);
        self.update_repository_controls();
        self.workspace_button.emit_clicked();
        glib::timeout_future(Duration::from_millis(400)).await;
        let reset = self.qa_state();
        self.open_button.emit_clicked();
        glib::timeout_future(Duration::from_millis(400)).await;
        let reopened = self.qa_state();
        let draft = self.commit_entry.text().to_string();
        self.groups[0].unselect_all();
        let position = self.files.borrow()[0].iter().position(|f| f.path == "a -> b.txt").unwrap();
        self.groups[0].select_row(self.groups[0].row_at_index(position as i32).as_ref());
        self.transfer(0, false);
        glib::timeout_future(Duration::from_millis(400)).await;
        let staged = self.qa_state();
        let workspace_untouched = !workspace.join(".git").exists() && git::snapshot(&workspace).is_err();
        json!({"initial":initial,"opened":opened,"independent":independent,"cancelled":cancelled,
            "invalid":invalid,"during_operation":during_operation,"reset":reset,"reopened":reopened,
            "draft":draft,"staged":staged,"workspace_untouched":workspace_untouched})
    }
}
