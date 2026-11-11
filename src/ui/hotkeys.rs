
use gtk::prelude::*;

const GROUPS: &[(&str, &[(&str, &str)])] = &[
    (
        "COMPOSITION",
        &[
            ("Enter", "send; Shift+Enter steers a running turn"),
            ("Shift+Enter", "new line"),
            ("Alt+Enter", "queue a follow-up"),
            ("Ctrl+Shift+Enter", "run as bash outside context"),
            ("↑ / ↓ in the empty field", "prompt history"),
            ("/", "command and skill palette"),
            ("@", "skills and workspace files"),
            ("/checkpoints · /rollback", "view and restore files before write/edit"),
            ("/plan · /todos", "read-only plan mode and plan progress"),
            ("/guard", "permission mode: ask, trusted or plan"),
            ("/memory", "see/save memory between sessions"),
            ("/sessions", "search the content of past sessions"),
            ("Ctrl+V", "paste an image as an attachment"),
            ("Esc", "stop and clear the queue (text returns to the field)"),
        ],
    ),
    (
        "NAVIGATION",
        &[
            ("Ctrl+K", "commands, sessions and actions"),
            ("Ctrl+F", "search in chat"),
            ("Ctrl+Shift+F", "search for sessions"),
            ("Shift+Tab", "change persona"),
            ("Ctrl+P", "next model"),
            ("Ctrl+T", "next reasoning level"),
            ("Ctrl+N", "new session"),
            ("Ctrl+,", "settings"),
            ("Ctrl+/", "this sheet"),
            ("Ctrl+Shift+G", "Git review: diff, stage and commit"),
        ],
    ),
    (
        "SESSION AND MESSAGE",
        &[
            ("Right-click", "branch / copy"),
            ("Ctrl+C in selection", "copy text"),
        ],
    ),
];

pub fn show(parent: &impl IsA<gtk::Window>) {
    let window = gtk::Window::new();
    window.set_transient_for(Some(parent));
    window.set_modal(true);
    window.set_destroy_with_parent(true);
    window.set_decorated(false);
    if let Some(app) = parent.as_ref().application() {
        window.set_application(Some(&app));
    }
    window.add_css_class("pi-dialog");
    window.set_title(Some("Shortcuts"));
    crate::ui::fit::fit(&window, 620, 620);

    let title = gtk::Label::new(Some("Shortcuts"));
    title.add_css_class("dialog-title");
    title.set_xalign(0.0);
    title.set_hexpand(true);

    let escape = gtk::Label::new(Some("esc"));
    escape.add_css_class("dialog-note");

    let header = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    header.add_css_class("pi-dialog-header");
    header.append(&title);
    header.append(&escape);

    let body = gtk::Box::new(gtk::Orientation::Vertical, 14);
    body.add_css_class("pi-dialog-body");
    body.set_vexpand(true);

    for (group, rows) in GROUPS {
        let label = gtk::Label::new(Some(group));
        label.add_css_class("dialog-note");
        label.set_xalign(0.0);
        body.append(&label);

        for (keys, description) in rows.iter() {
            let line = gtk::Box::new(gtk::Orientation::Horizontal, 12);
            let key = gtk::Label::new(Some(keys));
            key.add_css_class("palette-name");
            key.set_xalign(0.0);
            key.set_size_request(220, -1);
            let desc = gtk::Label::new(Some(description));
            desc.add_css_class("palette-desc");
            desc.set_xalign(0.0);
            desc.set_hexpand(true);
            desc.set_wrap(true);
            line.append(&key);
            line.append(&desc);
            body.append(&line);
        }
    }

    let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
    root.add_css_class("pi-dialog");
    root.append(&header);
    root.append(&body);
    window.set_child(Some(&root));

    let key = gtk::EventControllerKey::new();
    key.set_propagation_phase(gtk::PropagationPhase::Capture);
    let window_c = window.clone();
    key.connect_key_pressed(move |_, key, _, _| match key {
        gtk::gdk::Key::Escape | gtk::gdk::Key::q | gtk::gdk::Key::Q => {
            window_c.close();
            glib::Propagation::Stop
        }
        _ => glib::Propagation::Proceed,
    });
    window.add_controller(key);

    window.present();
}
