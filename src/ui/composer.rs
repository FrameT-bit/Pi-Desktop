
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk::prelude::*;

use crate::extension_ui::Placement;
use crate::icons::{self, Tone};
use crate::images;
use crate::mentions::{self, Mention};
use crate::prompt_history::PromptHistory;
use crate::state::CommandItem;

use super::level_slider::{level_label, LevelSlider};
use super::particles::Particles;
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Attachment {
    pub mime: String,
    pub data: String,
    pub name: String,
    pub bytes: usize,
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
    attach_thumb: gtk::Box,
    attach_name: gtk::Label,
    attach_meta: gtk::Label,
    attach_remove: gtk::Button,
    attachment: RefCell<Option<Attachment>>,
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
    chat_think: gtk::ToggleButton,
    chat_search: gtk::ToggleButton,
    chat_tools: gtk::Box,
    chat_mode: Cell<bool>,
    chat_toggle_guard: Cell<bool>,
    think: gtk::Button,
    think_label: gtk::Label,
    think_popover: gtk::Popover,
    think_body: gtk::Box,
    think_slider: RefCell<Option<Rc<LevelSlider>>>,
    think_levels: RefCell<Vec<String>>,
    think_active: RefCell<Vec<String>>,
    think_index: Cell<usize>,
    think_turbo: Cell<bool>,
    particles: Rc<Particles>,
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
        text.set_size_request(-1, 40);

        let placeholder = gtk::Label::new(Some("message... (/ commands)"));
        placeholder.add_css_class("composer-placeholder");
        placeholder.set_halign(gtk::Align::Start);
        placeholder.set_valign(gtk::Align::Start);
        placeholder.set_xalign(0.0);
        placeholder.set_can_target(false);

        let overlay = gtk::Overlay::new();
        overlay.set_child(Some(&text));
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

        let particles = Particles::new();
        let think_body = gtk::Box::new(gtk::Orientation::Vertical, 6);
        think_body.add_css_class("think-popover-body");
        let think_shell = gtk::Overlay::new();
        think_shell.set_child(Some(&think_body));
        think_shell.add_overlay(&particles.area);
        think_shell.set_measure_overlay(&particles.area, false);
        particles.area.set_halign(gtk::Align::Fill);
        particles.area.set_valign(gtk::Align::Fill);
        let think_popover = gtk::Popover::new();
        think_popover.add_css_class("think-popover");
        think_popover.set_has_arrow(false);
        think_popover.set_position(gtk::PositionType::Top);
        think_popover.set_child(Some(&think_shell));
        think_popover.set_parent(&think);

        let chat_think = gtk::ToggleButton::new();
        chat_think.add_css_class("composer-chip");
        let chat_think_body = gtk::Box::new(gtk::Orientation::Horizontal, 5);
        chat_think_body.append(&icons::icon("sliders", Tone::Muted));
        chat_think_body.append(&gtk::Label::new(Some("Thinking")));
        chat_think.set_child(Some(&chat_think_body));
        chat_think.set_valign(gtk::Align::End);
        chat_think.set_visible(false);
        chat_think.set_tooltip_text(Some("turn on/off the model's reasoning"));

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

        let attach_thumb = gtk::Box::new(gtk::Orientation::Vertical, 0);
        attach_thumb.add_css_class("attach-slot");
        let attach_name = gtk::Label::new(None);
        attach_name.add_css_class("attach-name");
        attach_name.set_xalign(0.0);
        attach_name.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
        attach_name.set_hexpand(true);
        let attach_meta = gtk::Label::new(None);
        attach_meta.add_css_class("attach-meta");
        attach_meta.set_xalign(0.0);
        let attach_info = gtk::Box::new(gtk::Orientation::Vertical, 2);
        attach_info.set_hexpand(true);
        attach_info.set_valign(gtk::Align::Center);
        attach_info.append(&attach_name);
        attach_info.append(&attach_meta);
        let attach_remove = gtk::Button::new();
        attach_remove.add_css_class("attach-remove");
        attach_remove.set_child(Some(&icons::icon("cross", Tone::Muted)));
        attach_remove.set_tooltip_text(Some("Remove attachment"));
        attach_remove.set_valign(gtk::Align::Center);
        let attach_strip = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        attach_strip.add_css_class("attach-strip");
        attach_strip.append(&attach_thumb);
        attach_strip.append(&attach_info);
        attach_strip.append(&attach_remove);
        attach_strip.set_visible(false);

        let input_column = gtk::Box::new(gtk::Orientation::Vertical, 0);
        input_column.set_hexpand(true);
        input_column.append(&overlay);
        input_column.append(&hint);
        input_column.append(&chat_tools);

        frame.append(&input_column);
        frame.append(&actions);

        let column = gtk::Box::new(gtk::Orientation::Vertical, 0);
        column.append(&widget_above);
        column.append(&attach_strip);
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
            attach_thumb,
            attach_name,
            attach_meta,
            attach_remove,
            attachment: RefCell::new(None),
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
            chat_search,
            chat_tools,
            chat_mode: Cell::new(false),
            chat_toggle_guard: Cell::new(false),
            think,
            think_label,
            think_popover,
            think_body,
            think_slider: RefCell::new(None),
            think_levels: RefCell::new(Vec::new()),
            think_active: RefCell::new(Vec::new()),
            think_index: Cell::new(0),
            think_turbo: Cell::new(false),
            particles,
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
        self.chat_think.connect_toggled(move |button| {
            if me.chat_toggle_guard.get() {
                return;
            }
            if !me.chat_mode.get() {
                return;
            }
            if button.is_active() {
                let len = me.think_active.borrow().len();
                if len == 0 {
                    me.set_chat_think_active(false);
                    return;
                }
                let index = me.think_index.get().min(len - 1);
                me.apply_think_level(index, true);
            } else {
                me.apply_off(true);
            }
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
        self.attach_remove.connect_clicked(move |_| {
            me.clear_attachment();
            me.focus();
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
        if !self.think.get_visible() {
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
        if chat {
            self.think_popover.popdown();
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
        self.chat_toggle_guard.set(true);
        self.chat_think.set_active(active);
        self.chat_toggle_guard.set(false);
    }

    pub fn set_thinking_levels(self: &Rc<Self>, levels: &[String], current: &str) {
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
            self.particles.set_active(false);
            return;
        }
        self.rebuild_think(&active);
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

    fn rebuild_think(self: &Rc<Self>, levels: &[String]) {
        while let Some(child) = self.think_body.first_child() {
            self.think_body.remove(&child);
        }

        let title = gtk::Label::new(Some("REASONING"));
        title.add_css_class("palette-group");
        title.set_xalign(0.0);
        self.think_body.append(&title);

        let slider = LevelSlider::new(levels.to_vec());
        let me = self.clone();
        slider.set_on_change(move |index| me.apply_think_level(index, true));
        *self.think_slider.borrow_mut() = Some(slider.clone());
        let slider_wrap = gtk::Box::new(gtk::Orientation::Vertical, 0);
        slider_wrap.add_css_class("think-slider-wrap");
        slider_wrap.set_hexpand(true);
        slider_wrap.set_size_request(-1, 64);
        slider_wrap.append(&slider.area);
        slider.track_on(&slider_wrap);
        self.think_body.append(&slider_wrap);

        let off = gtk::Button::with_label("turn off reasoning");
        off.add_css_class("rail-open");
        off.set_halign(gtk::Align::Start);
        let me = self.clone();
        off.connect_clicked(move |_| me.apply_off(true));
        self.think_body.append(&off);

        let hint = gtk::Label::new(Some("the last level enables turbo"));
        hint.add_css_class("palette-desc");
        hint.set_xalign(0.0);
        self.think_body.append(&hint);
    }

    fn apply_off(self: &Rc<Self>, notify: bool) {
        self.think_label.set_text("OFF");
        self.think.add_css_class("off");
        self.think_turbo.set(false);
        self.think.remove_css_class("turbo");
        if let Some(slider) = self.think_slider.borrow().as_ref() {
            slider.set_turbo(false);
            slider.set_index(0);
        }
        self.particles.set_active(false);
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
        self.think.remove_css_class("off");

        let turbo = levels.len() > 1 && index == levels.len() - 1;
        self.think_turbo.set(turbo);
        if turbo {
            self.think.add_css_class("turbo");
        } else {
            self.think.remove_css_class("turbo");
        }
        if let Some(slider) = self.think_slider.borrow().as_ref() {
            slider.set_turbo(turbo);
        }
        self.particles.set_active(turbo);
        self.set_chat_think_active(true);

        if let Some(slider) = self.think_slider.borrow().as_ref() {
            slider.set_index(index);
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


    pub fn take_attachment(&self) -> Option<Attachment> {
        let taken = self.attachment.borrow_mut().take();
        if taken.is_some() {
            self.refresh_attachment();
        }
        taken
    }

    pub fn set_attachment(&self, attachment: Attachment) {
        *self.attachment.borrow_mut() = Some(attachment);
        self.refresh_attachment();
    }

    pub fn clear_attachment(&self) {
        if self.attachment.borrow_mut().take().is_some() {
            self.refresh_attachment();
        }
    }

    pub fn attach_bytes(&self, bytes: &[u8], name: &str) -> bool {
        let Some(mime) = images::sniff_mime(bytes) else {
            self.notice(IMAGE_TYPE_NOTICE.to_string());
            return false;
        };
        if images::too_large(bytes.len()) {
            self.notice(IMAGE_SIZE_NOTICE.to_string());
            return false;
        }
        let attachment = Attachment {
            mime: mime.to_string(),
            data: images::encode_base64(bytes),
            name: name.to_string(),
            bytes: bytes.len(),
        };
        self.set_attachment(attachment);
        true
    }

    pub fn attach_file(&self, path: &str) -> AttachFile {
        use std::io::Read;

        let Ok(mut file) = std::fs::File::open(path) else {
            return AttachFile::NotImage;
        };
        let metadata = file.metadata().ok();
        let mut head = [0u8; 16];
        let read = file.read(&mut head).unwrap_or(0);
        if images::sniff_mime(&head[..read]).is_none() {
            return AttachFile::NotImage;
        }
        if metadata
            .as_ref()
            .is_some_and(|meta| images::too_large(meta.len() as usize))
        {
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
        let name = std::path::Path::new(path)
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| path.to_string());
        if self.attach_bytes(&bytes, &name) {
            AttachFile::Attached
        } else {
            AttachFile::Rejected
        }
    }

    fn notice(&self, message: String) {
        if let Some(callback) = self.on_notice.borrow().as_ref() {
            callback(message);
        }
    }

    fn refresh_send(&self) {
        let text = self.text();
        let has_text = !text.trim().is_empty();
        let has_attach = self.attachment.borrow().is_some();
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

    fn refresh_attachment(&self) {
        let attachment = self.attachment.borrow().clone();
        let Some(attachment) = attachment else {
            self.attach_strip.set_visible(false);
            self.refresh_send();
            return;
        };
        self.attach_name.set_text(&attachment.name);
        self.attach_name.set_tooltip_text(Some(&attachment.mime));
        self.attach_meta.set_text(&format!(
            "{} · {}",
            attachment.mime,
            format_size(attachment.bytes)
        ));
        while let Some(child) = self.attach_thumb.first_child() {
            self.attach_thumb.remove(&child);
        }
        if let Some(widget) = thumb::from_base64(&attachment.data, 40, "attach-thumb") {
            self.attach_thumb.append(&widget);
        }
        self.attach_strip.set_visible(true);
        self.refresh_send();
    }

    pub fn append_paths(&self, paths: &[String]) {
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


