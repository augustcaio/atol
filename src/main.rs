use adw::prelude::*;

mod bridge;
mod dock;

const APP_ID: &str = "io.github.augustcaio.Atol.Dock";

// A janela é transparente; só a pílula da dock tem fundo. A margem afasta a pílula
// da borda da tela e dá espaço para a sombra. O backdrop-filter vira blur do
// compositor (protocolo ext-background-effect do Mutter 51).
// Medidas em rem (1rem = fonte da interface, 11pt ≈ 14.7px); 1px só nos filetes de brilho.
const CSS: &str = "
window.atol { background: none; }
.dock {
    /* sem altura fixa: o botão quadrado (ícone 2.5rem + 0.75rem) mais o padding dão 4rem.
       Padding igual nos quatro lados: os cantos do hover ficam concêntricos aos da pílula. */
    padding: 0.375rem;
    border-spacing: 0.25rem;
    margin: 0.5rem 1rem 0.75rem;
    border-radius: 0.625rem;
    /* pouco preenchimento: aparece o fundo desfocado, não uma camada leitosa */
    background-color: alpha(@window_bg_color, 0.07);
    /* reflexo diagonal, como luz batendo no vidro */
    background-image: linear-gradient(135deg, alpha(white, 0.12), alpha(white, 0.03) 45%,
                                      transparent 60%, alpha(white, 0.06));
    backdrop-filter: blur(1.5rem);
    /* brilho especular na aresta de cima, reflexo fraco embaixo e brilho interno; sem contorno */
    box-shadow:
        inset 0 1px 0 alpha(white, 0.35),
        inset 0 -1px 0 alpha(white, 0.08),
        inset 0 0 0.75rem alpha(white, 0.05),
        0 0.4rem 1.25rem alpha(black, 0.2);
}
.dock button.app {
    padding: 0.375rem;
    /* raio da pílula menos o padding dela (0.625rem − 0.375rem): encaixe concêntrico */
    border-radius: 0.25rem;
    background: none;
    box-shadow: none;
}
.dock button.app:hover { background-color: alpha(white, 0.1); }
.dock button.app:active { background-color: alpha(white, 0.18); }
.dock button.app > image { -gtk-icon-size: 2.5rem; }
.dock > separator {
    margin: 0.5rem 0.125rem;
    background-color: alpha(@window_fg_color, 0.18);
}
/* um ponto por app aberto, mais largo no app em foco, na cor de destaque se pede atenção */
.indicator {
    min-width: 0.3rem;
    min-height: 0.3rem;
    margin-bottom: 0.125rem;
    border-radius: 1rem;
    background-color: alpha(@window_fg_color, 0.6);
}
.indicator.focused { min-width: 1rem; background-color: @window_fg_color; }
.indicator.urgent { background-color: @accent_bg_color; }
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
        // Abrir o Atol de novo só traz a dock que já existe.
        if let Some(window) = app.active_window() {
            window.present();
            return;
        }
        let conn = app.dbus_connection().expect("o app se registra no D-Bus antes do activate");
        let dock = dock::Dock::new(conn);
        let window = gtk::ApplicationWindow::builder()
            .application(app)
            .title("Atol")
            .decorated(false)
            .resizable(false)
            .css_classes(["atol"])
            .child(&dock.widget)
            .build();
        // A dock vive enquanto a janela existir.
        window.connect_destroy(move |_| drop(dock.clone()));
        window.present();
    });

    app.run()
}
