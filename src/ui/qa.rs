//! Opt-in native regression fixture. All mutations are in-memory; run with the
//! offline RPC fixture and an isolated PI_DESKTOP_CONFIG, not a live session.
use std::rc::Rc;
use std::time::Duration;

use gtk::prelude::*;
use serde_json::{json, Value};

use super::AppUi;
use crate::config::ViewMode;

pub(super) fn schedule(ui: &Rc<AppUi>) {
    let Ok(path) = std::env::var("PI_DESKTOP_QA_REPORT") else { return };
    let ui = ui.clone();
    glib::timeout_add_local_once(Duration::from_secs(3), move || {
        glib::spawn_future_local(async move {
            let scenario = std::env::var("PI_DESKTOP_QA_SCENARIO").unwrap_or_else(|_| "code".into());
            ui.window.unmaximize();
            if let Ok(size) = std::env::var("PI_DESKTOP_QA_SIZE") {
                let mut parts = size.split('x');
                let width = parts.next().and_then(|value| value.parse::<i32>().ok()).unwrap_or(720);
                let height = parts.next().and_then(|value| value.parse::<i32>().ok()).unwrap_or(700);
                ui.window.set_default_size(width, height);
            } else if std::env::var("PI_DESKTOP_QA_SMALL").is_ok() {
                ui.window.set_default_size(420, 700);
            } else {
                ui.window.set_default_size(1280, if scenario == "gallery" { 720 } else { 600 });
            }
            ui.apply_view(ViewMode::Code, false);
            let mut messages = vec![
                json!({"role":"user","content":"Check the session metadata, theme contrast, reasoning and rail."}),
                json!({"role":"assistant","content":[{"type":"text","text":"## Native UI checks\n\nThe selected themes are preserved when switching appearance.\n\n```rust\nlet theme = preferences.dark;\nprintln!(\"Readable code on every theme\");\n```\n\n[Pi documentation](https://pi.dev)"}]}),
                json!({"role":"toolResult","tool":"bash","toolCallId":"qa-tool","content":[{"type":"text","text":"Checks passed. Tool output remains readable on the card surface."}]}),
            ];
            if matches!(scenario.as_str(), "gallery" | "viewer") {
                let first = crate::images::encode_base64(include_bytes!("../../resources/brand/logo-256.png"));
                let second = crate::images::encode_base64(include_bytes!("../../resources/brand/mark-on-dark-256.png"));
                messages = vec![json!({"role":"assistant","content":[
                    {"type":"text","text":"## Images in the conversation\n\nThe gallery belongs between these two paragraphs."},
                    {"type":"image","mimeType":"image/png","data":first},
                    {"type":"image","mimeType":"image/png","data":second},
                    {"type":"text","text":"Source: Pi Desktop artwork."}
                ]})];
            } else if let Ok(path) = std::env::var("PI_DESKTOP_QA_IMAGE") {
                messages.push(json!({"role":"assistant","content":[{"type":"text","text":format!("Here is the generated plot.\n\n```pi-image\n{{\"path\":\"{path}\",\"alt\":\"generated plot\"}}\n```")}]}));
            }
            if std::env::var("PI_DESKTOP_QA_CLARIFY").is_ok() {
                messages.push(json!({"role":"assistant","content":[{"type":"text","text":"I need one detail before writing the migration.\n\n```pi-clarify\n{\"question\":\"Which database should the migration target?\",\"options\":[\"SQLite\",\"PostgreSQL\"]}\n```"}]}));
            }
            ui.chat.load_messages(&messages);
            if let Ok(paths) = std::env::var("PI_DESKTOP_QA_ATTACH") {
                let paths: Vec<String> = paths
                    .split(',')
                    .map(str::trim)
                    .filter(|path| !path.is_empty())
                    .map(str::to_string)
                    .collect();
                ui.composer.append_paths(&paths);
            }
            ui.on_extension_ui(&json!({
                "type":"extension_ui_request","method":"setWidget","widgetKey":"subagent-async","widgetPlacement":"aboveEditor",
                "widgetLines":[format!("{}{}", crate::subagents::WIDGET_PREFIX, json!({
                    "kind":"pi-subagents.async-status-snapshot","version":1,
                    "runs":[{"id":"qa-worker","label":"worker","state":"running","children":[{
                        "id":"qa-step","label":"Review UI","state":"running","model":"openai/gpt-6.1-sol","thinking":"high",
                        "activity":{"turnCount":17,"toolCount":17,"currentTool":"read"}
                    }]}]
                }))]
            }));
            ui.composer.focus();
            glib::timeout_future(Duration::from_millis(200)).await;
            let theme_button = descendants(ui.header.root.upcast_ref()).into_iter()
                .find(|widget| widget.has_css_class("appearance-toggle")).unwrap().downcast::<gtk::Button>().unwrap();
            let before = crate::tokens::active("pageBg");
            theme_button.emit_clicked();
            let opposite = crate::tokens::active("pageBg");
            theme_button.emit_clicked();
            let restored = crate::tokens::active("pageBg");
            let mut resize = Vec::new();
            for wanted in [300, 520, 340] {
                ui.rail_width.set(wanted);
                ui.apply_rail_layout();
                glib::timeout_future(Duration::from_millis(180)).await;
                resize.push(json!({"wanted":wanted,"actual":ui.rail_column.width(),"paned":ui.rail_paned.width(),"position":ui.rail_paned.position()}));
            }
            let before_drag = ui.rail_column.width();
            let grip = descendants(ui.rail_column.upcast_ref()).into_iter().find(|widget| widget.has_css_class("rail-grip")).unwrap();
            let controllers = grip.observe_controllers();
            let drag = (0..controllers.n_items()).filter_map(|i| controllers.item(i)).find_map(|item| item.downcast::<gtk::GestureDrag>().ok()).unwrap();
            drag.emit_by_name::<()>("drag-begin", &[&0.0_f64, &0.0_f64]);
            drag.emit_by_name::<()>("drag-update", &[&(-140.0_f64), &0.0_f64]);
            drag.emit_by_name::<()>("drag-end", &[&(-140.0_f64), &0.0_f64]);
            glib::timeout_future(Duration::from_millis(180)).await;
            let after_drag = ui.rail_column.width();
            ui.rail_width.set(340);
            ui.apply_rail_layout();
            if matches!(scenario.as_str(), "chat" | "gallery" | "viewer") {
                ui.apply_view(ViewMode::Chat, false);
            }
            if scenario == "tabs" {
                ui.header.set_session_tabs(true);
                ui.refresh_header();
            }
            if scenario == "reasoning" {
                ui.apply_view(ViewMode::Chat, false);
                glib::timeout_future(Duration::from_millis(200)).await;
                if let Some(popover) = descendants(ui.composer.root.upcast_ref()).into_iter().find(|widget| widget.has_css_class("think-popover")) {
                    popover.set_width_request(390);
                }
                ui.composer.open_thinking();
                if let Some(button) = descendants(ui.composer.root.upcast_ref()).iter()
                    .filter_map(|widget| widget.downcast_ref::<gtk::ToggleButton>())
                    .find(|button| button.has_css_class("reasoning-option") && button.label().as_deref() == Some("Low")) {
                    button.emit_clicked();
                }
            }
            if let Ok(tab) = std::env::var("PI_DESKTOP_QA_SETTINGS_TAB") {
                ui.open_settings_tab(&tab);
            } else if scenario == "settings" {
                ui.open_settings_tab("PERSONALIZATION");
            }
            let mut feature = Value::Null;
            if scenario == "models" {
                button(ui.window.upcast_ref(), |button| button.has_css_class("model-chip")).emit_clicked();
                glib::timeout_future(Duration::from_millis(150)).await;
                let search = descendants(ui.window.upcast_ref()).into_iter()
                    .find(|widget| widget.has_css_class("palette-search-entry") && widget.is_mapped())
                    .and_downcast::<gtk::Entry>().unwrap();
                let focused = gtk::prelude::GtkWindowExt::focus(&ui.window)
                    .is_some_and(|focus| focus == search || focus.is_ancestor(&search))
                    && super::focus_is_text_input(&ui.window);
                let mut end = 0;
                search.insert_text("test/fixture-cheap", &mut end);
                feature = json!({"focused": focused, "text":search.text().as_str(),
                    "matches":descendants(ui.window.upcast_ref()).into_iter().filter(|w| w.is_mapped() && w.has_css_class("palette-model-id")).count()});
            } else if matches!(scenario.as_str(), "gallery" | "viewer") {
                let root = ui.window.upcast_ref();
                let gallery = descendants(root).into_iter().find(|w| w.has_css_class("chat-gallery")).unwrap();
                let next = button(&gallery, |b| b.tooltip_text().as_deref() == Some("Next image"));
                next.emit_clicked();
                if scenario == "viewer" {
                    button(&gallery, |b| b.has_css_class("chat-gallery-image")).emit_clicked();
                    let viewer = window("Image preview");
                    viewer.set_size_request(-1, -1);
                    viewer.set_default_size(390, 520);
                    glib::timeout_future(Duration::from_millis(350)).await;
                    feature = json!({"width":viewer.width(), "height":viewer.height(),
                        "next":bounds(&button(viewer.upcast_ref(), |b| b.tooltip_text().as_deref() == Some("Next image")).upcast(), viewer.upcast_ref())});
                } else {
                    glib::timeout_future(Duration::from_millis(200)).await;
                    feature = json!({"window_width":ui.window.width(), "gallery":bounds(&gallery, root),
                        "next":bounds(next.upcast_ref(), root),
                        "page_two":descendants(&gallery).into_iter().find_map(|w| w.downcast::<gtk::Label>().ok().filter(|l| l.text() == "2 / 2")).is_some()});
                }
            } else if scenario.starts_with("compact-") {
                let (tab, label) = match scenario.as_str() {
                    "compact-memory" => ("MEMORY", "Compact memory"),
                    "compact-system" => ("SOUL", "Compact system prompt"),
                    _ => ("SOUL", "Compact personality"),
                };
                ui.open_settings_tab(tab);
                let settings = window("settings");
                button(settings.upcast_ref(), |b| b.label().as_deref() == Some(label)).emit_clicked();
                let picker = window(label);
                let dropdown = descendants(picker.upcast_ref()).into_iter()
                    .find_map(|w| w.downcast::<gtk::DropDown>().ok()).unwrap();
                dropdown.set_selected(1);
                button(picker.upcast_ref(), |b| b.label().as_deref() == Some("Compact")).emit_clicked();
                for _ in 0..20 {
                    glib::timeout_future(Duration::from_millis(100)).await;
                    let applied = descendants(settings.upcast_ref()).into_iter().any(|w|
                        w.downcast::<gtk::Label>().is_ok_and(|l| l.text().contains("characters. Review and save")));
                    if applied {
                        feature = json!({"applied":true});
                        break;
                    }
                }
            } else if scenario == "updates" {
                ui.open_settings_tab("UPDATES");
                let settings = window("settings");
                button(settings.upcast_ref(), |b| b.label().as_deref() == Some("Check now")).emit_clicked();
                for _ in 0..50 {
                    glib::timeout_future(Duration::from_millis(100)).await;
                    if descendants(settings.upcast_ref()).into_iter().any(|w| w.downcast::<gtk::LinkButton>()
                        .is_ok_and(|b| b.label().as_deref() == Some("Open release on GitHub"))) {
                        feature = json!({"release_link":true});
                        break;
                    }
                }
            }
            glib::timeout_future(Duration::from_millis(450)).await;
            let root: &gtk::Widget = ui.window.upcast_ref();
            let widgets = descendants(root);
            let controls: Vec<Value> = widgets.iter().filter(|widget|
                widget.is_mapped() && ["model-chip","mode-switch","mode-option","settings-button","reasoning-option","rail-toggle"]
                    .iter().any(|class| widget.has_css_class(class)))
                .map(|widget| json!({"classes":widget.css_classes().iter().map(|name| name.to_string()).collect::<Vec<_>>(),"bounds":bounds(widget,root)})).collect();
            let labels: Vec<String> = widgets.iter().filter_map(|widget| widget.downcast_ref::<gtk::Label>())
                .map(|label| label.text().to_string()).collect();
            let horizontal_overflow = [ui.header.root.upcast_ref::<gtk::Widget>(), ui.stack.upcast_ref(), ui.rail_column.upcast_ref()]
                .iter().filter(|widget| widget.is_mapped()).any(|widget| widget.compute_bounds(root)
                    .is_some_and(|rect| rect.x() < 0.0 || rect.x() + rect.width() > ui.window.width() as f32 + 1.0));
            let report = json!({
                "window":[ui.window.width(),ui.window.height()], "scenario":scenario,
                "feature":feature,
                "theme_round_trip":{"before":before,"opposite":opposite,"restored":restored},
                "rail_resizes":resize,"rail":bounds(ui.rail_column.upcast_ref(),root),
                "drag_handler":{"before":before_drag,"after":after_drag},
                "controls":controls,
                "raw_snapshot_visible":labels.iter().any(|text| text.contains(crate::subagents::WIDGET_PREFIX)),
                "connected_visible":ui.status.conn_text.is_visible() && ui.status.conn_text.is_mapped(),
                "tabs_visible":widgets.iter().any(|widget| widget.has_css_class("session-tabs") && widget.is_mapped()),
                "code_foreground":crate::tokens::active("mdCodeBlock"),"card_background":crate::tokens::active("cardBg"),
                "settings_visible":ui.header.settings.is_mapped(),
                "horizontal_overflow":horizontal_overflow,
                "popovers":widgets.iter().filter(|widget| widget.is::<gtk::Popover>()).map(|widget| json!({"mapped":widget.is_mapped(),"visible":widget.is_visible(),"bounds":bounds(widget,root)})).collect::<Vec<_>>(),
                "text":["Sub-agent JSON is parsed into the rail, not hidden in the composer."],
            });
            std::fs::write(&path, serde_json::to_string_pretty(&report).unwrap()).expect("write QA report");
        });
    });
}

fn button(root: &gtk::Widget, predicate: impl Fn(&gtk::Button) -> bool) -> gtk::Button {
    descendants(root).into_iter().filter_map(|w| w.downcast::<gtk::Button>().ok())
        .find(predicate).unwrap()
}

fn window(title: &str) -> gtk::Window {
    let windows = gtk::Window::toplevels();
    (0..windows.n_items()).filter_map(|i| windows.item(i).and_downcast::<gtk::Window>())
        .find(|w| w.title().as_deref() == Some(title) && w.is_visible()).unwrap()
}

fn descendants(root: &gtk::Widget) -> Vec<gtk::Widget> {
    let mut widgets = vec![root.clone()];
    let mut index = 0;
    while index < widgets.len() {
        let mut child = widgets[index].first_child();
        while let Some(widget) = child {
            child = widget.next_sibling();
            widgets.push(widget);
        }
        index += 1;
    }
    widgets
}

fn bounds(widget: &gtk::Widget, root: &gtk::Widget) -> Value {
    widget.compute_bounds(root).map(|rect| json!([rect.x(),rect.y(),rect.width(),rect.height()])).unwrap_or(Value::Null)
}
