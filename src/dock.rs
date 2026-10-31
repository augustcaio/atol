// A pílula da dock: favoritos, apps abertos e o botão de aplicativos, como no Dash do GNOME.
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use gtk::glib::{self, variant::ToVariant};
use gtk::prelude::*;
use gtk::gio;

use crate::bridge::{self, RunningApp};

// Os mesmos favoritos do Dash do GNOME.
const FAVORITES: &str = "favorite-apps";

/// Favoritos, na ordem do usuário, e depois os apps abertos que não são favoritos.
type Layout = (Vec<String>, Vec<String>);

pub struct Dock {
    pub widget: gtk::Box,
    conn: gio::DBusConnection,
    shell: gio::Settings,
    running: RefCell<Vec<RunningApp>>,
    layout: RefCell<Option<Layout>>,
    indicators: RefCell<HashMap<String, gtk::Box>>,
    // Reconstruir a dock com um menu aberto destruiria o menu; espera ele fechar.
    menu_open: Cell<bool>,
    subscription: RefCell<Option<gio::SignalSubscription>>,
}

impl Dock {
    pub fn new(conn: gio::DBusConnection) -> Rc<Self> {
        let dock = Rc::new(Self {
            widget: gtk::Box::builder().css_classes(["dock"]).build(),
            conn,
            shell: gio::Settings::new("org.gnome.shell"),
            running: RefCell::default(),
            layout: RefCell::default(),
            indicators: RefCell::default(),
            menu_open: Cell::new(false),
            subscription: RefCell::default(),
        });

        let weak = Rc::downgrade(&dock);
        dock.shell.connect_changed(Some(FAVORITES), move |_, _| {
            if let Some(dock) = weak.upgrade() {
                dock.refresh();
            }
        });

        let weak = Rc::downgrade(&dock);
        let subscription = bridge::watch(&dock.conn, move |apps| {
            if let Some(dock) = weak.upgrade() {
                dock.running.replace(apps);
                dock.refresh();
            }
        });
        dock.subscription.replace(Some(subscription));

        dock.refresh();
        dock
    }

    fn refresh(self: &Rc<Self>) {
        if self.menu_open.get() {
            return;
        }
        let favorites: Vec<String> = self.shell.strv(FAVORITES).iter().map(ToString::to_string).collect();
        let layout = layout(&favorites, &self.running.borrow(), |id| {
            gio_unix::DesktopAppInfo::new(id).is_some()
        });
        // Só reconstrói quando a lista muda; foco e urgência só trocam os indicadores.
        if self.layout.borrow().as_ref() != Some(&layout) {
            self.rebuild(&layout);
            self.layout.replace(Some(layout));
        }
        self.sync_indicators();
    }

    fn rebuild(self: &Rc<Self>, (pinned, others): &Layout) {
        while let Some(child) = self.widget.first_child() {
            self.widget.remove(&child);
        }
        self.indicators.borrow_mut().clear();

        for id in pinned {
            self.widget.append(&self.app_button(id));
        }
        if !pinned.is_empty() && !others.is_empty() {
            self.widget.append(&gtk::Separator::new(gtk::Orientation::Vertical));
        }
        for id in others {
            self.widget.append(&self.app_button(id));
        }
        if !pinned.is_empty() || !others.is_empty() {
            self.widget.append(&gtk::Separator::new(gtk::Orientation::Vertical));
        }

        let show_apps = gtk::Button::builder()
            .icon_name("view-app-grid-symbolic")
            .tooltip_text("Mostrar aplicativos")
            .css_classes(["app"])
            .build();
        let conn = self.conn.clone();
        show_apps.connect_clicked(move |_| bridge::call(&conn, "ToggleAppGrid", ()));
        self.widget.append(&show_apps);
    }

    fn app_button(self: &Rc<Self>, id: &str) -> gtk::Overlay {
        let info = gio_unix::DesktopAppInfo::new(id).expect("layout() só deixa apps instalados");
        let icon = gtk::Image::new();
        match info.icon() {
            Some(gicon) => icon.set_from_gicon(&gicon),
            None => icon.set_icon_name(Some("application-x-executable")),
        }
        let button = gtk::Button::builder()
            .child(&icon)
            .tooltip_text(info.name())
            .css_classes(["app"])
            .build();

        let (conn, app_id) = (self.conn.clone(), id.to_owned());
        button.connect_clicked(move |_| bridge::call(&conn, "Activate", (app_id.as_str(),)));

        // Clique do meio abre nova janela, como no Dash do GNOME.
        let middle = gtk::GestureClick::builder().button(gtk::gdk::BUTTON_MIDDLE).build();
        let (conn, app_id) = (self.conn.clone(), id.to_owned());
        middle.connect_released(move |_, _, _, _| bridge::call(&conn, "NewWindow", (app_id.as_str(),)));
        button.add_controller(middle);

        let right = gtk::GestureClick::builder().button(gtk::gdk::BUTTON_SECONDARY).build();
        let (weak, app_id) = (Rc::downgrade(self), id.to_owned());
        right.connect_pressed(move |gesture, _, _, _| {
            if let (Some(dock), Some(button)) = (weak.upgrade(), gesture.widget()) {
                dock.open_menu(&button, &app_id);
            }
        });
        button.add_controller(right);

        // O ponto fica sobre a borda de baixo do botão e não pega cliques.
        let dot = gtk::Box::builder()
            .css_classes(["indicator"])
            .halign(gtk::Align::Center)
            .valign(gtk::Align::End)
            .can_target(false)
            .visible(false)
            .build();
        let item = gtk::Overlay::builder().child(&button).build();
        item.add_overlay(&dot);
        self.indicators.borrow_mut().insert(id.to_owned(), dot);
        item
    }

    fn open_menu(self: &Rc<Self>, button: &gtk::Widget, id: &str) {
        let Some(info) = gio_unix::DesktopAppInfo::new(id) else { return };
        let running = self.running.borrow().iter().any(|a| a.id == id);
        let pinned = self.shell.strv(FAVORITES).iter().any(|f| f == id);
        let details = gio_unix::DesktopAppInfo::new("org.gnome.Software.desktop").is_some();

        let popover = gtk::PopoverMenu::from_model(Some(&menu_model(&info, running, pinned, details)));
        popover.insert_action_group("item", Some(&self.item_actions(id)));
        popover.set_parent(button);
        self.menu_open.set(true);

        let weak = Rc::downgrade(self);
        popover.connect_closed(move |popover| {
            let (popover, weak) = (popover.clone(), weak.clone());
            // A ação do item escolhido roda antes; só depois o menu é solto.
            glib::idle_add_local_once(move || {
                popover.unparent();
                if let Some(dock) = weak.upgrade() {
                    dock.menu_open.set(false);
                    dock.refresh();
                }
            });
        });
        popover.popup();
    }

    fn item_actions(self: &Rc<Self>, id: &str) -> gio::SimpleActionGroup {
        let group = gio::SimpleActionGroup::new();

        let launch = gio::SimpleAction::new("launch-action", Some(glib::VariantTy::STRING));
        let (conn, app_id) = (self.conn.clone(), id.to_owned());
        launch.connect_activate(move |_, param| {
            if let Some(action) = param.and_then(|p| p.get::<String>()) {
                bridge::call(&conn, "LaunchAction", (app_id.as_str(), action.as_str()));
            }
        });
        group.add_action(&launch);

        for (name, method) in [("new-window", "NewWindow"), ("quit", "Quit")] {
            let action = gio::SimpleAction::new(name, None);
            let (conn, app_id) = (self.conn.clone(), id.to_owned());
            action.connect_activate(move |_, _| bridge::call(&conn, method, (app_id.as_str(),)));
            group.add_action(&action);
        }

        let toggle_pin = gio::SimpleAction::new("toggle-pin", None);
        let (weak, app_id) = (Rc::downgrade(self), id.to_owned());
        toggle_pin.connect_activate(move |_, _| {
            if let Some(dock) = weak.upgrade() {
                dock.toggle_pin(&app_id);
            }
        });
        group.add_action(&toggle_pin);

        let details = gio::SimpleAction::new("details", None);
        let (conn, app_id) = (self.conn.clone(), id.to_owned());
        details.connect_activate(move |_, _| open_details(&conn, &app_id));
        group.add_action(&details);

        group
    }

    // Fixar põe o app no fim dos favoritos, como o Dash do GNOME.
    fn toggle_pin(&self, id: &str) {
        let mut favorites: Vec<String> = self.shell.strv(FAVORITES).iter().map(ToString::to_string).collect();
        match favorites.iter().position(|f| f == id) {
            Some(i) => {
                favorites.remove(i);
            }
            None => favorites.push(id.to_owned()),
        }
        let favorites: Vec<&str> = favorites.iter().map(String::as_str).collect();
        if let Err(e) = self.shell.set_strv(FAVORITES, favorites.as_slice()) {
            glib::g_warning!("atol", "não deu para salvar os favoritos: {e}");
        }
    }

    fn sync_indicators(&self) {
        let running = self.running.borrow();
        for (id, dot) in self.indicators.borrow().iter() {
            let app = running.iter().find(|a| &a.id == id);
            dot.set_visible(app.is_some());
            let mut classes = vec!["indicator"];
            if app.is_some_and(|a| a.focused) {
                classes.push("focused");
            }
            if app.is_some_and(|a| a.urgent) {
                classes.push("urgent");
            }
            dot.set_css_classes(&classes);
        }
    }
}

fn layout(favorites: &[String], running: &[RunningApp], installed: impl Fn(&str) -> bool) -> Layout {
    let pinned: Vec<String> = favorites.iter().filter(|id| installed(id)).cloned().collect();
    let others = running
        .iter()
        .map(|app| &app.id)
        .filter(|id| !pinned.contains(id) && installed(id))
        .cloned()
        .collect();
    (pinned, others)
}

// Mesma ordem do menu do Dash: ações do app, nova janela, fixar, detalhes, sair.
fn menu_model(info: &gio_unix::DesktopAppInfo, running: bool, pinned: bool, details: bool) -> gio::Menu {
    let menu = gio::Menu::new();
    let actions = info.list_actions();

    let section = gio::Menu::new();
    for action in &actions {
        let item = gio::MenuItem::new(Some(&info.action_name(action)), None);
        item.set_action_and_target_value(Some("item.launch-action"), Some(&action.to_variant()));
        section.append_item(&item);
    }
    menu.append_section(None, &section);

    let section = gio::Menu::new();
    if !actions.iter().any(|a| a == "new-window") {
        section.append(Some("Nova janela"), Some("item.new-window"));
    }
    let pin_label = if pinned { "Desafixar da dock" } else { "Fixar na dock" };
    section.append(Some(pin_label), Some("item.toggle-pin"));
    if details {
        section.append(Some("Detalhes do app"), Some("item.details"));
    }
    menu.append_section(None, &section);

    if running {
        let section = gio::Menu::new();
        section.append(Some("Sair"), Some("item.quit"));
        menu.append_section(None, &section);
    }
    menu
}

// Abre a página do app no GNOME Software, do mesmo jeito que o Shell faz.
fn open_details(conn: &gio::DBusConnection, id: &str) {
    let params = ("details", vec![(id, "").to_variant()], HashMap::<String, glib::Variant>::new());
    conn.call(
        Some("org.gnome.Software"),
        "/org/gnome/Software",
        "org.gtk.Actions",
        "Activate",
        Some(&params.to_variant()),
        None,
        gio::DBusCallFlags::NONE,
        -1,
        gio::Cancellable::NONE,
        |res| {
            if let Err(e) = res {
                glib::g_warning!("atol", "GNOME Software não abriu: {e}");
            }
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app(id: &str) -> RunningApp {
        RunningApp { id: id.into(), focused: false, urgent: false }
    }

    #[test]
    fn favorites_first_then_other_running_apps() {
        let favorites = ["a".to_string(), "b".to_string()];
        let running = [app("c"), app("a"), app("d")];
        let (pinned, others) = layout(&favorites, &running, |_| true);
        assert_eq!(pinned, ["a", "b"]);
        assert_eq!(others, ["c", "d"]);
    }

    #[test]
    fn apps_without_desktop_file_are_left_out() {
        let favorites = ["gone".to_string(), "a".to_string()];
        let running = [app("window:7"), app("b")];
        let (pinned, others) = layout(&favorites, &running, |id| !matches!(id, "gone" | "window:7"));
        assert_eq!(pinned, ["a"]);
        assert_eq!(others, ["b"]);
    }
}
