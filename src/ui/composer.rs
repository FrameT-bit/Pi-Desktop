
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk::prelude::*;

use crate::extension_ui::Placement;
use crate::icons::{self, Tone};
use crate::images;
use crate::mentions::{self, Mention};
use crate::prompt_history::PromptHistory;
use crate::state::CommandItem;

use super::reasoning::{level_label, LevelPicker};
use super::popup::InlineList;
use super::thumb;

pub type ThinkingFn = Box<dyn Fn(String)>;
pub type SearchToggleFn = Box<dyn Fn(bool)>;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SubmitKind {
    Enter,
    AltEnter,
    CtrlShiftEnter,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DropOutcome {
    Insert(String),
    Notice(&'static str),
    Ignore,
}

pub const TEXTURE_DROP_NOTICE: &str =
    "to attach: drag the image file or paste with Ctrl+V";

pub const IMAGE_TYPE_NOTICE: &str =
    "image format not supported (png, jpeg, gif, webp)";

pub const IMAGE_SIZE_NOTICE: &str = "too large image (max 8 MB)";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttachmentKind {
    Image,
    File,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Attachment {
    pub kind: AttachmentKind,
    /// Images: the sniffed/declared image mime. Files: best-effort mime.
    pub mime: String,
    /// Images: base64 payload sent over RPC. Files: empty.
    pub data: String,
    pub name: String,
    pub bytes: usize,
    /// Files only: absolute path the agent can read.
    pub path: Option<String>,
    /// Files only: extracted text, when the file is text-like.
    pub text: Option<String>,
}

impl Attachment {
    pub fn is_image(&self) -> bool {
        self.kind == AttachmentKind::Image
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttachFile {
    Attached,
    Rejected,
    NotImage,
}

pub fn drop_paths(paths: &[String]) -> DropOutcome {
    let text = format_dropped_paths(paths);
    if text.is_empty() {
        DropOutcome::Ignore
    } else {
        DropOutcome::Insert(text)
    }
}

pub fn drop_texture() -> DropOutcome {
    DropOutcome::Notice(TEXTURE_DROP_NOTICE)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ListQuery {
    None,
    Commands(String),
    Skills(Mention),
}

pub fn command_query(text: &str) -> Option<String> {
    if !text.starts_with('/') || text.contains('\n') {
        return None;
    }
    let command = text.trim_start_matches('/');
    if command.chars().any(char::is_whitespace) || command.contains('/') {
        return None;
    }
    Some(command.to_lowercase())
}

pub fn list_query(text: &str, cursor: usize) -> ListQuery {
    if let Some(query) = command_query(text) {
        return ListQuery::Commands(query);
    }
    match mentions::mention_at(text, cursor) {
        Some(mention) => ListQuery::Skills(mention),
        None => ListQuery::None,
    }
}

struct Reentry<'a> {
    flag: &'a Cell<bool>,
}

impl<'a> Reentry<'a> {
    fn enter(flag: &'a Cell<bool>) -> Option<Self> {
        if flag.replace(true) {
            None
        } else {
            Some(Self { flag })
        }
    }
}

impl Drop for Reentry<'_> {
    fn drop(&mut self) {
        self.flag.set(false);
    }
}

pub type SubmitFn = Box<dyn Fn(SubmitKind)>;
pub type StopFn = Box<dyn Fn()>;
pub type CommandFn = Box<dyn Fn(String)>;
pub type NoticeFn = Box<dyn Fn(String)>;
pub type LoopStopFn = Box<dyn Fn()>;

pub fn format_dropped_paths(paths: &[String]) -> String {
    paths
        .iter()
        .map(|path| path.trim())
        .filter(|path| !path.is_empty())
        .map(|path| {
            if path.chars().any(char::is_whitespace) {
                format!("\"{path}\"")
            } else {
                path.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub struct Composer {
    pub root: gtk::Overlay,
    pub text: gtk::TextView,
    pub stop: gtk::Button,
    anchor: gtk::Box,
    placeholder: gtk::Label,
    send: gtk::Button,
    send_icon: gtk::Image,
    attach_strip: gtk::Box,
    attach_scroll: gtk::ScrolledWindow,
    attachments: RefCell<Vec<Attachment>>,
    palette: Rc<InlineList>,
    skills: Rc<InlineList>,
    last_mention: RefCell<Option<Mention>>,
    commands: Rc<RefCell<Vec<CommandItem>>>,
    last_query: RefCell<String>,
    palette_targets: RefCell<Vec<String>>,
    hint: gtk::Label,
    hint_source: RefCell<Option<glib::SourceId>>,
    suppress_palette: Cell<bool>,
    syncing: Cell<bool>,
    queue: gtk::Label,
    attach: gtk::Button,
    chat_think: gtk::Button,
    chat_think_label: gtk::Label,
    chat_search: gtk::ToggleButton,
    chat_tools: gtk::Box,
    chat_mode: Cell<bool>,
    think: gtk::Button,
    think_label: gtk::Label,
    think_popover: gtk::Popover,
    think_body: gtk::Box,
    think_picker: RefCell<Option<Rc<LevelPicker>>>,
    think_levels: RefCell<Vec<String>>,
    think_active: RefCell<Vec<String>>,
    think_index: Cell<usize>,
    on_thinking: Rc<RefCell<Option<ThinkingFn>>>,
    on_search_toggle: Rc<RefCell<Option<SearchToggleFn>>>,
    loop_chip: gtk::Button,
    loop_text: gtk::Label,
    widget_above: gtk::Label,
    widget_below: gtk::Label,
    running: Cell<bool>,
    bash_running: Cell<bool>,
    history: RefCell<PromptHistory>,
    files: RefCell<Vec<String>>,
    history_applying: Cell<bool>,
    on_submit: Rc<RefCell<Option<SubmitFn>>>,
    on_stop: Rc<RefCell<Option<StopFn>>>,
    on_command: Rc<RefCell<Option<CommandFn>>>,
    on_notice: Rc<RefCell<Option<NoticeFn>>>,
    on_loop_stop: Rc<RefCell<Option<LoopStopFn>>>,
}

impl Composer {
    pub fn new() -> Rc<Self> {
        let frame = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        frame.add_css_class("composer-box");

        let text = gtk::TextView::new();
        text.add_css_class("composer-input");
        text.set_wrap_mode(gtk::WrapMode::WordChar);
        text.set_accepts_tab(false);
        text.set_left_margin(0);
        text.set_right_margin(0);
        text.set_top_margin(0);
        text.set_bottom_margin(0);
        text.set_hexpand(true);

        let scroll = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vscrollbar_policy(gtk::PolicyType::Automatic)
            .child(&text)
            .build();
        scroll.add_css_class("composer-scroll");
        scroll.set_propagate_natural_height(true);
        scroll.set_min_content_height(40);
        scroll.set_max_content_height(340);
        scroll.set_hexpand(true);
        scroll.set_overlay_scrolling(false);

        let placeholder = gtk::Label::new(Some("message... (/ commands)"));
        placeholder.add_css_class("composer-placeholder");
        placeholder.set_halign(gtk::Align::Start);
        placeholder.set_valign(gtk::Align::Start);
        placeholder.set_xalign(0.0);
        placeholder.set_can_target(false);

        let overlay = gtk::Overlay::new();
        overlay.set_child(Some(&scroll));
        overlay.add_overlay(&placeholder);
        overlay.set_hexpand(true);

        let send = gtk::Button::new();
        send.add_css_class("composer-send");
        send.add_css_class("idle");
        let send_icon = icons::icon("send", Tone::Muted);
        send.set_child(Some(&send_icon));
        send.set_tooltip_text(Some("Send (Enter)"));
        send.set_valign(gtk::Align::End);
        send.set_sensitive(false);

        let stop = gtk::Button::new();
        stop.add_css_class("composer-stop");
        let stop_icon = icons::icon("stop", Tone::Accent);
        stop.set_child(Some(&stop_icon));
        stop.set_tooltip_text(Some("Stop the running turn (Esc)"));
        stop.set_valign(gtk::Align::End);
        stop.set_visible(false);

        let actions = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        actions.set_valign(gtk::Align::End);

        let loop_text = gtk::Label::new(None);
        loop_text.add_css_class("composer-loop-text");
        let loop_chip = gtk::Button::new();
        loop_chip.add_css_class("composer-loop");
        loop_chip.set_child(Some(&loop_text));
        loop_chip.set_valign(gtk::Align::Center);
        loop_chip.set_visible(false);
        loop_chip.set_tooltip_text(Some("active loop · click to stop"));

        let queue = gtk::Label::new(None);
        queue.add_css_class("composer-queue");
        queue.set_valign(gtk::Align::Center);
        queue.set_visible(false);
        let attach = gtk::Button::new();
        attach.add_css_class("composer-tool");
        attach.set_child(Some(&icons::icon("plus", Tone::Muted)));
        attach.set_tooltip_text(Some("attach image or file"));
        attach.set_valign(gtk::Align::End);

        let think_label = gtk::Label::new(None);
        think_label.add_css_class("composer-think-label");
        let think_inner = gtk::Box::new(gtk::Orientation::Horizontal, 5);
        think_inner.append(&icons::icon("sliders", Tone::Muted));
        think_inner.append(&think_label);
        let think = gtk::Button::new();
        think.add_css_class("composer-tool");
        think.add_css_class("composer-think");
        think.set_child(Some(&think_inner));
        think.set_valign(gtk::Align::End);
        think.set_visible(false);
        think.set_tooltip_text(Some("model reasoning level"));

        let think_body = gtk::Box::new(gtk::Orientation::Vertical, 6);
        think_body.add_css_class("think-popover-body");
        let think_popover = gtk::Popover::new();
        think_popover.add_css_class("think-popover");
        think_popover.set_has_arrow(false);
        think_popover.set_position(gtk::PositionType::Top);
        think_popover.set_child(Some(&think_body));
        think_popover.set_parent(&think);

        let chat_think = gtk::Button::new();
        chat_think.add_css_class("composer-chip");
        let chat_think_body = gtk::Box::new(gtk::Orientation::Horizontal, 5);
        chat_think_body.append(&icons::icon("sliders", Tone::Muted));
        let chat_think_label = gtk::Label::new(Some("Reasoning"));
        chat_think_body.append(&chat_think_label);
        chat_think.set_child(Some(&chat_think_body));
        chat_think.set_valign(gtk::Align::End);
        chat_think.set_visible(false);
        chat_think.set_tooltip_text(Some("Choose the model's reasoning level"));

        let chat_search = gtk::ToggleButton::new();
        chat_search.add_css_class("composer-chip");
        let chat_search_body = gtk::Box::new(gtk::Orientation::Horizontal, 5);
        chat_search_body.append(&icons::icon("search", Tone::Muted));
        chat_search_body.append(&gtk::Label::new(Some("Search")));
        chat_search.set_child(Some(&chat_search_body));
        chat_search.set_valign(gtk::Align::End);
        chat_search.set_visible(false);
        chat_search.set_tooltip_text(Some("request web search when the answer needs current data"));

        let chat_tools = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        chat_tools.add_css_class("composer-chat-tools");
        chat_tools.set_halign(gtk::Align::Start);
        chat_tools.append(&chat_think);
        chat_tools.append(&chat_search);
        chat_tools.set_visible(false);

        actions.append(&attach);
        actions.append(&think);
        actions.append(&loop_chip);
        actions.append(&queue);
        actions.append(&stop);
        actions.append(&send);

        let widget_above = extension_widget_label();
        let widget_below = extension_widget_label();
        let hint = gtk::Label::new(None);
        hint.add_css_class("composer-hint");
        hint.set_xalign(0.0);
        hint.set_wrap(true);
        hint.set_visible(false);

        let attach_strip = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        attach_strip.add_css_class("attach-strip");
        attach_strip.set_visible(false);
        let attach_scroll = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Automatic)
            .vscrollbar_policy(gtk::PolicyType::Never)
            .child(&attach_strip)
            .build();
        attach_scroll.add_css_class("attach-scroll");
        attach_scroll.set_propagate_natural_height(true);
        attach_scroll.set_visible(false);

        let input_column = gtk::Box::new(gtk::Orientation::Vertical, 0);
        input_column.set_hexpand(true);
        input_column.append(&overlay);
        input_column.append(&hint);
        input_column.append(&chat_tools);

        frame.append(&input_column);
        frame.append(&actions);

        let column = gtk::Box::new(gtk::Orientation::Vertical, 0);
        column.append(&widget_above);
        column.append(&attach_scroll);
        column.append(&frame);
        column.append(&widget_below);

        let root = gtk::Overlay::new();
        root.add_css_class("composer-wrap");
        root.set_child(Some(&column));

        let palette = InlineList::new();
        let skills = InlineList::new();
        for list in [palette.widget(), skills.widget()] {
            list.set_valign(gtk::Align::End);
            list.set_halign(gtk::Align::Fill);
            list.set_margin_bottom(4);
        }
        root.add_overlay(palette.widget());
        root.add_overlay(skills.widget());

        let composer = Rc::new(Self {
            root,
            text,
            stop,
            anchor: frame,
            placeholder,
            send,
            send_icon,
            attach_strip,
            attach_scroll,
            attachments: RefCell::new(Vec::new()),
            palette,
            skills,
            last_mention: RefCell::new(None),
            commands: Rc::new(RefCell::new(Vec::new())),
            last_query: RefCell::new(String::new()),
            palette_targets: RefCell::new(Vec::new()),
            hint,
            hint_source: RefCell::new(None),
            suppress_palette: Cell::new(false),
            syncing: Cell::new(false),
            queue,
            attach,
            chat_think,
            chat_think_label,
            chat_search,
            chat_tools,
            chat_mode: Cell::new(false),
            think,
            think_label,
            think_popover,
            think_body,
            think_picker: RefCell::new(None),
            think_levels: RefCell::new(Vec::new()),
            think_active: RefCell::new(Vec::new()),
            think_index: Cell::new(0),
            on_thinking: Rc::new(RefCell::new(None)),
            on_search_toggle: Rc::new(RefCell::new(None)),
            loop_chip,
            loop_text,
            widget_above,
            widget_below,
            running: Cell::new(false),
            bash_running: Cell::new(false),
            history: RefCell::new(PromptHistory::load()),
            history_applying: Cell::new(false),
            files: RefCell::new(Vec::new()),
            on_submit: Rc::new(RefCell::new(None)),
            on_stop: Rc::new(RefCell::new(None)),
            on_command: Rc::new(RefCell::new(None)),
            on_notice: Rc::new(RefCell::new(None)),
            on_loop_stop: Rc::new(RefCell::new(None)),
        });
        composer.wire();
        composer
    }

    fn wire(self: &Rc<Self>) {
        let me = self.clone();
        self.chat_think.connect_clicked(move |_| {
            if me.think_popover.is_visible() { me.think_popover.popdown(); }
            else { me.open_thinking(); }
        });

        let me = self.clone();
        self.chat_search.connect_toggled(move |button| {
            if let Some(callback) = me.on_search_toggle.borrow().as_ref() {
                callback(button.is_active());
            }
        });

        let me = self.clone();
        self.text.buffer().connect_changed(move |_| {
            if !me.history_applying.get() {
                me.history.borrow_mut().reset();
            }
            me.refresh_send();
        });

        let me = self.clone();
        self.text
            .buffer()
            .connect_changed(move |buffer| me.sync_lists(buffer));

        let me = self.clone();
        self.text
            .buffer()
            .connect_notify_local(Some("cursor-position"), move |buffer, _| {
                me.sync_lists(buffer);
            });

        let key = gtk::EventControllerKey::new();
        key.set_propagation_phase(gtk::PropagationPhase::Capture);
        let me = self.clone();
        key.connect_key_pressed(move |_, key, _code, state| {
            let is_return = key == gtk::gdk::Key::Return || key == gtk::gdk::Key::KP_Enter;
            let shift = state.contains(gtk::gdk::ModifierType::SHIFT_MASK);
            let ctrl = state.contains(gtk::gdk::ModifierType::CONTROL_MASK);
            let alt = state.contains(gtk::gdk::ModifierType::ALT_MASK);

            if ctrl && matches!(key, gtk::gdk::Key::v | gtk::gdk::Key::V) {
                if let Some(clipboard) =
                    gtk::gdk::Display::default().map(|display| display.clipboard())
                {
                    let has_image = clipboard
                        .formats()
                        .mime_types()
                        .iter()
                        .any(|mime| mime.starts_with("image/"));
                    if has_image {
                        let me = me.clone();
                        glib::spawn_future_local(async move {
                            match clipboard.read_texture_future().await {
                                Ok(Some(texture)) => {
                                    let bytes = texture.save_to_png_bytes();
                                    me.attach_bytes(bytes.as_ref(), "glued image");
                                }
                                Ok(None) => {
                                    me.notice("clipboard without readable image".to_string());
                                }
                                Err(_) => {
                                    me.notice("fail to read clipboard image".to_string());
                                }
                            }
                        });
                        return glib::Propagation::Stop;
                    }
                }
            }

            if me.palette.is_open() {
                match key {
                    gtk::gdk::Key::Up | gtk::gdk::Key::KP_Up => {
                        me.palette.move_selection(-1);
                        me.show_palette_hint();
                        return glib::Propagation::Stop;
                    }
                    gtk::gdk::Key::Down | gtk::gdk::Key::KP_Down => {
                        me.palette.move_selection(1);
                        me.show_palette_hint();
                        return glib::Propagation::Stop;
                    }
                    gtk::gdk::Key::Escape => {
                        me.palette.close();
                        me.hint.set_visible(false);
                        return glib::Propagation::Stop;
                    }
                    _ => {}
                }
                let tab = key == gtk::gdk::Key::Tab && !shift && !ctrl && !alt;
                if tab && me.palette.len() > 0 {
                    me.complete_palette_selection();
                    return glib::Propagation::Stop;
                }
                if is_return && !shift && me.palette.len() > 0 {
                    me.palette.activate_selected();
                    return glib::Propagation::Stop;
                }
            }

            if me.skills.is_open() {
                match key {
                    gtk::gdk::Key::Up | gtk::gdk::Key::KP_Up => {
                        me.skills.move_selection(-1);
                        return glib::Propagation::Stop;
                    }
                    gtk::gdk::Key::Down | gtk::gdk::Key::KP_Down => {
                        me.skills.move_selection(1);
                        return glib::Propagation::Stop;
                    }
                    gtk::gdk::Key::Escape => {
                        me.skills.close();
                        return glib::Propagation::Stop;
                    }
                    _ => {}
                }
                let tab = key == gtk::gdk::Key::Tab && !shift && !ctrl && !alt;
                if ((is_return && !shift) || tab) && me.skills.len() > 0 {
                    me.skills.activate_selected();
                    return glib::Propagation::Stop;
                }
            }

            if !ctrl && !alt && !shift && !is_return {
                let up = matches!(key, gtk::gdk::Key::Up | gtk::gdk::Key::KP_Up);
                let down = matches!(key, gtk::gdk::Key::Down | gtk::gdk::Key::KP_Down);
                if up || down {
                    let empty = me.is_empty();
                    let navigating = me.history.borrow().navigating();
                    if empty || navigating {
                        let current = me.text();
                        let next = if up {
                            me.history.borrow_mut().previous(&current)
                        } else {
                            me.history.borrow_mut().next()
                        };
                        if let Some(text) = next {
                            me.apply_history(text);
                            return glib::Propagation::Stop;
                        }
                    }
                }
            }

            if is_return && ctrl && shift {
                me.palette.close();
                me.skills.close();
                me.remember();
                if let Some(callback) = me.on_submit.borrow().as_ref() {
                    callback(SubmitKind::CtrlShiftEnter);
                }
                return glib::Propagation::Stop;
            }

            if !is_return || shift {
                return glib::Propagation::Proceed;
            }
            me.palette.close();
            me.skills.close();
            me.remember();
            if let Some(callback) = me.on_submit.borrow().as_ref() {
                callback(if alt {
                    SubmitKind::AltEnter
                } else {
                    SubmitKind::Enter
                });
            }
            glib::Propagation::Stop
        });
        self.text.add_controller(key);

        let frame = self.anchor.clone();
        let enter_frame = frame.clone();
        let leave_frame = frame.clone();
        let focus = gtk::EventControllerFocus::new();
        focus.connect_enter(move |_| enter_frame.add_css_class("focused"));
        focus.connect_leave(move |_| leave_frame.remove_css_class("focused"));
        self.text.add_controller(focus);

        let me = self.clone();
        self.anchor.connect_map(move |_| {
            me.place_lists();
            me.refresh_list_anchor();
        });

        let me = self.clone();
        self.send.connect_clicked(move |_| {
            me.palette.close();
            me.remember();
            if let Some(callback) = me.on_submit.borrow().as_ref() {
                callback(SubmitKind::Enter);
            }
        });

        let me = self.clone();
        self.attach.connect_clicked(move |_| me.pick_files());

        let me = self.clone();
        self.think.connect_clicked(move |_| {
            if me.think_popover.is_visible() {
                me.think_popover.popdown();
            } else {
                me.think_popover.popup();
            }
        });

        let on_stop_c = self.on_stop.clone();
        self.stop.connect_clicked(move |_| {
            if let Some(callback) = on_stop_c.borrow().as_ref() {
                callback();
            }
        });

        let on_loop_stop = self.on_loop_stop.clone();
        self.loop_chip.connect_clicked(move |_| {
            if let Some(callback) = on_loop_stop.borrow().as_ref() {
                callback();
            }
        });
    }

    pub fn set_on_submit(&self, callback: impl Fn(SubmitKind) + 'static) {
        *self.on_submit.borrow_mut() = Some(Box::new(callback));
    }

    pub fn open_thinking(&self) {
        if self.think_active.borrow().is_empty() {
            return;
        }
        self.think_popover.popup();
    }

    pub fn set_on_thinking(&self, callback: impl Fn(String) + 'static) {
        *self.on_thinking.borrow_mut() = Some(Box::new(callback));
    }

    pub fn set_on_search_toggle(&self, callback: impl Fn(bool) + 'static) {
        *self.on_search_toggle.borrow_mut() = Some(Box::new(callback));
    }

    pub fn set_chat_mode(&self, chat: bool) {
        self.chat_mode.set(chat);
        let anchor = if chat { self.chat_think.upcast_ref::<gtk::Widget>() } else { self.think.upcast_ref::<gtk::Widget>() };
        if self.think_popover.parent().as_ref() != Some(anchor) {
            self.think_popover.popdown();
            self.think_popover.unparent();
            self.think_popover.set_parent(anchor);
        }
        let has_thinking = !self.think_active.borrow().is_empty();
        self.think.set_visible(!chat && has_thinking);
        self.chat_think.set_visible(chat && has_thinking);
        self.chat_search.set_visible(chat);
        self.chat_tools.set_visible(chat);
        self.placeholder.set_text(if chat {
            "Ask me something..."
        } else {
            "message... (/ commands)"
        });
    }

    pub fn set_search_active(&self, active: bool) {
        self.chat_search.set_active(active);
    }

    fn set_chat_think_active(&self, active: bool) {
        if active { self.chat_think.add_css_class("active"); }
        else { self.chat_think.remove_css_class("active"); }
    }

    pub fn set_thinking_levels(self: &Rc<Self>, levels: &[String], current: &str) {
        let changed = self.think_picker.borrow().is_none() || self.think_levels.borrow().as_slice() != levels;
        *self.think_levels.borrow_mut() = levels.to_vec();
        let active: Vec<String> = levels
            .iter()
            .filter(|level| level.as_str() != "off")
            .cloned()
            .collect();
        *self.think_active.borrow_mut() = active.clone();
        self.think.set_visible(!self.chat_mode.get() && !active.is_empty());
        self.chat_think
            .set_visible(self.chat_mode.get() && !active.is_empty());
        if active.is_empty() {
            self.think_popover.popdown();
            return;
        }
        if changed { self.rebuild_think(); }
        if current == "off" {
            self.apply_off(false);
        } else {
            let index = active
                .iter()
                .position(|level| level == current)
                .unwrap_or_else(|| active.len().saturating_sub(1));
            self.apply_think_level(index, false);
        }
    }

    fn rebuild_think(self: &Rc<Self>) {
        while let Some(child) = self.think_body.first_child() {
            self.think_body.remove(&child);
        }

        let title = gtk::Label::new(Some("REASONING"));
        title.add_css_class("palette-group");
        title.set_xalign(0.0);
        self.think_body.append(&title);

        let picker = LevelPicker::new(&self.think_levels.borrow());
        let me = self.clone();
        picker.set_on_change(move |level| {
            if level == "off" { me.apply_off(true); }
            else {
                let index = me.think_active.borrow().iter().position(|value| *value == level);
                if let Some(index) = index { me.apply_think_level(index, true); }
            }
        });
        self.think_body.append(&picker.root);
        *self.think_picker.borrow_mut() = Some(picker);
    }

    fn apply_off(self: &Rc<Self>, notify: bool) {
        self.think_label.set_text("OFF");
        self.chat_think_label.set_text("Reasoning · Off");
        self.think.add_css_class("off");
        if let Some(picker) = self.think_picker.borrow().as_ref() {
            picker.set_level("off");
        }
        self.set_chat_think_active(false);
        if notify {
            if let Some(callback) = self.on_thinking.borrow().as_ref() {
                callback("off".to_string());
            }
        }
    }

    fn apply_think_level(self: &Rc<Self>, index: usize, notify: bool) {
        let levels = self.think_active.borrow().clone();
        if levels.is_empty() {
            return;
        }
        let index = index.min(levels.len() - 1);
        self.think_index.set(index);
        self.think_label.set_text(&level_label(&levels[index]));
        self.chat_think_label.set_text(&format!("Reasoning · {}", level_label(&levels[index])));
        self.think.remove_css_class("off");

        self.set_chat_think_active(true);

        if let Some(picker) = self.think_picker.borrow().as_ref() {
            picker.set_level(&levels[index]);
        }

        if notify {
            if let Some(callback) = self.on_thinking.borrow().as_ref() {
                callback(levels[index].clone());
            }
        }
    }

    fn pick_files(self: &Rc<Self>) {
        let dialog = gtk::FileDialog::builder()
            .title("attach image or file")
            .modal(true)
            .build();
        let window = self
            .root
            .root()
            .and_then(|root| root.downcast::<gtk::Window>().ok());
        let me = self.clone();
        dialog.open_multiple(window.as_ref(), None::<&gtk::gio::Cancellable>, move |result| {
            let Ok(files) = result else {
                return;
            };
            let mut paths = Vec::new();
            for index in 0..files.n_items() {
                let Some(item) = files.item(index) else {
                    continue;
                };
                let Ok(file) = item.downcast::<gtk::gio::File>() else {
                    continue;
                };
                if let Some(path) = file.path() {
                    paths.push(path.to_string_lossy().to_string());
                }
            }
            if !paths.is_empty() {
                me.append_paths(&paths);
                me.focus();
            }
        });
    }

    pub fn set_on_stop(&self, callback: impl Fn() + 'static) {
        *self.on_stop.borrow_mut() = Some(Box::new(callback));
    }

    pub fn set_on_command(&self, callback: impl Fn(String) + 'static) {
        *self.on_command.borrow_mut() = Some(Box::new(callback));
    }

    pub fn set_on_notice(&self, callback: impl Fn(String) + 'static) {
        *self.on_notice.borrow_mut() = Some(Box::new(callback));
    }

    pub fn set_on_loop_stop(&self, callback: impl Fn() + 'static) {
        *self.on_loop_stop.borrow_mut() = Some(Box::new(callback));
    }


    pub fn take_attachments(self: &Rc<Self>) -> Vec<Attachment> {
        let taken = std::mem::take(&mut *self.attachments.borrow_mut());
        if !taken.is_empty() {
            self.refresh_attachments();
        }
        taken
    }

    /// Backwards-compatible single-attachment view used by restore paths.
    pub fn take_attachment(self: &Rc<Self>) -> Option<Attachment> {
        self.take_attachments().into_iter().next()
    }

    pub fn set_attachments(self: &Rc<Self>, attachments: Vec<Attachment>) {
        *self.attachments.borrow_mut() = attachments;
        self.refresh_attachments();
    }

    pub fn set_attachment(self: &Rc<Self>, attachment: Attachment) {
        self.set_attachments(vec![attachment]);
    }

    pub fn attachments(&self) -> Vec<Attachment> {
        self.attachments.borrow().clone()
    }

    pub fn clear_attachment(self: &Rc<Self>) {
        if !self.attachments.borrow_mut().is_empty() {
            self.attachments.borrow_mut().clear();
            self.refresh_attachments();
        }
    }

    fn remove_attachment(self: &Rc<Self>, index: usize) {
        {
            let mut attachments = self.attachments.borrow_mut();
            if index >= attachments.len() {
                return;
            }
            attachments.remove(index);
        }
        self.refresh_attachments();
    }

    pub fn attach_bytes(self: &Rc<Self>, bytes: &[u8], name: &str) -> bool {
        let Some(mime) = images::sniff_mime(bytes) else {
            self.notice(IMAGE_TYPE_NOTICE.to_string());
            return false;
        };
        if images::too_large(bytes.len()) {
            self.notice(IMAGE_SIZE_NOTICE.to_string());
            return false;
        }
        let attachment = Attachment {
            kind: AttachmentKind::Image,
            mime: mime.to_string(),
            data: images::encode_base64(bytes),
            name: name.to_string(),
            bytes: bytes.len(),
            path: None,
            text: None,
        };
        self.attachments.borrow_mut().push(attachment);
        self.refresh_attachments();
        true
    }

    pub fn attach_file(self: &Rc<Self>, path: &str) -> AttachFile {
        use std::io::Read;

        let Ok(mut file) = std::fs::File::open(path) else {
            self.notice("cannot open the file".to_string());
            return AttachFile::Rejected;
        };
        let metadata = file.metadata().ok();
        let total = metadata.as_ref().map(|meta| meta.len() as usize).unwrap_or(0);
        let mut head = [0u8; 16];
        let read = file.read(&mut head).unwrap_or(0);

        if images::sniff_mime(&head[..read]).is_some() {
            if images::too_large(total) {
                self.notice(IMAGE_SIZE_NOTICE.to_string());
                return AttachFile::Rejected;
            }
            let mut bytes = head[..read].to_vec();
            if file.read_to_end(&mut bytes).is_err() {
                self.notice("fail to read the image".to_string());
                return AttachFile::Rejected;
            }
            if images::too_large(bytes.len()) {
                self.notice(IMAGE_SIZE_NOTICE.to_string());
                return AttachFile::Rejected;
            }
            let name = file_name_of(path);
            return if self.attach_bytes(&bytes, &name) {
                AttachFile::Attached
            } else {
                AttachFile::Rejected
            };
        }

        if total > MAX_FILE_BYTES {
            self.notice("file too large (max 64 MB)".to_string());
            return AttachFile::Rejected;
        }
        let name = file_name_of(path);
        let mime = file_mime(path);
        let text = read_text_attachment(path, &mime);
        self.attachments.borrow_mut().push(Attachment {
            kind: AttachmentKind::File,
            mime,
            data: String::new(),
            name,
            bytes: total,
            path: Some(path.to_string()),
            text,
        });
        self.refresh_attachments();
        AttachFile::Attached
    }

    fn notice(&self, message: String) {
        if let Some(callback) = self.on_notice.borrow().as_ref() {
            callback(message);
        }
    }

    fn refresh_send(&self) {
        let text = self.text();
        let has_text = !text.trim().is_empty();
        let has_attach = !self.attachments.borrow().is_empty();
        self.placeholder.set_visible(text.is_empty());
        self.send.set_sensitive(has_text || has_attach);
        if has_text || has_attach {
            self.send.remove_css_class("idle");
            icons::set_icon(&self.send_icon, "send", Tone::Inverse);
        } else {
            self.send.add_css_class("idle");
            icons::set_icon(&self.send_icon, "send", Tone::Muted);
        }
    }

    fn refresh_attachments(self: &Rc<Self>) {
        while let Some(child) = self.attach_strip.first_child() {
            self.attach_strip.remove(&child);
        }
        let attachments = self.attachments.borrow();
        for (index, attachment) in attachments.iter().enumerate() {
            self.attach_strip
                .append(&self.attachment_chip(index, attachment));
        }
        let has = !attachments.is_empty();
        drop(attachments);
        self.attach_strip.set_visible(has);
        self.attach_scroll.set_visible(has);
        self.refresh_send();
    }

    fn attachment_chip(self: &Rc<Self>, index: usize, attachment: &Attachment) -> gtk::Box {
        let chip = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        chip.add_css_class("attach-chip");

        let slot = gtk::Box::new(gtk::Orientation::Vertical, 0);
        slot.add_css_class("attach-slot");
        slot.set_valign(gtk::Align::Center);
        let thumb: Option<gtk::Widget> = match attachment.kind {
            AttachmentKind::Image => {
                thumb::from_base64(&attachment.data, 36, "attach-thumb").map(|w| w.upcast())
            }
            AttachmentKind::File => {
                let icon = icons::icon("file", Tone::Muted);
                icon.set_pixel_size(22);
                let holder = gtk::Box::new(gtk::Orientation::Vertical, 0);
                holder.add_css_class("attach-file-icon");
                holder.set_valign(gtk::Align::Center);
                holder.set_halign(gtk::Align::Center);
                holder.append(&icon);
                Some(holder.upcast::<gtk::Widget>())
            }
        };
        if let Some(thumb) = thumb {
            slot.append(&thumb);
        }
        chip.append(&slot);

        let info = gtk::Box::new(gtk::Orientation::Vertical, 2);
        info.set_hexpand(true);
        info.set_valign(gtk::Align::Center);
        let name = gtk::Label::new(Some(&attachment.name));
        name.add_css_class("attach-name");
        name.set_xalign(0.0);
        name.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
        name.set_max_width_chars(22);
        let meta = gtk::Label::new(Some(&format!(
            "{} · {}",
            if attachment.mime.is_empty() {
                "file"
            } else {
                &attachment.mime
            },
            format_size(attachment.bytes)
        )));
        meta.add_css_class("attach-meta");
        meta.set_xalign(0.0);
        info.append(&name);
        info.append(&meta);
        chip.append(&info);

        let remove = gtk::Button::new();
        remove.add_css_class("attach-remove");
        remove.set_child(Some(&icons::icon("cross", Tone::Muted)));
        remove.set_tooltip_text(Some("Remove attachment"));
        remove.set_valign(gtk::Align::Center);
        let weak = Rc::downgrade(self);
        remove.connect_clicked(move |_| {
            if let Some(me) = weak.upgrade() {
                me.remove_attachment(index);
            }
        });
        chip.append(&remove);
        chip.set_tooltip_text(Some(
            attachment
                .path
                .as_deref()
                .unwrap_or(attachment.mime.as_str()),
        ));
        chip
    }

    pub fn append_paths(self: &Rc<Self>, paths: &[String]) {
        let mut rest: Vec<String> = Vec::new();
        for path in paths {
            match self.attach_file(path) {
                AttachFile::Attached | AttachFile::Rejected => {}
                AttachFile::NotImage => rest.push(path.clone()),
            }
        }
        if let DropOutcome::Insert(text) = drop_paths(&rest) {
            self.append_text(&text);
        }
    }

    pub fn append_text(&self, text: &str) {
        self.palette.close();
        self.skills.close();
        let _guard = Reentry::enter(&self.syncing);
        let buffer = self.text.buffer();
        let current = buffer.text(&buffer.start_iter(), &buffer.end_iter(), false);
        if !current.is_empty() && !current.ends_with('\n') {
            let mut end = buffer.end_iter();
            buffer.insert(&mut end, "\n");
        }
        let mut end = buffer.end_iter();
        buffer.insert(&mut end, text);
        self.focus();
    }

    pub fn set_workspace_files(self: &Rc<Self>, files: Vec<String>) {
        *self.files.borrow_mut() = files;
        if self.skills.is_open() {
            if let Some(mention) = self.last_mention.borrow().clone() {
                self.render_skills(&mention.query.to_lowercase(), mention);
            }
        }
    }

    pub fn set_commands(self: &Rc<Self>, commands: Vec<CommandItem>) {        *self.commands.borrow_mut() = commands;
        if self.palette.is_open() {
            let query = self.last_query.borrow().clone();
            self.render_palette(&query);
        }
        if self.skills.is_open() {
            if let Some(mention) = self.last_mention.borrow().clone() {
                self.render_skills(&mention.query.to_lowercase(), mention);
            }
        }
    }

    pub fn set_text(&self, text: &str) {
        self.text.buffer().set_text(text);
    }

    fn apply_history(&self, text: String) {
        self.history_applying.set(true);
        self.text.buffer().set_text(&text);
        let buffer = self.text.buffer();
        buffer.place_cursor(&buffer.end_iter());
        self.history_applying.set(false);
    }

    fn remember(&self) {
        let text = self.text();
        let mut history = self.history.borrow_mut();
        if text.trim().is_empty() {
            return;
        }
        history.push(&text);
        history.save();
    }

    pub fn text(&self) -> String {
        let buffer = self.text.buffer();
        buffer.text(&buffer.start_iter(), &buffer.end_iter(), false).to_string()
    }

    pub fn take_text(&self) -> String {
        let buffer = self.text.buffer();
        let text = buffer.text(&buffer.start_iter(), &buffer.end_iter(), false);
        buffer.set_text("");
        text.trim().to_string()
    }

    pub fn set_running(&self, running: bool) {
        self.running.set(running);
        self.sync_stop();
    }

    pub fn set_bash_running(&self, running: bool) {
        self.bash_running.set(running);
        if running {
            self.stop.add_css_class("bash");
        } else {
            self.stop.remove_css_class("bash");
        }
        self.sync_stop();
    }

    fn sync_stop(&self) {
        let running = self.running.get();
        let bash = self.bash_running.get();
        self.stop.set_visible(running || bash);
        let tooltip = if bash {
            "Stop the bash command"
        } else {
            "Stop the running turn (Esc)"
        };
        self.stop.set_tooltip_text(Some(tooltip));
    }

    pub fn set_queue(&self, count: usize) {
        if count == 0 {
            self.queue.set_visible(false);
            return;
        }
        self.queue.set_text(&if count == 1 {
            "1 QUEUED".to_string()
        } else {
            format!("{count} QUEUED")
        });
        self.queue.set_visible(true);
    }

    pub fn set_loop(&self, text: Option<&str>) {
        match text {
            Some(text) => {
                self.loop_text.set_text(text);
                self.loop_chip.set_visible(true);
            }
            None => {
                self.loop_text.set_text("");
                self.loop_chip.set_visible(false);
            }
        }
    }

    pub fn set_extension_widget(&self, placement: Placement, text: Option<&str>) {
        let label = match placement {
            Placement::Above => &self.widget_above,
            Placement::Below => &self.widget_below,
        };
        match text.filter(|text| !text.trim().is_empty()) {
            Some(text) => {
                label.set_text(text);
                label.set_visible(true);
            }
            None => {
                label.set_text("");
                label.set_visible(false);
            }
        }
    }

    pub fn focus(&self) {
        self.text.grab_focus();
    }

    #[allow(dead_code)]
    pub fn has_focus(&self) -> bool {
        self.text.has_focus()
    }

    pub fn open_commands(self: &Rc<Self>) {
        let query = self.last_query.borrow().clone();
        self.render_palette(&query);
    }

    pub fn palette_open(&self) -> bool {
        self.palette.is_open()
    }

    pub fn close_palette(&self) {
        self.palette.close();
    }

    pub fn skill_popup_open(&self) -> bool {
        self.skills.is_open()
    }

    pub fn close_skill_popup(&self) {
        self.skills.close();
    }

    pub fn is_empty(&self) -> bool {
        let buffer = self.text.buffer();
        buffer
            .text(&buffer.start_iter(), &buffer.end_iter(), false)
            .trim()
            .is_empty()
    }

    pub fn run_palette_selection(self: &Rc<Self>) {
        self.palette.activate_selected();
    }

    fn place_lists(&self) {
        let height = if self.anchor.width() > 0 {
            let measured = self
                .anchor
                .measure(gtk::Orientation::Vertical, self.anchor.width())
                .1;
            self.anchor.height().max(measured)
        } else {
            self.anchor.height()
        };
        let bottom = height + self.widget_below.height() + 4;
        self.palette.set_bottom_margin(bottom);
        self.skills.set_bottom_margin(bottom);
    }

    fn refresh_list_anchor(self: &Rc<Self>) {
        let me = self.clone();
        glib::timeout_add_local_once(std::time::Duration::from_millis(16), move || {
            me.place_lists();
        });
    }

    fn sync_lists(self: &Rc<Self>, buffer: &gtk::TextBuffer) {
        let Some(_guard) = Reentry::enter(&self.syncing) else {
            return;
        };
        if self.suppress_palette.get() {
            return;
        }
        self.place_lists();
        let text = buffer.text(&buffer.start_iter(), &buffer.end_iter(), false);
        let cursor = buffer.cursor_position().max(0) as usize;
        match list_query(&text, cursor) {
            ListQuery::Commands(query) => {
                self.skills.close();
                *self.last_mention.borrow_mut() = None;
                if self.palette.is_open() && *self.last_query.borrow() == query {
                    return;
                }
                self.render_palette(&query);
            }
            ListQuery::Skills(mention) => {
                if self.skills.is_open() && self.last_mention.borrow().as_ref() == Some(&mention) {
                    return;
                }
                *self.last_mention.borrow_mut() = Some(mention.clone());
                self.render_skills(&mention.query.to_lowercase(), mention);
            }
            ListQuery::None => {
                self.palette.close();
                self.hint.set_visible(false);
                if let Some(source) = self.hint_source.borrow_mut().take() {
                    source.remove();
                }
                self.skills.close();
                *self.last_mention.borrow_mut() = None;
            }
        }
    }

    fn render_skills(self: &Rc<Self>, query: &str, mention: Mention) {
        self.palette.close();
        self.skills.clear();

        let commands = self.commands.borrow();
        let matched: Vec<&CommandItem> = commands
            .iter()
            .filter(|command| command.source == "skill")
            .filter(|command| {
                let name = skill_display_name(&command.name).to_lowercase();
                query.is_empty()
                    || name.contains(query)
                    || command.description.to_lowercase().contains(query)
            })
            .collect();

        for command in matched {
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            row.set_hexpand(true);
            row.append(&icons::icon("file", Tone::Muted));

            let name = gtk::Label::new(Some(&skill_display_name(&command.name)));
            name.add_css_class("palette-name");
            name.set_xalign(0.0);

            let badge = gtk::Label::new(Some("SKILL"));
            badge.add_css_class("palette-badge");
            badge.add_css_class("skill");
            badge.set_valign(gtk::Align::Center);

            let description = gtk::Label::new(Some(&command.description));
            description.add_css_class("palette-desc");
            description.set_xalign(0.0);
            description.set_hexpand(true);
            description.set_ellipsize(gtk::pango::EllipsizeMode::End);

            row.append(&name);
            row.append(&badge);
            row.append(&description);

            let target = command.name.clone();
            let mention = mention.clone();
            let skills = self.skills.clone();
            let me = self.clone();
            self.skills.add(&row, move || {
                skills.close();
                me.apply_skill(&target, &mention);
            });
        }

        drop(commands);

        let files = self.files.borrow();
        let chosen: Vec<&String> = files
            .iter()
            .filter(|path| {
                query.is_empty() || path.to_lowercase().contains(query)
            })
            .take(10)
            .collect();

        for path in chosen {
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            row.set_hexpand(true);
            row.append(&icons::icon("file", Tone::Muted));

            let name = gtk::Label::new(Some(path));
            name.add_css_class("palette-name");
            name.set_xalign(0.0);
            name.set_hexpand(true);
            name.set_ellipsize(gtk::pango::EllipsizeMode::Middle);

            let badge = gtk::Label::new(Some("FILE"));
            badge.add_css_class("palette-badge");
            badge.add_css_class("prompt");
            badge.set_valign(gtk::Align::Center);

            row.append(&name);
            row.append(&badge);

            let target = path.clone();
            let mention = mention.clone();
            let skills = self.skills.clone();
            let me = self.clone();
            self.skills.add(&row, move || {
                skills.close();
                me.apply_file(&target, &mention);
            });
        }

        drop(files);

        if self.skills.len() == 0 {
            self.skills.note("Nothing married to @");
        }

        self.skills.open();
        if self.skills.len() > 0 {
            self.skills.select_first();
        }
        self.refresh_list_anchor();
    }

    fn apply_file(&self, path: &str, mention: &Mention) {
        let buffer = self.text.buffer();
        let text = buffer.text(&buffer.start_iter(), &buffer.end_iter(), false);
        let (new_text, cursor) = mentions::replace_mention(
            &text,
            mention.start,
            mention.end,
            &format!("@{path} "),
        );
        self.history_applying.set(true);
        buffer.set_text(&new_text);
        buffer.place_cursor(&buffer.iter_at_offset(cursor as i32));
        self.history_applying.set(false);
        self.focus();
    }

    fn apply_skill(&self, command: &str, mention: &Mention) {        let buffer = self.text.buffer();
        let text = buffer.text(&buffer.start_iter(), &buffer.end_iter(), false);
        let (new_text, cursor) = mentions::replace_mention(
            &text,
            mention.start,
            mention.end,
            &skill_insert(command),
        );
        buffer.set_text(&new_text);
        buffer.place_cursor(&buffer.iter_at_offset(cursor as i32));
        self.focus();
    }

    fn render_palette(self: &Rc<Self>, query: &str) {
        self.skills.close();
        *self.last_mention.borrow_mut() = None;
        *self.last_query.borrow_mut() = query.to_string();
        self.palette.clear();
        self.palette_targets.borrow_mut().clear();

        let commands = self.commands.borrow();
        let matched: Vec<&CommandItem> = commands
            .iter()
            .filter(|command| {
                query.is_empty()
                    || command.name.to_lowercase().contains(query)
                    || command.description.to_lowercase().contains(query)
            })
            .collect();

        if matched.is_empty() {
            self.palette.note(if commands.is_empty() {
                "no command — waiting for Pi"
            } else {
                "no command matches"
            });
        }

        for command in matched {
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            row.set_hexpand(true);

            let icon_name = match command.source.as_str() {
                "skill" => "file",
                "prompt" => "message",
                _ => "terminal",
            };
            row.append(&icons::icon(icon_name, Tone::Muted));

            let name = gtk::Label::new(Some(&format!("/{}", command.name)));
            name.add_css_class("palette-name");
            name.set_xalign(0.0);

            let badge = gtk::Label::new(Some(&source_label(&command.source)));
            badge.add_css_class("palette-badge");
            badge.add_css_class(&command.source);
            badge.set_valign(gtk::Align::Center);

            let description = gtk::Label::new(Some(&command.description));
            description.add_css_class("palette-desc");
            description.set_xalign(0.0);
            description.set_hexpand(true);
            description.set_ellipsize(gtk::pango::EllipsizeMode::End);

            row.append(&name);
            row.append(&badge);
            row.append(&description);

            self.palette_targets.borrow_mut().push(command.name.clone());
            let on_command = self.on_command.clone();
            let palette = self.palette.clone();
            let target = command.name.clone();
            self.palette.add(&row, move || {
                palette.close();
                if let Some(callback) = on_command.borrow().as_ref() {
                    callback(target.clone());
                }
            });
        }

        drop(commands);

        self.palette.open();
        if self.palette.len() > 0 {
            self.palette.select_first();
        }
        self.show_palette_hint();
        self.schedule_command_help();
        self.refresh_list_anchor();
    }

    pub fn complete_palette_selection(self: &Rc<Self>) {
        let index = self.palette.selected_index();
        let target = self.palette_targets.borrow().get(index).cloned();
        let Some(name) = target else {
            return;
        };
        self.palette.close();
        if let Some(source) = self.hint_source.borrow_mut().take() {
            source.remove();
        }
        let description = self
            .commands
            .borrow()
            .iter()
            .find(|command| command.name == name)
            .map(|command| command.description.clone())
            .unwrap_or_default();
        self.suppress_palette.set(true);
        self.set_text(&format!("/{name}"));
        let buffer = self.text.buffer();
        buffer.place_cursor(&buffer.end_iter());
        self.palette.close();
        let me = self.clone();
        glib::idle_add_local_once(move || me.suppress_palette.set(false));
        self.show_hint(
            &format!("/{name} — {description}"),
            "full tab · enter run",
        );
    }

    fn show_palette_hint(self: &Rc<Self>) {
        let Some(name) = self
            .palette_targets
            .borrow()
            .get(self.palette.selected_index())
            .cloned()
        else {
            self.hint.set_visible(false);
            return;
        };
        self.show_hint(&format!("/{name}"), "full tab · enter execute · ↑↓ navigate");
    }

    fn show_hint(&self, text: &str, suffix: &str) {
        self.hint.set_text(&format!("{text}  ·  {suffix}"));
        self.hint.set_visible(true);
    }

    fn schedule_command_help(self: &Rc<Self>) {
        if let Some(source) = self.hint_source.borrow_mut().take() {
            source.remove();
        }
        let me = self.clone();
        let source = glib::timeout_add_local_once(std::time::Duration::from_millis(2_500), move || {
            if !me.palette.is_open() {
                return;
            }
            let Some(name) = me
                .palette_targets
                .borrow()
                .get(me.palette.selected_index())
                .cloned()
            else {
                return;
            };
            let description = me
                .commands
                .borrow()
                .iter()
                .find(|command| command.name == name)
                .map(|command| command.description.clone())
                .unwrap_or_default();
            if !description.is_empty() {
                me.show_hint(
                    &format!("/{name} — {description}"),
                    "full tab · enter run",
                );
            }
        });
        *self.hint_source.borrow_mut() = Some(source);
    }
}

fn format_size(bytes: usize) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} kB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

const MAX_FILE_BYTES: usize = 64 * 1024 * 1024;
const MAX_TEXT_BYTES: usize = 256 * 1024;

fn file_name_of(path: &str) -> String {
    std::path::Path::new(path)
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string())
}

fn file_mime(path: &str) -> String {
    let extension = std::path::Path::new(path)
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let mime = match extension.as_str() {
        "pdf" => "application/pdf",
        "txt" | "log" => "text/plain",
        "md" | "markdown" => "text/markdown",
        "json" => "application/json",
        "jsonl" => "application/x-ndjson",
        "csv" => "text/csv",
        "tsv" => "text/tab-separated-values",
        "yaml" | "yml" => "application/yaml",
        "toml" => "application/toml",
        "html" | "htm" => "text/html",
        "css" => "text/css",
        "rs" => "text/x-rust",
        "py" => "text/x-python",
        "js" | "mjs" | "cjs" => "text/javascript",
        "ts" | "tsx" => "text/typescript",
        "go" => "text/x-go",
        "c" | "h" => "text/x-c",
        "cpp" | "hpp" | "cc" => "text/x-c++",
        "java" => "text/x-java",
        "sh" | "bash" | "zsh" | "fish" => "text/x-shellscript",
        "sql" => "application/sql",
        "xml" => "application/xml",
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "zip" => "application/zip",
        _ => "application/octet-stream",
    };
    mime.to_string()
}

fn is_text_mime(mime: &str) -> bool {
    mime.starts_with("text/")
        || matches!(
            mime,
            "application/json"
                | "application/x-ndjson"
                | "application/yaml"
                | "application/toml"
                | "application/xml"
                | "application/sql"
        )
        || mime.ends_with("+json")
}

/// Reads text-like attachments and extracts PDF text when `pdftotext` exists,
/// so the agent receives content instead of just a path.
fn read_text_attachment(path: &str, mime: &str) -> Option<String> {
    if mime == "application/pdf" {
        // Extraction runs on the UI thread, so keep it to documents where
        // pdftotext is near-instant.
        let size = std::fs::metadata(path).ok()?.len();
        if size > 20 * 1024 * 1024 {
            return None;
        }
        return pdf_text(path);
    }
    if !is_text_mime(mime) {
        return None;
    }
    let metadata = std::fs::metadata(path).ok()?;
    if metadata.len() as usize > MAX_TEXT_BYTES {
        return None;
    }
    std::fs::read_to_string(path).ok()
}

fn pdf_text(path: &str) -> Option<String> {
    let output = std::process::Command::new("pdftotext")
        .args(["-layout", "-enc", "UTF-8", path, "-"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout).to_string();
    if text.trim().is_empty() {
        None
    } else {
        Some(text)
    }
}

fn extension_widget_label() -> gtk::Label {
    let label = gtk::Label::new(None);
    label.add_css_class("extension-widget");
    label.set_xalign(0.0);
    label.set_halign(gtk::Align::Fill);
    label.set_wrap(true);
    label.set_selectable(true);
    label.set_visible(false);
    label
}

fn source_label(source: &str) -> String {
    match source {
        "skill" => "skill".to_string(),
        "prompt" => "prompt".to_string(),
        "extension" => "ext".to_string(),
        other => other.to_string(),
    }
}

fn skill_display_name(name: &str) -> String {
    name.strip_prefix("skill:").unwrap_or(name).to_string()
}

fn skill_insert(name: &str) -> String {
    let name = name.strip_prefix("skill:").unwrap_or(name);
    format!("/skill:{name} ")
}
