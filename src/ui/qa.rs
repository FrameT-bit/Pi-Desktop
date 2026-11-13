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
            ui.window.set_default_size(1280, 600);
            ui.apply_view(ViewMode::Code, false);
            ui.chat.load_messages(&[
                json!({"role":"user","content":"Check the session metadata, theme contrast, reasoning and rail."}),
                json!({"role":"assistant","content":[{"type":"text","text":"## Native UI checks\n\nThe selected themes are preserved when switching appearance.\n\n```rust\nlet theme = preferences.dark;\nprintln!(\"Readable code on every theme\");\n```\n\n[Pi documentation](https://pi.dev)"}]}),
                json!({"role":"toolResult","tool":"bash","toolCallId":"qa-tool","content":[{"type":"text","text":"Checks passed. Tool output remains readable on the card surface."}]}),
            ]);
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
            if scenario == "chat" { ui.apply_view(ViewMode::Chat, false); }
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
            if scenario == "settings" { ui.open_settings_tab("PERSONALIZATION"); }
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
