use adw::prelude::*;

const APP_ID: &str = "io.github.augustcaio.Atol.Dock";

// A janela é transparente; só a pílula da dock tem fundo.
const CSS: &str = "
window.atol { background: none; }
.dock { background-color: alpha(@window_bg_color, 0.85); border-radius: 24px; }
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
        let dock = gtk::Box::builder()
            .css_classes(["dock"])
            .width_request(480)
            .height_request(64)
            .build();
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
