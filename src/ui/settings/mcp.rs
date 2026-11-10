
use std::cell::RefCell;
use std::rc::Rc;

use gtk::prelude::*;

use crate::mcp::{self, McpServer};
use crate::state::resolve_pi_bin;
use crate::ui::dialog;
use crate::ui::settings::pages::{block, dropdown, row, text_entry};

pub fn page(window: &gtk::Window, bin: &str) -> gtk::Box {
    let page = gtk::Box::new(gtk::Orientation::Vertical, 0);
    page.add_css_class("settings-page-content");

    let status = gtk::Label::new(None);
    status.add_css_class("settings-error");
    status.set_xalign(0.0);
    status.set_wrap(true);
    status.set_visible(false);

    let list = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let list_block = block(&page, "SERVANTS");

    let list_header = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let spinner_text = gtk::Label::new(Some("reading from pi..."));
    spinner_text.add_css_class("settings-item-dec");
    spinner_text.set_visible(false);
    let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    spacer.set_hexpand(true);
    let refresh_button = gtk::Button::with_label("refresh");
    refresh_button.add_css_class("settings-mini");
    list_header.append(&spinner_text);
    list_header.append(&spacer);
    list_header.append(&refresh_button);
    list_block.append(&list_header);
    list_block.append(&list);


    let add_block = block(&page, "Add");

    let modes: &'static [&'static str] = &["http", "stdio"];
    let mode = dropdown(modes, &["HTTP (url)", "Stdio (command)"], 0);
    add_block.append(&row(
        "transporte",
        "HTTP uses --url; stdio runs a local command",
        &mode,
    ));

    let name = text_entry("", 200);
    add_block.append(&row("name", "short identifier, e.g.: firecrawl", &name));

    let url = text_entry("", 320);
    add_block.append(&row(
        "Url (http)",
        "HTTP streaming server, e.g.: https://example.com/mcp",
        &url,
    ));

    let headers_view = gtk::TextView::new();
    headers_view.add_css_class("settings-editor");
    headers_view.set_monospace(true);
    let headers_scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vscrollbar_policy(gtk::PolicyType::Automatic)
        .min_content_height(70)
        .child(&headers_view)
        .build();
    headers_scroll.add_css_class("settings-view");
    add_block.append(&row(
        "headers",
        "One per line: KEY=VALUE (Authorization, X-Api-Key...)",
        &headers_scroll,
    ));

    let bearer = text_entry("", 240);
    add_block.append(&row(
        "bearer (env var)",
        "name of the environment variable; the header turns Authorization: Bearer",
        &bearer,
    ));

    let command = text_entry("", 320);
    add_block.append(&row(
        "command (stdio)",
        "e.g.: npx -y @modelcontextprotocol/server-filesystem /tmp",
        &command,
    ));

    let env_view = gtk::TextView::new();
    env_view.add_css_class("settings-editor");
    env_view.set_monospace(true);
    let env_scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vscrollbar_policy(gtk::PolicyType::Automatic)
        .min_content_height(70)
        .child(&env_view)
        .build();
    env_scroll.add_css_class("settings-view");
    add_block.append(&row(
        "environment (stdio)",
        "one per line: KEY=VALUE",
        &env_scroll,
    ));

    let description = text_entry("", 320);
    add_block.append(&row(
        "description",
        "what the server offers (go to system prompt)",
        &description,
    ));

    let add = gtk::Button::with_label("add server");
    add.add_css_class("dialog-accept");
    let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    actions.add_css_class("settings-actions");
    actions.append(&add);
    add_block.append(&actions);

    page.append(&status);


    let reload: Rc<RefCell<Option<Rc<dyn Fn()>>>> = Rc::new(RefCell::new(None));
    let window = window.clone();
    let bin = bin.to_string();

    let render = Rc::new({
        let list = list.clone();
        let status = status.clone();
        let spinner_text = spinner_text.clone();
        let refresh_button = refresh_button.clone();
        let reload = reload.clone();
        let window = window.clone();
        let bin = bin.clone();
        move |servers: Vec<McpServer>, errors: Vec<String>| {
            spinner_text.set_visible(false);
            refresh_button.set_sensitive(true);
            while let Some(child) = list.first_child() {
                list.remove(&child);
            }
            if errors.is_empty() {
                status.set_visible(false);
            } else {
                status.set_text(&errors.join(" · "));
                status.set_visible(true);
            }
            if servers.is_empty() {
                let empty = gtk::Label::new(Some("no MCP server configured"));
                empty.add_css_class("settings-empty");
                empty.set_xalign(0.0);
                list.append(&empty);
                return;
            }
            for server in servers {
                let line = gtk::Box::new(gtk::Orientation::Horizontal, 10);
                line.add_css_class("settings-item");

                let dot = gtk::Box::new(gtk::Orientation::Horizontal, 0);
                dot.add_css_class("settings-dot");
                dot.add_css_class(if server.ok() { "ok" } else { "bad" });

                let info = gtk::Box::new(gtk::Orientation::Vertical, 1);
                info.set_hexpand(true);
                let title = gtk::Label::new(Some(&format!("{} · {}", server.name, server.status())));
                title.add_css_class("settings-item-titles");
                title.set_xalign(0.0);
                let tool_preview = server
                    .tools
                    .iter()
                    .take(3)
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", ");
                let detail_text = match &server.error {
                    Some(error) => format!("{} — {}", server.transport, truncate(error, 140)),
                    None => {
                        let scope = if server.scope.is_empty() {
                            "global".to_string()
                        } else {
                            server.scope.clone()
                        };
                        if tool_preview.is_empty() {
                            format!(
                                "{scope} · {} · {} tool(s) · {}",
                                server.transport,
                                server.tools.len(),
                                server.exposure
                            )
                        } else {
                            format!(
                                "{scope} · {} · {} tool(s) · {} · {}",
                                server.transport,
                                server.tools.len(),
                                server.exposure,
                                tool_preview
                            )
                        }
                    }
                };
                let detail = gtk::Label::new(Some(&detail_text));
                detail.add_css_class("settings-item-dec");
                detail.set_xalign(0.0);
                detail.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
                detail.set_tooltip_text(Some(
                    server
                        .error
                        .as_deref()
                        .map(str::to_string)
                        .unwrap_or_else(|| {
                            if server.tools.is_empty() {
                                server.transport.clone()
                            } else {
                                format!("tools: {}", server.tools.join(", "))
                            }
                        })
                        .as_str(),
                ));
                info.append(&title);
                info.append(&detail);

                let is_http = server.transport.starts_with("http://")
                    || server.transport.starts_with("https://");
                let login = gtk::Button::with_label("login");
                login.add_css_class("settings-mini");
                login.set_tooltip_text(Some("OAuth: Opens the browser via `pi mcp login`"));
                login.set_visible(is_http);

                let remove = gtk::Button::with_label("remove");
                remove.add_css_class("settings-mini");
                remove.add_css_class("danger");

                line.append(&dot);
                line.append(&info);
                line.append(&login);
                line.append(&remove);
                list.append(&line);

                {
                    let status = status.clone();
                    let reload = reload.clone();
                    let bin = bin.clone();
                    let server_name = server.name.clone();
                    login.connect_clicked(move |_| {
                        let status = status.clone();
                        let reload = reload.clone();
                        status.remove_css_class("settings-error");
                        status.add_css_class("settings-ok");
                        status.set_text(&format!(
                            "login for {server_name}: finish in the browser..."
                        ));
                        status.set_visible(true);
                        let server_name = server_name.clone();
                        run_pi_async(bin.clone(), mcp::login_args(&server_name), move |result| {
                            match result {
                                Ok(output) => {
                                    let text = output.trim();
                                    status.remove_css_class("settings-error");
                                    status.add_css_class("settings-ok");
                                    status.set_text(if text.is_empty() {
                                        "login complete"
                                    } else {
                                        text
                                    });
                                    status.set_visible(true);
                                    if let Some(reload) = reload.borrow().as_ref() {
                                        reload();
                                    }
                                }
                                Err(err) => {
                                    status.remove_css_class("settings-ok");
                                    status.add_css_class("settings-error");
                                    status.set_text(&format!("login failed: {err}"));
                                    status.set_visible(true);
                                }
                            }
                        });
                    });
                }

                {
                    let status = status.clone();
                    let reload = reload.clone();
                    let window = window.clone();
                    let bin = bin.clone();
                    let server_name = server.name.clone();
                    let detail = server.transport.clone();
                    remove.connect_clicked(move |_| {
                        let status = status.clone();
                        let reload = reload.clone();
                        let bin = bin.clone();
                        let server_name = server_name.clone();
                        dialog::confirm(
                            &window,
                            "remove mcp server?",
                            &format!("{server_name}\n{detail}\n\nremove from mcp.json via `pi mcp remove`"),
                            "remove",
                            move || {
                                let status = status.clone();
                                let reload = reload.clone();
                                status.set_text("removing...");
                                status.set_visible(true);
                                run_pi_async(
                                    bin.clone(),
                                    mcp::remove_args(&server_name),
                                    move |result| match result {
                                        Ok(_) => {
                                            status.set_visible(false);
                                            if let Some(reload) = reload.borrow().as_ref() {
                                                reload();
                                            }
                                        }
                                        Err(err) => {
                                            status.set_text(&err);
                                            status.set_visible(true);
                                        }
                                    },
                                );
                            },
                        );
                    });
                }
            }
        }
    });

    let start_load = {
        let render = render.clone();
        let status = status.clone();
        let spinner_text = spinner_text.clone();
        let refresh_button = refresh_button.clone();
        let bin = bin.clone();
        Rc::new(move || {
            let spinner_text = spinner_text.clone();
            let refresh_button = refresh_button.clone();
            spinner_text.set_visible(true);
            refresh_button.set_sensitive(false);
            let render = render.clone();
            let status = status.clone();
            let (tx, rx) =
                async_channel::bounded::<Result<(Vec<McpServer>, Vec<String>), String>>(1);
            let bin = bin.clone();
            std::thread::spawn(move || {
                let args = vec![
                    "mcp".to_string(),
                    "list".to_string(),
                    "--json".to_string(),
                ];
                let result = run_pi_bin(&bin, &args)
                    .map(|text| (mcp::parse_list(&text), mcp::parse_errors(&text)));
                let _ = tx.send_blocking(result);
            });
            glib::spawn_future_local(async move {
                match rx.recv().await {
                    Ok(Ok((servers, errors))) => render(servers, errors),
                    Ok(Err(err)) => {
                        spinner_text.set_visible(false);
                        refresh_button.set_sensitive(true);
                        status.set_text(&err);
                        status.set_visible(true);
                    }
                    Err(_) => {
                        spinner_text.set_visible(false);
                        refresh_button.set_sensitive(true);
                    }
                }
            });
        })
    };

    *reload.borrow_mut() = Some(start_load.clone());
    start_load();

    {
        let start_load = start_load.clone();
        refresh_button.connect_clicked(move |_| start_load());
    }


    {
        let http_widgets = vec![
            url.clone().upcast::<gtk::Widget>(),
            headers_scroll.clone().upcast::<gtk::Widget>(),
            bearer.clone().upcast::<gtk::Widget>(),
        ];
        let stdio_widgets = vec![
            command.clone().upcast::<gtk::Widget>(),
            env_scroll.clone().upcast::<gtk::Widget>(),
        ];
        let apply = move |is_http: bool| {
            for widget in &http_widgets {
                widget.set_visible(is_http);
            }
            for widget in &stdio_widgets {
                widget.set_visible(!is_http);
            }
        };
        apply(true);
        mode.connect_selected_notify(move |dropdown| apply(dropdown.selected() == 0));
    }


    {
        let start_load = start_load.clone();
        let status = status.clone();
        let name = name.clone();
        let url = url.clone();
        let headers_view = headers_view.clone();
        let bearer = bearer.clone();
        let command = command.clone();
        let env_view = env_view.clone();
        let description = description.clone();
        let mode = mode.clone();
        let bin = bin.clone();
        add.connect_clicked(move |_| {
            let server = name.text().trim().to_string();
            if server.is_empty() {
                status.set_text("empty server name");
                status.set_visible(true);
                return;
            }
            let description_text = description.text().trim().to_string();
            let is_http = mode.selected() == 0;

            let mut args = if is_http {
                let url_text = url.text().trim().to_string();
                if url_text.is_empty() {
                    status.set_text("enter the HTTP server URL");
                    status.set_visible(true);
                    return;
                }
                if let Err(message) = crate::websearch::validate_url(&url_text) {
                    status.set_text(&format!("url: {message}"));
                    status.set_visible(true);
                    return;
                }
                let headers = mcp::parse_pairs(&buffer_text(&headers_view));
                mcp::add_url_args(&server, &url_text, &headers, Some(bearer.text().as_str()))
            } else {
                let parts = mcp::parse_command(&command.text());
                if parts.is_empty() {
                    status.set_text("empty command");
                    status.set_visible(true);
                    return;
                }
                let envs = mcp::parse_pairs(&buffer_text(&env_view));
                mcp::add_stdio_args(&server, &envs, &parts)
            };

            if !description_text.is_empty() {
                match args.iter().position(|item| item == "--") {
                    Some(index) => {
                        args.insert(index, description_text);
                        args.insert(index, "--description".to_string());
                    }
                    None => {
                        args.push("--description".to_string());
                        args.push(description_text);
                    }
                }
            }

            status.set_text("adding...");
            status.set_visible(true);
            let status = status.clone();
            let start_load = start_load.clone();
            run_pi_async(bin.clone(), args, move |result| match result {
                Ok(_) => {
                    status.set_visible(false);
                    start_load();
                }
                Err(err) => {
                    status.set_text(&err);
                    status.set_visible(true);
                }
            });
        });
    }

    page
}

fn buffer_text(view: &gtk::TextView) -> String {
    let buffer = view.buffer();
    buffer
        .text(&buffer.start_iter(), &buffer.end_iter(), false)
        .to_string()
}

fn truncate(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let mut out: String = text.chars().take(max).collect();
    out.push('…');
    out
}

fn run_pi_async(bin: String, args: Vec<String>, done: impl FnOnce(Result<String, String>) + 'static) {
    let (tx, rx) = async_channel::bounded::<Result<String, String>>(1);
    std::thread::spawn(move || {
        let _ = tx.send_blocking(run_pi_bin(&bin, &args));
    });
    glib::spawn_future_local(async move {
        if let Ok(result) = rx.recv().await {
            done(result);
        }
    });
}

fn run_pi_bin(bin: &str, args: &[String]) -> Result<String, String> {
    let bin = resolve_pi_bin(bin).map_err(|expected| format!("not found: {expected}"))?;
    let output = std::process::Command::new(bin)
        .args(args)
        .env("PI_CODING_AGENT_DIR", crate::config::pi_agent_dir())
        .stdin(std::process::Stdio::null())
        .output()
        .map_err(|err| format!("failed to run pi: {err}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    if output.status.success() || !stdout.trim().is_empty() {
        return Ok(stdout);
    }
    Err(format!(
        "pi {} come out with {}: {}",
        args.join(" "),
        output.status,
        String::from_utf8_lossy(&output.stderr).trim()
    ))
}
