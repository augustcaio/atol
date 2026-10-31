use adw::prelude::*;

const APP_ID: &str = "io.github.augustcaio.Atol.Dock";

// A janela é transparente; só a pílula da dock tem fundo. A margem afasta a pílula
// da borda da tela e dá espaço para a sombra. O backdrop-filter vira blur do
// compositor (protocolo ext-background-effect do Mutter 51).
const CSS: &str = "
window.atol { background: none; }
.dock {
    min-width: 480px;
    min-height: 64px;
    margin: 8px 16px 12px;
    border-radius: 28px;
    background-color: alpha(@window_bg_color, 0.35);
    background-image: linear-gradient(to bottom, alpha(white, 0.14), alpha(white, 0.02));
    backdrop-filter: blur(24px);
    box-shadow: 0 6px 20px alpha(black, 0.25);
}
";

fn main() -> gtk::glib::ExitCode {
    let app = adw::Application::builder().application_id(APP_ID).build();

    app.connect_startup(|_| {
        let css = gtk::CssProvider::new();
        css.load_from_string(CSS);
        gtk::style_context_add_provider_for_display(
            &gtk::gdk::Display::default().expect("sem display"),
            &css,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    });

    app.connect_activate(|app| {
        let dock = gtk::Box::builder().css_classes(["dock"]).build();
        gtk::ApplicationWindow::builder()
            .application(app)
            .title("Atol")
            .decorated(false)
            .resizable(false)
            .css_classes(["atol"])
            .child(&dock)
            .build()
            .present();
    });

    app.run()
}
