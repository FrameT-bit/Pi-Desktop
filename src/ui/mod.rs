
pub mod artifacts;
pub mod chat;
pub mod command_palette;
pub mod composer;
pub mod dialog;
pub mod fit;
pub mod git;
pub mod header;
pub mod hotkeys;
pub mod jobs;
pub mod level_slider;
pub mod menu;
pub mod particles;
pub mod plugin_config;
pub mod plugins;
pub mod popup;
pub mod rail;
pub mod settings;
pub mod sidebar;
pub mod status;
pub mod thumb;
pub mod trash;
pub mod tree;

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use adw::prelude::*;
use serde_json::{json, Value};

use crate::config::{Config, ViewMode};
use crate::icons::{self, Tone};
use crate::extension_ui::{self, Placement};
use crate::history;
use crate::images::ImageBlock;
use crate::catalog::{self as catalog_io, CatalogEvent};
use crate::loops::{self, LoopCommand, LoopState};
use crate::plugins::{self as plugin_io, PluginEvent};
use crate::rpc::{self, RpcEvent, RpcHandle};
use crate::session_ops;
use crate::state::{
    format_cost, format_duration, format_elapsed, format_tokens, slug_from_prompt, AppState,
    CommandItem, Connection, ModelItem, RemoteSession, SessionActivity, Transport, TurnPhase,
};
use crate::theme::{Theme, ThemeMode};
use crate::updater;

use chat::ChatView;
use composer::{Composer, SubmitKind};
use menu as context_menu;
use status::StatusUi;

#[derive(Clone, Copy, PartialEq, Eq)]
enum PendingQuery {
    Steer,
    Queue,
}

struct ActiveLoop {
    state: LoopState,
    next_fire: Instant,
}

struct PendingRemote {
    op: RemoteOp,
    path: String,
    return_to: Option<String>,
    step_id: String,
    stage: RemoteStage,
}

enum RemoteOp {
    Rename(String),
    Export(String),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum RemoteStage {
    Switching,
    Running,
}

struct PendingDelete {
    path: std::path::PathBuf,
}

type SlotKey = Rc<RefCell<String>>;

pub struct AppUi {
    pub window: adw::ApplicationWindow,
    pub state: Rc<RefCell<AppState>>,
    pub rpc: RefCell<RpcHandle>,
    active_key: RefCell<SlotKey>,
    pool: RefCell<HashMap<String, (SlotKey, RpcHandle)>>,
    background_requests: RefCell<HashMap<String, Vec<Value>>>,
    activity: RefCell<HashMap<String, SessionActivity>>,
    pub theme: Theme,
    pub config: RefCell<Config>,
    pub transport: RefCell<Transport>,
    pub next_id: Cell<u64>,
    pub header: header::HeaderUi,
    pub sidebar: Rc<sidebar::SidebarUi>,
    pub chat: Rc<ChatView>,
    pub composer: Rc<Composer>,
    pub plugins: Rc<plugins::PluginsUi>,
    pub jobs: Rc<jobs::JobsUi>,
    pub tree: Rc<tree::TreeUi>,
    pub artifacts: Rc<artifacts::ArtifactsUi>,
    pub git: Rc<git::GitUi>,
    pub rail: Rc<rail::RailUi>,
    rail_handle: gtk::Box,
    rail_column: gtk::Box,
    rail_paned: gtk::Paned,
    rail_width: Rc<Cell<i32>>,
    rail_toggle: gtk::Button,
    rail_toggle_image: gtk::Image,
    rail_open: Cell<bool>,
    content: gtk::Paned,
    pub status: StatusUi,
    pub stack: gtk::Stack,
    #[allow(dead_code)]
    toast: gtk::Overlay,
    toast_card: gtk::Box,
    toast_label: gtk::Label,
    toast_seq: std::cell::Cell<u32>,
    banner: gtk::Box,
    banner_label: gtk::Label,
    banner_text: RefCell<String>,
    ext_status: RefCell<extension_ui::Statuses>,
    ext_widgets: RefCell<extension_ui::Widgets>,
    bash_running: Cell<bool>,
    bash_seq: Cell<u64>,
    bash_id: RefCell<Option<String>>,
    title_override: RefCell<Option<String>>,
    loop_state: RefCell<Option<ActiveLoop>>,
    pending_query: RefCell<Option<PendingQuery>>,
    empty_title: gtk::Label,
    empty_body: gtk::Label,
    history_tx: async_channel::Sender<Vec<RemoteSession>>,
    history_rx: async_channel::Receiver<Vec<RemoteSession>>,
    plugin_tx: async_channel::Sender<PluginEvent>,
    plugin_rx: async_channel::Receiver<PluginEvent>,
    catalog_tx: async_channel::Sender<CatalogEvent>,
    catalog_rx: async_channel::Receiver<CatalogEvent>,
    pending_switch: Cell<Option<usize>>,
    fork_points: RefCell<Vec<(String, String)>>,
    pending_remote: RefCell<Option<PendingRemote>>,
    announce_clone: Cell<bool>,
    pending_delete: RefCell<Option<PendingDelete>>,
    configured_thinking: RefCell<Option<String>>,
    session_override: Option<String>,
    workspace_override: Option<std::path::PathBuf>,
    closed_tabs: RefCell<std::collections::HashSet<String>>,
    drafts: RefCell<std::collections::HashMap<String, String>>,
    pending_draft_restore: RefCell<Option<String>>,
    view: Cell<ViewMode>,
    syncing_view: Cell<bool>,
    chat_search: Cell<bool>,
    new_session_pending: Cell<bool>,
}

pub fn present(app: &adw::Application, theme: Theme, cli: &crate::config::CliArgs) {
    if let Some(window) = app.active_window() {
        window.present();
        return;
    }

    let ui = AppUi::new(app, theme, cli);
    ui.window.present();
    ui.wire();
    let view = ui.config.borrow().view;
    ui.apply_view(view, false);
    ui.composer.focus();
}

impl AppUi {
    fn new(app: &adw::Application, theme: Theme, cli: &crate::config::CliArgs) -> Rc<Self> {
        let mut config = Config::load();
        config.apply_overrides(cli.session.clone(), cli.workspace.clone());
        if !config.start_last && config.session_override().is_none() {
            let launch = config.new_launch_session_id();
            config.set_launch_session_id(Some(launch));
        }
        let initial_view = config.view;
        let session_override = config.session_override().map(str::to_string);
        let transport = Transport::from_config(&config);
        theme.set_font_scale(config.font_scale as u32);
        theme.set_font(config.font);
        let configured_thinking = match config.thinking {
            crate::config::Thinking::Unset => None,
            level => Some(level.as_str().to_string()),
        };
        let state = {
            let mut state = AppState::default();
            state.apply_config(&config);
            Rc::new(RefCell::new(state))
        };
        let restored_loop = loops::load()
            .filter(|state| state.active)
            .map(|state| ActiveLoop {
                next_fire: Instant::now() + Duration::from_secs(state.interval),
                state,
            });
        if restored_loop.is_some() {
            state.borrow_mut().jobs = 1;
        }
        let header = header::build(&state.borrow().model, &theme);
        let sidebar = Rc::new(sidebar::SidebarUi::build(&state.borrow()));
        let chat = ChatView::new(&theme);
        chat.set_show_timestamps(config.timestamps);
        chat.set_auto_scroll(config.auto_scroll);
        chat.set_chat_width(config.chat_width);
        let composer = Composer::new();
        let plugins = plugins::PluginsUi::new();
        let jobs = jobs::JobsUi::build();
        let tree = tree::TreeUi::build();
        let artifacts = artifacts::ArtifactsUi::build(crate::config::pi_agent_dir());
        let git = git::GitUi::build(crate::config::expand_home(config.effective_workspace()));
        let rail = rail::RailUi::build();
        let status = status::build();
        status.set_show_costs(config.show_costs);
        let (history_tx, history_rx) = async_channel::unbounded::<Vec<RemoteSession>>();
        let (plugin_tx, plugin_rx) = async_channel::unbounded::<PluginEvent>();
        let (catalog_tx, catalog_rx) = async_channel::unbounded::<CatalogEvent>();

        let right = gtk::Box::new(gtk::Orientation::Vertical, 0);
        right.set_hexpand(true);
        right.set_vexpand(true);
        right.append(&chat.root);
        right.append(&composer.root);

        let empty_title = gtk::Label::new(None);
        empty_title.add_css_class("panel-title");
        let empty_body = gtk::Label::new(None);
        empty_body.add_css_class("empty-page");
        empty_body.set_wrap(true);
        empty_body.set_justify(gtk::Justification::Center);
        let empty_button = gtk::Button::with_label("Soon");
        empty_button.add_css_class("empty-action");
        empty_button.set_halign(gtk::Align::Center);
        empty_button.set_sensitive(false);
        let empty = gtk::Box::new(gtk::Orientation::Vertical, 8);
        empty.set_halign(gtk::Align::Center);
        empty.set_valign(gtk::Align::Center);
        empty.set_hexpand(true);
        empty.set_vexpand(true);
        empty.append(&empty_title);
        empty.append(&empty_body);
        empty.append(&empty_button);

        let stack = gtk::Stack::new();
        stack.add_css_class("content-panel");
        stack.set_transition_type(gtk::StackTransitionType::Crossfade);
        stack.set_transition_duration(140);
        stack.set_hexpand(true);
        stack.set_vexpand(true);
        stack.add_named(&right, Some("chat"));
        stack.add_named(&artifacts.root, Some("artifacts"));
        stack.add_named(&empty, Some("empty"));
        stack.add_named(&jobs.root, Some("jobs"));
        stack.add_named(&tree.root, Some("tree"));
        stack.add_named(&plugins.root, Some("plugins"));
        stack.add_named(&git.root, Some("git"));
        stack.set_visible_child_name("chat");

        let rail_toggle_image = icons::icon("chevron-right", Tone::Muted);
        let rail_toggle = gtk::Button::new();
        rail_toggle.add_css_class("rail-toggle");
        rail_toggle.set_child(Some(&rail_toggle_image));
        rail_toggle.set_valign(gtk::Align::Start);
        rail_toggle.set_tooltip_text(Some("collapse panel"));

        let rail_grip = gtk::Box::new(gtk::Orientation::Vertical, 0);
        rail_grip.add_css_class("rail-grip");
        rail_grip.set_vexpand(true);
        rail_grip.set_cursor_from_name(Some("col-resize"));

        let rail_handle = gtk::Box::new(gtk::Orientation::Vertical, 0);
        rail_handle.add_css_class("rail-handle");
        rail_handle.set_size_request(18, -1);
        rail_handle.append(&rail_toggle);
        rail_handle.append(&rail_grip);

        let rail_column = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        rail_column.add_css_class("rail-column");
        rail_column.append(&rail_handle);
        rail_column.append(&rail.root);

        let rail_paned = gtk::Paned::new(gtk::Orientation::Horizontal);
        rail_paned.add_css_class("rail-split");
        rail_paned.set_start_child(Some(&stack));
        rail_paned.set_end_child(Some(&rail_column));
        rail_paned.set_resize_start_child(true);
        rail_paned.set_shrink_start_child(false);
        rail_paned.set_shrink_end_child(true);
        rail_paned.set_wide_handle(false);
        let rail_width = Rc::new(Cell::new(340i32));
        {
            let paned = rail_paned.clone();
            let width = rail_width.clone();
            paned.connect_map(move |paned| {
                let total = paned.width();
                if total > 1 {
                    paned.set_position((total - width.get().clamp(280, 460)).max(0));
                }
            });
        }
        {
            let paned = rail_paned.clone();
            let width = rail_width.clone();
            let start_width = Rc::new(Cell::new(340i32));
            let drag = gtk::GestureDrag::new();
            let drag_begin = paned.clone();
            let begin_width = start_width.clone();
            drag.connect_drag_begin(move |_, _, _| {
                let total = drag_begin.width();
                let position = drag_begin.position();
                let current = if total > 1 { total - position } else { 340 };
                begin_width.set(current.clamp(280, 460));
            });
            let drag_update = paned.clone();
            let update_width = width.clone();
            let update_start = start_width.clone();
            drag.connect_drag_update(move |_, offset_x, _| {
                let total = drag_update.width();
                let wanted = (update_start.get() as f64 - offset_x).clamp(280.0, 460.0) as i32;
                drag_update.set_position((total - wanted).max(0));
                update_width.set(wanted);
            });
            rail_grip.add_controller(drag);
        }

        let content = gtk::Paned::new(gtk::Orientation::Horizontal);
        content.add_css_class("content-split");
        content.set_start_child(Some(&sidebar.root));
        content.set_end_child(Some(&rail_paned));
        content.set_position(264);
        content.set_resize_start_child(false);
        content.set_shrink_start_child(false);
        content.set_wide_handle(false);
        content.set_hexpand(true);
        content.set_vexpand(true);

        let sidebar_for_clamp = sidebar.root.clone();
        content.connect_notify_local(Some("position"), move |paned, _| {
            let clamped = if sidebar_for_clamp.is_visible() {
                paned.position().clamp(200, 360)
            } else {
                0
            };
            if paned.position() != clamped {
                paned.set_position(clamped);
            }
        });

        let root = gtk::Box::new(gtk::Orientation::Vertical, 12);
        root.add_css_class("app-shell");
        root.set_margin_start(24);
        root.set_margin_end(24);
        root.set_margin_top(12);
        root.set_margin_bottom(12);
        root.append(&header.root);
        root.append(&content);
        root.append(&status.root);

        let grid = gtk::DrawingArea::new();
        grid.add_css_class("bg-grid");
        grid.set_hexpand(true);
        grid.set_vexpand(true);
        grid.set_can_target(false);
        let theme_draw = theme.clone();
        grid.set_draw_func(move |_, cr, width, height| {
            let (r, g, b) = theme_draw.page_bg_rgb();
            cr.set_source_rgb(r, g, b);
            let _ = cr.paint();

            let (r, g, b) = theme_draw.border_rgb();
            cr.set_source_rgba(r, g, b, 0.06);
            cr.set_line_width(1.0);
            let step = 20.0;
            let mut x = 0.5;
            while x < width as f64 {
                cr.move_to(x, 0.0);
                cr.line_to(x, height as f64);
                x += step;
            }
            let mut y = 0.5;
            while y < height as f64 {
                cr.move_to(0.0, y);
                cr.line_to(width as f64, y);
                y += step;
            }
            let _ = cr.stroke();
        });

        let grid_c = grid.clone();
        theme.connect_changed(move || {
            grid_c.queue_draw();
        });

        let shell = gtk::Overlay::new();
        shell.set_child(Some(&grid));
        shell.add_overlay(&root);

        let banner_label = gtk::Label::new(None);
        banner_label.add_css_class("banner-text");
        banner_label.set_xalign(0.0);
        banner_label.set_wrap(true);
        banner_label.set_selectable(true);
        banner_label.set_hexpand(true);
        let banner_copy = gtk::Button::with_label("copy");
        banner_copy.add_css_class("banner-action");
        let banner_close = gtk::Button::with_label("close");
        banner_close.add_css_class("banner-action");
        let banner = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        banner.add_css_class("error-banner");
        banner.set_visible(false);
        banner.append(&banner_label);
        banner.append(&banner_copy);
        banner.append(&banner_close);
        right.insert_child_after(&banner, Some(&chat.root));

        let toast_card = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        toast_card.add_css_class("toast-card");
        toast_card.set_margin_start(12);
        toast_card.set_margin_end(12);
        toast_card.set_margin_bottom(8);
        toast_card.set_visible(false);
        right.insert_child_after(&toast_card, Some(&chat.root));
        let toast_label = gtk::Label::new(None);
        toast_label.add_css_class("toast-text");
        toast_label.set_wrap(true);
        toast_label.set_xalign(0.0);
        toast_label.set_max_width_chars(56);
        toast_card.append(&toast_label);

        let toast = gtk::Overlay::new();
        toast.set_child(Some(&shell));

        let window = adw::ApplicationWindow::builder()
            .application(app)
            .title("pi")
            .default_width(1280)
            .default_height(840)
            .build();
        window.set_size_request(960, 560);
        window.set_icon_name(Some("pi-desktop"));
        window.set_content(Some(&toast));

        let rpc = rpc::spawn(transport.clone());
        let active_key = Rc::new(RefCell::new(config.effective_session_id().to_string()));

        Rc::new(Self {
            window,
            state,
            rpc: RefCell::new(rpc),
            active_key: RefCell::new(active_key),
            pool: RefCell::new(HashMap::new()),
            background_requests: RefCell::new(HashMap::new()),
            activity: RefCell::new(HashMap::new()),
            theme,
            config: RefCell::new(config),
            transport: RefCell::new(transport),
            next_id: Cell::new(1),
            header,
            sidebar,
            chat,
            composer,
            plugins,
            jobs,
            tree,
            artifacts,
            git,
            rail,
            rail_handle,
            rail_column,
            rail_paned,
            rail_width,
            rail_toggle,
            rail_toggle_image,
            rail_open: Cell::new(true),
            content,
            status,
            stack,
            toast,
            toast_card,
            toast_label,
            toast_seq: std::cell::Cell::new(0),
            banner,
            banner_label,
            banner_text: RefCell::new(String::new()),
            ext_status: RefCell::new(extension_ui::Statuses::default()),
            ext_widgets: RefCell::new(extension_ui::Widgets::default()),
            bash_running: Cell::new(false),
            bash_seq: Cell::new(1),
            bash_id: RefCell::new(None),
            title_override: RefCell::new(None),
            loop_state: RefCell::new(restored_loop),
            pending_query: RefCell::new(None),
            empty_title,
            empty_body,
            history_tx,
            history_rx,
            plugin_tx,
            plugin_rx,
            catalog_tx,
            catalog_rx,
            pending_switch: Cell::new(None),
            fork_points: RefCell::new(Vec::new()),
            pending_remote: RefCell::new(None),
            announce_clone: Cell::new(false),
            pending_delete: RefCell::new(None),
            configured_thinking: RefCell::new(configured_thinking),
            session_override,
            workspace_override: cli.workspace.clone(),
            closed_tabs: RefCell::new(std::collections::HashSet::new()),
            drafts: RefCell::new(std::collections::HashMap::new()),
            pending_draft_restore: RefCell::new(None),
            view: Cell::new(initial_view),
            syncing_view: Cell::new(false),
            chat_search: Cell::new(false),
            new_session_pending: Cell::new(false),
        })
    }

    pub fn apply_view(self: &Rc<Self>, view: ViewMode, persist: bool) {
        let chat = view.is_chat();
        self.syncing_view.set(true);
        self.view.set(view);
        self.header.set_view(chat);
        self.header.mode_chat.set_active(chat);
        self.header.mode_code.set_active(!chat);
        self.sidebar.set_chat_mode(chat);
        self.chat.set_chat_mode(chat);
        self.composer.set_chat_mode(chat);
        self.status.set_chat_mode(chat);
        if chat {
            self.chat_search.set(false);
            self.composer.set_search_active(false);
            self.sidebar.set_active(2);
            self.stack.set_visible_child_name("chat");
            self.rail.root.set_visible(false);
            self.rail_handle.set_visible(false);
            self.rail_column.set_visible(false);
        } else {
            self.chat_search.set(false);
            self.composer.set_search_active(false);
            let is_chat_page = self.stack.visible_child_name().as_deref() == Some("chat");
            self.rail_column.set_visible(is_chat_page);
            self.rail.root
                .set_visible(is_chat_page && self.rail_open.get());
            self.rail_handle.set_visible(is_chat_page);
            self.apply_rail_layout();
            self.update_rail_toggle();
        }
        let title = if chat {
            self.state.borrow().display_name()
        } else {
            String::new()
        };
        self.header.set_chat_title(&title);
        self.refresh_status();
        self.syncing_view.set(false);
        if persist {
            let mut config = self.config.borrow_mut();
            config.view = view;
            let _ = config.save();
        }
    }

    fn toggle_sidebar(&self) {
        let visible = self.sidebar.root.is_visible();
        self.sidebar.root.set_visible(!visible);
        self.content.set_position(if visible { 0 } else { 264 });
    }

    fn id(&self) -> String {
        let id = self.next_id.get();
        self.next_id.set(id + 1);
        format!("req-{id}")
    }

    fn send(&self, value: Value) {
        self.rpc.borrow().send(value);
    }

    fn wire(self: &Rc<Self>) {
        {
            let me = self.clone();
            self.header
                .help
                .connect_clicked(move |_| hotkeys::show(&me.window));
        }
        {
            let me = self.clone();
            self.header.mode_chat.connect_clicked(move |_| {
                if me.syncing_view.get() {
                    return;
                }
                me.apply_view(ViewMode::Chat, true);
            });
        }
        {
            let me = self.clone();
            self.header.mode_code.connect_clicked(move |_| {
                if me.syncing_view.get() {
                    return;
                }
                me.apply_view(ViewMode::Code, true);
            });
        }
        {
            let me = self.clone();
            self.header
                .sidebar_toggle
                .connect_clicked(move |_| me.toggle_sidebar());
        }
        {
            let me = self.clone();
            self.rail_toggle
                .connect_clicked(move |_| me.toggle_rail());
        }
        {
            let me = self.clone();
            self.composer
                .set_on_search_toggle(move |active| me.chat_search.set(active));
        }
        {
            let me = self.clone();
            self.git
                .set_on_count(move |count| me.sidebar.set_git_count(count));
        }
        {
            let me = self.clone();
            self.artifacts.set_on_count(move |count| {
                me.state.borrow_mut().artifacts = count;
                me.sidebar.set_artifact_count(count);
            });
        }
        {
            let me = self.clone();
            self.status
                .plan_button
                .connect_clicked(move |_| me.send_extension_command_text("/plan"));
        }
        {
            let me = self.clone();
            self.status.conn_button.connect_clicked(move |_| {
                let config = me.config.borrow().clone();
                me.reconnect(config);
            });
        }
        {
            let me = self.clone();
            let banner = self.banner.clone();
            let banner_label = self.banner_label.clone();
            if let Some(copy) = banner_label.next_sibling().and_then(|w| w.downcast::<gtk::Button>().ok()) {
                let me_copy = me.clone();
                let banner_text = self.banner_text.clone();
                copy.connect_clicked(move |_| {
                    let text = banner_text.borrow().clone();
                    if me_copy.copy_to_clipboard(&text) {
                        me_copy.status.flash_notice("copied error");
                    }
                });
            }
            if let Some(close) = banner_label.next_sibling().and_then(|w| w.next_sibling()).and_then(|w| w.downcast::<gtk::Button>().ok()) {
                close.connect_clicked(move |_| banner.set_visible(false));
            }
        }
        let me = self.clone();
        self.composer.set_on_submit(move |kind| me.submit(kind));

        {
            let config = self.config.borrow();
            if matches!(config.target, crate::config::Target::Local) {
                let root = crate::config::expand_home(config.effective_workspace());
                let files = crate::files::list_files(&root, crate::files::MAX_FILES);
                self.composer.set_workspace_files(files);
            }
        }

        let me = self.clone();
        self.composer.set_on_stop(move || me.stop());

        let me = self.clone();
        self.composer.set_on_command(move |name| {
            me.composer.set_text(&format!("/{name}"));
            me.submit(SubmitKind::Enter);
        });

        let me = self.clone();
        self.composer.set_on_loop_stop(move || me.stop_loop(None));

        let me = self.clone();
        self.jobs.set_on_stop(move || me.stop_loop(None));

        let me = self.clone();
        self.tree.set_on_reload(move || me.ask_tree());

        let me = self.clone();
        self.tree.set_on_fork(move |entry| me.fork_from(&entry));

        let me = self.clone();
        self.sidebar.new_session.connect_clicked(move |_| me.new_session());

        for (index, row) in self.sidebar.nav.iter().enumerate() {
            let me = self.clone();
            row.button.connect_clicked(move |_| me.select_nav(index));
        }

        let me = self.clone();
        self.sidebar.set_on_session(move |index| me.select_session(index));

        let me = self.clone();
        self.sidebar.set_on_rename(move |index, name| me.rename_session(index, &name));

        let me = self.clone();
        self.sidebar.set_on_duplicate(move |index| me.duplicate_session(index));

        let me = self.clone();
        self.sidebar.set_on_delete(move |index| me.delete_session(index));

        let me = self.clone();
        self.sidebar.set_on_export(move |index| me.export_session(index));

        let me = self.clone();
        self.sidebar.set_on_open_side(move |index| me.open_side(index));

        let me = self.clone();
        self.chat.set_on_empty_action(move |action| match action {
            "provider" => me.open_settings_tab("PROVIDERS"),
            "sessions" => me.sidebar.open_search(),
            "new" => me.composer.focus(),
            "hotkeys" => hotkeys::show(&me.window),
            _ => {}
        });

        let me = self.clone();
        self.chat.set_on_user_menu(move |anchor, text| {
            me.show_message_menu(&anchor, &text)
        });

        {
            let me = self.clone();
            let button = self.status.perm_button.clone();
            let popover = gtk::Popover::new();
            popover.add_css_class("permission-popover");
            popover.set_has_arrow(true);
            popover.set_position(gtk::PositionType::Top);
            popover.set_parent(&button);
            let menu = gtk::Box::new(gtk::Orientation::Vertical, 0);
            menu.add_css_class("permission-menu");
            for permission in crate::state::Permission::all() {
                let item = gtk::Button::with_label(permission.label());
                item.add_css_class("permission-option");
                item.set_halign(gtk::Align::Fill);
                let me_item = me.clone();
                let popover_item = popover.clone();
                item.connect_clicked(move |_| {
                    popover_item.popdown();
                    me_item.set_permission(permission);
                });
                menu.append(&item);
            }
            popover.set_child(Some(&menu));
            button.connect_clicked(move |_| popover.popup());
        }

        let me = self.clone();
        let (provider, id) = {
            let state = self.state.borrow();
            (state.model_provider.clone(), state.model_id.clone())
        };
        self.header.set_on_model(move |model_provider, model_id, name| {
            me.set_model(model_provider, model_id, name)
        });
        self.header.set_menu(&self.state.borrow().models, &provider, &id);
        self.composer.set_thinking_levels(
            &self.state.borrow().thinking_levels,
            &self.state.borrow().thinking_level,
        );

        let me = self.clone();
        self.composer
            .set_on_thinking(move |level| me.set_thinking_level(&level));

        let me = self.clone();
        self.header.settings.connect_clicked(move |_| me.open_settings());
        let me = self.clone();
        self.header.set_on_session(move |index| me.select_session(index));
        let me = self.clone();
        self.header.set_on_new(move || me.new_session());
        let me = self.clone();
        self.header.set_on_close(move |index| me.close_tab(index));

        let me = self.clone();
        let key = gtk::EventControllerKey::new();
        key.set_propagation_phase(gtk::PropagationPhase::Capture);
        key.connect_key_pressed(move |_, key, _, state| {
            let ctrl = state.contains(gtk::gdk::ModifierType::CONTROL_MASK)
                && !state.contains(gtk::gdk::ModifierType::ALT_MASK);
            let shift = state.contains(gtk::gdk::ModifierType::SHIFT_MASK);

            if !ctrl
                && !state.contains(gtk::gdk::ModifierType::ALT_MASK)
                && shift
                && matches!(key, gtk::gdk::Key::Tab | gtk::gdk::Key::ISO_Left_Tab)
            {
                me.send_extension_command_text("/persona");
                return glib::Propagation::Stop;
            }

            if ctrl {
                match key {
                    gtk::gdk::Key::k | gtk::gdk::Key::K => {
                        me.open_palette();
                        return glib::Propagation::Stop;
                    }
                    gtk::gdk::Key::p | gtk::gdk::Key::P => {
                        me.cycle_model();
                        return glib::Propagation::Stop;
                    }
                    gtk::gdk::Key::n | gtk::gdk::Key::N => {
                        me.new_session();
                        return glib::Propagation::Stop;
                    }
                    gtk::gdk::Key::t | gtk::gdk::Key::T => {
                        me.cycle_thinking();
                        return glib::Propagation::Stop;
                    }
                    gtk::gdk::Key::comma => {
                        me.open_settings();
                        return glib::Propagation::Stop;
                    }
                    gtk::gdk::Key::g | gtk::gdk::Key::G
                        if state.contains(gtk::gdk::ModifierType::SHIFT_MASK) =>
                    {
                        me.select_nav(5);
                        return glib::Propagation::Stop;
                    }
                    gtk::gdk::Key::slash => {
                        hotkeys::show(&me.window);
                        return glib::Propagation::Stop;
                    }
                    gtk::gdk::Key::f | gtk::gdk::Key::F => {
                        let shift = state.contains(gtk::gdk::ModifierType::SHIFT_MASK);
                        if shift {
                            me.sidebar.open_search();
                        } else if me.chat.find_is_open() {
                            me.chat.find_close();
                            me.composer.focus();
                        } else {
                            if me.stack.visible_child_name().as_deref() != Some("chat") {
                                me.select_nav(2);
                            }
                            me.chat.find_open();
                        }
                        return glib::Propagation::Stop;
                    }
                    _ => {}
                }
            }

            if key == gtk::gdk::Key::Escape {
                if me.chat.find_is_open() {
                    me.chat.find_close();
                    me.composer.focus();
                    return glib::Propagation::Stop;
                }
                let models = me.header.models_open();
                let commands = me.composer.palette_open() || me.composer.skill_popup_open();
                if models || commands {
                    if models {
                        me.header.close_models();
                    }
                    if commands {
                        me.composer.close_palette();
                        me.composer.close_skill_popup();
                    }
                    me.composer.focus();
                    return glib::Propagation::Stop;
                }
                if me.state.borrow().running {
                    me.escape_streaming();
                    return glib::Propagation::Stop;
                }
                return glib::Propagation::Proceed;
            }

            if key == gtk::gdk::Key::slash
                && !ctrl
                && me.stack.visible_child_name().as_deref() == Some("chat")
                && me.composer.is_empty()
                && !focus_is_text_input(&me.window)
            {
                me.composer.focus();
                me.composer.set_text("/");
                return glib::Propagation::Stop;
            }

            glib::Propagation::Proceed
        });
        self.window.add_controller(key);

        let drop = gtk::DropTarget::new(
            gtk::gdk::FileList::static_type(),
            gtk::gdk::DragAction::COPY,
        );
        drop.connect_accept(|_, drop| {
            drop.formats()
                .contains_type(gtk::gdk::FileList::static_type())
        });
        let me = self.clone();
        drop.connect_drop(move |_, value, _, _| {
            let Ok(list) = value.get::<gtk::gdk::FileList>() else {
                return false;
            };
            let paths: Vec<String> = list
                .files()
                .iter()
                .filter_map(|file| file.path())
                .map(|path| path.to_string_lossy().to_string())
                .collect();
            let mut texts: Vec<String> = Vec::new();
            let mut handled = false;
            for path in paths {
                match me.composer.attach_file(&path) {
                    composer::AttachFile::Attached | composer::AttachFile::Rejected => {
                        handled = true;
                    }
                    composer::AttachFile::NotImage => texts.push(path),
                }
            }
            if !texts.is_empty() {
                me.composer.append_paths(&texts);
                handled = true;
            }
            handled
        });
        self.window.add_controller(drop);

        let texture = gtk::DropTarget::new(
            gtk::gdk::Texture::static_type(),
            gtk::gdk::DragAction::COPY,
        );
        texture.connect_accept(|_, drop| {
            let formats = drop.formats();
            formats.contains_type(gtk::gdk::Texture::static_type())
                || formats.contain_mime_type("image/png")
                || formats.contain_mime_type("image/jpeg")
        });
        let me = self.clone();
        texture.connect_drop(move |_, value, _, _| {
            let Ok(texture) = value.get::<gtk::gdk::Texture>() else {
                return false;
            };
            let bytes = texture.save_to_png_bytes();
            me.composer.attach_bytes(bytes.as_ref(), "drag image")
        });
        self.window.add_controller(texture);

        let me = self.clone();
        self.composer
            .set_on_notice(move |message| me.status.flash_notice(&message));

        let events = self.rpc.borrow().events.clone();
        let key = self.active_key.borrow().clone();
        self.start_event_loop(key, events);

        let me = self.clone();
        let history_rx = self.history_rx.clone();
        glib::spawn_future_local(async move {
            while let Ok(list) = history_rx.recv().await {
                me.on_history(list);
            }
        });

        let me = self.clone();
        self.plugins.set_on_install(move |input| me.install_plugin(&input));

        let me = self.clone();
        self.plugins.set_on_remove(move |spec| me.remove_plugin(&spec));

        let me = self.clone();
        self.plugins
            .set_on_configure(move |spec| me.configure_plugin(&spec));

        let me = self.clone();
        self.sidebar
            .set_on_delete_workspace(move |label, dir, paths| {
                me.delete_workspace(label, dir, paths)
            });

        let me = self.clone();
        self.sidebar.set_on_open_trash(move || me.show_trash());

        let me = self.clone();
        self.plugins
            .set_on_search(move |query| me.search_catalog(&query));

        let me = self.clone();
        self.plugins
            .set_on_catalog_install(move |name| me.install_catalog_package(&name));

        let me = self.clone();
        self.plugins.set_on_catalog_more(move || me.more_catalog());

        let me = self.clone();
        let catalog_rx = self.catalog_rx.clone();
        glib::spawn_future_local(async move {
            while let Ok(event) = catalog_rx.recv().await {
                me.on_catalog_event(event);
            }
        });

        let me = self.clone();
        let plugin_rx = self.plugin_rx.clone();
        glib::spawn_future_local(async move {
            while let Ok(event) = plugin_rx.recv().await {
                me.on_plugin_event(event);
            }
        });

        let me = self.clone();
        glib::timeout_add_seconds_local(1, move || {
            me.tick();
            me.rail.tick();
            glib::ControlFlow::Continue
        });

        let me = self.clone();
        glib::timeout_add_seconds_local(1, move || {
            me.loop_tick();
            glib::ControlFlow::Continue
        });

        let me = self.clone();
        glib::timeout_add_seconds_local(5, move || {
            if me.state.borrow().connection.is_connected() {
                me.ask_stats();
                if me.state.borrow().running {
                    me.ask_state();
                }
            }
            glib::ControlFlow::Continue
        });

        self.refresh_header();
        self.refresh_status();
        self.refresh_sidebar();
        self.refresh_loop_chrome();
        self.fetch_history();
        self.fetch_plugins();

        if updater::check_due() {
            let (tx, rx) = async_channel::bounded::<updater::UpdateState>(1);
            updater::spawn_check(tx);
            let me = self.clone();
            glib::spawn_future_local(async move {
                if let Ok(state) = rx.recv().await {
                    if state.has_update() {
                        me.status.flash_notice(&format!(
                            "{} update available · Settings → Credits",
                            state.latest
                        ));
                    }
                }
            });
        }

        if std::env::var("PI_DESKTOP_SNAPSHOT").is_ok()
            && std::env::var("PI_DESKTOP_DEMO").is_err()
        {
            let me = self.clone();
            glib::timeout_add_local_once(std::time::Duration::from_millis(200), move || {
                me.select_nav(4);
                if let Ok(query) = std::env::var("PI_DESKTOP_CATALOG_QUERY") {
                    me.plugins.search_entry.set_text(&query);
                }
                me.plugins.open_catalog();
            });
        }

        if let Ok(prompt) = std::env::var("PI_DESKTOP_PROMPT") {
            let me = self.clone();
            glib::timeout_add_seconds_local(2, move || {
                me.composer.set_text(&prompt);
                me.submit(SubmitKind::Enter);
                glib::ControlFlow::Break
            });
        }

        if let Some(seconds) = std::env::var("PI_DESKTOP_COPY_AFTER")
            .ok()
            .and_then(|value| value.parse::<u32>().ok())
        {
            let me = self.clone();
            glib::timeout_add_seconds_local(seconds.max(1), move || {
                me.composer.set_text("/copy");
                me.submit(SubmitKind::Enter);
                glib::ControlFlow::Break
            });
        }

        if let Ok(demo) = std::env::var("PI_DESKTOP_DEMO") {
            let me = self.clone();
            glib::timeout_add_seconds_local(2, move || {
                match demo.as_str() {
                    "commands" => {
                        me.composer.set_text("/");
                        me.composer.focus();
                    }
                    "filter" => {
                        me.composer.set_text("/tel");
                        me.composer.focus();
                    }
                    "command-tab" => {
                        me.composer.set_text("/loo");
                        me.composer.focus();
                        let composer = me.composer.clone();
                        glib::timeout_add_local_once(
                            std::time::Duration::from_millis(500),
                            move || composer.complete_palette_selection(),
                        );
                    }
                    "command-help" => {
                        let text = std::env::var("PI_DESKTOP_DEMO_QUERY")
                            .unwrap_or_else(|_| "/memory".to_string());
                        me.composer.set_text(&text);
                        me.composer.focus();
                    }
                    "palette" => {
                        me.open_palette();
                    }
                    "files" => {
                        me.composer.set_text("@");
                        me.composer.focus();
                    }
                    "find" => {
                        me.select_nav(2);
                        if let Ok(query) = std::env::var("PI_DESKTOP_DEMO_QUERY") {
                            me.chat.find_open();
                            me.chat.find_query(&query);
                        } else {
                            me.chat.find_open();
                            me.chat.find_query("error");
                        }
                    }
                    "hotkeys" => {
                        hotkeys::show(&me.window);
                    }
                    "drop" => {
                        if let composer::DropOutcome::Notice(notice) = composer::drop_texture() {
                            me.status.flash_notice(notice);
                        }
                        me.composer.append_paths(&[
                            "/tmp/user/projects/AGENTS.md".to_string(),
                            "/tmp/user/Meus Documentos/nota final.txt".to_string(),
                        ]);
                    }
                    "title" => {
                        {
                            let mut st = me.state.borrow_mut();
                            st.first_prompt = Some(
                                "AI session naming is not working yet; when I give a prompt, the AI should automatically assign a title"
                                    .to_string(),
                            );
                            st.state_synced = true;
                        }
                        me.maybe_name_session();
                    }
                    "think" => {
                        me.select_nav(2);
                    }
                    "think-expand" => {
                        me.select_nav(2);
                        let me_c = me.clone();
                        glib::timeout_add_local_once(
                            std::time::Duration::from_millis(5_500),
                            move || me_c.chat.expand_thinking(),
                        );
                    }
                    "git" => {
                        me.select_nav(5);
                    }
                    "activity" => {
                        me.select_nav(2);
                        me.rail.activity_start("demo-tool", "bash");
                        me.rail.activity_update("demo-tool", "test load\n... 234 passed");
                        me.rail.activity_end("demo-tool", true, "example error:\ncc: linker failed\ncomplete output available when clicking");
                    }
                    "plugins" => {
                        me.select_nav(4);
                        me.plugins.open_installed();
                        let me_c = me.clone();
                        glib::timeout_add_seconds_local(2, move || {
                            me_c.select_nav(4);
                            glib::ControlFlow::Break
                        });
                    }
                    "compact-result" => {
                        me.chat.note("compacting context...", false);
                        me.on_compaction_end(&json!({
                            "type": "compaction_end",
                            "reason": "manual",
                            "result": {
                                "tokensBefore": 150000,
                                "estimatedTokensAfter": 32000
                            },
                            "aborted": false,
                            "willRetry": false
                        }));
                    }
                    "provider-logos" => {
                        let models: Vec<ModelItem> = crate::providers::KNOWN
                            .iter()
                            .map(|provider| ModelItem {
                                provider: provider.id.to_string(),
                                id: format!("{}-demo", provider.id),
                                name: provider.label.to_string(),
                            })
                            .collect();
                        me.state.borrow_mut().models = models.clone();
                        me.header.set_menu(&models, "", "");
                        me.header.open_models();
                        if let Ok(query) = std::env::var("PI_DESKTOP_DEMO_QUERY") {
                            me.header.filter_models(&query);
                        }
                    }
                    "multi-both" => {
                        me.select_nav(2);
                        me.composer.set_text(
                            "Run the shell command 'sleep 15' with bash, then reply with the single word: done",
                        );
                        let submit = me.clone();
                        glib::timeout_add_local_once(
                            std::time::Duration::from_millis(600),
                            move || submit.submit(SubmitKind::Enter),
                        );
                        let next = me.clone();
                        glib::timeout_add_seconds_local(3, move || {
                            next.new_session();
                            glib::ControlFlow::Break
                        });
                        let second = me.clone();
                        glib::timeout_add_seconds_local(4, move || {
                            second.composer.set_text("Reply with the single word: ok");
                            second.submit(SubmitKind::Enter);
                            glib::ControlFlow::Break
                        });
                    }
                    "multi-switch" => {
                        me.select_nav(2);
                        me.composer.set_text(
                            "Run the shell command 'sleep 20' with bash, then reply with the single word: done",
                        );
                        let submit = me.clone();
                        glib::timeout_add_local_once(
                            std::time::Duration::from_millis(600),
                            move || submit.submit(SubmitKind::Enter),
                        );
                        let next = me.clone();
                        glib::timeout_add_seconds_local(4, move || {
                            next.new_session();
                            glib::ControlFlow::Break
                        });
                        let back = me.clone();
                        glib::timeout_add_seconds_local(7, move || {
                            let index = back.state.borrow().sessions.iter().position(|session| {
                                session
                                    .preview
                                    .as_deref()
                                    .is_some_and(|preview| preview.contains("sleep 20"))
                            });
                            if let Some(index) = index {
                                back.select_session(index);
                            }
                            glib::ControlFlow::Break
                        });
                    }
                    "multi-session" => {
                        me.select_nav(2);
                        me.composer.set_text(
                            "Run the shell command 'sleep 20' with bash, then reply with the single word: done",
                        );
                        let submit = me.clone();
                        glib::timeout_add_local_once(
                            std::time::Duration::from_millis(600),
                            move || submit.submit(SubmitKind::Enter),
                        );
                        let next = me.clone();
                        glib::timeout_add_seconds_local(4, move || {
                            next.new_session();
                            glib::ControlFlow::Break
                        });
                    }
                    "big-prompt" => {
                        let text = (1..=40)
                            .map(|line| format!("line {line}: a long prompt that should scroll"))
                            .collect::<Vec<_>>()
                            .join("\n");
                        me.composer.set_text(&text);
                        me.composer.focus();
                    }
                    "workspace-trash" => {
                        me.show_trash();
                    }
                    "plugin-config" => {
                        me.configure_plugin("npm:pi-guard");
                    }
                    "plugin-settings" => {
                        me.configure_plugin("npm:pi-web-access");
                    }
                    "rail" => {
                        me.select_nav(2);
                        me.toggle_rail();
                    }
                    "rail-wide" => {
                        me.select_nav(2);
                        me.rail_width.set(460);
                        me.apply_rail_layout();
                    }
                    "rail-narrow" => {
                        me.select_nav(2);
                        me.rail_width.set(280);
                        me.apply_rail_layout();
                    }
                    "background" => {
                        me.select_nav(2);
                        let candidate = {
                            let state = me.state.borrow();
                            state
                                .sessions
                                .iter()
                                .find(|session| session.session_id != state.session_id)
                                .cloned()
                        };
                        if let Some(session) = candidate {
                            me.activity.borrow_mut().insert(
                                session.session_id.clone(),
                                SessionActivity {
                                    running: true,
                                    phase: "THINKING".to_string(),
                                    started: Some(Instant::now()),
                                    unread: false,
                                    waiting: false,
                                },
                            );
                            me.refresh_sidebar();
                            me.refresh_header();
                        }
                    }
                    "plan" => {
                        let record = json!({
                            "type": "extension_ui_request",
                            "id": "demo-plan",
                            "method": "setWidget",
                            "widgetKey": "plan-all",
                            "widgetLines": [
                                "\u{1b}[38;5;145m] Decide defaults\u{1b}[39m]",
                                "\u{1b}[38;5;145m] Create file structure\u{1b}[39m]",
                                "\u{1b}[38;5;145m☐ Engine of rules (engine.js)\u{1b}[39m]",
                                "\u{1b}[38;5;145m ☐ IA of bots and tests\u{1b}[39m",
                            ],
                        });
                        me.on_extension_ui(&record);
                        let status = json!({
                            "type": "extension_ui_request",
                            "id": "demo-plan-status",
                            "method": "Status set",
                            "statusKey": "plan-mode",
                            "statusText": "\u{1b}[38;5;145m[plan]\u{1b}[39m",
                        });
                        me.on_extension_ui(&status);
                    }
                    "banner" => {
                        me.show_error_banner("test load: link error in the environment — copyable detail to paste into the report");
                    }
                    "git-diff" => {
                        me.select_nav(5);
                        let me_prev = me.clone();
                        glib::timeout_add_local_once(
                            std::time::Duration::from_millis(700),
                            move || me_prev.git.preview_first(),
                        );
                        let me_nav = me.clone();
                        glib::timeout_add_local_once(
                            std::time::Duration::from_millis(1_500),
                            move || me_nav.select_nav(5),
                        );
                    }
                    "model-menu" => {
                        me.header.open_models();
                        if let Ok(query) = std::env::var("PI_DESKTOP_DEMO_QUERY") {
                            me.header.filter_models(&query);
                        }
                    }
                    "think-off" => {
                        let levels: Vec<String> = ["off", "low", "medium", "high", "max"]
                            .iter()
                            .map(|level| level.to_string())
                            .collect();
                        me.composer.set_thinking_levels(&levels, "off");
                        me.composer.open_thinking();
                    }
                    "think-slider" => {
                        let levels: Vec<String> = ["off", "low", "medium", "high", "max"]
                            .iter()
                            .map(|level| level.to_string())
                            .collect();
                        me.composer.set_thinking_levels(&levels, "max");
                        me.composer.open_thinking();
                    }
                    "think-history" => {
                        me.select_nav(2);
                        let me_c = me.clone();
                        glib::timeout_add_local_once(
                            std::time::Duration::from_secs(9),
                            move || me_c.chat.expand_thinking(),
                        );
                    }
                    "think-real" => {
                        me.select_nav(2);
                        me.composer.set_text("Explain in a sentence why the sky is blue.");
                        let me_c = me.clone();
                        glib::timeout_add_local_once(
                            std::time::Duration::from_millis(900),
                            move || {
                                me_c.submit(SubmitKind::Enter);
                            },
                        );
                    }
                    "tab-switch" => {
                        let me_q = me.clone();
                        glib::timeout_add_local_once(
                            std::time::Duration::from_millis(500),
                            move || {
                                me_q.composer
                                    .set_text("Explain in a sentence why the sky is blue.");
                                me_q.submit(SubmitKind::Enter);
                            },
                        );
                        let me_a = me.clone();
                        glib::timeout_add_local_once(
                            std::time::Duration::from_secs(9),
                            move || me_a.select_nav(4),
                        );
                        let me_b = me.clone();
                        glib::timeout_add_local_once(
                            std::time::Duration::from_secs(12),
                            move || me_b.select_nav(2),
                        );
                    }
                    "toast" => {
                        me.show_toast("Note extension: export ready", "warning", 60);
                    }
                    "new-session-from-plugins" => {
                        me.select_nav(4);
                        let me_c = me.clone();
                        glib::timeout_add_local_once(
                            std::time::Duration::from_millis(900),
                            move || me_c.new_session(),
                        );
                    }
                    "drop-image" => {
                        let base = env!("CARGO_MANIFEST_DIR");
                        me.composer.append_paths(&[
                            format!("{base}/resources/logos/anthropic-dark.png"),
                            format!("{base}/README.md"),
                        ]);
                        let me_c = me.clone();
                        glib::timeout_add_local_once(
                            std::time::Duration::from_millis(700),
                            move || {
                                me_c.submit(SubmitKind::Enter);
                            },
                        );
                    }
                    "attach-send" => {
                        me.composer.attach_bytes(
                            include_bytes!("../../resources/logos/anthropic-dark.png"),
                            "capture.png",
                        );
                        me.composer.set_text("What's in this picture?");
                        let me_c = me.clone();
                        glib::timeout_add_local_once(
                            std::time::Duration::from_millis(700),
                            move || {
                                me_c.submit(SubmitKind::Enter);
                            },
                        );
                    }
                    "attach" => {
                        me.composer.attach_bytes(
                            include_bytes!("../../resources/logos/anthropic-dark.png"),
                            "capture.png",
                        );
                        me.composer.set_text("What's in this picture?");
                        me.composer.focus();
                    }
                    "icons" => {
                        me.demo_history();
                        me.composer.append_paths(&[
                            "/tmp/user/projects/AGENTS.md".to_string(),
                            "/tmp/user/Meus Documentos/nota final.txt".to_string(),
                        ]);
                    }
                    "models" => me.header.open_models(),
                    "sessions" => me.sidebar.open_context_menu(0),
                    "clone" => {
                        let me_c = me.clone();
                        glib::timeout_add_local_once(
                            std::time::Duration::from_millis(2500),
                            move || {
                                me_c.duplicate_session(0);
                            },
                        );
                    }
                    "rename" => me.sidebar.open_rename(0),
                    "rename-rpc" => {
                        let me_c = me.clone();
                        glib::timeout_add_local_once(
                            std::time::Duration::from_millis(2500),
                            move || me_c.rename_session(0, "v45 rpc rename"),
                        );
                    }
                    "rename-disk" => {
                        let me_c = me.clone();
                        glib::timeout_add_local_once(
                            std::time::Duration::from_millis(2500),
                            move || me_c.rename_session(0, "v45 disk rename"),
                        );
                    }
                    "delete" => me.delete_session(0),
                    "export" => me.export_session(0),
                    "fork" => {
                        let me_c = me.clone();
                        glib::timeout_add_local_once(
                            std::time::Duration::from_millis(3000),
                            move || {
                                let entry = me_c
                                    .fork_points
                                    .borrow()
                                    .first()
                                    .map(|(entry, _)| entry.clone());
                                match entry {
                                    Some(entry) => me_c.fork_from(&entry),
                                    None => me_c
                                        .status
                                        .flash_notice("demo fork: no fork point"),
                                }
                            },
                        );
                    }
                    "trash" => {
                        let (path, open) = {
                            let state = me.state.borrow();
                            match state.sessions.first() {
                                Some(session) => (session.path.clone(), session_open_in(&state, 0)),
                                None => (String::new(), false),
                            }
                        };
                        if !path.is_empty() {
                            me.confirm_delete(path, open);
                        }
                    }
                    "search" => me.sidebar.set_query("a"),
                    "search-pi" => me.sidebar.set_query("pi"),
                    "plugins-installed" => {
                        me.select_nav(4);
                        me.plugins.open_installed();
                    }
                    "chat" => {
                        me.apply_view(ViewMode::Chat, false);
                        me.composer.focus();
                    }
                    "chat-collapsed" => {
                        me.apply_view(ViewMode::Chat, false);
                        me.toggle_sidebar();
                        me.composer.focus();
                    }
                    "chat-history" => {
                        me.apply_view(ViewMode::Chat, false);
                        me.demo_diff();
                    }
                    "chat-search" => {
                        me.apply_view(ViewMode::Chat, false);
                        me.composer.set_search_active(true);
                        me.composer.focus();
                    }
                    "chat-toggle" => {
                        me.apply_view(ViewMode::Chat, false);
                        let me_c = me.clone();
                        glib::timeout_add_local_once(
                            std::time::Duration::from_millis(1_200),
                            move || me_c.header.mode_code.emit_clicked(),
                        );
                    }
                    "settings" => me.open_settings(),
                    "select" => {
                        let record = json!({
                            "type": "extension_ui_request",
                            "id": "demo-select",
                            "method": "select",
                            "title": "Dangerous command?",
                            "options": ["allow", "block", "Ask later"],
                            "timeout": 120000
                        });
                        me.on_record(&record);
                    }
                    "confirm" => {
                        let record = json!({
                            "type": "extension_ui_request",
                            "id": "demo-confirm",
                            "method": "confirm",
                            "title": "Clear session?",
                            "message": "All messages will be lost.",
                            "timeout": 120000
                        });
                        me.on_record(&record);
                    }
                    "input" => {
                        let record = json!({
                            "type": "extension_ui_request",
                            "id": "demo-input",
                            "method": "input",
                            "title": "Profile",
                            "placeholder": "Enter the name..."
                        });
                        me.on_record(&record);
                    }
                    "editor" => {
                        let record = json!({
                            "type": "extension_ui_request",
                            "id": "demo-editor",
                            "method": "editor",
                            "title": "Edit Instructions",
                            "prefill": "You're the project assistant.\nRespond to pt-BR.\n\nUse tables when helping."
                        });
                        me.on_record(&record);
                    }
                    "notify" => {
                        let record = json!({
                            "type": "extension_ui_request",
                            "id": "demo-notify",
                            "method": "notify",
                            "message": "User-locked command",
                            "notificationType": "warning"
                        });
                        me.on_record(&record);
                    }
                    "extstatus" => {
                        for record in [
                            json!({
                                "type": "extension_ui_request",
                                "id": "demo-status",
                                "method": "Status set",
                                "statusKey": "telegram",
                                "statusText": "\u{1b}[32mtelegram connected\u{1b}[0m"
                            }),
                            json!({
                                "type": "extension_ui_request",
                                "id": "demo-widget",
                                "method": "setWidget",
                                "widgetKey": "demo",
                                "widgetLines": [
                                    "- active extension -",
                                    "shift: 3",
                                    "model: deepseek-flash",
                                    "row: 0"
                                ],
                                "widgetPlace": "AboveEditor"
                            }),
                        ] {
                            me.on_record(&record);
                        }
                    }
                    "thinking" => {
                        {
                            let mut state = me.state.borrow_mut();
                            state.thinking_levels = vec![
                                "off".into(),
                                "low".into(),
                                "high".into(),
                                "max".into(),
                            ];
                            state.thinking_level = "high".into();
                        }
                        me.refresh_header();
                        me.refresh_status();
                        me.header.open_models();
                    }
                    "queue" => {
                        {
                            let mut state = me.state.borrow_mut();
                            state.running = true;
                            state.pending_messages = 2;
                        }
                        me.composer.set_running(true);
                        me.composer.set_queue(2);
                        me.refresh_status();
                    }
                    "bash" => {
                        me.chat.tool_start(
                            "bash-demo",
                            "bash",
                            &json!({"command": "ls - the ~/pi-assist"}),
                        );
                        me.chat.tool_append(
                            "bash-demo",
                            "total 48\ndrwxr-xr-x 1 server 4096 Nov 9 03:12 .\n",
                        );
                        me.chat.bash_finish(
                            "bash-demo",
                            "total 48\ndrwxr-xr-x 1 server 4096 Nov 9 03:12 .\ndrwxr-xr-xr 1 server 4096 Nov 8 21:26 ..\n-rw-r--r-- 1 server 842 Nov 9 02:50 AGENTS.md\n-rwxr-xr-xr 1 server 612 Nov 8 22:04 run.sh\n-rwxr-xr-xr 1 server 98 Nov 8 22:04 set-token.sh",
                            Some(0),
                            false,
                            false,
                        );
                    }
                    "bashrun" => {
                        me.chat.tool_start(
                            "bash-run",
                            "bash",
                            &json!({"command": "Sleep 30 &&Echo Ready"}),
                        );
                        me.chat
                            .tool_append("bash-run", "Waiting for the command to finish...");
                        me.bash_running.set(true);
                        me.composer.set_bash_running(true);
                    }
                    "session" => me.select_session(0),
                    "history" => me.demo_history(),
                    "session-file" => me.demo_diff(),
                    "diff-expand" => {
                        me.demo_diff();
                        let me_c = me.clone();
                        glib::timeout_add_local_once(
                            std::time::Duration::from_millis(600),
                            move || me_c.chat.expand_diffs(),
                        );
                    }
                    "artifacts" => me.select_nav(0),
                    "tree" => me.select_nav(3),
                    "catalog" => {
                        me.select_nav(4);
                        me.plugins.open_catalog();
                    }
                    "ctx" => {
                        me.state.borrow_mut().context_percent = Some(87.0);
                        me.refresh_status();
                    }
                    "marked" => {
                        me.chat.append_user_marked(
                            "stop listing directories — the error is in the cc link",
                            "STEER",
                        );
                        me.chat.append_user_marked(
                            "then spin `test load' and summarize what failed",
                            "QUEUED",
                        );
                        {
                            let mut state = me.state.borrow_mut();
                            state.running = true;
                            state.pending_messages = 1;
                        }
                        me.composer.set_running(true);
                        me.composer.set_queue(1);
                    }
                    "loop" => me.demo_loop(),
                    "jobs" => {
                        me.demo_loop();
                        me.select_nav(1);
                    }
                    "mentions" => {
                        let skills = vec![
                            CommandItem {
                                name: "skill:bug-hunting".to_string(),
                                description: "plays and investigates a bug with minimal steps"
                                    .to_string(),
                                source: "skill".to_string(),
                            },
                            CommandItem {
                                name: "skill:build-check".to_string(),
                                description: "compiles, runs tests and summary failures".to_string(),
                                source: "skill".to_string(),
                            },
                            CommandItem {
                                name: "skill:background-wait".to_string(),
                                description: "waits for a process to end without holding the turn"
                                    .to_string(),
                                source: "skill".to_string(),
                            },
                        ];
                        me.state.borrow_mut().commands = skills.clone();
                        me.composer.set_commands(skills);
                        me.composer.set_text("@bug");
                        me.composer.focus();
                    }
                    "cmdrun" => {
                        me.composer.set_text("/telegram-status");
                        me.composer.open_commands();
                        glib::timeout_add_local_once(
                            std::time::Duration::from_millis(400),
                            {
                                let me = me.clone();
                                move || me.composer.run_palette_selection()
                            },
                        );
                    }
                    "newname" => {
                        me.new_session();
                        glib::timeout_add_local_once(
                            std::time::Duration::from_millis(1500),
                            {
                                let me = me.clone();
                                move || {
                                    me.composer
                                        .set_text("v2 automatic name test");
                                    me.submit(SubmitKind::Enter);
                                }
                            },
                        );
                    }
                    "model" => {
                        me.new_session();
                        glib::timeout_add_local_once(std::time::Duration::from_millis(1500), {
                            let me = me.clone();
                            move || {
                                me.composer.set_text("zz tests change model");
                                me.submit(SubmitKind::Enter);
                            }
                        });
                        glib::timeout_add_local_once(std::time::Duration::from_secs(10), {
                            let me = me.clone();
                            move || {
                                me.set_model(
                                    "deepseek".to_string(),
                                    "deepseek-v4-pro".to_string(),
                                    "DeepSeek V4 Pro".to_string(),
                                );
                            }
                        });
                        glib::timeout_add_local_once(std::time::Duration::from_secs(12), {
                            let me = me.clone();
                            move || me.fetch_history()
                        });
                    }
                    _ => {}
                }
                glib::ControlFlow::Break
            });
        }

        if let Some(mode) = std::env::var("PI_DESKTOP_THEME_SWITCH")
            .ok()
            .and_then(|value| ThemeMode::parse(&value))
        {
            let me = self.clone();
            glib::timeout_add_seconds_local(5, move || {
                me.theme.set_mode(mode);
                glib::ControlFlow::Break
            });
        }
    }


    fn submit(self: &Rc<Self>, kind: SubmitKind) {
        let text = self.composer.take_text();
        let attachment = self.composer.take_attachment();
        {
            let id = self.state.borrow().session_id.clone();
            self.drafts.borrow_mut().remove(&id);
        }
        if text.is_empty() && attachment.is_none() {
            return;
        }

        self.upsert_current_session(true);

        if self.handle_local_command(&text) {
            self.restore_attachment(attachment);
            return;
        }

        let (connected, running) = {
            let state = self.state.borrow();
            (state.connection.is_connected(), state.running)
        };

        if !connected {
            self.chat
                .note("no connection to Pi — message not sent", true);
            self.restore_composer(&text, attachment);
            return;
        }

        if self.is_extension_command(&text) {
            self.send_extension_command_text(&text);
            return;
        }

        if let Some(command) = text.strip_prefix('!') {
            self.restore_attachment(attachment);
            let command = command.trim();
            if command.is_empty() {
                self.chat.note("bash: empty command", true);
                return;
            }
            let exclude = kind == SubmitKind::CtrlShiftEnter;
            self.run_bash(command, exclude);
            return;
        }

        let images: Vec<composer::Attachment> = attachment.iter().cloned().collect();
        if !running || !images.is_empty() {
            if !self.send_prompt(&text, true, &images) {
                self.restore_composer(&text, attachment);
            }
            return;
        }

        let id = self.id();
        let message = self.wire_text(&text);
        let command = submit_command(&id, kind, true, &message);
        self.send(command);

        {
            let mut state = self.state.borrow_mut();
            state.pending_messages += 1;
            let pending = state.pending_messages;
            drop(state);
            self.composer.set_queue(pending);
        }
        self.composer.set_running(true);
        self.refresh_status();
        self.ask_state();
    }

    fn restore_composer(&self, text: &str, attachment: Option<composer::Attachment>) {
        self.composer.set_text(text);
        self.restore_attachment(attachment);
        self.composer.focus();
    }

    fn restore_attachment(&self, attachment: Option<composer::Attachment>) {
        if let Some(attachment) = attachment {
            self.composer.set_attachment(attachment);
        }
    }

    fn wire_text(&self, text: &str) -> String {
        let expanded = if text.contains('@') {
            let config = self.config.borrow();
            if matches!(config.target, crate::config::Target::Local) {
                let root = crate::config::expand_home(config.effective_workspace());
                crate::files::expand_mentions(text, &root)
            } else {
                text.to_string()
            }
        } else {
            text.to_string()
        };
        if self.chat_search.get() {
            with_search_hint(&expanded)
        } else {
            expanded
        }
    }

    fn send_prompt(
        self: &Rc<Self>,
        text: &str,
        record_user: bool,
        images: &[composer::Attachment],
    ) -> bool {
        if !self.state.borrow().connection.is_connected() {
            self.chat
                .note("no connection to Pi — message not sent", true);
            return false;
        }

        if record_user {
            if images.is_empty() {
                self.chat.append_user(text);
            } else {
                let blocks: Vec<ImageBlock> = images
                    .iter()
                    .map(|image| ImageBlock {
                        mime: image.mime.clone(),
                        data: image.data.clone(),
                    })
                    .collect();
                self.chat.append_user_images(text, &blocks);
            }
            let mut state = self.state.borrow_mut();
            let replace = state.first_prompt.as_deref().map_or(true, |first| {
                let first = first.trim();
                first.starts_with('/') && !first.contains(' ')
            });
            if replace {
                state.first_prompt = Some(text.to_string());
            }
        }

        if record_user {
            self.maybe_name_session();
        }

        let id = self.id();
        let message = self.wire_text(text);
        let mut command = json!({"id": id, "type": "prompt", "message": message});
        if !images.is_empty() {
            command["images"] = Value::Array(
                images
                    .iter()
                    .map(|image| {
                        json!({
                            "type": "image",
                            "data": image.data,
                            "mimeType": image.mime,
                        })
                    })
                    .collect(),
            );
        }
        self.send(command);

        {
            let mut state = self.state.borrow_mut();
            let was_running = state.running;
            state.running = true;
            if !was_running {
                state.reset_turn();
            }
        }
        self.composer.set_running(true);
        self.refresh_status();
        self.refresh_turn();
        self.refresh_sidebar();
        self.ask_state();
        true
    }

    fn stop(self: &Rc<Self>) {
        if self.bash_running.get() {
            self.send(json!({"id": self.id(), "type": "abort_bash"}));
        } else {
            self.abort();
        }
    }

    fn abort(&self) {
        self.send(json!({"id": self.id(), "type": "abort"}));
    }


    fn send_extension_command_text(self: &Rc<Self>, text: &str) {
        let id = self.id();
        let message = self.wire_text(text);
        self.send(json!({"id": id, "type": "prompt", "message": message}));
    }

    fn is_extension_command(&self, text: &str) -> bool {
        let trimmed = text.trim_start();
        let Some(rest) = trimmed.strip_prefix('/') else {
            return false;
        };
        let name = rest.split_whitespace().next().unwrap_or("");
        if name.is_empty() {
            return false;
        }
        let loaded = self
            .state
            .borrow()
            .commands
            .iter()
            .any(|command| command.name == name && command.source == "extension");
        if loaded {
            return true;
        }
        matches!(
            name,
            "plan"
                | "guard"
                | "memory"
                | "sessions"
                | "checkpoints"
                | "rollback"
                | "all"
                | "persona"
                | "subagents"
                | "subagents-stop"
        )
    }

    fn handle_local_command(self: &Rc<Self>, text: &str) -> bool {
        let Some((command, argument)) = local_command(text) else {
            return false;
        };
        match command {
            "/loop" => {
                self.handle_loop(argument);
                true
            }
            "/steer" => {
                self.handle_steer(argument);
                true
            }
            "/queue" => {
                self.handle_queue(argument);
                true
            }
            "/copy" => {
                self.copy_last_assistant();
                true
            }
            "/compact" => {
                self.handle_compact(argument);
                true
            }
            _ => false,
        }
    }

    fn handle_compact(self: &Rc<Self>, argument: &str) {
        if !self.state.borrow().connection.is_connected() {
            self.chat
                .note("no connection to Pi — compact not sent", true);
            return;
        }
        if self.state.borrow().running {
            self.chat
                .note("compact · stop the current turn first", true);
            return;
        }
        let instructions = argument.trim();
        let mut request = json!({"id": self.id(), "type": "compact"});
        if !instructions.is_empty() {
            request["customInstructions"] = Value::String(instructions.to_string());
        }
        self.send(request);
    }

    fn handle_loop(self: &Rc<Self>, argument: &str) {
        match loops::parse_command(argument) {
            LoopCommand::Status => {
                self.chat.note(&self.loop_status_text(), false);
            }
            LoopCommand::Stop => self.stop_loop(None),
            LoopCommand::Invalid(reason) => {
                self.chat.note(&format!("loop: {reason}"), true);
            }
            LoopCommand::Start { interval, task } => {
                let replaced = self.loop_state.borrow().is_some();
                let state = LoopState {
                    task: task.clone(),
                    interval,
                    iteration: 0,
                    active: true,
                };
                loops::save(&state);
                *self.loop_state.borrow_mut() = Some(ActiveLoop {
                    next_fire: Instant::now() + Duration::from_secs(interval),
                    state,
                });
                self.refresh_loop_chrome();
                if replaced {
                    self.chat.note("Previous loop replaced", false);
                }
                self.chat.note(
                    &format!(
                        "loop · {} interval · task: {task}",
                        loops::format_interval(interval)
                    ),
                    false,
                );
            }
        }
    }

    fn loop_status_text(&self) -> String {
        let guard = self.loop_state.borrow();
        let Some(active) = guard.as_ref() else {
            return "no active loops".to_string();
        };
        let remaining = active
            .next_fire
            .saturating_duration_since(Instant::now())
            .as_secs();
        format!(
            "loop · iteration {} · near {} · interval {}\ntask: {}",
            active.state.iteration,
            loops::format_countdown(remaining),
            loops::format_interval(active.state.interval),
            active.state.task
        )
    }

    fn stop_loop(self: &Rc<Self>, complete: Option<u64>) {
        let had = self.loop_state.borrow_mut().take().is_some();
        if had {
            loops::clear();
        }
        self.refresh_loop_chrome();
        if let Some(iteration) = complete {
            self.toast_text(&format!("loop completed at iteration {iteration}"));
        } else if had {
            self.chat.note("loop stopped", false);
        } else {
            self.chat.note("no active loops", false);
        }
    }

    fn loop_tick(self: &Rc<Self>) {
        if self.loop_state.borrow().is_none() {
            return;
        }
        self.refresh_loop_chrome();
        if !self.loop_due() {
            return;
        }
        let (running, connected) = {
            let state = self.state.borrow();
            (state.running, state.connection.is_connected())
        };
        if running || !connected {
            return;
        }
        self.loop_fire();
    }

    fn notify_settle(&self, aborted: bool) {
        if aborted || !self.config.borrow().notify {
            return;
        }
        if self.window.is_active() {
            return;
        }
        if self.state.borrow().run_elapsed < Duration::from_secs(5) {
            return;
        }
        if self.loop_state.borrow().is_some() {
            return;
        }
        let summary = {
            let state = self.state.borrow();
            match state.session_name.as_deref().filter(|name| !name.trim().is_empty()) {
                Some(name) => name.to_string(),
                None => "Pi Desktop".to_string(),
            }
        };
        let _ = std::process::Command::new("notify-send")
            .args([
                "-a",
                "Pi Desktop",
                "-i",
                "io.github.FrameT.PiDesktop",
                &summary,
                "completed shift",
            ])
            .spawn();
    }

    fn loop_after_settle(self: &Rc<Self>) {        let (running, connected) = {
            let state = self.state.borrow();
            (state.running, state.connection.is_connected())
        };
        if self.loop_due() && !running && connected {
            self.loop_fire();
        }
    }

    fn loop_due(&self) -> bool {
        self.loop_state
            .borrow()
            .as_ref()
            .map(|active| Instant::now() >= active.next_fire)
            .unwrap_or(false)
    }

    fn loop_fire(self: &Rc<Self>) {
        let (task, iteration) = {
            let guard = self.loop_state.borrow();
            let Some(active) = guard.as_ref() else {
                return;
            };
            (active.state.task.clone(), active.state.iteration + 1)
        };
        let text = loops::wakeup_text(&task, iteration);
        if !self.send_prompt(&text, false, &[]) {
            return;
        }

        let snapshot = {
            let mut guard = self.loop_state.borrow_mut();
            let Some(active) = guard.as_mut() else {
                return;
            };
            active.state.iteration = iteration;
            active.next_fire = Instant::now() + Duration::from_secs(active.state.interval);
            active.state.clone()
        };
        loops::save(&snapshot);
        self.chat
            .note(&format!("loop · wakeup #{iteration}"), false);
        self.refresh_loop_chrome();
    }

    fn refresh_loop_chrome(self: &Rc<Self>) {
        let view = {
            let guard = self.loop_state.borrow();
            guard.as_ref().map(|active| jobs::LoopView {
                task: active.state.task.clone(),
                iteration: active.state.iteration,
                interval: active.state.interval,
                remaining: active
                    .next_fire
                    .saturating_duration_since(Instant::now())
                    .as_secs(),
            })
        };
        let active = view.is_some();
        match view.as_ref() {
            Some(view) => {
                self.composer.set_loop(Some(&format!(
                    "LOOP · {} · {}",
                    view.iteration,
                    loops::format_countdown(view.remaining)
                )));
                self.jobs.set_loop(Some(view));
            }
            None => {
                self.composer.set_loop(None);
                self.jobs.set_loop(None);
            }
        }
        self.state.borrow_mut().jobs = active as usize;
        self.sidebar.set_job_count(active as usize);
    }

    fn handle_steer(self: &Rc<Self>, argument: &str) {
        let text = argument.trim();
        if text.is_empty() {
            self.show_pending(PendingQuery::Steer);
            return;
        }
        if !self.state.borrow().connection.is_connected() {
            self.chat
                .note("no connection to Pi — steer not sent", true);
            return;
        }
        if self.state.borrow().running {
            self.send(json!({"id": self.id(), "type": "steer", "message": text}));
            self.chat.append_user_marked(text, "STEER");
            let pending = {
                let mut state = self.state.borrow_mut();
                state.pending_messages += 1;
                state.pending_messages
            };
            self.composer.set_queue(pending);
            self.ask_state();
        } else {
            self.send_prompt(text, true, &[]);
            self.status
                .flash_notice("nothing is running — sent as a new prompt");
        }
    }

    fn handle_queue(self: &Rc<Self>, argument: &str) {
        let text = argument.trim();
        if text.is_empty() {
            self.show_pending(PendingQuery::Queue);
            return;
        }
        if !self.state.borrow().connection.is_connected() {
            self.chat
                .note("no connection to Pi — message not queued", true);
            return;
        }
        if text.eq_ignore_ascii_case("clear") {
            self.send(json!({"id": self.id(), "type": "clear_queue"}));
            self.chat
                .note("queue cleared · text restored", false);
            return;
        }
        self.send(json!({"id": self.id(), "type": "follow_up", "message": text}));
        self.chat.append_user_marked(text, "QUEUED");
        let pending = {
            let mut state = self.state.borrow_mut();
            state.pending_messages += 1;
            state.pending_messages
        };
        self.composer.set_queue(pending);
        self.ask_state();
    }

    fn show_pending(self: &Rc<Self>, kind: PendingQuery) {
        let count = self.state.borrow().pending_messages;
        if kind == PendingQuery::Queue {
            self.composer.set_queue(count);
        }
        if !self.state.borrow().connection.is_connected() {
            let label = if kind == PendingQuery::Steer {
                "steer"
            } else {
                "queue"
            };
            self.chat.note(
                &format!("{label} · {count} pending (last read)"),
                false,
            );
            return;
        }
        *self.pending_query.borrow_mut() = Some(kind);
        self.ask_state();
    }

    fn toast_text(&self, message: &str) {
        self.show_toast(message, "info", 4);
    }

    fn show_error_banner(&self, message: &str) {
        let message = message.trim();
        if message.is_empty() {
            return;
        }
        self.banner_text.replace(message.to_string());
        self.banner_label.set_text(message);
        self.banner.set_visible(true);
    }

    fn show_toast(&self, message: &str, kind: &str, seconds: u32) {
        self.toast_label.set_text(message);
        for class in ["info", "warning", "error"] {
            self.toast_card.remove_css_class(class);
        }
        self.toast_card.add_css_class(kind);
        self.toast_card.set_visible(true);

        let seq = self.toast_seq.get() + 1;
        self.toast_seq.set(seq);
        let weak = self.toast_card.downgrade();
        let own = self.toast_seq.clone();
        glib::timeout_add_seconds_local(seconds, move || {
            if own.get() == seq {
                if let Some(card) = weak.upgrade() {
                    card.set_visible(false);
                }
            }
            glib::ControlFlow::Break
        });
    }

    fn escape_streaming(self: &Rc<Self>) {
        self.bash_running.set(false);
        self.composer.set_bash_running(false);
        self.send(json!({"id": self.id(), "type": "clear_queue"}));
        self.send(json!({"id": self.id(), "type": "abort"}));
    }

    fn run_bash(self: &Rc<Self>, command: &str, exclude_from_context: bool) {
        if self.bash_running.get() {
            self.chat
                .note("there is already a bash running — stop before running another", true);
            return;
        }
        let id = format!("bash-{}", self.bash_seq.get());
        self.bash_seq.set(self.bash_seq.get() + 1);
        *self.bash_id.borrow_mut() = Some(id.clone());
        self.bash_running.set(true);
        self.composer.set_bash_running(true);
        self.chat
            .tool_start(&id, "bash", &json!({"command": command}));
        self.send(bash_command(&id, command, exclude_from_context));
    }

    fn finish_bash(self: &Rc<Self>, data: Option<&Value>) {
        let Some(id) = self.bash_id.borrow_mut().take() else {
            return;
        };
        self.bash_running.set(false);
        self.composer.set_bash_running(false);

        let output = data
            .and_then(|data| data.get("output"))
            .and_then(Value::as_str)
            .unwrap_or("");
        let exit_code = data
            .and_then(|data| data.get("exitCode"))
            .and_then(Value::as_i64);
        let cancelled = data
            .and_then(|data| data.get("cancelled"))
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let truncated = data
            .and_then(|data| data.get("truncated"))
            .and_then(Value::as_bool)
            .unwrap_or(false);
        self.chat
            .bash_finish(&id, output, exit_code, cancelled, truncated);
    }

    fn close_tab(self: &Rc<Self>, index: usize) {
        let (path, active) = {
            let state = self.state.borrow();
            (
                state.sessions.get(index).map(|session| session.path.clone()),
                state.active_session,
            )
        };
        let Some(path) = path else {
            return;
        };
        self.closed_tabs.borrow_mut().insert(path);
        if active == Some(index) {
            let next = {
                let state = self.state.borrow();
                let closed = self.closed_tabs.borrow();
                state
                    .sessions
                    .iter()
                    .enumerate()
                    .filter(|(_, session)| !closed.contains(&session.path))
                    .map(|(index, _)| index)
                    .next()
            };
            if let Some(next) = next {
                self.select_session(next);
                return;
            }
            self.new_session();
            return;
        }
        self.refresh_header();
        self.refresh_sidebar();
    }

    fn spawn_slot(
        self: &Rc<Self>,
        key: String,
        path: Option<String>,
        id: Option<String>,
    ) -> (SlotKey, RpcHandle) {
        let config = self
            .config
            .borrow()
            .for_session(path.as_deref(), id.as_deref());
        let transport = Transport::from_config(&config);
        let handle = rpc::spawn(transport);
        let slot_key = Rc::new(RefCell::new(key));
        self.start_event_loop(slot_key.clone(), handle.events.clone());
        (slot_key, handle)
    }

    fn remember_active_activity(&self) {
        let (running, phase, started, session_id, session_file, active_key) = {
            let state = self.state.borrow();
            let active = self.active_key.borrow().clone();
            let active_key = active.borrow().clone();
            (
                state.running,
                state.phase.label().to_string(),
                state.run_started,
                state.session_id.clone(),
                state.session_file.clone(),
                active_key,
            )
        };
        if !running {
            return;
        }
        let entry = SessionActivity {
            running: true,
            phase,
            started,
            unread: false,
            waiting: false,
        };
        let mut activity = self.activity.borrow_mut();
        for key in [session_id, session_file.unwrap_or_default(), active_key] {
            if !key.trim().is_empty() {
                activity.insert(key, entry.clone());
            }
        }
    }

    fn new_session(self: &Rc<Self>) {
        self.select_nav(2);
        self.composer.focus();

        if self.new_session_pending.get() {
            return;
        }
        if self.chat.is_empty() && self.state.borrow().first_prompt.is_none() {
            self.composer.focus();
            return;
        }
        if !self.state.borrow().connection.is_connected() {
            self.status.flash_notice("no connection — cannot create session");
            return;
        }
        self.new_session_pending.set(true);
        self.save_draft();
        self.remember_active_activity();

        let was_running = self.state.borrow().running;
        let old_key = self.active_key.borrow().clone();
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_millis())
            .unwrap_or(0);
        let new_id = format!("desktop-{stamp}");
        let (new_key, new_handle) =
            self.spawn_slot(new_id.clone(), None, Some(new_id.clone()));
        let old_handle = self.rpc.replace(new_handle);
        *self.active_key.borrow_mut() = new_key;
        if was_running {
            let old_key_value = old_key.borrow().clone();
            self.pool
                .borrow_mut()
                .insert(old_key_value, (old_key, old_handle));
        }

        *self.pending_draft_restore.borrow_mut() = None;
        self.pending_switch.set(None);
        *self.title_override.borrow_mut() = None;
        self.composer.set_text("");
        self.chat.clear();
        self.rail.clear_activity();
        self.composer.set_running(false);
        self.composer.set_queue(0);
        {
            let mut state = self.state.borrow_mut();
            state.reset_session_counters();
            state.session_name = None;
            state.session_id = new_id;
            state.session_file = None;
            state.active_session = None;
            state.first_prompt = None;
            state.title_requested = false;
            state.auto_named = false;
            state.messages_loaded = false;
            state.pending_messages = 0;
            state.running = false;
            state.phase = TurnPhase::Sent;
            state.run_started = None;
            state.run_elapsed = Duration::ZERO;
            state.connection = Connection::Connecting;
        }
        self.new_session_pending.set(false);
        self.sidebar.set_active(2);
        self.stack.set_visible_child_name("chat");
        self.composer.focus();
        self.ask_state();
        self.ask_stats();
        self.ask_commands();
        self.ask_models();
        self.ask_thinking_levels();
        self.refresh_header();
        self.refresh_sidebar();
        self.refresh_status();

        if let Some(pending) = self.pending_delete.borrow_mut().take() {
            if was_running {
                self.status
                    .flash_notice("session is still running; it was not deleted");
            } else {
                let path = pending.path.to_string_lossy().to_string();
                self.trash_session(&path);
                self.state
                    .borrow_mut()
                    .sessions
                    .retain(|session| session.path != path);
                self.refresh_sidebar();
                self.fetch_history();
            }
        }
    }

    fn update_rail_toggle(&self) {
        let open = self.rail_open.get();
        icons::set_icon(
            &self.rail_toggle_image,
            if open { "chevron-right" } else { "chevron-left" },
            Tone::Muted,
        );
        self.rail_toggle
            .set_tooltip_text(Some(if open { "collapse panel" } else { "expand panel" }));
        if open {
            self.rail_handle.remove_css_class("closed");
        } else {
            self.rail_handle.add_css_class("closed");
        }
    }

    fn toggle_rail(&self) {
        let open = !self.rail_open.get();
        self.rail_open.set(open);
        let page = !self.view.get().is_chat()
            && self.stack.visible_child_name().as_deref() == Some("chat");
        self.rail_column.set_visible(page);
        self.rail.root.set_visible(page && open);
        self.rail_handle.set_visible(page);
        self.apply_rail_layout();
        self.update_rail_toggle();
    }

    fn apply_rail_layout(&self) {
        let total = self.rail_paned.width();
        if total <= 1 {
            return;
        }
        let width = if self.rail_open.get() {
            self.rail_width.get().clamp(280, 460)
        } else {
            18
        };
        self.rail_paned.set_position((total - width).max(0));
    }

    fn select_nav(self: &Rc<Self>, index: usize) {
        if self.view.get().is_chat() && index != 2 {
            return;
        }
        self.sidebar.set_active(index);
        let show_rail = !self.view.get().is_chat() && index == 2;
        self.rail_column.set_visible(show_rail);
        self.rail.root
            .set_visible(show_rail && self.rail_open.get());
        self.rail_handle.set_visible(show_rail);
        self.apply_rail_layout();
        self.update_rail_toggle();
        if index == 2 {
            self.stack.set_visible_child_name("chat");
            self.fetch_history();
            return;
        }
        if index == 1 {
            self.refresh_loop_chrome();
            self.stack.set_visible_child_name("jobs");
            return;
        }
        if index == 3 {
            self.stack.set_visible_child_name("tree");
            self.refresh_sidebar();
            self.ask_tree();
            return;
        }
        if index == 5 {
            let repo = {
                let config = self.config.borrow();
                crate::config::expand_home(config.effective_workspace())
            };
            self.git.set_repo(repo);
            self.stack.set_visible_child_name("git");
            self.git.refresh();
            return;
        }
        if index == 4 {
            self.stack.set_visible_child_name("plugins");
            self.refresh_sidebar();
            if !self.plugins.is_busy() {
                if self.plugins.count() == 0 {
                    self.plugins.set_status("Loading packages...", false);
                }
                self.fetch_plugins();
            }
            return;
        }
        if index == 0 {
            self.stack.set_visible_child_name("artifacts");
            self.artifacts.refresh();
            return;
        }
        self.empty_title.set_text("Artifacts");
        self.empty_body.set_text("Nothing here yet.");
        self.stack.set_visible_child_name("empty");
    }

    fn save_draft(&self) {
        let id = self.state.borrow().session_id.clone();
        let text = self.composer.text();
        if text.trim().is_empty() {
            self.drafts.borrow_mut().remove(&id);
        } else {
            self.drafts.borrow_mut().insert(id, text);
        }
    }

    fn restore_draft(&self, id: &str) {
        let text = self.drafts.borrow().get(id).cloned().unwrap_or_default();
        self.composer.set_text(&text);
    }

    fn select_session(self: &Rc<Self>, index: usize) {
        let target = {
            let state = self.state.borrow();
            match state.sessions.get(index) {
                Some(session) => session.clone(),
                None => return,
            }
        };
        let active = self.active_key.borrow().clone();
        let active_value = active.borrow().clone();
        if active_value == target.path || active_value == target.session_id {
            self.stack.set_visible_child_name("chat");
            self.sidebar.set_active(2);
            return;
        }

        self.save_draft();
        self.remember_active_activity();
        let existing = {
            let mut pool = self.pool.borrow_mut();
            pool.remove(&target.path)
                .or_else(|| pool.remove(&target.session_id))
        };
        let (slot_key, handle) = match existing {
            Some(slot) => slot,
            None => self.spawn_slot(
                target.path.clone(),
                Some(target.path.clone()),
                Some(target.session_id.clone()),
            ),
        };

        let was_running = self.state.borrow().running;
        let old_key = self.active_key.borrow().clone();
        let old_handle = self.rpc.replace(handle);
        *self.active_key.borrow_mut() = slot_key;
        if was_running {
            let old_key_value = old_key.borrow().clone();
            self.pool
                .borrow_mut()
                .insert(old_key_value, (old_key, old_handle));
        }

        let activity = {
            let activity = self.activity.borrow();
            activity
                .get(&target.path)
                .or_else(|| activity.get(&target.session_id))
                .cloned()
        };
        let running = activity.as_ref().is_some_and(|activity| activity.running);
        let phase = activity
            .as_ref()
            .map(|activity| activity.phase.clone())
            .unwrap_or_else(|| "SENT".to_string());
        {
            let mut state = self.state.borrow_mut();
            state.reset_session_counters();
            state.active_session = Some(index);
            state.session_id = target.session_id.clone();
            state.session_file = Some(target.path.clone());
            state.session_name = target.name.clone();
            state.first_prompt = target.preview.clone();
            state.messages_loaded = false;
            state.pending_messages = 0;
            state.running = running;
            state.phase = TurnPhase::from_label(&phase);
            state.run_started = activity.as_ref().and_then(|activity| activity.started);
            state.run_elapsed = Duration::ZERO;
        }
        *self.pending_draft_restore.borrow_mut() = None;
        self.pending_switch.set(None);
        *self.title_override.borrow_mut() = None;
        self.chat.clear();
        self.rail.clear_activity();
        self.restore_draft(&target.session_id);
        self.composer.set_running(running);
        self.composer.set_queue(0);
        self.sidebar.set_active(2);
        self.stack.set_visible_child_name("chat");

        self.ask_state();
        self.ask_stats();
        self.ask_commands();
        self.ask_models();
        self.ask_thinking_levels();
        self.ask_fork_messages();
        self.send(json!({"id": self.id(), "type": "get_messages"}));

        let waiting = {
            let mut requests = self.background_requests.borrow_mut();
            requests
                .remove(&target.path)
                .or_else(|| requests.remove(&target.session_id))
        };
        if let Some(requests) = waiting {
            for request in requests {
                self.on_extension_ui(&request);
            }
        }
        {
            let mut activity = self.activity.borrow_mut();
            if let Some(entry) = activity.get_mut(&target.path) {
                entry.unread = false;
            }
            if let Some(entry) = activity.get_mut(&target.session_id) {
                entry.unread = false;
            }
        }
        self.refresh_header();
        self.refresh_sidebar();
        self.refresh_status();
    }

    fn open_session_path(&self) -> Option<String> {
        let state = self.state.borrow();
        state.session_file.clone().or_else(|| {
            state
                .active_session
                .and_then(|index| state.sessions.get(index))
                .map(|session| session.path.clone())
        })
    }

    fn rename_session(self: &Rc<Self>, index: usize, name: &str) {
        let name = name.trim().to_string();
        if name.is_empty() {
            return;
        }
        let (path, open) = {
            let state = self.state.borrow();
            let Some(session) = state.sessions.get(index) else {
                return;
            };
            (session.path.clone(), session_open_in(&state, index))
        };

        if open {
            self.send(json!({
                "id": self.id(),
                "type": "set_session_name",
                "name": name,
            }));
            return;
        }

        match session_ops::rename_session_file(std::path::Path::new(&path), &name) {
            Ok(true) => {
                {
                    let mut state = self.state.borrow_mut();
                    if let Some(session) = state.sessions.get_mut(index) {
                        session.name = Some(name.clone());
                    }
                }
                self.refresh_sidebar();
                self.status
                    .flash_notice(&format!("renamed session · {name}"));
            }
            Ok(false) => self.start_remote_op(
                RemoteOp::Rename(name),
                path,
                self.open_session_path(),
            ),
            Err(error) => self
                .status
                .flash_notice(&format!("rename failed: {error}")),
        }
    }

    fn start_remote_op(
        self: &Rc<Self>,
        op: RemoteOp,
        path: String,
        return_to: Option<String>,
    ) {
        let id = self.id();
        *self.pending_remote.borrow_mut() = Some(PendingRemote {
            op,
            path: path.clone(),
            return_to: return_to.filter(|back| back != &path),
            step_id: id.clone(),
            stage: RemoteStage::Switching,
        });
        self.send(json!({
            "id": id,
            "type": "switch_session",
            "sessionPath": path,
        }));
    }

    fn on_remote_step(self: &Rc<Self>, record: &Value) {
        let id = record.get("id").and_then(Value::as_str).unwrap_or("");
        let Some(stage) = self
            .pending_remote
            .borrow()
            .as_ref()
            .filter(|pending| pending.step_id == id)
            .map(|pending| pending.stage)
        else {
            return;
        };

        let success = record
            .get("success")
            .and_then(Value::as_bool)
            .unwrap_or(true);
        if !success {
            let error = record
                .get("error")
                .and_then(Value::as_str)
                .unwrap_or("Unknown error");
            let return_to = self
                .pending_remote
                .borrow_mut()
                .take()
                .and_then(|pending| pending.return_to);
            self.status.flash_notice(error);
            if let Some(back) = return_to {
                self.send(json!({
                    "id": self.id(),
                    "type": "switch_session",
                    "sessionPath": back,
                }));
            }
            return;
        }

        if stage == RemoteStage::Switching {
            let cancelled = record
                .get("data")
                .and_then(|data| data.get("cancelled"))
                .and_then(Value::as_bool)
                .unwrap_or(false);
            if cancelled {
                let return_to = self
                    .pending_remote
                    .borrow_mut()
                    .take()
                    .and_then(|pending| pending.return_to);
                self.status.flash_notice("operation canceled by session switch");
                if let Some(back) = return_to {
                    self.send(json!({
                        "id": self.id(),
                        "type": "switch_session",
                        "sessionPath": back,
                    }));
                }
                return;
            }

            let command = {
                let pending = self.pending_remote.borrow();
                let Some(pending) = pending.as_ref() else {
                    return;
                };
                match &pending.op {
                    RemoteOp::Rename(name) => json!({
                        "id": self.id(),
                        "type": "set_session_name",
                        "name": name,
                    }),
                    RemoteOp::Export(path) => json!({
                        "id": self.id(),
                        "type": "export_html",
                        "outputPath": path,
                    }),
                }
            };
            let next_id = command["id"].as_str().unwrap_or_default().to_string();
            if let Some(pending) = self.pending_remote.borrow_mut().as_mut() {
                pending.step_id = next_id;
                pending.stage = RemoteStage::Running;
            }
            self.send(command);
            return;
        }

        let Some(pending) = self.pending_remote.borrow_mut().take() else {
            return;
        };
        match pending.op {
            RemoteOp::Rename(name) => {
                {
                    let mut state = self.state.borrow_mut();
                    if let Some(session) = state
                        .sessions
                        .iter_mut()
                        .find(|session| session.path == pending.path)
                    {
                        session.name = Some(name.clone());
                    }
                    if state.session_file.as_deref() == Some(pending.path.as_str()) {
                        state.session_name = Some(name.clone());
                    }
                }
                self.refresh_header();
                self.refresh_sidebar();
                self.status
                    .flash_notice(&format!("renamed session · {name}"));
            }
            RemoteOp::Export(_) => {
                let path = record
                    .get("data")
                    .and_then(|data| data.get("path"))
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                self.notice_export(&path);
            }
        }

        match pending.return_to {
            Some(back) => {
                self.send(json!({
                    "id": self.id(),
                    "type": "switch_session",
                    "sessionPath": back,
                }));
            }
            None => self.fetch_history(),
        }
    }

    fn duplicate_session(self: &Rc<Self>, index: usize) {
        let (path, open) = {
            let state = self.state.borrow();
            let Some(session) = state.sessions.get(index) else {
                return;
            };
            (session.path.clone(), session_open_in(&state, index))
        };

        if open {
            self.send(json!({"id": self.id(), "type": "clone"}));
            return;
        }

        let new_id = session_ops::new_uuid();
        let now = glib::DateTime::now_local()
            .map(|now| session_ops::trash_stamp(&now))
            .unwrap_or_else(|_| "00000000-000000".to_string());
        match session_ops::clone_session_file(std::path::Path::new(&path), &new_id, &now) {
            Ok(target) => {
                let name = target
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("copy.jsonl")
                    .to_string();
                self.status
                    .flash_notice(&format!("duplicate session · {name}"));
                self.fetch_history();
            }
            Err(error) => self
                .status
                .flash_notice(&format!("duplicate failed: {error}")),
        }
    }

    fn delete_session(self: &Rc<Self>, index: usize) {
        let (path, title, open) = {
            let state = self.state.borrow();
            let Some(session) = state.sessions.get(index) else {
                return;
            };
            (
                session.path.clone(),
                session.title(),
                session_open_in(&state, index),
            )
        };

        let me = self.clone();
        dialog::confirm(&self.window, "Do you delete the session?", &title, "delete", move || {
            me.confirm_delete(path.clone(), open);
        });
    }

    fn confirm_delete(self: &Rc<Self>, path: String, open: bool) {
        if !open
            && self
                .activity
                .borrow()
                .get(&path)
                .is_some_and(|entry| entry.running)
        {
            self.status
                .flash_notice("stop the session before deleting it");
            return;
        }
        if open {
            *self.pending_delete.borrow_mut() = Some(PendingDelete {
                path: std::path::PathBuf::from(path),
            });
            self.new_session();
            return;
        }
        self.trash_session(&path);
        self.state
            .borrow_mut()
            .sessions
            .retain(|session| session.path != path);
        self.refresh_sidebar();
        self.fetch_history();
    }

    fn trash_session(self: &Rc<Self>, path: &str) {
        let stamp = glib::DateTime::now_local()
            .map(|now| session_ops::trash_stamp(&now))
            .unwrap_or_else(|_| "00000000-000000".to_string());
        match session_ops::move_to_trash(std::path::Path::new(path), &stamp) {
            Ok(target) => {
                let name = target
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("Session")
                    .to_string();
                self.status
                    .flash_notice(&format!("session moved to trash · {name}"));
            }
            Err(error) => self
                .status
                .flash_notice(&format!("delete failed: {error}")),
        }
    }

    fn export_session(self: &Rc<Self>, index: usize) {
        let (path, title, open) = {
            let state = self.state.borrow();
            let Some(session) = state.sessions.get(index) else {
                return;
            };
            (
                session.path.clone(),
                session.title(),
                session_open_in(&state, index),
            )
        };

        let stamp = glib::DateTime::now_local()
            .map(|now| session_ops::export_stamp(&now))
            .unwrap_or_else(|_| "00000000-0000".to_string());
        let destination = session_ops::export_path(&slug_from_prompt(&title), &stamp);
        if let Some(parent) = destination.parent() {
            if let Err(error) = std::fs::create_dir_all(parent) {
                self.status
                    .flash_notice(&format!("export failed: {error}"));
                return;
            }
        }
        let destination = destination.to_string_lossy().to_string();

        if open {
            self.send(json!({
                "id": self.id(),
                "type": "export_html",
                "outputPath": destination,
            }));
        } else {
            self.start_remote_op(RemoteOp::Export(destination), path, self.open_session_path());
        }
    }

    fn open_side(self: &Rc<Self>, index: usize) {
        let (id, path, same_window) = {
            let state = self.state.borrow();
            let Some(session) = state.sessions.get(index) else {
                return;
            };
            (
                session.session_id.clone(),
                session.path.clone(),
                session.session_id == state.session_id,
            )
        };
        if id.is_empty() {
            self.status.flash_notice("open in new window: session has no id");
            return;
        }
        if same_window {
            self.status
                .flash_notice("this session is already open in this window");
            return;
        }

        let exe = match std::env::current_exe() {
            Ok(exe) => exe,
            Err(error) => {
                self.status.flash_notice(&format!("open in new window failed: {error}"));
                return;
            }
        };
        let mut command = std::process::Command::new(exe);
        command.arg("--session").arg(&id);
        if let Some(project) = session_ops::slug_to_path(&session_ops::session_slug(&path)) {
            let current = self.state.borrow().workspace.clone();
            if project.is_dir() && project != std::path::Path::new(&current) {
                command.arg("--workspace").arg(project);
            }
        }
        if let Err(error) = command.spawn() {
            self.status.flash_notice(&format!("open in new window failed: {error}"));
        }
    }

    fn notice_export(self: &Rc<Self>, path: &str) {
        if path.trim().is_empty() {
            self.status.flash_notice("HTML exported");
            return;
        }
        let target = path.to_string();
        let short = target
            .rsplit('/')
            .next()
            .unwrap_or(&target)
            .to_string();
        self.status
            .flash_action(&format!("html · {short}"), "open", move || {
                let _ = std::process::Command::new("xdg-open")
                    .arg(&target)
                    .spawn();
            });
    }


    fn ask_fork_messages(&self) {
        self.send(json!({"id": self.id(), "type": "get_fork_messages"}));
    }

    fn show_message_menu(self: &Rc<Self>, anchor: &gtk::Box, text: &str) {
        let entry = {
            let points = self.fork_points.borrow();
            points
                .iter()
                .rev()
                .find(|(_, point)| point.trim() == text.trim())
                .map(|(entry, _)| entry.clone())
        };
        let Some(entry) = entry else {
            return;
        };
        let me = self.clone();
        context_menu::show(
            anchor,
            vec![(
                "Ramify from here",
                std::rc::Rc::new(move || me.fork_from(&entry)),
            )],
        );
    }

    fn fork_from(self: &Rc<Self>, entry_id: &str) {
        self.send(json!({
            "id": self.id(),
            "type": "fork",
            "enterId": entry_id,
        }));
    }


    fn ask_tree(self: &Rc<Self>) {
        if let Ok(path) = std::env::var("PI_DESKTOP_TREE_FIXTURE") {
            match std::fs::read_to_string(&path) {
                Ok(raw) => match serde_json::from_str::<Value>(&raw) {
                    Ok(value) => {
                        let view = crate::tree::parse(&value);
                        self.tree.set_view(&view);
                        self.refresh_sidebar();
                    }
                    Err(error) => self
                        .tree
                        .set_error(&format!("invalid fixture: {error}")),
                },
                Err(error) => self
                    .tree
                    .set_error(&format!("unread fixture: {error}")),
            }
            return;
        }
        self.tree.set_loading();
        self.send(json!({"id": self.id(), "type": "get_tree"}));
    }


    fn copy_last_assistant(&self) {
        self.send(json!({
            "id": self.id(),
            "type": "get_last_assistant_text",
        }));
    }

    fn copy_to_clipboard(&self, text: &str) -> bool {
        let Some(display) = gtk::gdk::Display::default() else {
            return false;
        };
        display.clipboard().set_text(text);
        true
    }

    fn demo_history(self: &Rc<Self>) {
        let messages = json!([
            {"role": "user", "content": [{"type": "text", "text": "lists the files and runs the tests"}]},
            {"role": "assistant", "content": [
                {"type": "text", "text": "I'll list the directory and run the suite."},
                {"type": "toolCall", "id": "call_1", "name": "bash", "arguments": {"command": "I'm--I'm--I'm--I'm--I'm--I'm--I'm--I'm--I'm--I'm--I'm--I'm--I'm--I'm--I'm--I'm--I'm--I'm--I'm--I'm--I'm--I'm--I'm--I'm--I'm--I'm--"}}
            ]},
            {"role": "toolResult", "toolCallId": "call_1", "tool": "bash", "isError": false,
             "content": [{"type": "text", "text": "total 48\ndrwxr-xr-x 1 user 4096 Nov 8 22:46 .\ndrwxr-xr-x 1 user 4096 Nov 8 21:26 ..\n-rw-r--r-- 1 user 2724 Nov 8 21:44 build.rs\ndrwxr-xr-x 1 user 332 Nov 8 22:48 docs\n-rw-r-- 1 user 142 Nov 8 22:34 src"}]},
            {"role": "assistant", "content": [
                {"type": "text", "text": "Now the tests."},
                {"type": "toolCall", "id": "call_2", "name": "bash", "arguments": {"command": "test load"}}
            ]},
            {"role": "toolResult", "toolCallId": "call_2", "tool": "bash", "isError": true,
             "content": [{"type": "text", "text": "error: linking with `cc` failed\nexit status: 1\n\nnote: see full log above"}]},
            {"role": "assistant", "content": [
                {"type": "text", "text": "**Summary:** listing ok; `cargo test` failed at link time (environment error, not code)."}
            ]},
            {"role": "assistant", "content": [
                {"type": "text", "text": "Provider catalog:\n\n-Provider . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . ."}
            ]},
            {"role": "assistant", "content": [
                {"type": "text", "text": "Links: [Pi docs](https://pi.dev) · [OpenAI](https://openai.com)"}
            ]}
        ]);
        if let Some(messages) = messages.as_array() {
            self.chat.load_messages(messages);
        }
    }

    fn demo_diff(self: &Rc<Self>) {
        let messages = std::env::var("PI_DESKTOP_SESSION_FILE")
            .ok()
            .map(std::path::PathBuf::from)
            .map(|path| history::read_messages(&path))
            .filter(|messages| !messages.is_empty())
            .unwrap_or_else(demo_diff_messages);
        self.chat.load_messages(&messages);
        let me = self.clone();
        glib::timeout_add_local_once(std::time::Duration::from_millis(800), move || {
            me.chat.scroll_to_last_diff();
        });
    }

    fn demo_loop(self: &Rc<Self>) {
        *self.loop_state.borrow_mut() = Some(ActiveLoop {
            state: LoopState {
                task: "review the kernel build and report regressions".to_string(),
                interval: 300,
                iteration: 3,
                active: true,
            },
            next_fire: Instant::now() + Duration::from_secs(298),
        });
        self.refresh_loop_chrome();
    }

    fn set_model(self: &Rc<Self>, provider: String, id: String, name: String) {
        if provider.trim().is_empty() || id.trim().is_empty() || id == "undefined" {
            self.chat
                .note("model unavailable — choose a model from the list", true);
            return;
        }
        let command = json!({
            "id": self.id(),
            "type": "set_model",
            "provider": provider,
            "modelId": id,
        });
        self.send(command);

        {
            let mut state = self.state.borrow_mut();
            state.model = name;
            state.model_id = id.clone();
            state.model_provider = provider.clone();
            let label = format!("{provider}/{id}");
            let active = state.active_session;
            if let Some(index) = active {
                if let Some(session) = state.sessions.get_mut(index) {
                    session.model = Some(label.clone());
                }
            }
        }
        self.refresh_header();
        self.refresh_sidebar();
        self.ask_thinking_levels();
    }

    fn cycle_model(self: &Rc<Self>) {
        self.send(json!({"id": self.id(), "type": "cycle_model"}));
    }

    fn set_permission(self: &Rc<Self>, permission: crate::state::Permission) {
        let config = {
            let mut config = self.config.borrow_mut();
            config.approve = permission.to_approve();
            config.clone()
        };
        if let Err(error) = config.save() {
            self.status.flash_notice(&format!("saving permissions: {error}"));
            return;
        }
        {
            let mut state = self.state.borrow_mut();
            state.permission = permission;
        }
        self.status.perm.set_text(permission.label());
        self.status
            .perm_button
            .set_tooltip_text(Some(permission.hint()));
        self.reconnect(config);
    }


    fn settings_models(&self) -> Vec<ModelItem> {
        let live = self.state.borrow().models.clone();
        if !live.is_empty() {
            return live;
        }
        crate::providers::known_models(&crate::config::pi_agent_dir())
            .into_iter()
            .map(|model| ModelItem {
                provider: model.provider,
                id: model.id,
                name: model.name,
            })
            .collect()
    }

    fn open_settings(self: &Rc<Self>) {
        let config = self.config.borrow().clone();
        let me = self.clone();
        let close_me = self.clone();
        settings::show(
            &self.window,
            config,
            self.settings_models(),
            move |config, reconnect| me.save_settings(config, reconnect),
            move || close_me.apply_provider_changes(),
            None,
        );
    }

    fn delete_workspace(self: &Rc<Self>, label: String, dir: String, paths: Vec<String>) {
        if dir.trim().is_empty() || paths.is_empty() {
            return;
        }
        if !matches!(&*self.transport.borrow(), Transport::Local(_)) {
            self.status
                .flash_notice("workspace deletion is available for local sessions only");
            return;
        }
        let active = self.state.borrow().session_file.clone();
        if active
            .as_deref()
            .is_some_and(|path| path.starts_with(&dir))
        {
            self.status
                .flash_notice("switch to another session before deleting this workspace");
            return;
        }
        let busy = {
            let state = self.state.borrow();
            let activity = self.activity.borrow();
            state
                .sessions
                .iter()
                .filter(|session| session.path.starts_with(&dir))
                .any(|session| {
                    activity
                        .get(&session.path)
                        .or_else(|| activity.get(&session.session_id))
                        .is_some_and(|entry| entry.running)
                })
        };
        if busy {
            self.status
                .flash_notice("stop running sessions in this workspace before deleting it");
            return;
        }
        let count = paths.len();
        let noun = if count == 1 { "session" } else { "sessions" };
        let perform: Rc<dyn Fn()> = Rc::new({
            let me = self.clone();
            let label = label.clone();
            let dir = dir.clone();
            move || me.trash_workspace(label.clone(), dir.clone(), count)
        });
        if count >= 5 {
            let second = perform.clone();
            let me_second = self.clone();
            let label_second = label.clone();
            let third = Rc::new(move || {
                let me_third = me_second.clone();
                let perform_third = second.clone();
                let message = format!(
                    "Delete {count} {noun} from '{label_second}'? After 14 days the trash is purged permanently."
                );
                dialog::confirm(
                    &me_third.window,
                    "Confirm workspace deletion",
                    &message,
                    "delete",
                    move || perform_third(),
                );
            });
            let me_first = self.clone();
            let third_c = third.clone();
            let first = format!(
                "'{label}' groups {count} {noun}. Every session will move to the trash. The project folder itself is not touched."
            );
            dialog::confirm(
                &self.window,
                "Delete workspace",
                &first,
                "continue",
                move || {
                    let me = me_first.clone();
                    let third_c = third_c.clone();
                    let second = format!(
                        "{count} {noun} will disappear from the sidebar and from Pi history. You can restore them for 14 days."
                    );
                    dialog::confirm(
                        &me.window,
                        "Move sessions to trash",
                        &second,
                        "continue",
                        move || third_c(),
                    );
                },
            );
        } else {
            let message = format!(
                "Delete '{label}'? {count} {noun} move to the trash and can be restored for 14 days. The project folder is not touched."
            );
            dialog::confirm(
                &self.window,
                "Delete workspace",
                &message,
                "delete",
                move || perform(),
            );
        }
    }

    fn trash_workspace(self: &Rc<Self>, label: String, dir: String, count: usize) {
        let stamp = glib::DateTime::now_local()
            .map(|now| session_ops::trash_stamp(&now))
            .unwrap_or_else(|_| "00000000-000000".to_string());
        match session_ops::trash_workspace(
            std::path::Path::new(&dir),
            &label,
            count,
            &stamp,
        ) {
            Ok(target) => {
                self.fetch_history();
                self.refresh_sidebar();
                let me = self.clone();
                self.status.flash_action(
                    &format!("workspace moved to trash · {label}"),
                    "undo",
                    move || {
                        let _ = session_ops::restore_workspace_path(&target);
                        me.fetch_history();
                        me.refresh_sidebar();
                    },
                );
            }
            Err(error) => self
                .status
                .flash_notice(&format!("delete failed: {error}")),
        }
    }

    fn show_trash(self: &Rc<Self>) {
        let me = self.clone();
        trash::show(&self.window, move || {
            me.fetch_history();
            me.refresh_sidebar();
        });
    }

    fn configure_plugin(self: &Rc<Self>, spec: &str) {
        match plugin_io::settings_for(spec) {
            Some(plugin_io::PluginSettings::Tab(tab)) => self.open_settings_tab(tab),
            Some(plugin_io::PluginSettings::Json { title, path }) => {
                plugin_config::show(
                    &self.window,
                    self.transport.borrow().clone(),
                    title,
                    path,
                    true,
                );
            }
            Some(plugin_io::PluginSettings::Text { title, path }) => {
                plugin_config::show(
                    &self.window,
                    self.transport.borrow().clone(),
                    title,
                    path,
                    false,
                );
            }
            None => self
                .status
                .flash_notice("this plugin does not expose settings"),
        }
    }

    fn open_settings_tab(self: &Rc<Self>, tab: &str) {
        let config = self.config.borrow().clone();
        let me = self.clone();
        let close_me = self.clone();
        settings::show(
            &self.window,
            config,
            self.settings_models(),
            move |config, reconnect| me.save_settings(config, reconnect),
            move || close_me.apply_provider_changes(),
            Some(tab.to_string()),
        );
    }

    fn apply_provider_changes(self: &Rc<Self>) {
        let busy = {
            let state = self.state.borrow();
            state.running || state.pending_messages > 0
        };
        if busy {
            self.toast_text("Saved Providers · Reconnect with Ctrl+K when shift is over");
            return;
        }
        let config = self.config.borrow().clone();
        self.toast_text("changed providers · reconnecting to Pi...");
        self.reconnect(config);
    }

    fn open_palette(self: &Rc<Self>) {
        use command_palette::Item;

        let toplevels = gtk::Window::toplevels();
        for index in 0..toplevels.n_items() {
            let Some(item) = toplevels.item(index) else {
                continue;
            };
            let Ok(window) = item.downcast::<gtk::Window>() else {
                continue;
            };
            if window.title().as_deref() == Some("Commands") {
                window.present();
                return;
            }
        }

        self.composer.close_palette();
        self.composer.close_skill_popup();
        self.header.close_models();

        let mut items: Vec<Item> = Vec::new();

        {
            let running = self.state.borrow().running;
            let me = self.clone();
            items.push(Item::new("New session", "ctrl+n", "ACTIONS", move || {
                me.new_session()
            }));
            let me = self.clone();
            items.push(Item::new("Change model", "header menu", "ACTIONS", move || {
                me.header.open_models()
            }));
            let me = self.clone();
            items.push(Item::new("Change persona", "shift+tab", "ACTIONS", move || {
                me.send_extension_command_text("/persona")
            }));
            let me = self.clone();
            items.push(Item::new("Next model", "ctrl+p", "ACTIONS", move || {
                me.cycle_model()
            }));
            let me = self.clone();
            items.push(Item::new("Next Reason", "ctrl+t", "ACTIONS", move || {
                me.cycle_thinking()
            }));
            let me = self.clone();
            items.push(Item::new("Settings", "ctrl+,", "ACTIONS", move || {
                me.open_settings()
            }));
            let me = self.clone();
            items.push(Item::new("Reconnect to Pi", "load auth/models", "ACTIONS", move || {
                let config = me.config.borrow().clone();
                me.reconnect(config);
            }));
            let me = self.clone();
            items.push(Item::new("Configure Providers", "keys and models", "ACTIONS", move || {
                me.open_settings_tab("PROVIDERS")
            }));
            let me = self.clone();
            items.push(Item::new("Configure Memory", "USER.md and MEMORY.md", "ACTIONS", move || {
                me.open_settings_tab("MEMORY")
            }));
            let me = self.clone();
            items.push(Item::new("Configure MCP", "servers and login", "ACTIONS", move || {
                me.open_settings_tab("MCP")
            }));
            let me = self.clone();
            items.push(Item::new("Copy Last Answer", "/copy", "ACTIONS", move || {
                me.copy_last_assistant()
            }));
            let me = self.clone();
            items.push(Item::new("Session Tree", "branch and rewind", "ACTIONS", move || {
                me.select_nav(3)
            }));
            let me = self.clone();
            items.push(Item::new("Tasks", "Active loop", "ACTIONS", move || me.select_nav(1)));
            let me = self.clone();
            items.push(Item::new("Extensions", "Pi packages", "ACTIONS", move || {
                me.select_nav(4)
            }));
            let me = self.clone();
            items.push(Item::new("Git review", "diff, internship and commit", "ACTIONS", move || {
                me.select_nav(5)
            }));
            let me = self.clone();
            items.push(Item::new("Find Sessions", "ctrl+shift+f", "ACTIONS", move || {
                me.sidebar.open_search()
            }));
            let me = self.clone();
            items.push(Item::new("Search chats", "ctrl+f", "ACTIONS", move || {
                if me.stack.visible_child_name().as_deref() != Some("chat") {
                    me.select_nav(2);
                }
                me.chat.find_open();
            }));
            let me = self.clone();
            items.push(Item::new("Change Flat Mode", "/plan", "ACTIONS", move || {
                me.composer.set_text("/plan");
                me.submit(SubmitKind::Enter);
            }));
            let me = self.clone();
            items.push(Item::new("View plan (/all)", "/todos", "ACTIONS", move || {
                me.composer.set_text("/todos");
                me.submit(SubmitKind::Enter);
            }));
            let me = self.clone();
            items.push(Item::new("Shortcuts", "ctrl+/", "ACTIONS", move || {
                hotkeys::show(&me.window)
            }));
            if running {
                let me = self.clone();
                items.push(Item::new("Stop Turn", "esc", "ACTIONS", move || {
                    me.escape_streaming()
                }));
            }
        }

        let (sessions, active) = {
            let state = self.state.borrow();
            (state.sessions.clone(), state.active_session)
        };
        if let Some(index) = active {
            if sessions.get(index).is_some() {
                let me = self.clone();
                items.push(Item::new(
                    "Export as HTML".to_string(),
                    "file",
                    "ACTIONS",
                    move || {
                        if let Some(index) = me.state.borrow().active_session {
                            me.export_session(index)
                        }
                    },
                ));
            }
        }

        for (index, session) in sessions.iter().enumerate().take(15) {
            let hint = session.model.clone().unwrap_or_else(|| "Session".to_string());
            let name = session.title();
            let me = self.clone();
            items.push(Item::new(name, hint, "SESSIONS", move || {
                me.select_session(index)
            }));
        }

        command_palette::show(&self.window, items);
    }

    fn save_settings(self: &Rc<Self>, config: Config, reconnect: bool) {
        let reconnect = reconnect || self.config.borrow().needs_reconnect(&config);
        let mut config = config;
        let previous = self.config.borrow();
        config.view = previous.view;
        config.notify = previous.notify;
        if let Some(id) = previous.launch_session_id() {
            config.set_launch_session_id(Some(id.to_string()));
        } else if !config.start_last && config.session_override().is_none() {
            let launch = config.new_launch_session_id();
            config.set_launch_session_id(Some(launch));
        }
        drop(previous);
        config.apply_overrides(self.session_override.clone(), self.workspace_override.clone());
        let mode = crate::theme::load_mode().unwrap_or(ThemeMode::System);
        self.theme.set_mode(mode);
        self.theme.set_font_scale(config.font_scale as u32);
        self.theme.set_font(config.font);
        self.chat.set_show_timestamps(config.timestamps);
        self.chat.set_auto_scroll(config.auto_scroll);
        self.chat.set_chat_width(config.chat_width);
        self.status.set_show_costs(config.show_costs);
        self.config.replace(config.clone());
        *self.configured_thinking.borrow_mut() = match config.thinking {
            crate::config::Thinking::Unset => None,
            level => Some(level.as_str().to_string()),
        };
        self.state.borrow_mut().apply_config(&config);
        self.refresh_header();
        self.refresh_sidebar();
        self.refresh_status();
        if reconnect {
            self.reconnect(config);
        }
    }

    fn reconnect(self: &Rc<Self>, config: Config) {
        let transport = Transport::from_config(&config);
        self.transport.replace(transport.clone());
        {
            let mut state = self.state.borrow_mut();
            state.messages_loaded = false;
            state.running = false;
            state.pending_messages = 0;
            state.connection = Connection::Connecting;
            state.conn_detail.clear();
            state.session_id = config.effective_session_id().to_string();
            state.reset_session_counters();
            state.session_id = config.effective_session_id().to_string();
        }
        self.composer.set_running(false);
        self.composer.set_queue(0);
        self.rpc.borrow().restart(transport);
        self.refresh_status();
        self.refresh_header();
        self.refresh_sidebar();
    }


    fn ask_thinking_levels(&self) {
        self.send(json!({"id": self.id(), "type": "get_available_thinking_levels"}));
    }

    fn apply_thinking_levels(self: &Rc<Self>, levels: Vec<String>) {
        let fallback = {
            let mut state = self.state.borrow_mut();
            state.thinking_levels = levels.clone();
            if let Some(level) = crate::state::thinking_fallback(&levels, &state.thinking_level) {
                state.thinking_level = level.clone();
                Some(level)
            } else {
                None
            }
        };
        if let Some(level) = fallback {
            self.send(json!({
                "id": self.id(),
                "type": "set_thinking_level",
                "level": level,
            }));
        }
        if let Some(level) = self.configured_thinking.borrow_mut().take() {
            if levels.iter().any(|candidate| candidate == &level) {
                self.state.borrow_mut().thinking_level = level.clone();
                self.send(json!({
                    "id": self.id(),
                    "type": "set_thinking_level",
                    "level": level,
                }));
            } else if !levels.is_empty() {
                self.chat.note(
                    &format!("thinking '{level}' not available in this model"),
                    false,
                );
            }
        }
        self.refresh_header();
        self.refresh_status();
    }

    fn set_thinking_level(self: &Rc<Self>, level: &str) {
        self.state.borrow_mut().thinking_level = level.to_string();
        self.send(json!({
            "id": self.id(),
            "type": "set_thinking_level",
            "level": level,
        }));
        self.refresh_header();
        self.refresh_status();
    }

    fn cycle_thinking(self: &Rc<Self>) {
        let has_levels = {
            let state = self.state.borrow();
            !(state.thinking_levels.is_empty()
                || (state.thinking_levels.len() == 1 && state.thinking_levels[0] == "off"))
        };
        if !has_levels {
            return;
        }
        self.send(json!({"id": self.id(), "type": "cycle_thinking_level"}));
    }


    fn fetch_history(&self) {
        history::spawn_fetch(self.transport.borrow().clone(), self.history_tx.clone());
    }

    fn on_history(self: &Rc<Self>, list: Vec<RemoteSession>) {
        let mut state = self.state.borrow_mut();
        state.sessions = list;
        state.history_loaded = true;
        state.resolve_active_session();
        drop(state);
        self.refresh_sidebar();
        self.sidebar.scroll_sessions_top();
    }

    fn upsert_current_session(&self, touch: bool) {
        let mut state = self.state.borrow_mut();
        let Some(path) = state.session_file.clone() else {
            return;
        };
        let id = state.session_id.trim().to_string();
        if id.is_empty() {
            return;
        }
        let name = state.session_name.clone().filter(|name| !name.trim().is_empty());
        let preview = state.first_prompt.clone();
        if preview.is_none() {
            return;
        }
        let model = if !state.model_provider.is_empty() && !state.model_id.is_empty() {
            Some(format!("{}/{}", state.model_provider, state.model_id))
        } else if !state.model_id.is_empty() {
            Some(state.model_id.clone())
        } else {
            None
        };
        let mtime = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_secs() as i64)
            .unwrap_or(0);

        if let Some(index) = state
            .sessions
            .iter()
            .position(|session| session.path == path || session.session_id == id)
        {
            let mut session = if touch && index > 0 {
                state.sessions.remove(index)
            } else {
                state.sessions.remove(index)
            };
            if name.is_some() {
                session.name = name;
            }
            if preview.is_some() {
                session.preview = preview;
            }
            if model.is_some() {
                session.model = model;
            }
            session.session_id = id;
            if touch {
                session.mtime = mtime;
            }
            if touch {
                state.sessions.insert(0, session);
                state.active_session = Some(0);
            } else {
                state.sessions.insert(index, session);
                state.active_session = Some(index);
            }
        } else {
            state.sessions.insert(
                0,
                RemoteSession {
                    path,
                    name,
                    model,
                    preview: preview.or_else(|| Some("new conversation".to_string())),
                    session_id: id,
                    mtime,
                },
            );
            state.active_session = Some(0);
        }
        drop(state);
        self.refresh_header();
        self.refresh_sidebar();
    }


    fn fetch_plugins(&self) {
        plugin_io::spawn_fetch(self.transport.borrow().clone(), self.plugin_tx.clone());
    }

    fn install_plugin(self: &Rc<Self>, input: &str) {
        let Some(spec) = plugin_io::normalize_spec(input) else {
            self.plugins
                .set_status("invalid name — use letters, numbers, . @ / - : _", true);
            return;
        };
        self.plugins.set_busy(true, &format!("installing {spec}..."));
        plugin_io::spawn_install(self.transport.borrow().clone(), spec, self.plugin_tx.clone());
    }

    fn remove_plugin(self: &Rc<Self>, spec: &str) {
        self.plugins.set_busy(true, &format!("removing {spec}..."));
        plugin_io::spawn_remove(
            self.transport.borrow().clone(),
            spec.to_string(),
            self.plugin_tx.clone(),
        );
    }

    fn on_plugin_event(self: &Rc<Self>, event: PluginEvent) {
        match event {
            PluginEvent::List(Ok(items)) => {
                let count = items.len();
                {
                    let mut state = self.state.borrow_mut();
                    state.plugins = items.clone();
                }
                self.plugins.set_items(&items);
                if self.plugins_root_visible() && !self.plugins.is_busy() {
                    self.plugins.set_status(&plugin_count_text(count), false);
                }
                self.refresh_sidebar();
            }
            PluginEvent::List(Err(error)) => {
                self.plugins.set_status(&error, true);
            }
            PluginEvent::Installed(result) => {
                self.plugins.set_busy(false, "");
                match result {
                    Ok(()) => {
                        self.plugins.entry.set_text("");
                        self.plugins.set_status("installed · updated list", false);
                    }
                    Err(error) => {
                        self.plugins
                            .set_status(&format!("install failed: {error}"), true);
                    }
                }
            }
            PluginEvent::Removed(result) => {
                self.plugins.set_busy(false, "");
                match result {
                    Ok(()) => {
                        self.plugins.set_status("removed · updated list", false);
                    }
                    Err(error) => {
                        self.plugins
                            .set_status(&format!("Failed to remove: {error}"), true);
                    }
                }
            }
        }
    }

    fn plugins_root_visible(&self) -> bool {
        self.stack.visible_child_name().as_deref() == Some("plugins")
    }


    fn search_catalog(self: &Rc<Self>, query: &str) {
        catalog_io::spawn_search(query.to_string(), 0, self.catalog_tx.clone());
    }

    fn more_catalog(self: &Rc<Self>) {
        let query = self.plugins.search_entry.text().trim().to_string();
        let from = self.plugins.catalog_count();
        catalog_io::spawn_search(query, from, self.catalog_tx.clone());
    }

    fn install_catalog_package(&self, name: &str) {
        catalog_io::spawn_install(
            self.transport.borrow().clone(),
            name.to_string(),
            self.catalog_tx.clone(),
        );
    }

    fn on_catalog_event(self: &Rc<Self>, event: CatalogEvent) {
        match event {
            CatalogEvent::Search(Ok((page, append))) => {
                self.plugins.set_catalog_page(&page, append)
            }
            CatalogEvent::Search(Err(error)) => self.plugins.set_catalog_error(&error),
            CatalogEvent::Installed { name, result } => {
                self.plugins.finish_catalog_install(&name, &result);
                if result.is_ok() {
                    self.fetch_plugins();
                }
            }
        }
    }


    fn start_event_loop(
        self: &Rc<Self>,
        key: SlotKey,
        events: async_channel::Receiver<RpcEvent>,
    ) {
        let me = self.clone();
        glib::spawn_future_local(async move {
            while let Ok(event) = events.recv().await {
                me.on_session_event(&key, event);
            }
        });
    }

    fn on_session_event(self: &Rc<Self>, key: &SlotKey, event: RpcEvent) {
        let active = self.active_key.borrow().clone();
        if Rc::ptr_eq(key, &active) {
            self.on_event(event);
        } else {
            self.on_background_event(key, event);
        }
    }

    fn on_background_event(self: &Rc<Self>, key: &SlotKey, event: RpcEvent) {
        let key = key.borrow().clone();
        match event {
            RpcEvent::Conn(connection) => {
                if !connection.is_connected() {
                    if let Some(activity) = self.activity.borrow_mut().get_mut(&key) {
                        activity.running = false;
                        activity.phase = "ERROR".to_string();
                    }
                    self.refresh_sidebar();
                    self.refresh_header();
                }
            }
            RpcEvent::Stderr(_) => {}
            RpcEvent::Record(record) => {
                let kind = record.get("type").and_then(Value::as_str).unwrap_or("");
                let mut changed = false;
                match kind {
                    "agent_start" => {
                        let mut activity = self.activity.borrow_mut();
                        let entry = activity.entry(key.clone()).or_insert_with(|| SessionActivity {
                            running: true,
                            phase: "THINKING".to_string(),
                            started: Some(Instant::now()),
                            unread: false,
                            waiting: false,
                        });
                        entry.running = true;
                        entry.phase = "THINKING".to_string();
                        entry.started = Some(Instant::now());
                        entry.unread = false;
                        entry.waiting = false;
                        changed = true;
                    }
                    "thinking_start" | "auto_retry_start" | "compaction_start" => {
                        if let Some(activity) = self.activity.borrow_mut().get_mut(&key) {
                            activity.phase = "THINKING".to_string();
                            changed = true;
                        }
                    }
                    "tool_execution_start" => {
                        if let Some(activity) = self.activity.borrow_mut().get_mut(&key) {
                            activity.phase = "TOOLS".to_string();
                            changed = true;
                        }
                    }
                    "message_start" | "message_update" => {
                        let update = record
                            .get("AssistantMessageEvent")
                            .and_then(|event| event.get("type"))
                            .and_then(Value::as_str)
                            .unwrap_or("");
                        if update == "text_delta" || kind == "message_start" {
                            if let Some(activity) = self.activity.borrow_mut().get_mut(&key) {
                                activity.phase = "WRITING".to_string();
                                changed = true;
                            }
                        }
                    }
                    "agent_settled" => {
                        if let Some(activity) = self.activity.borrow_mut().get_mut(&key) {
                            activity.running = false;
                            activity.phase = "DONE".to_string();
                            activity.unread = true;
                            activity.waiting = false;
                        }
                        self.pool.borrow_mut().remove(&key);
                        changed = true;
                    }
                    "extension_ui_request" => {
                        if let Some(request) = extension_ui::Request::parse(&record) {
                            if request.method.is_dialog() {
                                let was_waiting = self
                                    .activity
                                    .borrow()
                                    .get(&key)
                                    .map(|activity| activity.waiting)
                                    .unwrap_or(false);
                                let mut activity = self.activity.borrow_mut();
                                let entry = activity.entry(key.clone()).or_insert_with(|| SessionActivity {
                                    running: true,
                                    phase: "WAITING".to_string(),
                                    started: Some(Instant::now()),
                                    unread: true,
                                    waiting: true,
                                });
                                entry.phase = "WAITING".to_string();
                                entry.waiting = true;
                                entry.unread = true;
                                drop(activity);
                                let mut requests = self.background_requests.borrow_mut();
                                let list = requests.entry(key.clone()).or_default();
                                let id = record.get("id").and_then(Value::as_str).unwrap_or("");
                                if !list.iter().any(|item| {
                                    item.get("id").and_then(Value::as_str) == Some(id)
                                }) {
                                    list.push(record.clone());
                                }
                                if !was_waiting {
                                    self.status
                                        .flash_notice("a background session is waiting for approval");
                                }
                                changed = true;
                            }
                        }
                    }
                    "error" => {
                        if let Some(activity) = self.activity.borrow_mut().get_mut(&key) {
                            activity.phase = "ERROR".to_string();
                            changed = true;
                        }
                    }
                    _ => {}
                }
                if changed {
                    self.refresh_sidebar();
                    self.refresh_header();
                }
            }
        }
    }

    fn on_event(self: &Rc<Self>, event: RpcEvent) {
        match event {
            RpcEvent::Conn(connection) => {
                let connected = connection.is_connected();
                let was_running = {
                    let mut state = self.state.borrow_mut();
                    let was_running = state.running;
                    if let Connection::Disconnected(detail) = &connection {
                        state.conn_detail = detail.clone();
                        if !detail.trim().is_empty() {
                            self.show_error_banner(&format!("connection: {detail}"));
                        }
                        if let Some(start) = state.run_started.take() {
                            state.run_elapsed = start.elapsed();
                        }
                        state.running = false;
                    }
                    state.connection = connection;
                    was_running
                };
                if !connected && was_running {
                    self.composer.set_running(false);
                    self.chat.note("Connection lost during execution", true);
                    let (elapsed, tokens) = {
                        let state = self.state.borrow();
                        (state.elapsed(), state.turn_tokens)
                    };
                    self.state.borrow_mut().phase = TurnPhase::Error;
                    self.chat
                        .turn_finish("ERROR", &format_elapsed(elapsed), tokens);
                }
                self.refresh_status();
                self.refresh_sidebar();
                if connected {
                    self.ask_state();
                    self.ask_commands();
                    self.ask_models();
                    self.ask_thinking_levels();
                    if !self.state.borrow().messages_loaded {
                        self.send(json!({"id": self.id(), "type": "get_messages"}));
                    }
                    if self.state.borrow().sessions.is_empty() {
                        self.fetch_history();
                    }
                    self.ask_fork_messages();
                    self.fetch_plugins();
                }
            }
            RpcEvent::Record(record) => self.on_record(&record),
            RpcEvent::Stderr(line) => {
                self.state.borrow_mut().conn_detail = line;
            }
        }
    }

    fn on_record(self: &Rc<Self>, record: &Value) {
        let kind = record.get("type").and_then(Value::as_str).unwrap_or("");

        match kind {
            "agent_start" => {
                {
                    let mut state = self.state.borrow_mut();
                    if !state.running {
                        state.reset_turn();
                    }
                    state.running = true;
                    state.phase = TurnPhase::Thinking;
                }
                self.composer.set_running(true);
                self.refresh_status();
                self.refresh_turn();
                self.refresh_sidebar();
            }
            "agent_settled" => {
                let aborted = record
                    .get("aborted")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                let (elapsed, tokens) = {
                    let mut state = self.state.borrow_mut();
                    if let Some(start) = state.run_started.take() {
                        state.run_elapsed = start.elapsed();
                    }
                    state.running = false;
                    state.phase = TurnPhase::Done;
                    (state.elapsed(), state.turn_tokens)
                };
                self.chat.turn_finish(
                    if aborted { "STOPPED" } else { "DONE" },
                    &format_elapsed(elapsed),
                    tokens,
                );
                self.composer.set_running(false);
                self.ask_stats();
                self.ask_state();
                self.refresh_status();
                self.refresh_sidebar();
                self.notify_settle(aborted);
                self.loop_after_settle();
            }
            "message_start" => {
                if role_of(record.get("message")) == Some("assistant") {
                    self.chat.begin_assistant();
                }
            }
            "message_update" => {
                if let Some(usage) = record.get("usage") {
                    let mut state = self.state.borrow_mut();
                    if let Some(input) = usage.get("input").and_then(Value::as_u64) {
                        state.tokens_in = input;
                    }
                    if let Some(output) = usage.get("output").and_then(Value::as_u64) {
                        state.turn_tokens = state.turn_finished_tokens + output;
                    }
                }
                if let Some(update) = record.get("AssistantMessageEvent") {
                    self.on_assistant_update(update);
                }
                self.refresh_status();
            }
            "message_end" => {
                if std::env::var("PI_DESKTOP_DEBUG_SCROLL").is_ok() {
                    eprintln!("message_end role={:?}", role_of(record.get("message")));
                }
                if role_of(record.get("message")) == Some("assistant") {
                    if let Some(message) = record.get("message") {
                        self.chat.assistant_final(message);
                        let output = message
                            .get("usage")
                            .and_then(|usage| usage.get("output"))
                            .and_then(Value::as_u64)
                            .unwrap_or(0);
                        let mut state = self.state.borrow_mut();
                        state.turn_finished_tokens += output;
                        state.turn_tokens = state.turn_finished_tokens;
                    }
                    let text = record
                        .get("message")
                        .map(assistant_text)
                        .unwrap_or_default();
                    if loops::has_loop_complete(&text) {
                        let iteration = self
                            .loop_state
                            .borrow()
                            .as_ref()
                            .map(|active| active.state.iteration);
                        if let Some(iteration) = iteration {
                            self.stop_loop(Some(iteration));
                        }
                    }
                    self.maybe_name_session();
                    self.ask_stats();
                    self.refresh_status();
                } else if role_of(record.get("message")) == Some("user") {
                    if let Some(message) = record.get("message") {
                        self.chat.append_user_record(message);
                    }
                }
            }
            "tool_execution_start" => {
                let id = str_of(record, "toolCallId");
                let name = str_of(record, "tool");
                let args = record.get("args").cloned().unwrap_or(Value::Null);
                self.state.borrow_mut().phase = TurnPhase::Tools;
                self.refresh_turn();
                self.refresh_status();
                self.refresh_sidebar();
                if is_subagent_tool(&name) {
                    self.rail.subagent_start(
                        &id,
                        &subagent_label(&name, &args),
                        &subagent_detail(&args),
                    );
                } else {
                    self.rail.activity_start(&id, &name);
                }
                self.chat.tool_start(&id, &name, &args);
            }
            "tool_execution_update" => {
                let id = str_of(record, "toolCallId");
                if let Some(partial) = record.get("partialResult") {
                    self.chat.tool_update(&id, partial);
                    if is_subagent_tool(&str_of(record, "tool")) {
                        self.rail
                            .subagent_update(&id, &chat::text_of_result(partial));
                    } else {
                        self.rail
                            .activity_update(&id, &chat::text_of_result(partial));
                    }
                }
            }
            "tool_execution_end" => {
                let id = str_of(record, "toolCallId");
                let is_error = record
                    .get("isError")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                let duration = record.get("duration").and_then(Value::as_f64);
                let result = record.get("result").cloned().unwrap_or(Value::Null);
                self.chat.tool_end(&id, &result, is_error, duration);
                if is_subagent_tool(&str_of(record, "tool")) {
                    self.rail.subagent_end(&id, is_error);
                } else {
                    self.rail.activity_end(&id, is_error, &chat::text_of_result(&result));
                }
                self.refresh_status();
            }
            "bash_execution_update" => {
                let id = str_of(record, "id");
                if let Some(delta) = record.get("delta").and_then(Value::as_str) {
                    self.chat.tool_append(&id, delta);
                }
            }
            "extension_ui_request" => self.on_extension_ui(record),
            "session_info_changed" => {
                let name = record
                    .get("name")
                    .and_then(Value::as_str)
                    .filter(|name| !name.trim().is_empty())
                    .map(str::to_string);
                {
                    let mut state = self.state.borrow_mut();
                    state.session_name = name.clone();
                    if name.is_some() {
                        state.auto_named = true;
                    }
                    let active = state.active_session;
                    if let Some(index) = active {
                        if let Some(session) = state.sessions.get_mut(index) {
                            session.name = name;
                        }
                    }
                }
                self.refresh_header();
                self.refresh_sidebar();
            }
            "thinking_level_changed" => {
                if let Some(level) = record.get("level").and_then(Value::as_str) {
                    self.state.borrow_mut().thinking_level = level.to_string();
                    self.refresh_header();
                    self.refresh_status();
                }
            }
            "auto_retry_start" => {
                let attempt = record.get("attempt").and_then(Value::as_u64).unwrap_or(0);
                let max = record
                    .get("maxExpects")
                    .and_then(Value::as_u64)
                    .unwrap_or(0);
                let error = record
                    .get("ErrorMessage")
                    .and_then(Value::as_str)
                    .unwrap_or("");
                self.chat
                    .note(&format!("retry {attempt}/{max} — {error}"), false);
            }
            "compaction_start" => self.chat.note("compacting context...", false),
            "compaction_end" => self.on_compaction_end(record),
            "extension_error" => {
                let error = record
                    .get("error")
                    .and_then(Value::as_str)
                    .unwrap_or("Extension error");
                self.show_error_banner(error);
                self.chat.note(error, true);
            }
            "response" => self.on_response(record),
            _ => {}
        }
    }

    fn on_compaction_end(&self, record: &Value) {
        let aborted = record
            .get("aborted")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        if aborted {
            self.chat.note("compaction aborted", false);
            return;
        }
        if let Some(result) = record.get("result") {
            let reason = record
                .get("reason")
                .and_then(Value::as_str)
                .unwrap_or("");
            self.chat.note(&compaction_result_note(reason, result), false);
            if let Some(after) = result
                .get("estimatedTokensAfter")
                .and_then(Value::as_u64)
            {
                let mut state = self.state.borrow_mut();
                state.context_tokens = Some(after);
                state.context_percent = state
                    .context_window
                    .filter(|window| *window > 0)
                    .map(|window| after as f64 * 100.0 / window as f64);
            }
            self.refresh_status();
            return;
        }
        if let Some(error) = record.get("errorMessage").and_then(Value::as_str) {
            self.show_error_banner(error);
            self.chat.note(error, true);
        }
    }

    fn on_extension_ui(self: &Rc<Self>, record: &Value) {
        let Some(request) = extension_ui::Request::parse(record) else {
            return;
        };

        if request.method.is_dialog() {
            let me = self.clone();
            let id = request.id.clone();
            dialog::show(&self.window, &request, move |response| {
                me.send(extension_ui::response_json(&id, &response));
            });
            return;
        }

        match request.method {
            extension_ui::Method::Notify => self.toast_notify(&request),
            extension_ui::Method::SetStatus => {
                self.ext_status
                    .borrow_mut()
                    .set(&request.status_key, request.status_text.as_deref());
                self.refresh_ext_status();
            }
            extension_ui::Method::SetWidget => {
                if request.widget_key == "plan-all" {
                    let lines = request.widget_lines.clone().unwrap_or_default();
                    self.rail.set_plan(&lines);
                    self.ext_widgets.borrow_mut().set(
                        "plan-all",
                        None,
                        request.placement,
                    );
                    self.refresh_ext_widget();
                    return;
                }
                self.ext_widgets.borrow_mut().set(
                    &request.widget_key,
                    request.widget_lines.clone(),
                    request.placement,
                );
                self.refresh_ext_widget();
            }
            extension_ui::Method::SetTitle => {
                let title = extension_ui::strip_ansi(&request.title);
                let title = title.trim();
                let full = if title.is_empty() {
                    "pi".to_string()
                } else if title.to_lowercase().starts_with("pi") {
                    title.to_string()
                } else {
                    format!("pi — {title}")
                };
                *self.title_override.borrow_mut() = Some(full.clone());
                self.window.set_title(Some(&full));
            }
            extension_ui::Method::SetEditorText => {
                self.composer.set_text(&request.text);
                self.composer.focus();
            }
            _ => {}
        }
    }

    fn toast_notify(&self, request: &extension_ui::Request) {
        let message = extension_ui::strip_ansi(&request.message);
        if message.trim().is_empty() {
            return;
        }
        let seconds = match request.notify {
            extension_ui::NotifyType::Info => 3,
            extension_ui::NotifyType::Warning => 5,
            extension_ui::NotifyType::Error => 6,
        };
        self.show_toast(&message, request.notify.class(), seconds);
    }

    fn refresh_ext_status(&self) {
        let text = self.ext_status.borrow().text();
        self.status.set_extension_status(text.as_deref());
    }

    fn refresh_ext_widget(&self) {
        let above = self.ext_widgets.borrow().render(Placement::Above);
        let below = self.ext_widgets.borrow().render(Placement::Below);
        self.composer
            .set_extension_widget(Placement::Above, above.as_deref());
        self.composer
            .set_extension_widget(Placement::Below, below.as_deref());
    }

    fn on_assistant_update(self: &Rc<Self>, update: &Value) {
        let kind = update.get("type").and_then(Value::as_str).unwrap_or("");
        match kind {
            "text_delta" => {
                let index = update
                    .get("contentIndices")
                    .and_then(Value::as_i64)
                    .unwrap_or(0);
                if let Some(delta) = update.get("delta").and_then(Value::as_str) {
                    self.chat.assistant_delta(index, delta);
                    let mut state = self.state.borrow_mut();
                    state.streamed_chars = state.streamed_chars.saturating_add(delta.len() as u64);
                    if state.phase != TurnPhase::Writing {
                        state.phase = TurnPhase::Writing;
                        drop(state);
                        self.refresh_turn();
                        self.refresh_sidebar();
                    }
                }
            }
            "text_end" => {}
            "thinking_start" => {
                let level = self.state.borrow().thinking_level.clone();
                self.chat.thinking_start(&level);
                {
                    let mut state = self.state.borrow_mut();
                    state.phase = TurnPhase::Thinking;
                }
                self.refresh_turn();
                self.refresh_sidebar();
            }
            "thinking_delta" => {
                if let Some(delta) = update.get("delta").and_then(Value::as_str) {
                    self.chat.thinking_delta(delta);
                }
            }
            "thinking_end" => {
                let content = update.get("content").and_then(Value::as_str).unwrap_or("");
                self.chat.thinking_end(content);
            }
            "error" => {
                let error = update.get("error").and_then(Value::as_str).unwrap_or("error");
                self.show_error_banner(error);
                self.chat.note(error, true);
                self.state.borrow_mut().phase = TurnPhase::Error;
                self.refresh_turn();
            }
            _ => {}
        }
    }

    fn on_response(self: &Rc<Self>, record: &Value) {
        let command = record.get("command").and_then(Value::as_str).unwrap_or("");
        let success = record
            .get("success")
            .and_then(Value::as_bool)
            .unwrap_or(true);

        if self
            .pending_remote
            .borrow()
            .as_ref()
            .is_some_and(|pending| {
                record.get("id").and_then(Value::as_str) == Some(pending.step_id.as_str())
            })
        {
            self.on_remote_step(record);
            return;
        }

        if !success {
            // A failed compaction is reported by compaction_end, which carries
            // the same message and avoids a duplicate note here.
            if command == "compact" {
                return;
            }
            let error = record
                .get("error")
                .and_then(Value::as_str)
                .unwrap_or("Unknown error");
            self.show_error_banner(&format!("{command}: {error}"));
            self.chat.note(&format!("{command}: {error}"), true);
            if command == "prompt" {
                let (elapsed, tokens) = {
                    let mut state = self.state.borrow_mut();
                    state.running = false;
                    state.phase = TurnPhase::Error;
                    state.run_started = None;
                    (state.elapsed(), state.turn_tokens)
                };
                self.composer.set_running(false);
                self.chat
                    .turn_finish("ERROR", &format_elapsed(elapsed), tokens);
            }
            if command == "bash" {
                let failed = json!({"output": error, "exitCode": 1});
                self.finish_bash(Some(&failed));
            }
            if command == "steer" || command == "follow_up" {
                let pending = {
                    let mut state = self.state.borrow_mut();
                    state.pending_messages = state.pending_messages.saturating_sub(1);
                    state.pending_messages
                };
                self.composer.set_queue(pending);
            }
            if command == "new_session" {
                self.new_session_pending.set(false);
            }
            if command == "switch_session" {
                self.pending_switch.set(None);
            }
            if command == "export_html" {
                self.status
                    .flash_notice(&format!("export failed: {error}"));
            }
            return;
        }

        if command == "prompt"
            && record
                .get("data")
                .and_then(|data| data.get("disposition"))
                .and_then(Value::as_str)
                == Some("handled")
        {
            let (elapsed, tokens) = {
                let mut state = self.state.borrow_mut();
                state.running = false;
                state.phase = TurnPhase::Done;
                state.run_started = None;
                (state.elapsed(), state.turn_tokens)
            };
            self.composer.set_running(false);
            self.chat
                .turn_finish("DONE", &format_elapsed(elapsed), tokens);
            self.refresh_status();
            return;
        }

        if command == "set_session_name" {
            self.status.flash_notice("Renamed session");
            self.ask_state();
            return;
        }

        let Some(data) = record.get("data") else {
            return;
        };

        match command {
            "get_state" => {
                let mut missing_model = false;
                {
                    let mut state = self.state.borrow_mut();
                    if let Some(id) = data
                        .get("sessionId")
                        .or_else(|| data.get("Session"))
                        .and_then(Value::as_str)
                    {
                        state.session_id = id.to_string();
                        let active = self.active_key.borrow().clone();
                        *active.borrow_mut() = id.to_string();
                        self.config
                            .borrow_mut()
                            .set_launch_session_id(Some(id.to_string()));
                    }
                    state.session_name = data
                        .get("sessionName")
                        .and_then(Value::as_str)
                        .filter(|name| !name.trim().is_empty())
                        .map(str::to_string);
                    if let Some(file) = data
                        .get("sessionFile")
                        .or_else(|| data.get("Session"))
                        .and_then(Value::as_str)
                    {
                        state.session_file = Some(file.to_string());
                    }
                    if let Some(model) = data.get("model") {
                        apply_model(&mut state, model);
                    }
                    if data.get("model").is_none()
                        || data.get("model").is_some_and(Value::is_null)
                    {
                        state.model = crate::state::DEFAULT_MODEL.to_string();
                        state.model_id.clear();
                        state.model_provider.clear();
                        if !state.missing_model_noted {
                            state.missing_model_noted = true;
                            missing_model = true;
                        }
                    }
                    if let Some(level) = data.get("thinkingLevel").and_then(Value::as_str) {
                        state.thinking_level = level.to_string();
                    }
                    if let Some(pending) = pending_count(data) {
                        state.pending_messages = pending;
                    }
                    state.state_synced = true;
                    if let Some(streaming) = data
                        .get("isStreaming")
                        .or_else(|| data.get("IsStreaming"))
                        .and_then(Value::as_bool)
                    {
                        state.running = streaming;
                        if streaming && state.phase == TurnPhase::Sent {
                            state.phase = TurnPhase::Thinking;
                        }
                    }
                    state.resolve_active_session();
                    if let (Some(index), Some(name)) =
                        (state.active_session, state.session_name.clone())
                    {
                        if let Some(session) = state.sessions.get_mut(index) {
                            session.name = Some(name);
                        }
                    }
                }
                self.upsert_current_session(false);
                if missing_model {
                    self.chat.note(
                        "no model key — rotate `pi auth login mistral`",
                        true,
                    );
                }
                if self.announce_clone.get() {
                    let id = self.state.borrow().session_id.clone();
                    self.announce_clone.set(false);
                    self.status.flash_notice(&format!("duplicate session · {id}"));
                    self.fetch_history();
                }
                let streaming = self.state.borrow().running;
                let pending = self.state.borrow().pending_messages;
                self.composer.set_running(streaming);
                self.composer.set_queue(pending);
                if let Some(kind) = self.pending_query.borrow_mut().take() {
                    match kind {
                        PendingQuery::Steer => self.chat.note(
                            &format!("steer · {pending} message(s) queued"),
                            false,
                        ),
                        PendingQuery::Queue => {
                            self.composer.set_queue(pending);
                            self.chat
                                .note(&format!("queue · {pending} pending"), false);
                        }
                    }
                }
                self.refresh_header();
                self.refresh_sidebar();
                self.refresh_status();
            }
            "get_session_stats" => {
                {
                    let mut state = self.state.borrow_mut();
                    if let Some(tokens) = data.get("tokens") {
                        if let Some(input) = tokens.get("input").and_then(Value::as_u64) {
                            state.tokens_in = input;
                        }
                        if let Some(output) = tokens.get("output").and_then(Value::as_u64) {
                            state.tokens_out = output;
                        }
                        state.cache_read = tokens.get("cacheRead").and_then(Value::as_u64).unwrap_or(0);
                        state.cache_write = tokens.get("cacheWrite").and_then(Value::as_u64).unwrap_or(0);
                    }
                    if let Some(cost) = data.get("cost").and_then(Value::as_f64) {
                        state.cost = cost;
                    }
                    if let Some(ctx) = data
                        .get("contextUsage")
                        .or_else(|| data.get("context"))
                    {
                        state.context_percent = ctx.get("percent").and_then(Value::as_f64);
                        state.context_tokens = ctx.get("tokens").and_then(Value::as_u64);
                        state.context_window = ctx
                            .get("contextWindow")
                            .or_else(|| ctx.get("contextWindows"))
                            .and_then(Value::as_u64);
                    }
                    if let Some(file) = data
                        .get("sessionFile")
                        .or_else(|| data.get("Session"))
                        .and_then(Value::as_str)
                    {
                        state.session_file = Some(file.to_string());
                    }
                    if let Some(id) = data.get("sessionId").and_then(Value::as_str) {
                        state.session_id = id.to_string();
                    }
                    state.resolve_active_session();
                }
                self.refresh_status();
                self.refresh_sidebar();
            }
            "get_commands" => {
                let mut items = parse_commands(data);
                if !items.iter().any(|item| item.name == "compact") {
                    items.push(CommandItem {
                        name: "compact".to_string(),
                        description: "Summarize old context and report tokens saved".to_string(),
                        source: "local".to_string(),
                    });
                }
                self.state.borrow_mut().commands = items.clone();
                self.composer.set_commands(items);
            }
            "get_available_models" => {
                let items = parse_models(data);
                self.state.borrow_mut().models = items;
                self.refresh_header();
                self.refresh_sidebar();
            }
            "get_available_thinking_levels" => {
                let levels = data
                    .get("levels")
                    .and_then(Value::as_array)
                    .map(|levels| {
                        levels
                            .iter()
                            .filter_map(Value::as_str)
                            .map(str::to_string)
                            .collect()
                    })
                    .unwrap_or_default();
                self.apply_thinking_levels(levels);
            }
            "set_thinking_level" => {
                let level = data
                    .get("level")
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .unwrap_or_else(|| self.state.borrow().thinking_level.clone());
                self.state.borrow_mut().thinking_level = level;
                self.refresh_header();
                self.refresh_status();
            }
            "cycle_thinking_level" => {
                if let Some(level) = data.get("level").and_then(Value::as_str) {
                    self.state.borrow_mut().thinking_level = level.to_string();
                    self.refresh_header();
                    self.refresh_status();
                }
            }
            "set_model" => {
                let model = if data.get("id").is_some() {
                    data
                } else {
                    data.get("model").unwrap_or(data)
                };
                {
                    let mut state = self.state.borrow_mut();
                    apply_model(&mut state, model);
                    let label = format!("{}/{}", state.model_provider, state.model_id);
                    let active = state.active_session;
                    if let Some(index) = active {
                        if let Some(session) = state.sessions.get_mut(index) {
                            session.model = Some(label);
                        }
                    }
                }
                self.refresh_header();
                self.refresh_sidebar();
                self.ask_thinking_levels();
            }
            "cycle_model" => {
                let Some(model) = cycle_model_target(data) else {
                    self.chat.note("no other model to cycle", false);
                    return;
                };
                {
                    let mut state = self.state.borrow_mut();
                    apply_model(&mut state, model);
                    let label = format!("{}/{}", state.model_provider, state.model_id);
                    let active = state.active_session;
                    if let Some(index) = active {
                        if let Some(session) = state.sessions.get_mut(index) {
                            session.model = Some(label);
                        }
                    }
                }
                self.refresh_header();
                self.refresh_sidebar();
                self.ask_thinking_levels();
            }
            "get_messages" => {
                let messages = data
                    .get("messages")
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default();
                self.state.borrow_mut().messages_loaded = true;
                self.chat.load_messages(&messages);
                self.ask_fork_messages();
            }
            "get_fork_messages" => {
                let points = data
                    .get("messages")
                    .and_then(Value::as_array)
                    .map(|messages| {
                        messages
                            .iter()
                            .filter_map(|message| {
                                let entry = message.get("enterId").and_then(Value::as_str)?;
                                let text = message.get("text").and_then(Value::as_str)?;
                                Some((entry.to_string(), text.to_string()))
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                *self.fork_points.borrow_mut() = points;
            }
            "get_tree" => {
                let view = crate::tree::parse(data);
                self.tree.set_view(&view);
                self.refresh_sidebar();
            }
            "get_last_assistant_text" => {
                let text = data.get("text").and_then(Value::as_str);
                match text.filter(|text| !text.trim().is_empty()) {
                    Some(text) => {
                        if self.copy_to_clipboard(text) {
                            self.status.flash_notice("copied text");
                        } else {
                            self.status.flash_notice("clipboard not available");
                        }
                    }
                    None => self.status.flash_notice("nothing to copy yet"),
                }
            }
            "fork" => {
                let cancelled = data
                    .get("cancelled")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                if cancelled {
                    self.status.flash_notice("fork canceled");
                    return;
                }
                let text = data
                    .get("text")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                self.state.borrow_mut().messages_loaded = false;
                self.ask_state();
                self.ask_stats();
                self.send(json!({"id": self.id(), "type": "get_messages"}));
                self.fetch_history();
                if !text.trim().is_empty() {
                    self.composer.set_text(&text);
                }
                if self.stack.visible_child_name().as_deref() == Some("tree") {
                    self.select_nav(2);
                }
                self.composer.focus();
            }
            "clone" => {
                let cancelled = data
                    .get("cancelled")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                if cancelled {
                    self.announce_clone.set(false);
                    self.status.flash_notice("clone canceled");
                    return;
                }
                self.fetch_history();
                self.announce_clone.set(true);
                self.ask_state();
                self.ask_stats();
            }
            "export_html" => {
                let path = data
                    .get("path")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                self.notice_export(&path);
            }
            "switch_session" => {
                let cancelled = data
                    .get("cancelled")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                if cancelled {
                    self.pending_switch.set(None);
                    self.chat.note("Session switch canceled", true);
                    return;
                }
                let index = self.pending_switch.take();
                {
                    let current = self.state.borrow().session_id.clone();
                    let ready = self
                        .pending_draft_restore
                        .borrow()
                        .as_deref()
                        .is_some_and(|target| target == current);
                    if ready {
                        self.pending_draft_restore.borrow_mut().take();
                        self.restore_draft(&current);
                    }
                }
                *self.title_override.borrow_mut() = None;
                {
                    let mut state = self.state.borrow_mut();
                    state.active_session = index;
                    state.reset_session_counters();
                    state.session_name = None;
                    if let Some(index) = index {
                        let selected = state.sessions.get(index).map(|session| {
                            (session.session_id.clone(), session.preview.clone())
                        });
                        if let Some((session_id, preview)) = selected {
                            state.session_id = session_id;
                            state.first_prompt = preview;
                        }
                    }
                }
                self.refresh_header();
                self.refresh_sidebar();
                self.refresh_status();
                self.ask_state();
                self.ask_stats();
                self.ask_thinking_levels();
                self.send(json!({"id": self.id(), "type": "get_messages"}));
                self.ask_fork_messages();
            }
            "new_session" => {
                self.new_session_pending.set(false);
                let cancelled = data
                    .get("cancelled")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                if cancelled {
                    *self.pending_delete.borrow_mut() = None;
                    self.chat.note("new session canceled by extension", true);
                    return;
                }
                self.state.borrow_mut().reset_session_counters();
                self.ask_state();
                self.ask_stats();
                if let Some(pending) = self.pending_delete.borrow_mut().take() {
                    let path = pending.path.to_string_lossy().to_string();
                    self.trash_session(&path);
                    self.state
                        .borrow_mut()
                        .sessions
                        .retain(|session| session.path != path);
                    self.refresh_sidebar();
                    self.fetch_history();
                }
            }
            "clear_queue" => {
                let restored = restore_queue_texts(data);
                self.state.borrow_mut().pending_messages = 0;
                self.composer.set_queue(0);
                if !restored.is_empty() {
                    let current = self.composer.text();
                    let mut text = restored.join("\n");
                    if !current.trim().is_empty() {
                        text.push('\n');
                        text.push_str(&current);
                    }
                    self.composer.set_text(&text);
                    self.composer.focus();
                }
            }
            "abort" => {
                {
                    let mut state = self.state.borrow_mut();
                    if let Some(start) = state.run_started.take() {
                        state.run_elapsed = start.elapsed();
                    }
                    state.running = false;
                    state.pending_messages = 0;
                }
                self.composer.set_running(false);
                self.composer.set_queue(0);
                self.refresh_status();
            }
            "bash" => self.finish_bash(Some(data)),
            "prompt" | "steer" | "follow_up" => {
                self.ask_state();
            }
            _ => {}
        }
    }

    fn maybe_name_session(self: &Rc<Self>) {
        let (opener, derived, needs_set) = {
            let mut state = self.state.borrow_mut();
            if !state.state_synced || state.title_attempts >= 2 {
                return;
            }
            if state.title_requested {
                return;
            }
            let Some(prompt) = state.first_prompt.clone() else {
                return;
            };
            let derived = crate::title::derive(&prompt);
            if derived.trim().is_empty() {
                return;
            }
            let current = state
                .session_name
                .clone()
                .filter(|name| !name.trim().is_empty());
            let auto_like = current
                .as_deref()
                .map_or(true, |name| crate::title::auto_like(name, &derived));
            if !auto_like {
                return;
            }
            state.title_requested = true;
            state.title_attempts = state.title_attempts.saturating_add(1);
            state.auto_named = true;
            let needs_set = current.as_deref() != Some(derived.as_str());
            (prompt, derived, needs_set)
        };

        if needs_set {
            self.send(json!({
                "id": self.id(),
                "type": "set_session_name",
                "name": derived.clone(),
            }));
            let mut state = self.state.borrow_mut();
            state.session_name = Some(derived.clone());
            let active = state.active_session;
            if let Some(index) = active {
                if let Some(session) = state.sessions.get_mut(index) {
                    session.name = Some(derived.clone());
                }
            }
            drop(state);
            self.refresh_header();
            self.refresh_sidebar();
        }

        self.request_ai_title(opener, derived);
    }

    fn request_ai_title(self: &Rc<Self>, opener: String, derived: String) {
        let session_id = self.state.borrow().session_id.clone();
        let config = self.config.borrow().clone();
        if config.target != crate::config::Target::Local {
            return;
        }
        let (tx, rx) = async_channel::bounded::<Option<String>>(1);
        let opener_for_thread = opener.clone();
        std::thread::Builder::new()
            .name("pi-title".into())
            .spawn(move || {
                let _ =
                    tx.send_blocking(crate::title::spawn_blocking(&config, &opener_for_thread));
            })
            .ok();

        let me = self.clone();
        glib::spawn_future_local(async move {
            let Ok(Some(raw)) = rx.recv().await else {
                me.allow_title_retry(&session_id);
                return;
            };
            let Some(name) = crate::title::clean(&raw, &opener) else {
                me.allow_title_retry(&session_id);
                return;
            };
            if me.state.borrow().session_id == session_id {
                me.apply_ai_title(&name, &derived);
            }
        });
    }

    fn allow_title_retry(&self, session_id: &str) {
        let mut state = self.state.borrow_mut();
        if state.session_id == session_id && state.title_attempts < 2 {
            state.title_requested = false;
        }
    }

    fn apply_ai_title(self: &Rc<Self>, name: &str, derived: &str) {
        {
            let state = self.state.borrow();
            let Some(current) = state.session_name.as_deref() else {
                return;
            };
            if !crate::title::auto_like(current, derived) {
                return;
            }
        }
        self.send(json!({
            "id": self.id(),
            "type": "set_session_name",
            "name": name,
        }));
        {
            let mut state = self.state.borrow_mut();
            state.session_name = Some(name.to_string());
            let active = state.active_session;
            if let Some(index) = active {
                if let Some(session) = state.sessions.get_mut(index) {
                    session.name = Some(name.to_string());
                }
            }
        }
        self.refresh_header();
        self.refresh_sidebar();
        self.status.flash_notice(&format!("Session added: {name}"));
    }

    fn ask_state(self: &Rc<Self>) {
        self.send(json!({"id": self.id(), "type": "get_state"}));
    }

    fn ask_stats(&self) {
        self.send(json!({"id": self.id(), "type": "get_session_stats"}));
    }

    fn ask_commands(&self) {
        self.send(json!({"id": self.id(), "type": "get_commands"}));
    }

    fn ask_models(&self) {
        self.send(json!({"id": self.id(), "type": "get_available_models"}));
    }


    fn tick(&self) {
        let (time, rate, phase, elapsed, tokens, running) = {
            let state = self.state.borrow();
            (
                format_duration(state.session_elapsed()),
                state.tokens_per_sec(),
                state.phase.label().to_string(),
                state.elapsed(),
                state.turn_tokens,
                state.running,
            )
        };
        self.status.time.set_text(&time);
        self.status.rate.set_text(&format!("{rate:.0} tok/s"));
        if running {
            self.header
                .set_busy(true, &format!("{phase} · {}", format_elapsed(elapsed)));
            self.chat
                .turn_update(&phase, &format_elapsed(elapsed), tokens);
            let activity = {
                let state = self.state.borrow();
                self.activity_snapshot(&state)
            };
            self.sidebar.tick_activity(&activity);
        }
    }
    fn activity_snapshot(&self, state: &AppState) -> HashMap<String, SessionActivity> {
        let mut activity = self.activity.borrow().clone();
        let mut keys: Vec<String> = vec![state.session_id.clone()];
        if let Some(file) = state.session_file.as_ref() {
            keys.push(file.clone());
        }
        if let Some(index) = state.active_session {
            if let Some(session) = state.sessions.get(index) {
                keys.push(session.session_id.clone());
                keys.push(session.path.clone());
            }
        }
        for session in &state.sessions {
            if state.session_file.as_deref() == Some(session.path.as_str())
                || state.session_id == session.session_id
                || state.session_id == session.path
            {
                keys.push(session.session_id.clone());
                keys.push(session.path.clone());
            }
        }
        keys.retain(|key| !key.trim().is_empty());
        keys.sort();
        keys.dedup();
        if state.running {
            let entry = SessionActivity {
                running: true,
                phase: state.phase.label().to_string(),
                started: state.run_started,
                unread: false,
                waiting: false,
            };
            for key in keys {
                activity.insert(key, entry.clone());
            }
        } else {
            for key in keys {
                activity.remove(&key);
            }
        }
        activity
    }

    fn refresh_turn(&self) {
        let (phase, elapsed, tokens, running) = {
            let state = self.state.borrow();
            (
                state.phase.label().to_string(),
                state.elapsed(),
                state.turn_tokens,
                state.running,
            )
        };
        if running {
            self.chat
                .turn_update(&phase, &format_elapsed(elapsed), tokens);
        }
    }

    fn refresh_header(&self) {
        let state = self.state.borrow();
        let name = state.display_name();
        self.header.set_chat_title(&name);
        let activity = self.activity_snapshot(&state);
        self.header.set_sessions(
            &state.sessions,
            state.active_session,
            &self.closed_tabs.borrow(),
            &activity,
        );
        self.header.model.set_text(&state.model);
        self.header.set_model_provider(&state.model_provider);
        let title = match self.title_override.borrow().as_deref() {
            Some(title) => title.to_string(),
            None => format!("pi — {name}"),
        };
        self.window.set_title(Some(&title));
        self.header
            .set_menu(&state.models, &state.model_provider, &state.model_id);
        self.composer
            .set_thinking_levels(&state.thinking_levels, &state.thinking_level);
    }

    fn refresh_sidebar(&self) {
        let state = self.state.borrow();
        self.sidebar.set_counts(
            state.artifacts,
            state.jobs,
            state.sessions.len(),
            self.tree.count(),
            state.plugins.len(),
        );
        self.sidebar.set_current_session(&state.session_id);
        let activity = self.activity_snapshot(&state);
        self.sidebar
            .set_sessions(&state.sessions, state.active_session, &state.models, &activity);
    }

    fn refresh_status(&self) {
        let state = self.state.borrow();
        self.status.set_target(&state.transport_label);
        self.status.tokens.set_text(&format!(
            "{} / {}",
            format_tokens(state.tokens_in),
            format_tokens(state.tokens_out)
        ));
        self.status.cost.set_text(&format_cost(state.cost));
        self.status.set_context(state.context_percent);
        self.status
            .rate
            .set_text(&format!("{:.0} tok/s", state.tokens_per_sec()));
        self.status
            .time
            .set_text(&format_duration(state.session_elapsed()));
        self.status.perm.set_text(state.permission.label());
        self.status
            .perm_button
            .set_tooltip_text(Some(&format!("{} · click to open settings", state.permission.hint())));
        self.status.set_connection(&state.connection, &state.conn_detail);
        self.rail.set_stats(
            &state.model,
            &state.display_name(),
            state.tokens_in,
            state.tokens_out,
            state.cost,
            state.context_percent,
            state.cache_read,
            state.cache_write,
            state.context_tokens,
            state.context_window,
            &state.workspace,
        );
        self.rail.set_queue(state.pending_messages);
        if state.running {
            self.header.set_busy(
                true,
                &format!("{} · {}", state.phase.label(), format_elapsed(state.elapsed())),
            );
        } else {
            self.header.set_busy(false, "");
        }
        drop(state);
        self.refresh_ext_status();
    }
}

const DEMO_DIFF_PATCH: &str = "\
--- /tmp/user/projects/snake_game/index.html
+++ /tmp/user/projects/snake_game/index.html
@@ -2,16 +2,25 @@
 <html lang=\"en\">
 <head>
     <meta charset=\"UTF-8\">
     <meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\">
-    <title>Snake Game</title>
+    <title>Neon Snake</title>
     <link rel=\"stylesheet\" href=\"style.css\">
 </head>
 <body>
     <div class=\"game-container\">
-        <h1>Snake Game</h1>
-        <div id=\"score\">Score: 0</div>
-        <canvas id=\"snakeGame\" width=\"400\" height=\"400\"></canvas>
+        <div class=\"hud\">
+            <div id=\"score\">Score: 0</div>
+            <div id=\"highScore\">High Score: 0</div>
+        </div>
+        <div class=\"canvas-wrapper\">
+            <canvas id=\"snakeGame\" width=\"400\" height=\"400\"></canvas>
+            <div id=\"overlay\" class=\"overlay hidden\">
+                <h2 id=\"overlayText\">PAUSED</h2>
+                <p id=\"overlaySubtext\">Press SPACE to Resume</p>
+            </div>
+        </div>
+        <div class=\"controls-hint\">Use Arrow Keys to Move • Space to Pause</div>
     </div>
     <script src=\"script.js\"></script>
 </body>
 </html>";

fn demo_diff_messages() -> Vec<Value> {
    vec![
        json!({
            "role": "user",
            "content": [{"type": "text", "text": "make the snake look neon arcade"}],
        }),
        json!({
            "role": "assistant",
            "content": [{"type": "text", "text": "I changed the title and the HUD from `index.html`."}],
        }),
        json!({
            "role": "toolResult",
            "toolCallId": "call_demo_edit",
            "tool": "edit",
            "isError": false,
            "content": [{"type": "text", "text": "Successfully replaced 1 block in /tmp/user/projects/snake_game/index.html."}],
            "details": {
                "patch": DEMO_DIFF_PATCH,
                "FirstChangedLine": 6
            },
        }),
        json!({
            "role": "toolResult",
            "toolCallId": "call_demo_error",
            "tool": "edit",
            "isError": true,
            "content": [{"type": "text", "text": "Tool call \"edit\" was not executed: the answer hit the output token limit."}],
        }),
    ]
}

fn session_open_in(state: &AppState, index: usize) -> bool {
    let Some(session) = state.sessions.get(index) else {
        return false;
    };
    match state.session_file.as_deref() {
        Some(file) => file == session.path,
        None => state.active_session == Some(index),
    }
}

fn local_command(text: &str) -> Option<(&str, &str)> {
    let trimmed = text.trim();
    let (command, argument) = match trimmed.split_once(char::is_whitespace) {
        Some((command, rest)) => (command, rest.trim()),
        None => (trimmed, ""),
    };
    match command {
        "/loop" | "/steer" | "/queue" | "/copy" | "/compact" => Some((command, argument)),
        _ => None,
    }
}

fn compaction_result_note(reason: &str, result: &Value) -> String {
    let before = result.get("tokensBefore").and_then(Value::as_u64);
    let after = result
        .get("estimatedTokensAfter")
        .and_then(Value::as_u64);
    let prefix = if reason == "manual" {
        "compacted"
    } else {
        "auto-compacted"
    };
    match (before, after) {
        (Some(before), Some(after)) => {
            let saved = before.saturating_sub(after);
            let percent = if before > 0 {
                (saved as f64 * 100.0 / before as f64).round() as u64
            } else {
                0
            };
            format!(
                "{prefix} · {} → {} tokens (est.) · {} saved ({percent}%)",
                format_tokens(before),
                format_tokens(after),
                format_tokens(saved)
            )
        }
        (Some(before), None) => format!("{prefix} · {} tokens summarized", format_tokens(before)),
        _ => format!("{prefix} · context summarized"),
    }
}

fn submit_command(id: &str, kind: SubmitKind, running: bool, text: &str) -> Value {
    if !running {
        json!({"id": id, "type": "prompt", "message": text})
    } else if kind == SubmitKind::AltEnter {
        json!({"id": id, "type": "follow_up", "message": text})
    } else {
        json!({"id": id, "type": "steer", "message": text})
    }
}

fn bash_command(id: &str, command: &str, exclude_from_context: bool) -> Value {
    json!({
        "id": id,
        "type": "bash",
        "command": command,
        "excludedFromContext": exclude_from_context,
    })
}

fn restore_queue_texts(data: &Value) -> Vec<String> {
    let mut restored = Vec::new();
    for key in ["steering", "followUp"] {
        if let Some(items) = data.get(key).and_then(Value::as_array) {
            restored.extend(
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .filter(|text| !text.trim().is_empty())
                    .map(str::to_string),
            );
        }
    }
    restored
}

fn pending_count(data: &Value) -> Option<usize> {
    data.get("pendingMessageCount")
        .or_else(|| data.get("HangingMessageCount"))
        .and_then(Value::as_u64)
        .map(|count| count as usize)
}

fn assistant_text(message: &Value) -> String {
    match message.get("content") {
        Some(Value::Array(blocks)) => blocks
            .iter()
            .filter_map(|block| block.get("text").and_then(Value::as_str))
            .collect::<Vec<_>>()
            .join("\n"),
        Some(Value::String(text)) => text.clone(),
        _ => String::new(),
    }
}

fn cycle_model_target(data: &Value) -> Option<&Value> {
    let model = data.get("model").unwrap_or(data);
    model.get("id").and_then(Value::as_str).map(|_| model)
}

fn apply_model(state: &mut AppState, model: &Value) {
    let Some(id) = model.get("id").and_then(Value::as_str) else {
        return;
    };    let name = model
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or(id)
        .to_string();
    let provider = model
        .get("provider")
        .and_then(Value::as_str)
        .unwrap_or(&state.model_provider)
        .to_string();
    state.model_id = id.to_string();
    state.model_provider = provider;
    state.model = name;
}

fn parse_commands(data: &Value) -> Vec<CommandItem> {
    data.get("commands")
        .and_then(Value::as_array)
        .map(|commands| {
            commands
                .iter()
                .filter_map(|command| {
                    let name = command.get("name").and_then(Value::as_str)?.to_string();
                    Some(CommandItem {
                        name,
                        description: command
                            .get("description")
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .to_string(),
                        source: command
                            .get("source")
                            .and_then(Value::as_str)
                            .unwrap_or("extension")
                            .to_string(),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn parse_models(data: &Value) -> Vec<ModelItem> {
    data.get("models")
        .and_then(Value::as_array)
        .map(|models| {
            models
                .iter()
                .filter_map(|model| {
                    let id = model.get("id").and_then(Value::as_str)?.to_string();
                    let provider = model
                        .get("provider")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string();
                    let name = model
                        .get("name")
                        .and_then(Value::as_str)
                        .unwrap_or(&id)
                        .to_string();
                    Some(ModelItem {
                        provider,
                        id,
                        name,
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn with_search_hint(text: &str) -> String {
    format!(
        "[web search active: use the web_search tool when the answer depends on current information or external sources; cite sources]\n\n{text}"
    )
}

fn plugin_count_text(count: usize) -> String {
    if count == 1 {
        "1 package installed".to_string()
    } else {
        format!("{count} packages installed")
    }
}

fn workspace_branch(workspace: &str) -> String {
    let mut dir = crate::config::expand_home(workspace);
    for _ in 0..10 {
        let git = dir.join(".git");
        let head = if git.is_dir() {
            Some(git.join("HEAD"))
        } else {
            std::fs::read_to_string(&git).ok().and_then(|text| {
                text.trim()
                    .strip_prefix("gitdir:")
                    .map(|path| dir.join(path.trim()).join("HEAD"))
            })
        };
        if let Some(head) = head {
            if let Ok(text) = std::fs::read_to_string(head) {
                let text = text.trim();
                if let Some(branch) = text.strip_prefix("ref: refs/heads/") {
                    return branch.to_string();
                }
                return text.chars().take(8).collect();
            }
        }
        if !dir.pop() {
            break;
        }
    }
    String::new()
}

fn is_subagent_tool(name: &str) -> bool {
    matches!(name, "subagent" | "subagents_enable" | "bg_wait" | "bg_status" | "bg_list")
        || name.starts_with("bg_")
}

fn subagent_label(name: &str, args: &Value) -> String {
    let agent = args
        .get("agent")
        .and_then(Value::as_str)
        .unwrap_or(name);
    match args.get("tasks").and_then(Value::as_array).map(Vec::len) {
        Some(count) if count > 1 => format!("{agent} ×{count}"),
        _ => agent.to_string(),
    }
}

fn subagent_detail(args: &Value) -> String {
    if let Some(task) = args.get("task").and_then(Value::as_str) {
        return task.lines().next().unwrap_or("").chars().take(80).collect();
    }
    if let Some(tasks) = args.get("tasks").and_then(Value::as_array) {
        let names: Vec<&str> = tasks
            .iter()
            .filter_map(|task| task.get("agent").and_then(Value::as_str))
            .take(3)
            .collect();
        if !names.is_empty() {
            return names.join(", ");
        }
    }
    String::new()
}

fn role_of(message: Option<&Value>) -> Option<&str> {
    message?.get("role")?.as_str()
}

fn focus_is_text_input(window: &adw::ApplicationWindow) -> bool {
    gtk::prelude::GtkWindowExt::focus(window)
        .map(|widget| {
            widget.is::<gtk::TextView>()
                || widget.is::<gtk::Entry>()
                || widget.is::<gtk::SpinButton>()
                || widget.is::<gtk::SearchEntry>()
        })
        .unwrap_or(false)
}

fn str_of(record: &Value, key: &str) -> String {
    record
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compact_is_a_local_command() {
        assert_eq!(
            local_command("/compact focus on the API"),
            Some(("/compact", "focus on the API"))
        );
    }

    #[test]
    fn compaction_note_reports_tokens_saved() {
        let result = json!({
            "tokensBefore": 150000,
            "estimatedTokensAfter": 32000,
        });
        assert_eq!(
            compaction_result_note("manual", &result),
            "compacted · 150k → 32.0k tokens (est.) · 118k saved (79%)"
        );
    }

    #[test]
    fn compaction_note_distinguishes_automatic_runs() {
        let result = json!({
            "tokensBefore": 20000,
            "estimatedTokensAfter": 15000,
        });
        assert_eq!(
            compaction_result_note("threshold", &result),
            "auto-compacted · 20.0k → 15.0k tokens (est.) · 5.0k saved (25%)"
        );
    }
}


