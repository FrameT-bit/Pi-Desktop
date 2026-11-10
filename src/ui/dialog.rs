
use std::cell::Cell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use gtk::prelude::*;

use crate::extension_ui::{strip_ansi, Method, Request, Response};

type Reply = Rc<dyn Fn(Response)>;

pub fn show(parent: &impl IsA<gtk::Window>, request: &Request, reply: impl Fn(Response) + 'static) {
    let reply: Reply = Rc::new(reply);
    let done = Rc::new(Cell::new(false));
    match request.method {
        Method::Select => show_select(parent, request, reply, done),
        Method::Confirm => show_confirm(parent, request, reply, done),
        Method::Input => show_input(parent, request, reply, done),
        Method::Editor => show_editor(parent, request, reply, done),
        _ => {}
    }
}

fn respond(done: &Cell<bool>, reply: &Reply, response: Response) {
    if done.replace(true) {
        return;
    }
    reply(response);
}

fn arm_timeout(
    window: &gtk::Window,
    title: String,
    label: Option<gtk::Label>,
    timeout: Option<Duration>,
    done: Rc<Cell<bool>>,
    reply: Reply,
) {
    let Some(timeout) = timeout else {
        return;
    };
    let window = window.clone();
    let ends = Instant::now() + timeout;
    glib::timeout_add_local(Duration::from_millis(250), move || {
        if done.get() {
            return glib::ControlFlow::Break;
        }
        let remaining = ends.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            respond(&done, &reply, Response::Cancelled);
            window.close();
            return glib::ControlFlow::Break;
        }
        let seconds = remaining.as_secs() + u64::from(remaining.subsec_millis() > 0);
        let shown = format!("{title} · {seconds}s");
        if let Some(label) = label.as_ref() {
            label.set_text(&shown);
        }
        glib::ControlFlow::Continue
    });
}

fn shell(
    parent: &impl IsA<gtk::Window>,
    title: &str,
    width: i32,
    height: i32,
) -> (gtk::Window, gtk::Box, gtk::Label, gtk::Button) {
    let window = gtk::Window::new();
    window.set_transient_for(Some(parent));
    window.set_modal(true);
    window.set_destroy_with_parent(true);
    window.set_decorated(false);
    window.set_resizable(false);
    if let Some(app) = parent.as_ref().application() {
        window.set_application(Some(&app));
    }
    window.add_css_class("pi-dialog");
    window.set_title(Some(&strip_ansi(title)));
    crate::ui::fit::fit(&window, width, height);

    let title_label = gtk::Label::new(Some(&strip_ansi(title)));
    title_label.add_css_class("dialog-title");
    title_label.set_xalign(0.0);
    title_label.set_wrap(false);
    title_label.set_ellipsize(gtk::pango::EllipsizeMode::End);
    title_label.set_hexpand(true);

    let cancel = gtk::Button::with_label("cancel");
    cancel.add_css_class("dialog-cancel");
    cancel.set_valign(gtk::Align::Center);

    let header = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    header.add_css_class("pi-dialog-header");
    header.append(&title_label);
    header.append(&cancel);

    let content = gtk::Box::new(gtk::Orientation::Vertical, 12);
    content.add_css_class("pi-dialog-body");
    content.set_vexpand(true);

    let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
    root.add_css_class("pi-dialog");
    root.append(&header);
    root.append(&content);
    window.set_child(Some(&root));

    (window, content, title_label, cancel)
}

fn add_accept(header: &gtk::Box, label: &str) -> gtk::Button {
    let accept = gtk::Button::with_label(label);
    accept.add_css_class("dialog-accept");
    accept.set_valign(gtk::Align::Center);
    header.append(&accept);
    accept
}

fn show_select(
    parent: &impl IsA<gtk::Window>,
    request: &Request,
    reply: Reply,
    done: Rc<Cell<bool>>,
) {
    let height = (request.options.len() as i32 * 50 + 140).clamp(220, 560);
    let (window, content, title_label, cancel) = shell(parent, &request.title, 420, height);

    let list = gtk::Box::new(gtk::Orientation::Vertical, 6);
    list.set_hexpand(true);
    let mut first: Option<gtk::Button> = None;
    for option in &request.options {
        let button = gtk::Button::with_label(&strip_ansi(option));
        button.add_css_class("dialog-option");
        button.set_hexpand(true);

        let done_c = done.clone();
        let reply_c = reply.clone();
        let value = option.clone();
        let window_c = window.clone();
        button.connect_clicked(move |_| {
            respond(&done_c, &reply_c, Response::Value(value.clone()));
            window_c.close();
        });
        if first.is_none() {
            first = Some(button.clone());
        }
        list.append(&button);
    }
    if request.options.is_empty() {
        let note = gtk::Label::new(Some("the extension did not offer options"));
        note.add_css_class("dialog-note");
        note.set_xalign(0.0);
        list.append(&note);
    }

    let scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vscrollbar_policy(gtk::PolicyType::Automatic)
        .max_content_height(360)
        .propagate_natural_height(true)
        .child(&list)
        .build();
    scroll.set_vexpand(true);
    content.append(&scroll);

    wire_cancel_button(&cancel, &window, &done, &reply);
    wire_cancel(&window, &done, &reply);
    wire_escape(&window, &done, &reply);

    let title = strip_ansi(&request.title);
    arm_timeout(&window, title, Some(title_label), request.timeout, done, reply);
    window.present();
    if let Some(button) = first {
        glib::idle_add_local_once(move || {
            let _ = button.grab_focus();
        });
    }
}

fn show_confirm(
    parent: &impl IsA<gtk::Window>,
    request: &Request,
    reply: Reply,
    done: Rc<Cell<bool>>,
) {
    let extra = (strip_ansi(&request.message).chars().count() / 44) as i32 * 24;
    let height = (210 + extra).clamp(220, 420);
    let (window, content, title_label, cancel) = shell(parent, &request.title, 440, height);

    let message = gtk::Label::new(Some(&strip_ansi(&request.message)));
    message.add_css_class("dialog-message");
    message.set_xalign(0.0);
    message.set_wrap(true);
    message.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    message.set_hexpand(true);
    message.set_vexpand(true);
    message.set_halign(gtk::Align::Fill);
    message.set_valign(gtk::Align::Start);
    content.append(&message);

    let buttons = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    buttons.set_halign(gtk::Align::End);
    buttons.set_valign(gtk::Align::End);

    let no = gtk::Button::with_label("no, no, no, no, no, no, no, no, no, no, no, no, no, no, no, no, no, no, no, no, no, no, no, no, no, no, no, no, no, no, no, no, no, no, no, no, no, no, no, no, no, no, no, no, no, no, no, no, no, no, no, no, no, no, no, no, no, no, no, no, no, no, no, no, no, no, no, no, no, no, no, no, no, no, no, no, no, no, no,");
    no.add_css_class("dialog-option");
    no.add_css_class("dialog-option-cancel");
    let confirm = gtk::Button::with_label("confirmar");
    confirm.add_css_class("dialog-accept");
    confirm.set_receives_default(true);

    buttons.append(&no);
    buttons.append(&confirm);
    content.append(&buttons);

    {
        let done_c = done.clone();
        let reply_c = reply.clone();
        let window_c = window.clone();
        no.connect_clicked(move |_| {
            respond(&done_c, &reply_c, Response::Confirmed(false));
            window_c.close();
        });
    }
    {
        let done_c = done.clone();
        let reply_c = reply.clone();
        let window_c = window.clone();
        confirm.connect_clicked(move |_| {
            respond(&done_c, &reply_c, Response::Confirmed(true));
            window_c.close();
        });
    }

    wire_cancel_button(&cancel, &window, &done, &reply);
    wire_cancel(&window, &done, &reply);
    wire_escape(&window, &done, &reply);

    let title = strip_ansi(&request.title);
    arm_timeout(&window, title, Some(title_label), request.timeout, done, reply);
    window.present();
    glib::idle_add_local_once(move || {
        let _ = confirm.grab_focus();
    });
}

pub fn confirm(
    parent: &impl IsA<gtk::Window>,
    title: &str,
    message: &str,
    accept_label: &str,
    on_accept: impl Fn() + 'static,
) {
    let extra = (message.chars().count() / 44) as i32 * 24;
    let height = (210 + extra).clamp(220, 420);
    let (window, content, _title_label, cancel) = shell(parent, title, 440, height);

    let message_label = gtk::Label::new(Some(message));
    message_label.add_css_class("dialog-message");
    message_label.set_xalign(0.0);
    message_label.set_wrap(true);
    message_label.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    message_label.set_hexpand(true);
    message_label.set_vexpand(true);
    message_label.set_halign(gtk::Align::Fill);
    message_label.set_valign(gtk::Align::Start);
    content.append(&message_label);

    let buttons = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    buttons.set_halign(gtk::Align::End);
    buttons.set_valign(gtk::Align::End);

    let no = gtk::Button::with_label("cancel");
    no.add_css_class("dialog-option");
    no.add_css_class("dialog-option-cancel");
    let accept = gtk::Button::with_label(accept_label);
    accept.add_css_class("dialog-accept");
    accept.set_receives_default(true);
    buttons.append(&no);
    buttons.append(&accept);
    content.append(&buttons);

    {
        let window_c = window.clone();
        no.connect_clicked(move |_| window_c.close());
    }
    {
        let window_c = window.clone();
        accept.connect_clicked(move |_| {
            window_c.close();
            on_accept();
        });
    }

    let done = Rc::new(Cell::new(false));
    let reply: Reply = Rc::new(|_| {});
    wire_cancel_button(&cancel, &window, &done, &reply);
    wire_escape(&window, &done, &reply);

    window.present();
    glib::idle_add_local_once(move || {
        let _ = accept.grab_focus();
    });
}

fn show_input(
    parent: &impl IsA<gtk::Window>,
    request: &Request,
    reply: Reply,
    done: Rc<Cell<bool>>,
) {
    let (window, content, title_label, cancel) = shell(parent, &request.title, 440, 230);

    let row = gtk::Entry::new();
    row.add_css_class("dialog-entry");
    let placeholder = strip_ansi(&request.placeholder);
    row.set_placeholder_text(Some(if placeholder.is_empty() {
        "valor"
    } else {
        &placeholder
    }));
    row.set_hexpand(true);
    content.append(&row);

    let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    actions.set_halign(gtk::Align::End);
    actions.set_valign(gtk::Align::End);
    actions.set_margin_top(8);

    let accept = add_accept(&actions, "ok");
    accept.set_receives_default(true);
    content.append(&actions);

    let submit = {
        let done = done.clone();
        let reply = reply.clone();
        let window = window.clone();
        let row = row.clone();
        move || {
            let value = row.text().to_string();
            respond(&done, &reply, Response::Value(value));
            window.close();
        }
    };
    {
        let submit = submit.clone();
        accept.connect_clicked(move |_| submit());
    }
    {
        let submit = submit.clone();
        row.connect_activate(move |_| submit());
    }

    wire_cancel_button(&cancel, &window, &done, &reply);
    wire_cancel(&window, &done, &reply);
    wire_escape(&window, &done, &reply);

    let title = strip_ansi(&request.title);
    arm_timeout(&window, title, Some(title_label), request.timeout, done, reply);
    window.present();
    glib::idle_add_local_once(move || {
        let _ = row.grab_focus();
    });
}

fn show_editor(
    parent: &impl IsA<gtk::Window>,
    request: &Request,
    reply: Reply,
    done: Rc<Cell<bool>>,
) {
    let (window, content, title_label, cancel) = shell(parent, &request.title, 620, 480);

    let text = gtk::TextView::new();
    text.add_css_class("dialog-editor");
    text.set_wrap_mode(gtk::WrapMode::WordChar);
    text.set_hexpand(true);
    text.set_vexpand(true);
    text.set_left_margin(10);
    text.set_right_margin(10);
    text.set_top_margin(8);
    text.set_bottom_margin(8);
    text.buffer().set_text(&strip_ansi(&request.prefill));

    let scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vscrollbar_policy(gtk::PolicyType::Automatic)
        .vexpand(true)
        .hexpand(true)
        .child(&text)
        .build();
    scroll.add_css_class("dialog-editor-scroll");
    scroll.set_min_content_height(280);
    content.append(&scroll);

    let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    actions.set_halign(gtk::Align::End);
    let accept = add_accept(&actions, "ok");
    accept.set_receives_default(true);
    content.append(&actions);

    {
        let done_c = done.clone();
        let reply_c = reply.clone();
        let window_c = window.clone();
        let buffer = text.buffer();
        accept.connect_clicked(move |_| {
            let value = buffer.text(&buffer.start_iter(), &buffer.end_iter(), false);
            respond(&done_c, &reply_c, Response::Value(value.to_string()));
            window_c.close();
        });
    }

    wire_cancel_button(&cancel, &window, &done, &reply);
    wire_cancel(&window, &done, &reply);
    wire_escape(&window, &done, &reply);

    let title = strip_ansi(&request.title);
    arm_timeout(&window, title, Some(title_label), request.timeout, done, reply);
    window.present();
    glib::idle_add_local_once(move || {
        let _ = text.grab_focus();
    });
}

fn wire_cancel_button(
    button: &gtk::Button,
    window: &gtk::Window,
    done: &Rc<Cell<bool>>,
    reply: &Reply,
) {
    let done = done.clone();
    let reply = reply.clone();
    let window = window.clone();
    button.connect_clicked(move |_| {
        respond(&done, &reply, Response::Cancelled);
        window.close();
    });
}

fn wire_cancel(window: &gtk::Window, done: &Rc<Cell<bool>>, reply: &Reply) {
    let done = done.clone();
    let reply = reply.clone();
    window.connect_close_request(move |_| {
        respond(&done, &reply, Response::Cancelled);
        glib::Propagation::Proceed
    });
}

fn wire_escape(window: &gtk::Window, done: &Rc<Cell<bool>>, reply: &Reply) {
    let key = gtk::EventControllerKey::new();
    key.set_propagation_phase(gtk::PropagationPhase::Capture);
    let done = done.clone();
    let reply = reply.clone();
    let window_c = window.clone();
    key.connect_key_pressed(move |_, key, _, _| {
        if key == gtk::gdk::Key::Escape {
            respond(&done, &reply, Response::Cancelled);
            window_c.close();
            return glib::Propagation::Stop;
        }
        glib::Propagation::Proceed
    });
    window.add_controller(key);
}
