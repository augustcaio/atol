use adw::prelude::*;

const APP_ID: &str = "io.github.augustcaio.Atol.Dock";

// A janela é transparente; só a pílula da dock tem fundo. A margem afasta a pílula
// da borda da tela e dá espaço para a sombra. O backdrop-filter vira blur do
// compositor (protocolo ext-background-effect do Mutter 51).
// Medidas em rem (1rem = fonte da interface, 11pt ≈ 14.7px); 1px só nos filetes de brilho.
const CSS: &str = "
window.atol { background: none; }
.dock {
    min-width: 32rem;
    min-height: 4rem;
    margin: 0.5rem 1rem 0.75rem;
    border-radius: 1.25rem;
    /* pouco preenchimento: aparece o fundo desfocado, não uma camada leitosa */
    background-color: alpha(@window_bg_color, 0.12);
    /* reflexo diagonal, como luz batendo no vidro */
    background-image: linear-gradient(135deg, alpha(white, 0.16), alpha(white, 0.03) 45%,
                                      transparent 60%, alpha(white, 0.06));
    backdrop-filter: blur(1.5rem);
    /* brilho especular na aresta de cima, reflexo fraco embaixo e brilho interno; sem contorno */
    box-shadow:
        inset 0 1px 0 alpha(white, 0.35),
        inset 0 -1px 0 alpha(white, 0.08),
        inset 0 0 0.75rem alpha(white, 0.05),
        0 0.4rem 1.25rem alpha(black, 0.2);
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
