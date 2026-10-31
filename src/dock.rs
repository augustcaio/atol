// A pílula da dock: favoritos, apps abertos e o botão de aplicativos, como no Dash do GNOME.
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use adw::prelude::*;
use gtk::glib::{self, variant::ToVariant};
use gtk::gio;

use crate::bridge::{self, RunningApp};

// Os mesmos favoritos do Dash do GNOME.
const FAVORITES: &str = "favorite-apps";

/// Favoritos, na ordem do usuário, e depois os apps abertos que não são favoritos.
type Layout = (Vec<String>, Vec<String>);

/// Arrasto em andamento. A grade dos favoritos é medida antes de qualquer animação:
/// se a posição de destino seguisse os ícones deslizando, o espaço ficaria pulando.
struct Drag {
    /// Favoritos sem o app arrastado, na ordem da dock.
    others: Vec<String>,
    /// Borda esquerda do primeiro favorito e a largura de cada vaga.
    x0: f64,
    slot: f64,
    index: Option<usize>,
    gap: Option<gtk::Box>,
}

pub struct Dock {
    pub widget: gtk::Box,
    conn: gio::DBusConnection,
    shell: gio::Settings,
    running: RefCell<Vec<RunningApp>>,
    layout: RefCell<Option<Layout>>,
    indicators: RefCell<HashMap<String, gtk::Box>>,
    // Favoritos visíveis, na ordem da dock, com o widget de cada um.
    slots: RefCell<Vec<(String, gtk::Overlay)>>,
    drag: RefCell<Option<Drag>>,
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
            slots: RefCell::default(),
            drag: RefCell::default(),
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

        // Soltar um app na dock fixa ele ali, ou só muda a ordem se já é favorito.
        let drop = gtk::DropTarget::new(String::static_type(), gtk::gdk::DragAction::MOVE);
        let weak = Rc::downgrade(&dock);
        drop.connect_drop(move |_, value, _, _| match (weak.upgrade(), value.get::<String>()) {
            (Some(dock), Ok(id)) => dock.drop_here(&id),
            _ => false,
        });
        let weak = Rc::downgrade(&dock);
        drop.connect_motion(move |_, x, _| {
            if let Some(dock) = weak.upgrade() {
                dock.drag_motion(x);
            }
            gtk::gdk::DragAction::MOVE
        });
        let weak = Rc::downgrade(&dock);
        drop.connect_leave(move |_| {
            if let Some(dock) = weak.upgrade() {
                dock.drag_leave();
            }
        });
        dock.widget.add_controller(drop);

        dock.refresh();
        dock
    }

    fn refresh(self: &Rc<Self>) {
        // Com menu aberto ou arrasto em curso, reconstruir puxaria os widgets em uso.
        if self.menu_open.get() || self.drag.borrow().is_some() {
            return;
        }
        let layout = layout(&self.favorites(), &self.running.borrow(), |id| {
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

        let slots: Vec<_> = pinned.iter().map(|id| (id.clone(), self.app_button(id))).collect();
        for (_, item) in &slots {
            self.widget.append(item);
        }
        self.slots.replace(slots);
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
        // Soltar um favorito aqui desafixa, como no Dash do GNOME.
        let unpin = gtk::DropTarget::new(String::static_type(), gtk::gdk::DragAction::MOVE);
        let weak = Rc::downgrade(self);
        unpin.connect_drop(move |_, value, _, _| match (weak.upgrade(), value.get::<String>()) {
            (Some(dock), Ok(id)) if dock.favorites().contains(&id) => {
                let favorites = dock.favorites().into_iter().filter(|f| *f != id).collect();
                dock.save_favorites_later(favorites);
                true
            }
            _ => false,
        });
        show_apps.add_controller(unpin);
        self.widget.append(&show_apps);
    }

    fn app_button(self: &Rc<Self>, id: &str) -> gtk::Overlay {
        let info = gio_unix::DesktopAppInfo::new(id).expect("layout() só deixa apps instalados");
        let gicon = info.icon().unwrap_or_else(|| gio::ThemedIcon::new("application-x-executable").upcast());
        let icon = gtk::Image::from_gicon(&gicon);
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

        // Arrastar leva o id do app: para outra posição, para fixar ou para desafixar.
        let drag = gtk::DragSource::builder()
            .actions(gtk::gdk::DragAction::MOVE)
            .content(&gtk::gdk::ContentProvider::for_value(&id.to_value()))
            .build();
        let (weak, app_id) = (Rc::downgrade(self), id.to_owned());
        drag.connect_drag_begin(move |source, _| {
            // Ícone do tema, e não uma foto do botão: o botão some da dock durante o arrasto.
            let size = icon.width();
            let paintable = gtk::IconTheme::for_display(&icon.display()).lookup_by_gicon(
                &gicon,
                size,
                icon.scale_factor(),
                gtk::TextDirection::None,
                gtk::IconLookupFlags::empty(),
            );
            source.set_icon(Some(&paintable), size / 2, size / 2);
            if let Some(dock) = weak.upgrade() {
                dock.begin_drag(&app_id);
            }
        });
        let weak = Rc::downgrade(self);
        drag.connect_drag_end(move |_, _, _| {
            if let Some(dock) = weak.upgrade() {
                dock.end_drag();
            }
        });
        button.add_controller(drag);

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

    fn favorites(&self) -> Vec<String> {
        self.shell.strv(FAVORITES).iter().map(ToString::to_string).collect()
    }

    fn save_favorites(&self, favorites: &[String]) {
        let favorites: Vec<&str> = favorites.iter().map(String::as_str).collect();
        if let Err(e) = self.shell.set_strv(FAVORITES, favorites.as_slice()) {
            glib::g_warning!("atol", "não deu para salvar os favoritos: {e}");
        }
    }

    // Salvar reconstrói a dock; depois de um drop, espera o arrasto terminar.
    fn save_favorites_later(self: &Rc<Self>, favorites: Vec<String>) {
        let weak = Rc::downgrade(self);
        glib::idle_add_local_once(move || {
            if let Some(dock) = weak.upgrade() {
                dock.save_favorites(&favorites);
            }
        });
    }

    // Fixar põe o app no fim dos favoritos, como o Dash do GNOME.
    fn toggle_pin(&self, id: &str) {
        let mut favorites = self.favorites();
        match favorites.iter().position(|f| f == id) {
            Some(i) => {
                favorites.remove(i);
            }
            None => favorites.push(id.to_owned()),
        }
        self.save_favorites(&favorites);
    }

    // Regra dos métodos de arrasto: nenhum borrow de RefCell fica aberto durante chamadas
    // ao GTK. Inserir, esconder ou remover um widget sob o ponteiro dispara na hora eventos
    // de entrada/saída, que voltam para cá; um borrow aberto vira panic e derruba a dock.

    fn begin_drag(&self, id: &str) {
        let slots = self.slots.borrow().clone();
        let bounds: Vec<_> = slots.iter().filter_map(|(_, w)| w.compute_bounds(&self.widget)).collect();
        let (x0, slot) = match bounds.as_slice() {
            [] => (0.0, 1.0),
            [only] => (f64::from(only.x()), f64::from(only.width())),
            [first, second, ..] => {
                let slot = f64::from(second.x() - first.x());
                (f64::from(first.x()) - (slot - f64::from(first.width())) / 2.0, slot)
            }
        };
        let position = slots.iter().position(|(s, _)| s == id);
        // Um favorito arrastado deixa uma vaga do tamanho dele, sem animação, no lugar onde estava.
        let first_gap = position.map(|_| gap(slot));
        self.drag.replace(Some(Drag {
            others: slots.iter().map(|(s, _)| s.clone()).filter(|s| s != id).collect(),
            x0,
            slot,
            index: position,
            gap: first_gap.clone(),
        }));
        if let (Some(position), Some(first_gap)) = (position, first_gap) {
            let item = &slots[position].1;
            self.widget.insert_child_after(&first_gap, Some(item));
            item.set_visible(false);
        }
    }

    fn drag_motion(&self, x: f64) {
        let (old, new, after, width) = {
            let mut drag = self.drag.borrow_mut();
            let Some(drag) = drag.as_mut() else { return };
            let index = drop_index(x, drag.x0, drag.slot, drag.others.len());
            if drag.index == Some(index) {
                return;
            }
            // A vaga nova entra antes do favorito que vai ser empurrado, ou depois do último.
            let slots = self.slots.borrow();
            let widget_of = |id: &String| {
                slots.iter().find(|(s, _)| s == id).map(|(_, w)| w.clone().upcast::<gtk::Widget>())
            };
            let after = match drag.others.get(index) {
                Some(next) => widget_of(next).and_then(|w| w.prev_sibling()),
                None => drag.others.last().and_then(widget_of),
            };
            let new = gap(0.0);
            drag.index = Some(index);
            (drag.gap.replace(new.clone()), new, after, drag.slot)
        };
        if let Some(old) = old {
            close_gap(old);
        }
        self.widget.insert_child_after(&new, after.as_ref());
        animate_width(&new, width, |_| {});
    }

    // Saindo da dock (para desafixar no botão de aplicativos, por exemplo), a vaga fecha.
    fn drag_leave(&self) {
        let old = self.drag.borrow_mut().as_mut().and_then(|drag| {
            drag.index = None;
            drag.gap.take()
        });
        if let Some(old) = old {
            close_gap(old);
        }
    }

    fn drop_here(self: &Rc<Self>, id: &str) -> bool {
        // Qualquer texto pode ser arrastado até aqui; só apps instalados viram favoritos.
        if gio_unix::DesktopAppInfo::new(id).is_none() {
            return false;
        }
        let target = self.drag.borrow().as_ref().and_then(|d| Some((d.others.clone(), d.index?)));
        let Some((others, index)) = target else { return false };
        let before = others.get(index).map(String::as_str);
        self.save_favorites_later(move_favorite(&self.favorites(), id, before));
        true
    }

    // Terminou (soltou ou cancelou): a dock volta a ser desenhada a partir dos favoritos.
    // Fica para o idle: o GTK pode encerrar o arrasto no meio de outro callback
    // (até dentro do begin_drag, se cancelar ao esconder o botão).
    fn end_drag(self: &Rc<Self>) {
        let weak = Rc::downgrade(self);
        glib::idle_add_local_once(move || {
            if let Some(dock) = weak.upgrade() {
                dock.drag.replace(None);
                dock.layout.replace(None);
                dock.refresh();
            }
        });
    }

    fn sync_indicators(&self) {
        // Cópias: nenhum borrow aberto enquanto os widgets mudam.
        let running = self.running.borrow().clone();
        let indicators = self.indicators.borrow().clone();
        for (id, dot) in &indicators {
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

/// Posição de destino de um arrasto: o app entra antes do favorito cujo centro
/// fica à direita do ponteiro, contado na grade medida no começo do arrasto.
fn drop_index(x: f64, x0: f64, slot: f64, count: usize) -> usize {
    (((x - x0) / slot).round().max(0.0) as usize).min(count)
}

fn gap(width: f64) -> gtk::Box {
    gtk::Box::builder().css_classes(["gap"]).width_request(width as i32).build()
}

fn animate_width(widget: &gtk::Box, to: f64, done: impl Fn(&adw::TimedAnimation) + 'static) {
    let target = adw::CallbackAnimationTarget::new(glib::clone!(
        #[weak]
        widget,
        move |width| widget.set_width_request(width as i32)
    ));
    let animation = adw::TimedAnimation::new(widget, f64::from(widget.width_request()), to, 200, target);
    animation.set_easing(adw::Easing::EaseOutCubic);
    animation.connect_done(done);
    animation.play();
}

fn close_gap(gap: gtk::Box) {
    animate_width(&gap.clone(), 0.0, move |_| {
        if let Some(parent) = gap.parent().and_downcast::<gtk::Box>() {
            parent.remove(&gap);
        }
    });
}

/// Tira `id` dos favoritos e o põe antes de `before`, ou no fim.
fn move_favorite(favorites: &[String], id: &str, before: Option<&str>) -> Vec<String> {
    let mut moved: Vec<String> = favorites.iter().filter(|f| *f != id).cloned().collect();
    let index = before.and_then(|b| moved.iter().position(|f| f == b)).unwrap_or(moved.len());
    moved.insert(index, id.to_owned());
    moved
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
    fn moving_a_favorite_keeps_the_others_in_order() {
        let favorites: Vec<String> = ["a", "b", "c"].map(String::from).into();
        assert_eq!(move_favorite(&favorites, "c", Some("a")), ["c", "a", "b"]);
        assert_eq!(move_favorite(&favorites, "a", None), ["b", "c", "a"]);
        // um app novo entra como favorito; um `before` que sumiu manda para o fim
        assert_eq!(move_favorite(&favorites, "d", Some("b")), ["a", "d", "b", "c"]);
        assert_eq!(move_favorite(&favorites, "d", Some("x")), ["a", "b", "c", "d"]);
    }

    #[test]
    fn drop_index_follows_slot_centers() {
        // vagas de 50px a partir de x=10: centros em 35, 85, 135
        assert_eq!(drop_index(0.0, 10.0, 50.0, 3), 0);
        assert_eq!(drop_index(34.0, 10.0, 50.0, 3), 0);
        assert_eq!(drop_index(36.0, 10.0, 50.0, 3), 1);
        assert_eq!(drop_index(500.0, 10.0, 50.0, 3), 3);
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
