mod artifacts;
mod catalog;
mod config;
mod diff;
mod extension_ui;
mod files;
mod git;
mod history;
mod icons;
mod images;
mod logos;
mod loops;
mod markdown;
mod mcp;
mod mentions;
mod plugins;
mod prompt_history;
mod providers;
mod rpc;
mod session_ops;
mod state;
mod title;
mod theme;
mod theme_catalog;
mod tokens;
mod tree;
mod ui;
mod updater;
mod websearch;

use gtk::prelude::*;

const APP_ID: &str = "io.github.FrameT.PiDesktop";

fn main() -> glib::ExitCode {
    // Child processes (Pi, git, curl) otherwise inherit Portuguese libc messages
    // from the user's locale. Keep locale data for character handling, but force
    // diagnostics to English.
    std::env::set_var("LC_MESSAGES", "C");
    std::env::set_var("LANGUAGE", "C");

    let argv: Vec<String> = std::env::args().skip(1).collect();
    if argv.iter().any(|arg| arg == "--version" || arg == "-V") {
        println!("pi-desktop {}", env!("CARGO_PKG_VERSION"));
        return glib::ExitCode::SUCCESS;
    }
    let cli = config::Config::from_args(&argv);
    if let Some(error) = cli.error.as_deref() {
        eprintln!("pi-desktop: {error} (ignored)");
    }

    for arg in &cli.remaining {
        let path = std::path::PathBuf::from(arg);
        let looks_like_path = path.is_file()
            || arg.starts_with('/')
            || arg.starts_with("./");
        if looks_like_path {
            eprintln!(
                "pi-desktop: unknown file argument (Pi read by way): {}",
                path.display()
            );
        } else {
            eprintln!("pi-desktop: unknown unknown argument: {arg}");
        }
    }

    let mut builder = adw::Application::builder().application_id(APP_ID);
    if std::env::var("PI_DESKTOP_SNAPSHOT").is_ok() || cli.wants_instance() {
        builder = builder.flags(gtk::gio::ApplicationFlags::NON_UNIQUE);
    }
    let app = builder.build();

    app.connect_startup(|_| {
        glib::set_application_name("pi");
        gtk::Window::set_default_icon_name("pi-desktop");
    });

    let cli_for_activate = cli.clone();
    app.connect_activate(move |app| {
        let theme = theme::init();
        icons::attach_theme(&theme);
        ui::present(app, theme, &cli_for_activate);
        maybe_snapshot(app);
    });

    let program = std::env::args()
        .next()
        .unwrap_or_else(|| "pi-desktop".to_string());
    app.run_with_args(&[program])
}

fn maybe_snapshot(app: &adw::Application) {
    let Ok(path) = std::env::var("PI_DESKTOP_SNAPSHOT") else {
        return;
    };
    let delay = std::env::var("PI_DESKTOP_SNAPSHOT_DELAY")
        .ok()
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or(3);
    let begin = std::time::Instant::now();
    let wait = std::time::Duration::from_secs(delay as u64);
    let app = app.clone();
    glib::timeout_add_local(std::time::Duration::from_millis(300), move || {
        if begin.elapsed() < wait {
            return glib::ControlFlow::Continue;
        }
        let mut captured = false;
        let focus = std::env::var("PI_DESKTOP_SNAPSHOT_TARGET").unwrap_or_default();
        let main = app
            .windows()
            .into_iter()
            .find(|window| window.is::<adw::ApplicationWindow>());

        if focus == "popover" {
            if let Some(window) = main.as_ref() {
                let target = window
                    .child()
                    .unwrap_or_else(|| window.clone().upcast::<gtk::Widget>());
                let mut popovers = Vec::new();
                collect_popovers(&target, &mut popovers);
                for popover in popovers.iter().filter(|popover| popover.is_mapped()) {
                    let widget = popover.upcast_ref::<gtk::Widget>();
                    if render_widget(widget, window, &path) {
                        std::process::exit(0);
                    }
                }
            }
            return snapshot_timeout(begin, wait);
        }

        if focus == "dialog" {
            let toplevels = gtk::Window::toplevels();
            for index in 0..toplevels.n_items() {
                let Some(item) = toplevels.item(index) else {
                    continue;
                };
                let Ok(window) = item.downcast::<gtk::Window>() else {
                    continue;
                };
                if !window.is_mapped() {
                    continue;
                }
                if main.as_ref().is_some_and(|main| main == &window) {
                    continue;
                }
                if snapshot(&window, &path) {
                    std::process::exit(0);
                }
            }
            return snapshot_timeout(begin, wait);
        }

        if let Some(window) = main.as_ref() {
            captured = snapshot(window, &path);
        }
        let toplevels = gtk::Window::toplevels();
        for index in 0..toplevels.n_items() {
            let Some(item) = toplevels.item(index) else {
                continue;
            };
            let Ok(window) = item.downcast::<gtk::Window>() else {
                continue;
            };
            if !window.is_mapped() {
                continue;
            }
            if main.as_ref().is_some_and(|main| main == &window) {
                continue;
            }
            snapshot(&window, &format!("{path}.dialog{index}.png"));
        }
        if captured {
            std::process::exit(0);
        }
        snapshot_timeout(begin, wait)
    });
}

fn snapshot_timeout(begin: std::time::Instant, wait: std::time::Duration) -> glib::ControlFlow {
    if begin.elapsed() > wait + std::time::Duration::from_secs(30) {
        eprintln!("Snapshot: after 30s");
        std::process::exit(1);
    }
    glib::ControlFlow::Continue
}

fn snapshot(window: &gtk::Window, path: &str) -> bool {
    let target = window
        .child()
        .unwrap_or_else(|| window.clone().upcast::<gtk::Widget>());
    let ok = render_widget(&target, window, path);

    let mut popovers = Vec::new();
    collect_popovers(&target, &mut popovers);
    for (index, popover) in popovers.iter().enumerate() {
        if !popover.is_mapped() {
            continue;
        }
        if let Some(child) = popover.child() {
            render_widget(&child, window, &format!("{path}.popover{index}.png"));
        }
    }

    ok
}

fn render_widget(widget: &gtk::Widget, window: &gtk::Window, path: &str) -> bool {
    let width = widget.width().max(1) as f64;
    let height = widget.height().max(1) as f64;
    let paintable = gtk::WidgetPaintable::new(Some(widget));
    let snapshot = gtk::Snapshot::new();
    paintable.snapshot(&snapshot, width, height);
    let Some(node) = snapshot.to_node() else {
        return false;
    };
    let Some(surface) = window.surface() else {
        return false;
    };
    let Some(renderer) = gtk::gsk::Renderer::for_surface(&surface) else {
        return false;
    };
    if !renderer.is_realized() && renderer.realize(Some(&surface)).is_err() {
        return false;
    }
    let texture = renderer.render_texture(&node, None);
    texture.save_to_png(path).is_ok()
}

fn collect_popovers(widget: &gtk::Widget, out: &mut Vec<gtk::Popover>) {
    let mut current = widget.first_child();
    while let Some(child) = current {
        if let Ok(popover) = child.clone().downcast::<gtk::Popover>() {
            out.push(popover);
        }
        collect_popovers(&child, out);
        current = child.next_sibling();
    }
}
