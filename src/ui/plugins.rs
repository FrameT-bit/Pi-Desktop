
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk::prelude::*;

use crate::catalog;
use crate::icons::{self, Tone};
use crate::state::{CatalogItem, PluginItem};

pub type InstallFn = Box<dyn Fn(String)>;
pub type RemoveFn = Box<dyn Fn(String)>;
pub type ConfigureFn = Box<dyn Fn(String)>;
pub type SearchFn = Box<dyn Fn(String)>;
pub type CatalogInstallFn = Box<dyn Fn(String)>;
pub type MoreFn = Box<dyn Fn()>;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Installed,
    Catalog,
}

impl Tab {
    fn stack_name(self) -> &'static str {
        match self {
            Tab::Installed => "installed",
            Tab::Catalog => "catalog",
        }
    }
}

struct CatalogRow {
    name: String,
    button: gtk::Button,
    note: gtk::Label,
}

pub struct PluginsUi {
    pub root: gtk::Box,
    pub entry: gtk::Entry,
    pub install: gtk::Button,
    pub status: gtk::Label,
    pub list: gtk::Box,
    pub scroll: gtk::ScrolledWindow,
    pub empty: gtk::Label,
    pub tab_installed: gtk::ToggleButton,
    pub tab_catalog: gtk::ToggleButton,
    pub page_stack: gtk::Stack,
    pub search_entry: gtk::Entry,
    pub search_button: gtk::Button,
    pub catalog_status: gtk::Label,
    pub catalog_list: gtk::Box,
    pub catalog_scroll: gtk::ScrolledWindow,
    pub catalog_empty: gtk::Label,
    pub catalog_more: gtk::Button,
    kind: gtk::DropDown,
    sort: gtk::DropDown,
    count: Cell<usize>,
    busy: Cell<bool>,
    tab: Cell<Tab>,
    syncing_tab: Cell<bool>,
    catalog_loaded: Cell<bool>,
    catalog_busy: Cell<bool>,
    installing: RefCell<Option<String>>,
    installed_names: RefCell<Vec<PluginItem>>,
    catalog_rows: RefCell<Vec<CatalogRow>>,
    catalog_items: RefCell<Vec<CatalogItem>>,
    catalog_has_more: Cell<bool>,
    on_install: Rc<RefCell<Option<InstallFn>>>,
    on_remove: Rc<RefCell<Option<RemoveFn>>>,
    on_configure: Rc<RefCell<Option<ConfigureFn>>>,
    on_search: Rc<RefCell<Option<SearchFn>>>,
    on_catalog_install: Rc<RefCell<Option<CatalogInstallFn>>>,
    on_catalog_more: Rc<RefCell<Option<MoreFn>>>,
}

impl PluginsUi {
    pub fn new() -> Rc<Self> {
        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        root.add_css_class("plugins-page");
        root.set_hexpand(true);
        root.set_vexpand(true);

        let inner = gtk::Box::new(gtk::Orientation::Vertical, 16);
        inner.add_css_class("plugins-inner");
        inner.set_halign(gtk::Align::Fill);
        inner.set_valign(gtk::Align::Fill);
        inner.set_hexpand(true);
        inner.set_vexpand(true);

        let title = gtk::Label::new(Some("Extensions"));
        title.add_css_class("panel-title");
        title.set_xalign(0.0);

        let subtitle = gtk::Label::new(Some("installed packages and npm catalog"));
        subtitle.add_css_class("plugins-subtitle");
        subtitle.set_xalign(0.0);

        let heading = gtk::Box::new(gtk::Orientation::Vertical, 4);
        heading.append(&title);
        heading.append(&subtitle);
        inner.append(&heading);

        let tab_installed = gtk::ToggleButton::with_label("Installed");
        tab_installed.add_css_class("plugins-tab");
        tab_installed.set_active(true);

        let tab_catalog = gtk::ToggleButton::with_label("Catalog");
        tab_catalog.add_css_class("plugins-tab");
        tab_catalog.set_group(Some(&tab_installed));

        let tabs = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        tabs.add_css_class("plugins-tabs");
        tabs.append(&tab_installed);
        tabs.append(&tab_catalog);
        inner.append(&tabs);


        let entry = gtk::Entry::new();
        entry.add_css_class("plugin-entry");
        entry.set_placeholder_text(Some("npm:pi-telegram or package name"));
        entry.set_hexpand(true);

        let install = gtk::Button::with_label("Install");
        install.add_css_class("plugin-install");
        install.set_valign(gtk::Align::Center);

        let install_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        install_row.add_css_class("plugin-install-row");
        install_row.append(&entry);
        install_row.append(&install);

        let status = gtk::Label::new(Some(""));
        status.add_css_class("plugin-status");
        status.set_xalign(0.0);
        status.set_ellipsize(gtk::pango::EllipsizeMode::End);

        let list = gtk::Box::new(gtk::Orientation::Vertical, 0);
        list.add_css_class("plugin-list");
        list.set_visible(false);

        let empty = gtk::Label::new(Some("Loading packages..."));
        empty.add_css_class("plugin-empty");
        empty.set_xalign(0.0);
        empty.set_margin_top(12);
        empty.set_margin_bottom(12);
        empty.set_margin_start(12);
        empty.set_margin_end(12);

        let holder = gtk::Box::new(gtk::Orientation::Vertical, 0);
        holder.append(&list);
        holder.append(&empty);

        let scroll = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vscrollbar_policy(gtk::PolicyType::Automatic)
            .vexpand(true)
            .child(&holder)
            .build();
        scroll.add_css_class("plugin-scroll");

        let installed_page = gtk::Box::new(gtk::Orientation::Vertical, 12);
        installed_page.set_hexpand(true);
        installed_page.set_vexpand(true);
        installed_page.append(&install_row);
        installed_page.append(&status);
        installed_page.append(&scroll);


        let search_entry = gtk::Entry::new();
        search_entry.add_css_class("plugin-entry");
        search_entry.set_placeholder_text(Some("search Pi package"));
        search_entry.set_hexpand(true);

        let search_button = gtk::Button::new();
        search_button.add_css_class("plugin-install");
        search_button.set_valign(gtk::Align::Center);
        let search_body = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        search_body.set_halign(gtk::Align::Center);
        search_body.append(&icons::icon("search", Tone::Inverse));
        search_body.append(&gtk::Label::new(Some("Search")));
        search_button.set_child(Some(&search_body));

        let search_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        search_row.add_css_class("plugin-install-row");
        search_row.append(&search_entry);
        search_row.append(&search_button);

        let kind = catalog_dropdown(
            &["all types", "package", "extension", "skill", "theme", "prompt"],
            0,
        );
        let sort = catalog_dropdown(&["most downloads", "recent", "A-Z"], 0);
        let filters = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        filters.add_css_class("catalog-filters");
        filters.append(&kind);
        filters.append(&sort);

        let catalog_status = gtk::Label::new(Some(""));
        catalog_status.add_css_class("plugin-status");
        catalog_status.set_xalign(0.0);
        catalog_status.set_ellipsize(gtk::pango::EllipsizeMode::End);

        let catalog_list = gtk::Box::new(gtk::Orientation::Vertical, 0);
        catalog_list.add_css_class("plugin-list");
        catalog_list.set_visible(false);

        let catalog_empty = gtk::Label::new(Some(
            "enter a name or leave the search empty to see the popular",
        ));
        catalog_empty.add_css_class("plugin-empty");
        catalog_empty.set_xalign(0.0);
        catalog_empty.set_margin_top(12);
        catalog_empty.set_margin_bottom(12);
        catalog_empty.set_margin_start(12);
        catalog_empty.set_margin_end(12);

        let catalog_holder = gtk::Box::new(gtk::Orientation::Vertical, 0);
        catalog_holder.append(&catalog_list);
        catalog_holder.append(&catalog_empty);

        let catalog_scroll = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vscrollbar_policy(gtk::PolicyType::Always)
            .overlay_scrolling(false)
            .vexpand(true)
            .child(&catalog_holder)
            .build();
        catalog_scroll.add_css_class("catalog-scroll");

        let catalog_page = gtk::Box::new(gtk::Orientation::Vertical, 12);
        catalog_page.set_hexpand(true);
        catalog_page.set_vexpand(true);
        catalog_page.append(&search_row);
        catalog_page.append(&filters);
        catalog_page.append(&catalog_status);
        catalog_page.append(&catalog_scroll);
        let catalog_more = gtk::Button::with_label("load more");
        catalog_more.add_css_class("catalog-more");
        catalog_more.set_visible(false);
        catalog_page.append(&catalog_more);

        let page_stack = gtk::Stack::new();
        page_stack.set_transition_type(gtk::StackTransitionType::None);
        page_stack.set_hexpand(true);
        page_stack.set_vexpand(true);
        page_stack.add_named(&installed_page, Some("installed"));
        page_stack.add_named(&catalog_page, Some("catalog"));
        page_stack.set_visible_child_name("installed");
        inner.append(&page_stack);

        root.append(&inner);

        let ui = Rc::new(Self {
            root,
            entry,
            install,
            status,
            list,
            scroll,
            empty,
            tab_installed,
            tab_catalog,
            page_stack,
            search_entry,
            search_button,
            catalog_status,
            catalog_list,
            catalog_scroll,
            catalog_empty,
            catalog_more,
            kind,
            sort,
            count: Cell::new(0),
            busy: Cell::new(false),
            tab: Cell::new(Tab::Installed),
            syncing_tab: Cell::new(false),
            catalog_loaded: Cell::new(false),
            catalog_busy: Cell::new(false),
            installing: RefCell::new(None),
            installed_names: RefCell::new(Vec::new()),
            catalog_rows: RefCell::new(Vec::new()),
            catalog_items: RefCell::new(Vec::new()),
            catalog_has_more: Cell::new(false),
            on_install: Rc::new(RefCell::new(None)),
            on_remove: Rc::new(RefCell::new(None)),
            on_configure: Rc::new(RefCell::new(None)),
            on_search: Rc::new(RefCell::new(None)),
            on_catalog_install: Rc::new(RefCell::new(None)),
            on_catalog_more: Rc::new(RefCell::new(None)),
        });
        ui.wire();
        ui
    }

    fn wire(self: &Rc<Self>) {
        let me = self.clone();
        self.install.connect_clicked(move |_| me.trigger_install());

        let me = self.clone();
        self.entry.connect_activate(move |_| me.trigger_install());

        let me = self.clone();
        self.search_button
            .connect_clicked(move |_| me.trigger_search());

        let me = self.clone();
        self.search_entry.connect_activate(move |_| me.trigger_search());

        let me = self.clone();
        self.kind.connect_selected_notify(move |_| me.render_catalog());
        let me = self.clone();
        self.sort.connect_selected_notify(move |_| me.render_catalog());

        let me = self.clone();
        self.tab_installed.connect_toggled(move |button| {
            if button.is_active() {
                me.select_tab(Tab::Installed);
            }
        });

        let me = self.clone();
        self.tab_catalog.connect_toggled(move |button| {
            if button.is_active() {
                me.select_tab(Tab::Catalog);
            }
        });

        self.select_tab(Tab::Installed);
    }

    fn select_tab(self: &Rc<Self>, tab: Tab) {
        if self.syncing_tab.get() {
            return;
        }
        self.syncing_tab.set(true);

        self.tab.set(tab);
        self.page_stack.set_visible_child_name(tab.stack_name());

        for (candidate, button) in [
            (Tab::Installed, &self.tab_installed),
            (Tab::Catalog, &self.tab_catalog),
        ] {
            if candidate == tab {
                button.add_css_class("active");
                if !button.is_active() {
                    button.set_active(true);
                }
            } else {
                button.remove_css_class("active");
                if button.is_active() {
                    button.set_active(false);
                }
            }
        }

        self.syncing_tab.set(false);

        if tab == Tab::Catalog {
            if !self.catalog_loaded.get() {
                self.catalog_loaded.set(true);
                self.trigger_search();
            }
            self.search_entry.grab_focus();
        }
    }

    fn trigger_install(self: &Rc<Self>) {
        if self.busy.get() {
            return;
        }
        let text = self.entry.text().to_string();
        if text.trim().is_empty() {
            return;
        }
        if let Some(callback) = self.on_install.borrow().as_ref() {
            callback(text);
        }
    }

    fn trigger_search(self: &Rc<Self>) {
        if self.catalog_busy.get() {
            return;
        }
        self.catalog_loaded.set(true);
        self.catalog_busy.set(true);
        if self.catalog_rows.borrow().is_empty() {
            self.catalog_empty.set_text("Loading packages...");
            self.catalog_empty.set_visible(true);
        }
        self.set_catalog_status("Looking at npm...", false);
        let query = self.search_entry.text().trim().to_string();
        if let Some(callback) = self.on_search.borrow().as_ref() {
            callback(query);
        }
    }

    pub fn set_on_install(&self, callback: impl Fn(String) + 'static) {
        *self.on_install.borrow_mut() = Some(Box::new(callback));
    }

    pub fn set_on_remove(&self, callback: impl Fn(String) + 'static) {
        *self.on_remove.borrow_mut() = Some(Box::new(callback));
    }

    pub fn set_on_configure(&self, callback: impl Fn(String) + 'static) {
        *self.on_configure.borrow_mut() = Some(Box::new(callback));
    }

    pub fn set_on_search(&self, callback: impl Fn(String) + 'static) {
        *self.on_search.borrow_mut() = Some(Box::new(callback));
    }

    pub fn set_on_catalog_install(&self, callback: impl Fn(String) + 'static) {
        *self.on_catalog_install.borrow_mut() = Some(Box::new(callback));
    }

    pub fn set_on_catalog_more(self: &Rc<Self>, callback: impl Fn() + 'static) {
        *self.on_catalog_more.borrow_mut() = Some(Box::new(callback));
        let me = self.clone();
        self.catalog_more.connect_clicked(move |_| {
            me.catalog_more.set_sensitive(false);
            me.set_catalog_status("Loading more packages...", false);
            if let Some(callback) = me.on_catalog_more.borrow().as_ref() {
                callback();
            }
        });
    }

    pub fn catalog_count(&self) -> usize {
        self.catalog_items.borrow().len()
    }

    pub fn count(&self) -> usize {
        self.count.get()
    }

    pub fn is_busy(&self) -> bool {
        self.busy.get()
    }

    pub fn open_catalog(self: &Rc<Self>) {
        self.select_tab(Tab::Catalog);
    }

    pub fn open_installed(self: &Rc<Self>) {
        self.select_tab(Tab::Installed);
    }

    pub fn set_busy(&self, busy: bool, text: &str) {
        self.busy.set(busy);
        self.install.set_sensitive(!busy);
        self.entry.set_sensitive(!busy);
        self.set_status(text, false);
    }

    pub fn set_status(&self, text: &str, error: bool) {
        self.status.set_text(text);
        if error {
            self.status.add_css_class("error");
        } else {
            self.status.remove_css_class("error");
        }
    }

    fn set_catalog_status(&self, text: &str, error: bool) {
        self.catalog_status.set_text(text);
        if error {
            self.catalog_status.add_css_class("error");
        } else {
            self.catalog_status.remove_css_class("error");
        }
    }

    pub fn set_items(self: &Rc<Self>, items: &[PluginItem]) {
        while let Some(child) = self.list.first_child() {
            self.list.remove(&child);
        }

        self.count.set(items.len());
        self.installed_names.replace(items.to_vec());
        self.empty.set_visible(items.is_empty());
        if items.is_empty() {
            self.empty.set_text("no plugin installed — paste an npm package above");
            self.list.set_visible(false);
        } else {
            self.list.set_visible(true);

            for (index, item) in items.iter().enumerate() {
                let origin = gtk::Label::new(Some(&item.origin));
                origin.add_css_class("plugin-badge");
                origin.add_css_class(&item.origin);
                origin.set_valign(gtk::Align::Center);

                let name = gtk::Label::new(Some(&item.name()));
                name.add_css_class("plugin-name");
                name.set_xalign(0.0);
                name.set_hexpand(true);
                name.set_ellipsize(gtk::pango::EllipsizeMode::End);
                name.set_tooltip_text(Some(&item.spec));

                let spec = gtk::Label::new(Some(&item.spec));
                spec.add_css_class("plugin-spec");
                spec.set_xalign(0.0);
                spec.set_ellipsize(gtk::pango::EllipsizeMode::End);
                spec.set_visible(item.spec != item.name());

                let identity = gtk::Box::new(gtk::Orientation::Vertical, 2);
                identity.set_hexpand(true);
                identity.append(&name);
                identity.append(&spec);

                let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
                row.add_css_class("plugin-item");
                if index + 1 < items.len() {
                    row.add_css_class("with-divider");
                }
                row.append(&origin);
                row.append(&identity);
                if crate::plugins::settings_for(&item.spec).is_some() {
                    let configure = gtk::Button::new();
                    configure.add_css_class("plugin-config");
                    configure.set_valign(gtk::Align::Center);
                    configure.set_child(Some(&icons::icon("settings", Tone::Muted)));
                    configure.set_tooltip_text(Some(&format!("configure {}", item.name())));
                    let on_configure = self.on_configure.clone();
                    let spec_value = item.spec.clone();
                    configure.connect_clicked(move |_| {
                        if let Some(callback) = on_configure.borrow().as_ref() {
                            callback(spec_value.clone());
                        }
                    });
                    row.append(&configure);
                }

                let remove = gtk::Button::new();
                remove.add_css_class("plugin-remove");
                remove.set_valign(gtk::Align::Center);
                remove.set_child(Some(&icons::icon("trash", Tone::Muted)));
                remove.set_tooltip_text(Some(&format!("remove {}", item.name())));

                let on_remove = self.on_remove.clone();
                let spec_value = item.spec.clone();
                let me = self.clone();
                remove.connect_clicked(move |_| {
                    if me.busy.get() {
                        return;
                    }
                    if let Some(callback) = on_remove.borrow().as_ref() {
                        callback(spec_value.clone());
                    }
                });

                row.append(&remove);
                self.list.append(&row);
            }
        }

        self.scroll.vadjustment().set_value(0.0);
        self.refresh_catalog_buttons();
    }

    pub fn set_catalog_page(self: &Rc<Self>, page: &crate::catalog::CatalogPage, append: bool) {
        if !append {
            self.catalog_items.borrow_mut().clear();
            self.catalog_scroll.vadjustment().set_value(0.0);
        }
        self.catalog_items
            .borrow_mut()
            .extend(page.items.iter().cloned());
        self.catalog_has_more.set(page.has_more);
        self.catalog_busy.set(false);
        self.render_catalog();
    }

    fn render_catalog(self: &Rc<Self>) {
        while let Some(child) = self.catalog_list.first_child() {
            self.catalog_list.remove(&child);
        }
        self.catalog_rows.borrow_mut().clear();
        let items = self.catalog_items.borrow();
        if items.is_empty() {
            self.catalog_empty.set_text("no package matches this search");
            self.catalog_empty.set_visible(true);
            self.catalog_list.set_visible(false);
            self.catalog_more.set_visible(false);
            self.set_catalog_status("0 packages · npm", false);
            return;
        }
        let kind = match self.kind.selected() as usize {
            1 => "package",
            2 => "extension",
            3 => "skill",
            4 => "theme",
            5 => "prompt",
            _ => "",
        };
        let mut visible: Vec<&CatalogItem> = items
            .iter()
            .filter(|item| kind.is_empty() || item.kind == kind)
            .collect();
        match self.sort.selected() as usize {
            1 => visible.sort_by(|a, b| b.date.cmp(&a.date)),
            2 => visible.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase())),
            _ => visible.sort_by(|a, b| {
                b.downloads
                    .cmp(&a.downloads)
                    .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
            }),
        }
        if visible.is_empty() {
            self.catalog_empty.set_text("no such package in this search");
            self.catalog_empty.set_visible(true);
            self.catalog_list.set_visible(false);
            self.set_catalog_status(&format!("0 of {} packages · npm", items.len()), false);
            self.catalog_more.set_visible(self.catalog_has_more.get());
            return;
        }
        self.catalog_empty.set_visible(false);
        self.catalog_list.set_visible(true);
        let installing = self.installing.borrow().clone();
        for (index, item) in visible.iter().enumerate() {
            let row = self.build_catalog_row(item, index + 1 < visible.len(), installing.as_deref());
            self.catalog_list.append(&row);
        }
        let status = if visible.len() == items.len() {
            format!("{} packages · npm", items.len())
        } else {
            format!("{} of {} packages · npm", visible.len(), items.len())
        };
        self.set_catalog_status(&status, false);
        self.catalog_more
            .set_visible(self.catalog_has_more.get());
        self.catalog_more.set_sensitive(true);
        let adjustment = self.catalog_scroll.vadjustment();
        adjustment.set_value(0.0);
        glib::idle_add_local_once(move || adjustment.set_value(0.0));
    }

    fn build_catalog_row(
        self: &Rc<Self>,
        item: &CatalogItem,
        divider: bool,
        installing: Option<&str>,
    ) -> gtk::Box {
        let kind_icon = icons::icon(catalog_icon_name(&item.kind), Tone::Muted);
        kind_icon.add_css_class("catalog-kind-icon");
        kind_icon.set_valign(gtk::Align::Start);
        kind_icon.set_margin_top(2);

        let name = gtk::Label::new(Some(&item.name));
        name.add_css_class("catalog-name");
        name.set_xalign(0.0);
        name.set_single_line_mode(true);
        name.set_ellipsize(gtk::pango::EllipsizeMode::End);
        name.set_tooltip_text(Some(&item.name));

        let version = gtk::Label::new(Some(&format!("v{}", item.version)));
        version.add_css_class("catalog-version");
        version.set_xalign(0.0);

        let kind = gtk::Label::new(Some(&item.kind));
        kind.add_css_class("catalog-kind");
        kind.set_xalign(0.0);
        kind.set_halign(gtk::Align::Start);
        kind.set_valign(gtk::Align::Center);

        let chevron = icons::icon("chevron-right", Tone::Muted);
        chevron.add_css_class("catalog-open");
        chevron.set_valign(gtk::Align::Center);

        let title = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        title.append(&name);
        title.append(&version);
        title.append(&kind);
        let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        spacer.set_hexpand(true);
        title.append(&spacer);
        title.append(&chevron);

        let description = gtk::Label::new(Some(&catalog::preview(&item.description, 220)));
        description.add_css_class("catalog-desc");
        description.set_xalign(0.0);
        description.set_wrap(true);
        description.set_wrap_mode(gtk::pango::WrapMode::WordChar);
        description.set_lines(2);
        description.set_ellipsize(gtk::pango::EllipsizeMode::End);
        description.set_max_width_chars(76);
        description.set_visible(!item.description.is_empty());

        let author = if item.author.is_empty() {
            String::new()
        } else {
            format!(" · {}", item.author)
        };
        let meta = gtk::Label::new(Some(&format!(
            "{} downloads/month · {}{}",
            format_downloads(item.downloads),
            item.date.chars().take(10).collect::<String>(),
            author
        )));
        meta.add_css_class("catalog-meta");
        meta.set_xalign(0.0);

        let left = gtk::Box::new(gtk::Orientation::Vertical, 3);
        left.add_css_class("catalog-main");
        left.set_hexpand(true);
        left.append(&title);
        left.append(&description);
        left.append(&meta);
        left.set_tooltip_text(Some("open README, npm and repo"));
        left.set_cursor_from_name(Some("pointer"));
        let me_open = self.clone();
        let item_open = item.clone();
        let click = gtk::GestureClick::new();
        click.connect_released(move |_, _, _, _| me_open.open_catalog_detail(&item_open));
        left.add_controller(click);

        let note = gtk::Label::new(None);
        note.add_css_class("catalog-note");
        note.set_xalign(0.0);
        note.set_ellipsize(gtk::pango::EllipsizeMode::End);
        note.set_visible(false);

        let button = gtk::Button::with_label("Install");
        button.add_css_class("catalog-install");
        button.set_valign(gtk::Align::Start);
        let installed = catalog::is_installed(&item.name, &self.installed_names.borrow());
        let is_installing = installing == Some(item.name.as_str());
        apply_button_state(&button, installed, is_installing);

        let package = item.name.clone();
        let on_install = self.on_catalog_install.clone();
        let me = self.clone();
        button.connect_clicked(move |_| {
            if me.catalog_busy.get() || me.installing.borrow().is_some() {
                return;
            }
            if on_install.borrow().is_none() {
                return;
            }
            me.installing.replace(Some(package.clone()));
            me.refresh_catalog_buttons();
            if let Some(callback) = on_install.borrow().as_ref() {
                callback(package.clone());
            }
        });

        self.catalog_rows.borrow_mut().push(CatalogRow {
            name: item.name.clone(),
            button: button.clone(),
            note: note.clone(),
        });

        let right = gtk::Box::new(gtk::Orientation::Vertical, 5);
        right.set_valign(gtk::Align::Start);
        right.append(&button);
        right.append(&note);

        let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        row.add_css_class("plugin-item");
        row.add_css_class("catalog-row");
        if divider {
            row.add_css_class("with-divider");
        }
        row.append(&kind_icon);
        row.append(&left);
        row.append(&right);
        row
    }

    fn open_catalog_detail(self: &Rc<Self>, item: &CatalogItem) {
        let window = gtk::Window::new();
        window.set_title(Some(&item.name));
        window.set_modal(true);
        window.set_default_size(760, 640);
        window.add_css_class("pi-dialog");
        if let Some(root) = self.root.root().and_then(|root| root.downcast::<gtk::Window>().ok()) {
            window.set_transient_for(Some(&root));
        }

        let outer = gtk::Box::new(gtk::Orientation::Vertical, 14);
        outer.add_css_class("pi-dialog-body");

        let icon = icons::icon(catalog_icon_name(&item.kind), Tone::Muted);
        icon.set_pixel_size(32);
        let avatar = gtk::Overlay::new();
        avatar.add_css_class("catalog-avatar");
        avatar.set_child(Some(&icon));
        let picture = gtk::Picture::new();
        picture.set_can_shrink(true);
        picture.set_content_fit(gtk::ContentFit::Cover);
        picture.set_size_request(40, 40);
        picture.set_visible(false);
        avatar.add_overlay(&picture);

        let title = gtk::Label::new(Some(&item.name));
        title.add_css_class("dialog-title");
        title.set_xalign(0.0);
        title.set_hexpand(true);
        title.set_ellipsize(gtk::pango::EllipsizeMode::End);

        let meta = gtk::Label::new(Some(&format!(
            "{} · v{} · {} downloads/month · {}{}",
            item.kind,
            item.version,
            format_downloads(item.downloads),
            item.author,
            if item.homepage.is_empty() {
                String::new()
            } else {
                format!(" · {}", item.homepage)
            }
        )));
        meta.add_css_class("catalog-meta");
        meta.set_xalign(0.0);
        meta.set_ellipsize(gtk::pango::EllipsizeMode::End);

        let identity = gtk::Box::new(gtk::Orientation::Vertical, 2);
        identity.set_hexpand(true);
        identity.append(&title);
        identity.append(&meta);

        let close = gtk::Button::with_label("close");
        close.add_css_class("dialog-cancel");
        let header = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        header.append(&avatar);
        header.append(&identity);
        header.append(&close);
        outer.append(&header);

        let links = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        links.add_css_class("catalog-detail-links");
        let npm = gtk::Button::with_label("npm");
        npm.add_css_class("catalog-detail-link");
        let npm_url = item.npm_url.clone();
        npm.connect_clicked(move |_| open_url(&npm_url));
        links.append(&npm);
        if !item.repo_url.is_empty() {
            let repo = gtk::Button::with_label("repository");
            repo.add_css_class("catalog-detail-link");
            let repo_url = item.repo_url.clone();
            repo.connect_clicked(move |_| open_url(&repo_url));
            links.append(&repo);
        }
        if !item.homepage.is_empty() {
            let homepage = gtk::Button::with_label("site");
            homepage.add_css_class("catalog-detail-link");
            let homepage_url = item.homepage.clone();
            homepage.connect_clicked(move |_| open_url(&homepage_url));
            links.append(&homepage);
        }
        outer.append(&links);

        let readme_title = gtk::Label::new(Some("README"));
        readme_title.add_css_class("catalog-section");
        readme_title.set_xalign(0.0);
        outer.append(&readme_title);

        let readme_body = gtk::Box::new(gtk::Orientation::Vertical, 10);
        readme_body.add_css_class("catalog-readme-body");
        let loading = gtk::Label::new(Some("Reading README..."));
        loading.add_css_class("catalog-meta");
        loading.set_xalign(0.0);
        readme_body.append(&loading);
        let scroll = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vscrollbar_policy(gtk::PolicyType::Automatic)
            .vexpand(true)
            .child(&readme_body)
            .build();
        scroll.add_css_class("catalog-readme-scroll");
        outer.append(&scroll);

        window.set_child(Some(&outer));
        window.present();
        let window_close = window.clone();
        close.connect_clicked(move |_| window_close.close());

        if !item.thumb.is_empty() {
            let url = item.thumb.clone();
            let picture = picture.clone();
            let (tx, rx) = async_channel::bounded::<Option<std::path::PathBuf>>(1);
            std::thread::spawn(move || {
                let path = thumb_cache_path(&url);
                let ok = std::process::Command::new("curl")
                    .args([
                        "-L",
                        "--fail",
                        "--silent",
                        "--show-error",
                        "--max-time",
                        "10",
                        "-o",
                    ])
                    .arg(&path)
                    .arg(&url)
                    .status()
                    .map(|status| status.success())
                    .unwrap_or(false);
                let _ = tx.send_blocking(ok.then_some(path));
            });
            glib::spawn_future_local(async move {
                if let Ok(Some(path)) = rx.recv().await {
                    if let Ok(texture) = gtk::gdk::Texture::from_filename(&path) {
                        picture.set_paintable(Some(&texture));
                        picture.set_visible(true);
                    }
                }
            });
        }

        let name = item.name.clone();
        let (tx, rx) = async_channel::bounded::<Result<String, String>>(1);
        std::thread::spawn(move || {
            let _ = tx.send_blocking(catalog::fetch_readme(&name));
        });
        let readme_body_load = readme_body.clone();
        let loading = loading.clone();
        glib::spawn_future_local(async move {
            let Ok(result) = rx.recv().await else {
                return;
            };
            while let Some(child) = readme_body_load.first_child() {
                readme_body_load.remove(&child);
            }
            let text = match result {
                Ok(text) => {
                    let total = text.chars().count();
                    let mut body: String = text.chars().take(60_000).collect();
                    if total > 60_000 {
                        body.push_str("##############################################################################################################################################################");
                    }
                    body
                }
                Err(error) => {
                    loading.set_text(&format!("README not available: {error}"));
                    readme_body_load.append(&loading);
                    return;
                }
            };
            let widgets = crate::markdown::render(&text);
            if widgets.is_empty() {
                loading.set_text("empty README");
                readme_body_load.append(&loading);
                return;
            }
            for widget in widgets {
                readme_body_load.append(&widget);
            }
        });
    }

    fn refresh_catalog_buttons(&self) {
        let installed = self.installed_names.borrow();
        let installing = self.installing.borrow().clone();
        for row in self.catalog_rows.borrow().iter() {
            let is_installed = catalog::is_installed(&row.name, &installed);
            let is_installing = installing.as_deref() == Some(row.name.as_str());
            apply_button_state(&row.button, is_installed, is_installing);
        }
    }

    pub fn finish_catalog_install(self: &Rc<Self>, name: &str, result: &Result<(), String>) {
        self.installing.replace(None);
        self.refresh_catalog_buttons();
        let row = self
            .catalog_rows
            .borrow()
            .iter()
            .find(|row| row.name == name)
            .map(|row| (row.button.clone(), row.note.clone()));
        let Some((button, note)) = row else {
            return;
        };
        match result {
            Ok(()) => {
                note.remove_css_class("error");
                note.add_css_class("success");
                note.set_text("installed");
                note.set_visible(true);
                button.set_label("Installed");
                button.set_sensitive(false);
            }
            Err(error) => {
                note.remove_css_class("success");
                note.add_css_class("error");
                note.set_text(&format!("failure: {error}"));
                note.set_visible(true);
            }
        }
    }

    pub fn set_catalog_error(&self, message: &str) {
        self.catalog_busy.set(false);
        self.set_catalog_status(message, true);
        if self.catalog_rows.borrow().is_empty() {
            self.catalog_empty.set_text("no results - check the error above");
            self.catalog_empty.set_visible(true);
        }
    }
}

fn catalog_icon_name(kind: &str) -> &'static str {
    match kind {
        "extension" => "plug",
        "skill" => "file",
        "theme" => "moon",
        "prompt" => "message",
        _ => "box",
    }
}

fn thumb_cache_path(url: &str) -> std::path::PathBuf {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    url.hash(&mut hasher);
    std::env::temp_dir().join(format!("pi-plugin-thumb-{:016x}.img", hasher.finish()))
}

fn catalog_dropdown(labels: &[&str], selected: u32) -> gtk::DropDown {
    let list = gtk::StringList::new(labels);
    let dropdown = gtk::DropDown::new(Some(list), gtk::Expression::NONE);
    dropdown.add_css_class("catalog-dropdown");
    dropdown.set_selected(selected);
    dropdown
}

fn format_downloads(value: u64) -> String {
    if value >= 1_000_000 {
        format!("{:.1}M", value as f64 / 1_000_000.0)
    } else if value >= 1_000 {
        format!("{:.1}K", value as f64 / 1_000.0)
    } else {
        value.to_string()
    }
}

fn open_url(url: &str) {
    if let Err(error) = std::process::Command::new("xdg-open").arg(url).spawn() {
        eprintln!("plugins: failed to open {url}: {error}");
    }
}

fn apply_button_state(button: &gtk::Button, installed: bool, installing: bool) {
    if installed {
        button.set_label("Installed");
        button.set_sensitive(false);
    } else if installing {
        button.set_label("Installing...");
        button.set_sensitive(false);
    } else {
        button.set_label("Install");
        button.set_sensitive(true);
    }
}
