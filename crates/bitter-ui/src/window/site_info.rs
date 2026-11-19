//! The popover behind the address bar's connection icon: connection state,
//! content blocking for the site and its remembered permissions.

use super::*;

pub(super) fn connect(browser: &Rc<Browser>) {
    let weak = Rc::downgrade(browser);
    browser
        .toolbar
        .omnibar()
        .connect_icon_pressed(move |entry| {
            if let Some(browser) = weak.upgrade() {
                browser.show_site_info(entry);
            }
        });
}

impl Browser {
    fn show_site_info(self: &Rc<Self>, entry: &gtk4::Entry) {
        let Some(tab) = self.active() else { return };
        let uri = tab.uri();
        let Some(origin) = site_origin(&uri) else {
            return;
        };
        let host = url::Url::parse(&uri)
            .ok()
            .and_then(|url| url.host_str().map(str::to_ascii_lowercase))
            .unwrap_or_default();
        let certificate_failed = tab
            .view
            .get()
            .is_some_and(|view| view.certificate_failed.get());
        let content = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .spacing(12)
            .margin_top(6)
            .margin_bottom(6)
            .margin_start(6)
            .margin_end(6)
            .width_request(300)
            .build();
        content.append(
            &gtk4::Label::builder()
                .label(&host)
                .xalign(0.0)
                .css_classes(["title-4"])
                .ellipsize(gtk4::pango::EllipsizeMode::Middle)
                .build(),
        );
        let (icon, connection) = match Security::for_address(&uri, certificate_failed) {
            Security::Secure => ("channel-secure-symbolic", "Connection is secure"),
            _ => (
                "channel-insecure-symbolic",
                "Connection is not secure. Avoid entering passwords or payment details.",
            ),
        };
        let status = gtk4::Box::builder().spacing(8).build();
        status.append(&gtk4::Image::from_icon_name(icon));
        status.append(
            &gtk4::Label::builder()
                .label(connection)
                .xalign(0.0)
                .wrap(true)
                .hexpand(true)
                .build(),
        );
        content.append(&status);

        let list = gtk4::ListBox::builder()
            .selection_mode(gtk4::SelectionMode::None)
            .css_classes(["boxed-list"])
            .build();
        if self.blocker.is_enabled() {
            let row = adw::SwitchRow::builder()
                .title("Block ads and trackers")
                .active(!self.sites.borrow().is_unblocked(&host))
                .build();
            let weak = Rc::downgrade(self);
            let blocked_host = host.clone();
            row.connect_active_notify(move |row| {
                if let Some(browser) = weak.upgrade() {
                    browser.set_unblocked(&blocked_host, !row.is_active());
                    browser.with_view(|_, view| view.web.reload());
                }
            });
            list.append(&row);
        }
        let remembered: Vec<_> = self
            .sites
            .borrow()
            .permissions()
            .iter()
            .filter(|entry| entry.origin == origin)
            .cloned()
            .collect();
        for entry in remembered {
            let row = adw::ComboRow::builder()
                .title(entry.permission.label())
                .model(&gtk4::StringList::new(&["Allow", "Block"]))
                .selected(if entry.allowed { 0 } else { 1 })
                .build();
            let weak = Rc::downgrade(self);
            let origin = origin.clone();
            row.connect_selected_notify(move |row| {
                if let Some(browser) = weak.upgrade() {
                    browser.set_site_permission(
                        &origin,
                        entry.permission,
                        Some(row.selected() == 0),
                    );
                }
            });
            list.append(&row);
        }
        if list.first_child().is_some() {
            content.append(&list);
        }
        let settings = gtk4::Button::builder()
            .label("Site Permissions…")
            .css_classes(["flat"])
            .halign(gtk4::Align::Start)
            .build();
        content.append(&settings);

        let popover = gtk4::Popover::builder()
            .child(&content)
            .position(gtk4::PositionType::Bottom)
            .build();
        popover.set_parent(entry);
        popover.set_pointing_to(Some(&entry.icon_area(gtk4::EntryIconPosition::Primary)));
        popover.connect_closed(|popover| {
            let popover = popover.clone();
            glib::idle_add_local_once(move || popover.unparent());
        });
        let weak = Rc::downgrade(self);
        let weak_popover = popover.downgrade();
        settings.connect_clicked(move |_| {
            if let Some(popover) = weak_popover.upgrade() {
                popover.popdown();
            }
            if let Some(browser) = weak.upgrade() {
                browser.show_preferences_page(preferences::PERMISSIONS);
            }
        });
        popover.popup();
    }
}
