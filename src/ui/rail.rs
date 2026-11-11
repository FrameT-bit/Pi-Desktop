
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Instant;

use gtk::prelude::*;

use crate::icons::{self, Tone};

struct Subagent {
    id: String,
    running: Rc<std::cell::Cell<bool>>,
    progress: gtk::Label,
    time: gtk::Label,
    state: gtk::Label,
    started: Instant,
}

struct Activity {
    id: String,
    name: String,
    started: Instant,
    state: gtk::Label,
    time: gtk::Label,
    running: Rc<std::cell::Cell<bool>>,
    output: gtk::Label,
    expanded: Rc<std::cell::Cell<bool>>,
    marker: gtk::Image,
}

pub struct RailUi {
    pub root: gtk::Box,
    tabs: [gtk::ToggleButton; 3],
    stack: gtk::Stack,
    plan_box: gtk::Box,
    plan_list: gtk::Box,
    activity_list: gtk::Box,
    activity_empty: gtk::Label,
    subagents_list: gtk::Box,
    subagents_empty: gtk::Label,
    subagents: RefCell<Vec<Subagent>>,
    activities: RefCell<Vec<Activity>>,
    queue: gtk::Label,
    model: gtk::Label,
    session: gtk::Label,
    tokens: gtk::Label,
    cost: gtk::Label,
    context: gtk::Label,
    cache: gtk::Label,
    workspace: gtk::Label,
    preview_title: gtk::Label,
    preview_text: gtk::TextView,
    preview_picture: gtk::Picture,
    preview_empty: gtk::Label,
}

impl RailUi {
    pub fn build() -> Rc<Self> {
        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        root.add_css_class("rail");
        root.set_vexpand(true);

        let tab_bar = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        tab_bar.add_css_class("rail-tabs");
        let stack = gtk::Stack::new();
        stack.set_transition_type(gtk::StackTransitionType::Crossfade);
        stack.set_transition_duration(120);
        stack.set_vexpand(true);

        let activity = gtk::Box::new(gtk::Orientation::Vertical, 0);
        activity.add_css_class("rail-page");

        let plan_box = gtk::Box::new(gtk::Orientation::Vertical, 4);
        plan_box.add_css_class("rail-plan");
        plan_box.set_visible(false);
        let plan_title = gtk::Label::new(Some("plan"));
        plan_title.add_css_class("rail-block-title");
        plan_title.set_xalign(0.0);
        let plan_list = gtk::Box::new(gtk::Orientation::Vertical, 2);
        plan_box.append(&plan_title);
        plan_box.append(&plan_list);
        activity.append(&plan_box);

        let queue = gtk::Label::new(None);
        queue.add_css_class("rail-queue");
        queue.set_xalign(0.0);
        queue.set_visible(false);
        activity.append(&queue);
        let subagents_title = gtk::Label::new(Some("sub-agents"));
        subagents_title.add_css_class("rail-block-title");
        subagents_title.set_xalign(0.0);
        activity.append(&subagents_title);
        let subagents_empty = gtk::Label::new(Some("no sub-agents in this session"));
        subagents_empty.add_css_class("rail-empty");
        subagents_empty.set_xalign(0.0);
        activity.append(&subagents_empty);
        let subagents_list = gtk::Box::new(gtk::Orientation::Vertical, 0);
        subagents_list.add_css_class("rail-list");
        activity.append(&subagents_list);

        let tools_title = gtk::Label::new(Some("tools"));
        tools_title.add_css_class("rail-block-title");
        tools_title.set_xalign(0.0);
        activity.append(&tools_title);
        let activity_empty = gtk::Label::new(Some("Nothing running now"));
        activity_empty.add_css_class("rail-empty");
        activity_empty.set_xalign(0.0);
        activity.append(&activity_empty);
        let activity_list = gtk::Box::new(gtk::Orientation::Vertical, 0);
        activity_list.add_css_class("rail-list");
        let activity_scroll = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vscrollbar_policy(gtk::PolicyType::Automatic)
            .vexpand(true)
            .child(&activity_list)
            .build();
        activity.append(&activity_scroll);
        stack.add_named(&activity, Some("activity"));

        let preview = gtk::Box::new(gtk::Orientation::Vertical, 0);
        preview.add_css_class("rail-page");
        let preview_title = gtk::Label::new(Some("preview"));
        preview_title.add_css_class("rail-block-title");
        preview_title.set_xalign(0.0);
        preview_title.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
        preview.append(&preview_title);
        let open = gtk::Button::with_label("Open file...");
        open.add_css_class("rail-open");
        open.set_halign(gtk::Align::Start);
        preview.append(&open);
        let preview_empty = gtk::Label::new(Some(
            "images, Markdown, and workspace text appear here",
        ));
        preview_empty.add_css_class("rail-empty");
        preview_empty.set_xalign(0.0);
        preview_empty.set_wrap(true);
        preview.append(&preview_empty);
        let preview_text = gtk::TextView::new();
        preview_text.add_css_class("rail-preview");
        preview_text.set_editable(false);
        preview_text.set_monospace(true);
        preview_text.set_wrap_mode(gtk::WrapMode::WordChar);
        preview_text.set_visible(false);
        let preview_scroll = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vscrollbar_policy(gtk::PolicyType::Automatic)
            .vexpand(true)
            .child(&preview_text)
            .build();
        preview.append(&preview_scroll);
        let preview_picture = gtk::Picture::new();
        preview_picture.set_can_shrink(true);
        preview_picture.set_visible(false);
        preview.append(&preview_picture);
        stack.add_named(&preview, Some("preview"));

        let context_page = gtk::Box::new(gtk::Orientation::Vertical, 0);
        context_page.add_css_class("rail-page");
        let context_title = gtk::Label::new(Some("context"));
        context_title.add_css_class("rail-block-title");
        context_title.set_xalign(0.0);
        context_page.append(&context_title);
        let model = rail_value("model");
        let session = rail_value("Session");
        let tokens = rail_value("tokens");
        let cost = rail_value("cost");
        let context = rail_value("context");
        let cache = rail_value("cache");
        let workspace = rail_value("workspace");
        for (key, value) in [
            ("model", &model),
            ("Session", &session),
            ("tokens", &tokens),
            ("cost", &cost),
            ("context", &context),
            ("cache", &cache),
            ("workspace", &workspace),
        ] {
            context_page.append(&rail_row(key, value));
        }
        stack.add_named(&context_page, Some("context"));
        stack.set_visible_child_name("activity");

        let labels = ["activity", "preview", "context"];
        let mut tabs: Vec<gtk::ToggleButton> = Vec::new();
        for (index, label) in labels.iter().enumerate() {
            let button = gtk::ToggleButton::with_label(label);
            button.add_css_class("rail-tab");
            if let Some(first) = tabs.first() {
                button.set_group(Some(first));
            }
            if index == 0 {
                button.set_active(true);
            }
            tab_bar.append(&button);
            tabs.push(button);
        }
        let names = ["activity", "preview", "context"];
        for (index, button) in tabs.iter().enumerate() {
            let stack = stack.clone();
            let names = names;
            button.connect_toggled(move |button| {
                if button.is_active() {
                    stack.set_visible_child_name(names[index]);
                }
            });
        }
        root.append(&tab_bar);
        root.append(&stack);

        let rail = Rc::new(Self {
            root,
            tabs: [tabs[0].clone(), tabs[1].clone(), tabs[2].clone()],
            stack,
            plan_box,
            plan_list,
            activity_list,
            activity_empty,
            subagents_list,
            subagents_empty,
            subagents: RefCell::new(Vec::new()),
            activities: RefCell::new(Vec::new()),
            queue,
            model,
            session,
            tokens,
            cost,
            context,
            cache,
            workspace,
            preview_title,
            preview_text,
            preview_picture,
            preview_empty,
        });

        let me = rail.clone();
        open.connect_clicked(move |_| me.pick_preview());
        rail
    }

    pub fn set_tab(&self, index: usize) {
        if let Some(tab) = self.tabs.get(index.min(2)) {
            tab.set_active(true);
        }
    }

    pub fn set_plan(&self, lines: &[String]) {
        while let Some(child) = self.plan_list.first_child() {
            self.plan_list.remove(&child);
        }
        let cleaned: Vec<String> = lines
            .iter()
            .map(|line| crate::extension_ui::strip_ansi(line).trim().to_string())
            .filter(|line| !line.is_empty())
            .collect();
        if cleaned.is_empty() {
            self.plan_box.set_visible(false);
            return;
        }
        for line in cleaned.iter().take(8) {
            let done = line.starts_with('☑')
                || line.starts_with("[x]")
                || line.starts_with("[X]")
                || line.starts_with('✓');
            let text = line
                .trim_start_matches(['☑', '☐', '✓', ' '])
                .trim_start_matches("[x]")
                .trim_start_matches("[X]")
                .trim_start_matches(']')
                .trim();
            let label = gtk::Label::new(Some(&format!(
                "{} {text}",
                if done { "✓" } else { "○" }
            )));
            label.add_css_class("rail-plan-item");
            if done {
                label.add_css_class("done");
            }
            label.set_xalign(0.0);
            label.set_wrap(true);
            label.set_wrap_mode(gtk::pango::WrapMode::WordChar);
            label.set_max_width_chars(44);
            self.plan_list.append(&label);
        }
        if cleaned.len() > 8 {
            let more = gtk::Label::new(Some(&format!("+{} item(s)", cleaned.len() - 8)));
            more.add_css_class("rail-plan-more");
            more.set_xalign(0.0);
            self.plan_list.append(&more);
        }
        self.plan_box.set_visible(true);
    }

    pub fn subagent_start(self: &Rc<Self>, id: &str, label: &str, detail: &str) {
        if self.subagents.borrow().iter().any(|item| item.id == id) {
            return;
        }
        let row = gtk::Box::new(gtk::Orientation::Vertical, 2);
        row.add_css_class("rail-subagent");
        let head = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let dot = icons::icon("robot", Tone::Accent);
        dot.add_css_class("rail-pulse");
        head.append(&dot);
        let name = gtk::Label::new(Some(label));
        name.add_css_class("rail-activity-name");
        name.set_xalign(0.0);
        name.set_hexpand(true);
        name.set_ellipsize(gtk::pango::EllipsizeMode::End);
        head.append(&name);
        let time = gtk::Label::new(Some("0s"));
        time.add_css_class("rail-activity-time");
        head.append(&time);
        let state = gtk::Label::new(Some("running"));
        state.add_css_class("rail-activity-state");
        head.append(&state);
        row.append(&head);
        if !detail.trim().is_empty() {
            let detail = gtk::Label::new(Some(detail));
            detail.add_css_class("rail-subagent-task");
            detail.set_xalign(0.0);
            detail.set_ellipsize(gtk::pango::EllipsizeMode::End);
            detail.set_max_width_chars(44);
            row.append(&detail);
        }
        let progress = gtk::Label::new(None);
        progress.add_css_class("rail-subagent-progress");
        progress.set_xalign(0.0);
        progress.set_ellipsize(gtk::pango::EllipsizeMode::End);
        progress.set_max_width_chars(44);
        progress.set_visible(false);
        row.append(&progress);

        self.subagents_list.append(&row);
        self.subagents_empty.set_visible(false);
        self.subagents.borrow_mut().push(Subagent {
            id: id.to_string(),
            running: Rc::new(std::cell::Cell::new(true)),
            progress,
            time,
            state,
            started: Instant::now(),
        });
        let mut items = self.subagents.borrow_mut();
        while items.len() > 6 {
            items.remove(0);
        }
        if let Some(first) = self.subagents_list.first_child() {
            if items.len() >= 6 {
                self.subagents_list.remove(&first);
            }
        }
    }

    pub fn subagent_update(&self, id: &str, text: &str) {
        let items = self.subagents.borrow();
        if let Some(item) = items.iter().find(|item| item.id == id) {
            let line = text.lines().rev().find(|line| !line.trim().is_empty()).unwrap_or("");
            if !line.trim().is_empty() {
                item.progress.set_text(line.trim());
                item.progress.set_visible(true);
            }
        }
    }

    pub fn subagent_end(&self, id: &str, is_error: bool) {
        let items = self.subagents.borrow();
        if let Some(item) = items.iter().find(|item| item.id == id) {
            item.running.set(false);
            item.state.set_text(if is_error { "error" } else { "ok" });
            item.state.remove_css_class("rail-activity-state");
            item.state.add_css_class(if is_error {
                "rail-activity-error"
            } else {
                "rail-activity-ok"
            });
            item.time.set_text(&format_elapsed(item.started.elapsed().as_secs()));
        }
    }

    pub fn activity_start(self: &Rc<Self>, id: &str, name: &str) {
        if self
            .activities
            .borrow()
            .iter()
            .any(|activity| activity.id == id)
        {
            return;
        }
        let outer = gtk::Box::new(gtk::Orientation::Vertical, 0);
        outer.add_css_class("rail-activity");
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        row.set_cursor_from_name(Some("pointer"));
        let marker = icons::icon("chevron-right", Tone::Muted);
        marker.set_valign(gtk::Align::Center);
        row.append(&marker);
        let label = gtk::Label::new(Some(name));
        label.add_css_class("rail-activity-name");
        label.set_xalign(0.0);
        label.set_ellipsize(gtk::pango::EllipsizeMode::End);
        label.set_hexpand(true);
        row.append(&label);
        let time = gtk::Label::new(Some("0s"));
        time.add_css_class("rail-activity-time");
        row.append(&time);
        let state = gtk::Label::new(Some("running"));
        state.add_css_class("rail-activity-state");
        row.append(&state);
        outer.append(&row);
        let output = gtk::Label::new(None);
        output.add_css_class("rail-activity-output");
        output.set_xalign(0.0);
        output.set_wrap(true);
        output.set_wrap_mode(gtk::pango::WrapMode::WordChar);
        output.set_selectable(true);
        output.set_visible(false);
        outer.append(&output);

        let expanded = Rc::new(std::cell::Cell::new(false));
        {
            let output = output.clone();
            let marker = marker.clone();
            let expanded = expanded.clone();
            let click = gtk::GestureClick::new();
            click.set_button(gtk::gdk::BUTTON_PRIMARY);
            click.connect_released(move |_, _, _, _| {
                if output.text().trim().is_empty() {
                    return;
                }
                let next = !expanded.get();
                expanded.set(next);
                output.set_visible(next);
                icons::set_icon(
                    &marker,
                    if next { "chevron-down" } else { "chevron-right" },
                    Tone::Muted,
                );
            });
            row.add_controller(click);
        }

        self.activity_list.append(&outer);
        self.activity_empty.set_visible(false);
        self.activities.borrow_mut().push(Activity {
            id: id.to_string(),
            name: name.to_string(),
            started: Instant::now(),
            state,
            time,
            running: Rc::new(std::cell::Cell::new(true)),
            output,
            expanded,
            marker,
        });
        let mut activities = self.activities.borrow_mut();
        while activities.len() > 8 {
            activities.remove(0);
        }
        if let Some(first) = self.activity_list.first_child() {
            if activities.len() >= 8 {
                self.activity_list.remove(&first);
            }
        }
    }

    pub fn activity_update(&self, id: &str, text: &str) {
        let activities = self.activities.borrow();
        if let Some(activity) = activities.iter().find(|activity| activity.id == id) {
            let text = crate::extension_ui::strip_ansi(text);
            if !text.trim().is_empty() {
                activity.output.set_text(&truncate_output(&text));
                activity.marker.set_visible(true);
            }
        }
    }

    pub fn activity_end(&self, id: &str, is_error: bool, text: &str) {
        let activities = self.activities.borrow();
        let text = crate::extension_ui::strip_ansi(text);
        if let Some(activity) = activities.iter().find(|activity| activity.id == id) {
            activity.running.set(false);
            activity.state.set_text(if is_error { "error" } else { "ok" });
            activity.state.remove_css_class("rail-activity-state");
            activity.state.add_css_class(if is_error {
                "rail-activity-error"
            } else {
                "rail-activity-ok"
            });
            let elapsed = activity.started.elapsed().as_secs();
            activity.time.set_text(&format_elapsed(elapsed));
            if !text.trim().is_empty() {
                activity.output.set_text(&truncate_output(&text));
                activity.marker.set_visible(true);
                if is_error {
                    activity.expanded.set(true);
                    activity.output.set_visible(true);
                    icons::set_icon(&activity.marker, "chevron-down", Tone::Muted);
                }
            }
        }
    }

    pub fn tick(&self) {
        for subagent in self.subagents.borrow().iter() {
            if subagent.running.get() {
                subagent
                    .time
                    .set_text(&format_elapsed(subagent.started.elapsed().as_secs()));
            }
        }
        for activity in self.activities.borrow().iter() {
            if activity.running.get() {
                let elapsed = activity.started.elapsed().as_secs();
                activity.time.set_text(&format_elapsed(elapsed));
            }
        }
    }

    pub fn clear_activity(&self) {
        while let Some(child) = self.activity_list.first_child() {
            self.activity_list.remove(&child);
        }
        while let Some(child) = self.subagents_list.first_child() {
            self.subagents_list.remove(&child);
        }
        self.activities.borrow_mut().clear();
        self.subagents.borrow_mut().clear();
        self.activity_empty.set_visible(true);
        self.subagents_empty.set_visible(true);
    }

    pub fn set_queue(&self, count: usize) {
        self.queue.set_visible(count > 0);
        self.queue.set_text(&format!("{count} queued"));
    }

    pub fn set_stats(
        &self,
        model: &str,
        session: &str,
        tokens_in: u64,
        tokens_out: u64,
        cost: f64,
        context_percent: Option<f64>,
        cache_read: u64,
        cache_write: u64,
        context_tokens: Option<u64>,
        context_window: Option<u64>,
        workspace: &str,
    ) {
        self.model.set_text(model);
        self.session.set_text(session);
        self.tokens
            .set_text(&format!("{} ↓ · {} ↑", compact(tokens_in), compact(tokens_out)));
        self.cost.set_text(&format!("${cost:.3}"));
        self.context.set_text(&match (context_percent, context_tokens, context_window) {
            (Some(percent), Some(tokens), Some(window)) => {
                format!("{percent:.0}% · {} / {}", compact(tokens), compact(window))
            }
            (Some(percent), _, _) => format!("{percent:.0}%"),
            _ => "—".to_string(),
        });
        let total_input = tokens_in + cache_read;
        let hit = if total_input > 0 {
            format!("{:.1}% hit · {} lidos", 100.0 * cache_read as f64 / total_input as f64, compact(cache_read))
        } else {
            "—".to_string()
        };
        self.cache.set_text(&if cache_write > 0 {
            format!("{hit} · {} escritos", compact(cache_write))
        } else {
            hit
        });
        self.workspace.set_text(workspace);
    }

    pub fn set_context(&self, branch: &str) {
        if branch.trim().is_empty() {
            return;
        }
        self.workspace.set_tooltip_text(Some(branch));
    }

    pub fn set_preview(&self, title: &str, text: &str) {
        self.preview_title.set_text(title);
        self.preview_empty.set_visible(false);
        self.preview_picture.set_visible(false);
        self.preview_text.set_visible(true);
        self.preview_text.buffer().set_text(text);
        self.set_tab(1);
    }

    pub fn set_preview_error(&self, message: &str) {
        self.preview_title.set_text("preview");
        self.preview_picture.set_visible(false);
        self.preview_text.set_visible(false);
        self.preview_empty.set_text(message);
        self.preview_empty.set_visible(true);
        self.set_tab(1);
    }

    fn pick_preview(self: &Rc<Self>) {
        let dialog = gtk::FileDialog::builder()
            .title("open in preview")
            .modal(true)
            .build();
        let window = self
            .root
            .root()
            .and_then(|root| root.downcast::<gtk::Window>().ok());
        let me = self.clone();
        dialog.open(window.as_ref(), None::<&gtk::gio::Cancellable>, move |result| {
            let Ok(file) = result else {
                return;
            };
            let Some(path) = file.path() else {
                return;
            };
            me.load_preview(&path);
        });
    }

    fn load_preview(self: &Rc<Self>, path: &std::path::Path) {
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("file");
        let extension = path
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if matches!(extension.as_str(), "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp") {
            match gtk::gdk::Texture::from_filename(path) {
                Ok(texture) => {
                    self.preview_title.set_text(name);
                    self.preview_text.set_visible(false);
                    self.preview_empty.set_visible(false);
                    self.preview_picture.set_paintable(Some(&texture));
                    self.preview_picture.set_visible(true);
                    self.set_tab(1);
                }
                Err(err) => self.set_preview_error(&format!("I couldn't open the image: {err}")),
            }
            return;
        }
        let bytes = match std::fs::read(path) {
            Ok(bytes) => bytes,
            Err(err) => {
                self.set_preview_error(&format!("could not read: {err}"));
                return;
            }
        };
        let cap = 256 * 1024;
        let truncated = bytes.len() > cap;
        let slice = &bytes[..bytes.len().min(cap)];
        let text = String::from_utf8_lossy(slice);
        let mut body = text.to_string();
        if truncated {
            body.push_str("\n\n... file truncated to 256 KB");
        }
        self.set_preview(name, &body);
    }
}

fn rail_value(key: &str) -> gtk::Label {
    let label = gtk::Label::new(None);
    label.add_css_class("rail-value");
    label.set_xalign(0.0);
    label.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
    label.set_tooltip_text(Some(key));
    label
}

fn rail_row(key: &str, value: &gtk::Label) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Vertical, 1);
    row.add_css_class("rail-row");
    let key = gtk::Label::new(Some(&key.to_uppercase()));
    key.add_css_class("rail-key");
    key.set_xalign(0.0);
    row.append(&key);
    row.append(value);
    row
}

fn compact(value: u64) -> String {
    if value >= 1_000_000 {
        format!("{:.1}M", value as f64 / 1_000_000.0)
    } else if value >= 1_000 {
        format!("{:.1}k", value as f64 / 1_000.0)
    } else {
        value.to_string()
    }
}

fn truncate_output(text: &str) -> String {
    const MAX: usize = 4000;
    if text.chars().count() <= MAX {
        return text.to_string();
    }
    let mut out: String = text.chars().take(MAX).collect();
    out.push_str("\n... [trunk out at sidebar]");
    out
}

fn format_elapsed(seconds: u64) -> String {
    if seconds >= 60 {
        format!("{}m{:02}s", seconds / 60, seconds % 60)
    } else {
        format!("{seconds}s")
    }
}
