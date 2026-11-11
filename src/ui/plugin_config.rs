use std::thread;

use gtk::prelude::*;

use crate::plugins;
use crate::state::Transport;

use super::fit;

pub fn show(
    parent: &impl IsA<gtk::Window>,
    transport: Transport,
    title: &str,
    path: &str,
    json: bool,
) {
    let window = gtk::Window::new();
    window.set_transient_for(Some(parent));
    window.set_modal(true);
    window.set_destroy_with_parent(true);
    window.set_decorated(false);
    window.add_css_class("plugin-config-window");
    fit::fit(&window, 720, 560);
    if let Some(app) = parent.as_ref().application() {
        window.set_application(Some(&app));
    }
    window.set_title(Some(&format!("Configure {title}")));

    let heading = gtk::Label::new(Some(&format!("CONFIGURE {title}")));
    heading.add_css_class("settings-title");
    heading.set_xalign(0.0);
    let path_label = gtk::Label::new(Some(path));
    path_label.add_css_class("plugin-config-path");
    path_label.set_xalign(0.0);
    path_label.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
    path_label.set_tooltip_text(Some(path));
    let titles = gtk::Box::new(gtk::Orientation::Vertical, 1);
    titles.set_hexpand(true);
    titles.append(&heading);
    titles.append(&path_label);

    let controls = gtk::WindowControls::new(gtk::PackType::End);
    controls.set_valign(gtk::Align::Center);

    let header = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    header.add_css_class("settings-header");
    header.append(&titles);
    header.append(&controls);

    let text = gtk::TextView::new();
    text.add_css_class("plugin-config-text");
    text.set_monospace(true);
    text.set_wrap_mode(gtk::WrapMode::None);
    text.set_left_margin(12);
    text.set_right_margin(12);
    text.set_top_margin(10);
    text.set_bottom_margin(10);

    let scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Automatic)
        .vscrollbar_policy(gtk::PolicyType::Automatic)
        .vexpand(true)
        .child(&text)
        .build();

    let status = gtk::Label::new(Some("loading..."));
    status.add_css_class("plugin-config-status");
    status.set_xalign(0.0);
    status.set_hexpand(true);
    status.set_ellipsize(gtk::pango::EllipsizeMode::End);

    let cancel = gtk::Button::with_label("Cancel");
    cancel.add_css_class("settings-cancel");
    let save = gtk::Button::with_label("Save");
    save.add_css_class("settings-save");
    save.set_sensitive(false);

    let footer = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    footer.add_css_class("settings-footer");
    footer.append(&status);
    footer.append(&cancel);
    footer.append(&save);

    let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
    root.append(&header);
    root.append(&scroll);
    root.append(&footer);
    window.set_child(Some(&root));

    {
        let window = window.clone();
        cancel.connect_clicked(move |_| window.close());
    }

    let (load_tx, load_rx) = async_channel::bounded(1);
    let load_transport = transport.clone();
    let load_path = path.to_string();
    thread::Builder::new()
        .name("plugin-config-read".into())
        .spawn(move || {
            let _ = load_tx.send_blocking(plugins::read_config(&load_transport, &load_path));
        })
        .ok();

    {
        let text = text.clone();
        let status = status.clone();
        let save = save.clone();
        glib::spawn_future_local(async move {
            match load_rx.recv().await {
                Ok(Ok(value)) => {
                    let value = if json && value.trim().is_empty() {
                        "{\n}\n".to_string()
                    } else {
                        value
                    };
                    text.buffer().set_text(&value);
                    status.set_text("");
                    save.set_sensitive(true);
                }
                Ok(Err(error)) => {
                    status.set_text(&error);
                    status.add_css_class("error");
                }
                Err(_) => {
                    status.set_text("could not load settings");
                    status.add_css_class("error");
                }
            }
        });
    }

    {
        let window = window.clone();
        let text = text.clone();
        let status = status.clone();
        let save_c = save.clone();
        let transport = transport.clone();
        let path = path.to_string();
        save.connect_clicked(move |_| {
            let buffer = text.buffer();
            let value = buffer
                .text(&buffer.start_iter(), &buffer.end_iter(), true)
                .to_string();
            if json {
                if let Err(error) = serde_json::from_str::<serde_json::Value>(&value) {
                    status.set_text(&format!("invalid JSON: {error}"));
                    status.add_css_class("error");
                    return;
                }
            }
            status.remove_css_class("error");
            status.set_text("saving...");
            save_c.set_sensitive(false);

            let (tx, rx) = async_channel::bounded(1);
            let transport = transport.clone();
            let path = path.clone();
            let value = value.clone();
            thread::Builder::new()
                .name("plugin-config-write".into())
                .spawn(move || {
                    let _ = tx.send_blocking(plugins::write_config(&transport, &path, &value));
                })
                .ok();

            let window = window.clone();
            let status = status.clone();
            let save_c = save_c.clone();
            glib::spawn_future_local(async move {
                match rx.recv().await {
                    Ok(Ok(())) => window.close(),
                    Ok(Err(error)) => {
                        status.set_text(&error);
                        status.add_css_class("error");
                        save_c.set_sensitive(true);
                    }
                    Err(_) => {
                        status.set_text("could not save settings");
                        status.add_css_class("error");
                        save_c.set_sensitive(true);
                    }
                }
            });
        });
    }

    window.present();
}
