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
            if let Ok(pass) = std::env::var("PI_DESKTOP_QA_GEOMETRY_PASS") {
                if pass == "save" {
                    ui.window.unmaximize();
                    ui.window.set_default_size(1100, 720);
                    ui.rail_width.set(460);
                    ui.rail_open.set(false);
                    ui.apply_rail_layout();
                    glib::timeout_future(Duration::from_millis(400)).await;
                }
                let report = json!({"size":[ui.window.width(),ui.window.height()],
                    "maximized":ui.window.is_maximized(),"rail_width":ui.rail_width.get(),"rail_open":ui.rail_open.get()});
                std::fs::write(&path, serde_json::to_string_pretty(&report).unwrap()).unwrap();
                if pass == "restore" {
                    ui.window.maximize();
                    glib::timeout_future(Duration::from_millis(400)).await;
                }
                ui.window.close();
                return;
            }
            ui.window.unmaximize();
            if let Ok(size) = std::env::var("PI_DESKTOP_QA_SIZE") {
                let mut parts = size.split('x');
                let width = parts.next().and_then(|value| value.parse::<i32>().ok()).unwrap_or(720);
                let height = parts.next().and_then(|value| value.parse::<i32>().ok()).unwrap_or(700);
                ui.window.set_default_size(width, height);
            } else if std::env::var("PI_DESKTOP_QA_SMALL").is_ok() {
                ui.window.set_default_size(420, 700);
            } else {
                ui.window.set_default_size(1280, if scenario.starts_with("git") || scenario.starts_with("plan-") { 840 } else if scenario == "gallery" { 720 } else { 600 });
            }
            ui.apply_view(ViewMode::Code, false);
            ui.window.present();
            glib::timeout_future(Duration::from_millis(350)).await;
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
                allocate_window(&ui.window);
                glib::timeout_future(Duration::from_millis(180)).await;
                for _ in 0..8 {
                    if (ui.rail_column.width() - wanted).abs() <= 5 { break; }
                    ui.rail_paned.queue_allocate();
                    glib::timeout_future(Duration::from_millis(100)).await;
                }
                resize.push(json!({"wanted":wanted,"actual":ui.rail_column.width(),"paned":ui.rail_paned.width(),"position":ui.rail_paned.position()}));
            }
            let before_drag = ui.rail_column.width();
            let grip = descendants(ui.rail_column.upcast_ref()).into_iter().find(|widget| widget.has_css_class("rail-grip")).unwrap();
            let controllers = grip.observe_controllers();
            let drag = (0..controllers.n_items()).filter_map(|i| controllers.item(i)).find_map(|item| item.downcast::<gtk::GestureDrag>().ok()).unwrap();
            drag.emit_by_name::<()>("drag-begin", &[&0.0_f64, &0.0_f64]);
            drag.emit_by_name::<()>("drag-update", &[&(-140.0_f64), &0.0_f64]);
            drag.emit_by_name::<()>("drag-end", &[&(-140.0_f64), &0.0_f64]);
            allocate_window(&ui.window);
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
                    // Snapshot checks must not dismiss when the operator keeps
                    // typing in another window on the shared Wayland desktop.
                    if let Some(popover) = popover.downcast_ref::<gtk::Popover>() { popover.set_autohide(false); }
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
            if matches!(scenario.as_str(), "live-stream" | "live-wait") {
                ui.chat.clear();
                ui.config.borrow_mut().wait_for_response = scenario == "live-wait";
                ui.send(json!({"id":ui.id(),"type":"prompt","message":"Write 80 numbered short sentences about gardens. Use the word garden in each sentence. No introduction, no tools, no code."}));
                let mut early = false;
                let mut saw_running = false;
                let mut writing = false;
                for _ in 0..500 {
                    glib::timeout_future(Duration::from_millis(100)).await;
                    let running = ui.state.borrow().running;
                    saw_running |= running;
                    writing |= ui.state.borrow().phase.label() == "WRITING";
                    if running {
                        early |= descendants(ui.chat.list.upcast_ref()).iter().any(|w|
                            w.has_css_class("assistant-text") && w.downcast_ref::<gtk::Label>().is_some_and(|l| !l.text().is_empty()));
                    }
                    if saw_running && !running { break; }
                }
                let texts = label_texts(ui.chat.list.upcast_ref());
                feature = json!({"real_provider":true,"early":early,"running_early":saw_running,
                    "running_final":ui.state.borrow().running,"phase":if writing { "WRITING" } else { "THINKING" },
                    "final":texts.iter().any(|text| text.to_lowercase().contains("garden")),
                    "cursor_final":texts.iter().any(|text| text.ends_with('▍'))});
            } else if matches!(scenario.as_str(), "stream" | "thinking" | "wait") {
                ui.chat.clear();
                ui.config.borrow_mut().wait_for_response = scenario == "wait";
                ui.send(json!({"id":ui.id(),"type":"prompt","message":if scenario == "thinking" { "fixture-thinking" } else { "fixture-stream" }}));
                glib::timeout_future(Duration::from_millis(400)).await;
                let labels = label_texts(ui.chat.list.upcast_ref());
                let early = labels.iter().any(|text| text.contains(if scenario == "thinking" { "Thinking fixture is visible" } else { "abcdefghij" }));
                let phase = ui.state.borrow().phase.label().to_string();
                let running_early = ui.state.borrow().running;
                glib::timeout_future(Duration::from_millis(1900)).await;
                let labels = label_texts(ui.chat.list.upcast_ref());
                let marker = if scenario == "thinking" { "Thinking finished.".into() } else { "abcdefghij".repeat(80) };
                let final_text = labels.iter().any(|text| text.contains(&marker));
                feature = json!({"early":early,"phase":phase,"running_early":running_early,"running_final":ui.state.borrow().running,
                    "final":final_text,"cursor_final":labels.iter().any(|text| text.ends_with('▍'))});
            } else if scenario == "tools" {
                ui.chat.clear();
                let names = ["read", "edit", "write", "bash", "grep", "web_search", "subagent", "mcp__fixture", "todo"];
                for name in names {
                    ui.on_record(&json!({"type":"tool_execution_start","toolCallId":name,"toolName":name,"args":{"path":"fixture.txt"}}));
                    if name != "todo" {
                        ui.on_record(&json!({"type":"tool_execution_end","toolCallId":name,"toolName":name,"result":{"content":[{"type":"text","text":"Fixture completed"}]}}));
                    }
                }
                let widgets = descendants(ui.chat.list.upcast_ref());
                feature = json!({"icons":widgets.iter().filter(|w| w.has_css_class("tool-file-icon") && w.is_visible()).count(),
                    "running":widgets.iter().filter(|w| w.has_css_class("tool-running") && w.is_visible()).count(),
                    "names":label_texts(ui.chat.list.upcast_ref())});
            } else if scenario == "status" {
                ui.on_record(&json!({"type":"agent_start"}));
                ui.on_record(&json!({"type":"agent_settled"}));
                let active_done = !ui.state.borrow().running && ui.state.borrow().phase.label() == "DONE";
                let session = crate::state::RemoteSession {
                    path:"/fixture/background.jsonl".into(), session_id:"background".into(), name:Some("Background fixture".into()),
                    model:None, preview:None, mtime:0, last_response:None,
                };
                ui.state.borrow_mut().sessions.push(session.clone());
                let key = Rc::new(std::cell::RefCell::new(session.session_id.clone()));
                ui.activity.borrow_mut().insert(session.session_id.clone(), crate::state::SessionActivity {
                    running:true, phase:"WRITING".into(), started:Some(std::time::Instant::now()), unread:false, waiting:false,
                });
                ui.on_background_event(&key, crate::rpc::RpcEvent::Record(json!({"type":"agent_settled"})));
                let snapshot = ui.activity_snapshot(&ui.state.borrow());
                let background_done = [&session.path, &session.session_id].iter().all(|key|
                    snapshot.get(*key).is_some_and(|a| !a.running && a.phase == "DONE" && a.started.is_none()));
                let index = ui.state.borrow().sessions.len() - 1;
                ui.close_tab(index);
                let closed = ui.closed_tabs.borrow().contains(&session.path);
                ui.reopen_tab();
                let reopened = !ui.closed_tabs.borrow().contains(&session.path);
                for _ in 0..20 {
                    glib::timeout_future(Duration::from_millis(100)).await;
                    if ui.state.borrow().connection.is_connected() { break; }
                }
                feature = json!({"active_done":active_done,"background_done":background_done,"closed":closed,
                    "reopened":reopened,"running_after_select":ui.state.borrow().running});
            } else if matches!(scenario.as_str(), "git-ux" | "git-clean") {
                ui.select_nav(5);
                glib::timeout_future(Duration::from_millis(300)).await;
                feature = if scenario == "git-ux" { ui.git.qa_ux().await } else { ui.git.qa_clean().await };
            } else if scenario == "git-repository" {
                ui.select_nav(5);
                glib::timeout_future(Duration::from_millis(300)).await;
                let workspace_before = ui.config.borrow().effective_workspace().to_string();
                feature = ui.git.qa_repository().await;
                ui.select_nav(2);
                ui.select_nav(5);
                glib::timeout_future(Duration::from_millis(200)).await;
                feature["after_navigation"] = ui.git.qa_state();
                feature["chat_workspace_unchanged"] = json!(workspace_before == ui.config.borrow().effective_workspace());
            } else if scenario.starts_with("git") {
                ui.select_nav(5);
                glib::timeout_future(Duration::from_millis(300)).await;
                let initial = ui.git.qa_state();
                let texts = label_texts(ui.git.root.upcast_ref());
                let partial_rows = descendants(ui.git.root.upcast_ref()).into_iter()
                    .filter(|w| w.has_css_class("git-file") && label_texts(w).iter().any(|text| text == "partial.txt")).count();
                ui.git.preview_first();
                glib::timeout_future(Duration::from_millis(if scenario == "git-large" { 700 } else { 250 })).await;
                feature = json!({"state":initial,"partial_rows":partial_rows,"labels":texts,
                    "diff_rows":descendants(ui.git.root.upcast_ref()).iter().filter(|w| w.has_css_class("diff-line")).count()});
                if scenario == "git-large" {
                    let widgets = descendants(ui.git.root.upcast_ref());
                    let scroll = widgets.iter().find(|w| w.has_css_class("git-diff-scroll")).unwrap().downcast_ref::<gtk::ScrolledWindow>().unwrap();
                    feature["truncated"] = json!(label_texts(ui.git.root.upcast_ref()).iter().any(|t| t.contains("2,000 lines")));
                    feature["horizontal_scroll"] = json!(scroll.hadjustment().upper() > scroll.hadjustment().page_size());
                    feature["unellipsized"] = json!(widgets.iter().filter(|w| w.has_css_class("diff-text"))
                        .all(|w| w.downcast_ref::<gtk::Label>().is_some_and(|l| l.ellipsize() == gtk::pango::EllipsizeMode::None)));
                } else if scenario == "git-workbench" {
                    feature["workbench"] = ui.git.qa_workbench().await;
                    let controllers = ui.window.observe_controllers();
                    let controller = (0..controllers.n_items()).find_map(|i|
                        controllers.item(i).and_downcast::<gtk::EventControllerKey>()).unwrap();
                    let handled = controller.emit_by_name::<bool>("key-pressed",
                        &[&gtk::gdk::Key::f, &0_u32, &gtk::gdk::ModifierType::CONTROL_MASK]);
                    let focused = gtk::prelude::GtkWindowExt::focus(&ui.window)
                        .is_some_and(|w| w.is_ancestor(&ui.git.root));
                    feature["filter_shortcut"] = json!(handled && focused && ui.stack.visible_child_name().as_deref() == Some("git"));
                } else if scenario == "git-history" {
                    feature["history"] = ui.git.qa_history().await;
                } else if scenario == "git-390" {
                    // Exercise the Git component's actual narrow allocation, not
                    // the desktop shell's intentional minimum width.
                    let narrow = super::git::GitUi::build(std::path::PathBuf::from(ui.config.borrow().workspace.clone()));
                    let window = gtk::Window::builder().title("Git workbench")
                        .default_width(390).default_height(840).decorated(false).child(&narrow.root).build();
                    let hold = narrow.clone();
                    window.connect_destroy(move |_| { let _ = &hold; });
                    window.present();
                    glib::timeout_future(Duration::from_millis(600)).await;
                    feature["narrow"] = json!({"width":window.width(),"height":window.height(),
                        "state":narrow.qa_state(),"bounds":bounds(narrow.root.upcast_ref(), window.upcast_ref()),
                        "controls":descendants(narrow.root.upcast_ref()).iter()
                            .filter(|w| w.is_mapped() && w.is::<gtk::Button>())
                            .map(|w| bounds(w, window.upcast_ref())).collect::<Vec<_>>()});
                } else if scenario == "git-actions" {
                    button(ui.git.root.upcast_ref(), |b| b.label().as_deref() == Some("Stage all")).emit_clicked();
                    glib::timeout_future(Duration::from_millis(300)).await;
                    let staged = ui.git.qa_state();
                    let entry = descendants(ui.git.root.upcast_ref()).into_iter().find(|w| w.has_css_class("git-commit-entry")).unwrap().downcast::<gtk::Entry>().unwrap();
                    entry.set_text("Fixture commit");
                    button(ui.git.root.upcast_ref(), |b| b.label().as_deref() == Some("Commit")).emit_clicked();
                    glib::timeout_future(Duration::from_millis(400)).await;
                    let committed = ui.git.qa_state();
                    entry.set_text("Do not commit an empty index");
                    entry.emit_activate();
                    glib::timeout_future(Duration::from_millis(150)).await;
                    feature["staged_all"] = staged;
                    feature["committed"] = committed;
                    feature["empty_enter_status"] = ui.git.qa_state()["status"].clone();
                } else if scenario == "git-recovery" {
                    button(ui.git.root.upcast_ref(), |b| b.label().as_deref() == Some("Abort merge")).emit_clicked();
                    button(window("Abort operation?").upcast_ref(), |b| b.label().as_deref() == Some("Abort")).emit_clicked();
                    glib::timeout_future(Duration::from_millis(400)).await;
                    feature["aborted"] = ui.git.qa_state();
                    let dropdown = descendants(ui.git.root.upcast_ref()).into_iter().find(|w| w.has_css_class("git-branches"))
                        .unwrap().downcast::<gtk::DropDown>().unwrap();
                    dropdown.set_selected(1);
                    glib::timeout_future(Duration::from_millis(400)).await;
                    feature["switched"] = ui.git.qa_state();
                }
            } else if scenario == "plan-bar" {
                let before_bounds = bounds(ui.status.root.upcast_ref(), ui.window.upcast_ref());
                let mut single = json!({});
                for (key, name) in [("plan-a", "Session A"), ("plan-b", "Session B")] {
                    let request = crate::extension_ui::Request::parse(&json!({"type":"extension_ui_request","id":key,
                        "method":"select","title":"Plan mode - what next?",
                        "options":["Execute the plan (track progress)","Stay in plan mode","Refine the plan"]})).unwrap();
                    let handle = ui.rpc.borrow().clone();
                    let id = request.id.clone();
                    ui.plan_bar.push(key.into(), name.into(), request, move |response| handle.send(crate::extension_ui::response_json(&id, &response)));
                    if key == "plan-a" {
                        glib::timeout_future(Duration::from_millis(250)).await;
                        single = plan_state(&ui.plan_bar, ui.window.upcast_ref());
                    }
                }
                glib::timeout_future(Duration::from_millis(250)).await;
                let multiple = plan_state(&ui.plan_bar, ui.window.upcast_ref());
                descendants(ui.plan_bar.root.upcast_ref()).into_iter().find_map(|w| w.downcast::<gtk::MenuButton>().ok()
                    .filter(|b| b.tooltip_text().as_deref() == Some("Session A — Plan"))).unwrap().popup();
                button(ui.plan_bar.root.upcast_ref(), |b| b.label().as_deref() == Some("Stay in plan mode")).emit_clicked();
                let remaining = label_texts(ui.plan_bar.root.upcast_ref());
                let lines: Vec<_> = (0..12).map(|index| format!("{} Step {}", if index < 3 { "☑" } else { "☐" }, index + 1)).collect();
                ui.on_extension_ui(&json!({"type":"extension_ui_request","method":"setWidget","widgetKey":"plan-todos","widgetLines":lines}));
                ui.apply_view(ViewMode::Chat, false);
                glib::timeout_future(Duration::from_millis(300)).await;
                feature = json!({"both":multiple["menus"] == json!(["Session A — Plan","Session B — Plan"]),
                    "remaining_b":remaining.contains(&"Plan".into()) && !remaining.iter().any(|text| text.contains("Session A")),
                    "pending":ui.plan_bar.is_pending(),"rail_visible":ui.rail.root.is_mapped(),
                    "items":descendants(ui.rail.root.upcast_ref()).iter().filter(|w| w.has_css_class("rail-plan-item")).count(),
                    "current":descendants(ui.rail.root.upcast_ref()).iter().filter(|w| w.has_css_class("rail-plan-row") && w.has_css_class("current")).count(),
                    "counter":label_texts(ui.rail.root.upcast_ref()).contains(&"Plan 3/12".into()),
                    "single":single,"multiple":multiple,"remaining":plan_state(&ui.plan_bar,ui.window.upcast_ref()),
                    "bar_before":before_bounds,"bar_after":bounds(ui.plan_bar.root.upcast_ref(),ui.window.upcast_ref())});
            } else if scenario == "plan-single" {
                let before = bounds(ui.status.root.upcast_ref(),ui.window.upcast_ref());
                push_plan(&ui, "plan-single-select", "Single", false);
                glib::timeout_future(Duration::from_millis(300)).await;
                let initial = plan_state(&ui.plan_bar,ui.window.upcast_ref());
                button(ui.plan_bar.root.upcast_ref(), |b| b.label().as_deref() == Some("Refine")).emit_clicked();
                let select_answered = !ui.plan_bar.is_pending();
                push_plan(&ui, "plan-single-confirm", "Single", true);
                glib::timeout_future(Duration::from_millis(250)).await;
                let confirm = plan_state(&ui.plan_bar,ui.window.upcast_ref());
                button(ui.plan_bar.root.upcast_ref(), |b| b.label().as_deref() == Some("Stay in plan")).emit_clicked();
                let confirm_answered = !ui.plan_bar.is_pending();
                push_plan(&ui, "plan-single-preview", "Single", false);
                glib::timeout_future(Duration::from_millis(250)).await;
                feature = json!({"before":before,"initial":initial,"confirm":confirm,
                    "select_answered":select_answered,"confirm_answered":confirm_answered,
                    "final":plan_state(&ui.plan_bar,ui.window.upcast_ref())});
            } else if scenario == "plan-single-390" {
                let narrow = super::plan_bar::PlanBar::build();
                let window = gtk::Window::builder().title("Plan bar")
                    .default_width(390).default_height(48).decorated(false).child(&narrow.root).build();
                let request = crate::extension_ui::Request::parse(&json!({"type":"extension_ui_request","id":"narrow","method":"select",
                    "title":"Plan mode - what next?","options":["Execute the plan (track progress)","Stay in plan mode","Refine the plan"]})).unwrap();
                narrow.push("narrow".into(),"Single".into(),request, |_| {});
                let hold = narrow.clone();
                window.connect_destroy(move |_| { let _ = &hold; });
                window.present();
                glib::timeout_future(Duration::from_millis(500)).await;
                feature = json!({"width":window.width(),"height":window.height(),
                    "plan":plan_state(&narrow,window.upcast_ref())});
            } else if scenario == "models" {
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
            let horizontal_overflow = [ui.header.root.upcast_ref::<gtk::Widget>(), ui.stack.upcast_ref(), ui.rail_column.upcast_ref(), ui.plan_bar.root.upcast_ref()]
                .iter().filter(|widget| widget.is_mapped()).any(|widget| widget.compute_bounds(root)
                    .is_some_and(|rect| rect.x() < 0.0 || rect.x() + rect.width() > ui.window.width() as f32 + 1.0));
            if scenario.starts_with("git") {
                feature["geometry"] = json!({
                    "root":bounds(ui.git.root.upcast_ref(), root),
                    "controls":descendants(ui.git.root.upcast_ref()).iter()
                        .filter(|w| w.is_mapped() && (w.is::<gtk::Button>() || w.is::<gtk::ListBoxRow>() || w.is::<gtk::SearchEntry>()))
                        .map(|w| bounds(w, root)).collect::<Vec<_>>(),
                    "foreground":crate::tokens::active("text"), "background":crate::tokens::active("canvas"),
                    "selected_background":crate::tokens::active("selectedBg")});
            }
            let report = json!({
                "window":[ui.window.width(),ui.window.height()], "scenario":scenario,
                "app_version":crate::APP_VERSION,
                "footer_version":descendants(ui.sidebar.root.upcast_ref()).iter()
                    .find(|w| w.has_css_class("footer-version"))
                    .and_then(|w| w.downcast_ref::<gtk::Label>()).map(|label| label.text().to_string()),
                "feature":feature,
                "theme_round_trip":{"before":before,"opposite":opposite,"restored":restored},
                "rail_resizes":resize,"rail":bounds(ui.rail_column.upcast_ref(),root),
                "drag_handler":{"before":before_drag,"after":after_drag},
                "controls":controls,
                "header_borders":widgets.iter().filter(|widget| widget.is_mapped() &&
                    ["model-chip","theme-button","mode-switch","session-tab-group"].iter().any(|class| widget.has_css_class(class)))
                    .map(border_widths).collect::<Vec<_>>(),
                "header_outer_border":border_widths(ui.header.root.upcast_ref()),
                "header_icons":descendants(ui.header.root.upcast_ref()).iter()
                    .filter(|widget| widget.is_mapped() && widget.is::<gtk::Image>())
                    .map(|widget| json!({"detail":widget.has_css_class("model-chip-chevron"),
                        "bounds":bounds(widget,root)})).collect::<Vec<_>>(),
                "approval_bounds":bounds(ui.plan_bar.root.upcast_ref(),root),
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

fn push_plan(ui: &AppUi, id: &str, session: &str, confirm: bool) {
    let request = crate::extension_ui::Request::parse(&json!({"type":"extension_ui_request","id":id,
        "method":if confirm { "confirm" } else { "select" },
        "title":if confirm { "Execute the plan?" } else { "Plan mode - what next?" },
        "options":["Execute the plan (track progress)","Stay in plan mode","Refine the plan"]})).unwrap();
    let handle = ui.rpc.borrow().clone();
    let response_id = request.id.clone();
    ui.plan_bar.push(id.into(),session.into(),request,move |response|
        handle.send(crate::extension_ui::response_json(&response_id,&response)));
}

fn plan_state(plan: &super::plan_bar::PlanBar, root: &gtk::Widget) -> Value {
    let widgets = descendants(plan.root.upcast_ref());
    json!({"bar":bounds(plan.root.upcast_ref(),root),"border":border_widths(plan.root.upcast_ref()),
        "tag_foreground":crate::tokens::active("text"),"tag_background":crate::tokens::active("selectedBg"),
        "menus":widgets.iter().filter(|w| w.is_mapped()).filter_map(|w| w.downcast_ref::<gtk::MenuButton>())
            .filter_map(|b| b.tooltip_text().map(|text| text.to_string())).collect::<Vec<_>>(),
        "tags":widgets.iter().filter(|w| w.is_mapped() && w.has_css_class("plan-tag"))
            .map(|w| json!({"bounds":bounds(w,root),"border":border_widths(w)})).collect::<Vec<_>>(),
        "buttons":widgets.iter().filter(|w| w.is_mapped() && w.has_css_class("plan-choice"))
            .filter_map(|w| w.downcast_ref::<gtk::Button>())
            .map(|b| json!({"label":b.label().as_deref(),"bounds":bounds(b.upcast_ref(),root),"border":border_widths(b.upcast_ref())}))
            .collect::<Vec<_>>(),
        "menu_borders":widgets.iter().filter(|w| w.is_mapped() && w.has_css_class("plan-approval-title"))
            .filter_map(|w| w.first_child()).map(|w| border_widths(&w)).collect::<Vec<_>>()})
}

fn button(root: &gtk::Widget, predicate: impl Fn(&gtk::Button) -> bool) -> gtk::Button {
    descendants(root).into_iter().filter_map(|w| w.downcast::<gtk::Button>().ok())
        .find(predicate).unwrap()
}

fn label_texts(root: &gtk::Widget) -> Vec<String> {
    descendants(root).iter().filter_map(|w| w.downcast_ref::<gtk::Label>()).map(|label| label.text().to_string()).collect()
}

fn allocate_window(window: &adw::ApplicationWindow) {
    // Occluded Wayland windows can defer frame-clock layout indefinitely.
    // Flush GTK's actual allocation, not guessed geometry, before measuring it.
    window.queue_allocate();
    window.allocate(window.width(), window.height(), -1, None);
}

#[allow(deprecated)]
fn border_widths(widget: &gtk::Widget) -> Value {
    let border = widget.style_context().border();
    json!([border.top(), border.right(), border.bottom(), border.left()])
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
