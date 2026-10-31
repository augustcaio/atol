// Widget lançador de aplicativos: um painel que sobe da dock com os apps instalados.
use gtk::gio;
use gtk::glib;
use gtk::prelude::*;

// Ícone provisório: o Atol ainda não tem o seu (o oficial será animado).
const ICON: &str = "view-app-grid-symbolic";
// Grade de 4 × 4 apps visíveis; o resto rola. A célula tem tamanho fixo (ver CSS) para
// que 4 linhas sejam exatamente 4 × TILE.
const COLUMNS: u32 = 4;
const ROWS: i32 = 4;
const TILE: i32 = 96;

pub fn new() -> gtk::MenuButton {
    let grid = gtk::FlowBox::builder()
        .min_children_per_line(COLUMNS)
        .max_children_per_line(COLUMNS)
        .selection_mode(gtk::SelectionMode::None)
        .homogeneous(true)
        .build();
    let scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .min_content_height(ROWS * TILE)
        .max_content_height(ROWS * TILE)
        .propagate_natural_width(true)
        .child(&grid)
        .build();
    // O vidro e a animação ficam neste widget, e não no popover: o `contents` do popover
    // é só um nó de CSS, e o GTK ignora transform nele.
    let panel = gtk::Box::builder().css_classes(["panel"]).build();
    panel.append(&scroll);
    let popover = gtk::Popover::builder()
        .has_arrow(false)
        .css_classes(["apps"])
        .child(&panel)
        .build();
    // Painel a 8 da dock: 4 do recuo do botão + 8, menos os 16 de margem para a sombra.
    popover.set_offset(0, 4);

    // Montar ~100 botões leva uns 300 ms: a lista fica pronta antes e só é refeita
    // quando apps são instalados ou removidos.
    let fill = glib::clone!(
        #[weak]
        grid,
        #[weak]
        popover,
        move || {
            grid.remove_all();
            for app in installed_apps() {
                grid.append(&tile(&app, &popover));
            }
        }
    );
    fill();
    gio::AppInfoMonitor::get().connect_changed(move |_| fill());

    // A classe dispara a animação de subida (ver CSS); tirar e pôr a reinicia.
    popover.connect_show(|popover| popover.add_css_class("open"));
    popover.connect_closed(|popover| popover.remove_css_class("open"));

    gtk::MenuButton::builder()
        .icon_name(ICON)
        .tooltip_text("Aplicativos")
        .direction(gtk::ArrowType::Up)
        .popover(&popover)
        .css_classes(["launcher"])
        .build()
}

/// Apps que aparecem em menus, em ordem alfabética.
fn installed_apps() -> Vec<gio::AppInfo> {
    let mut apps: Vec<_> = gio::AppInfo::all().into_iter().filter(|a| a.should_show()).collect();
    apps.sort_by_cached_key(|a| a.display_name().to_lowercase());
    apps
}

fn tile(app: &gio::AppInfo, popover: &gtk::Popover) -> gtk::Button {
    let icon = gtk::Image::builder().pixel_size(48).build();
    if let Some(gicon) = app.icon() {
        icon.set_from_gicon(&gicon);
    } else {
        icon.set_icon_name(Some("application-x-executable"));
    }
    let label = gtk::Label::builder()
        .label(app.display_name())
        .ellipsize(gtk::pango::EllipsizeMode::End)
        .max_width_chars(10)
        .build();
    let content = gtk::Box::new(gtk::Orientation::Vertical, 6);
    content.append(&icon);
    content.append(&label);
    let button = gtk::Button::builder()
        .child(&content)
        .tooltip_text(app.display_name())
        .css_classes(["app"])
        .build();
    let (app, popover) = (app.clone(), popover.clone());
    button.connect_clicked(move |button| {
        let context = button.display().app_launch_context();
        if let Err(e) = app.launch(&[], Some(&context)) {
            glib::g_warning!("atol", "{} não abriu: {e}", app.display_name());
        }
        popover.popdown();
    });
    button
}
