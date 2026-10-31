use adw::prelude::*;

const APP_ID: &str = "io.github.augustcaio.Atol.Dock";

// A dock é uma casca vazia: o conteúdo vem dos widgets que o usuário acopla a ela.
// Sem widgets, ela é um quadrado de 56 px de vidro.
//
// A janela é transparente; só a dock tem fundo. A margem afasta a dock da borda da
// tela e dá espaço para a sombra. O backdrop-filter vira blur do compositor
// (protocolo ext-background-effect do Mutter 51).
// Medidas em px inteiros: em rem (1rem ≈ 14.7px) tudo cai em fração de pixel e borra.
const CSS: &str = "
window.atol { background: none; }
.dock {
    min-width: 56px;
    min-height: 56px;
    /* espaço para a sombra em volta; a dock flutua acima da borda da tela */
    margin: 12px 16px 16px;
    /* tema padrão: mais quadrado, cantos levemente arredondados */
    border-radius: 12px;
    /* Vidro translúcido, sem bordas. No tema claro ele escurece o bastante para conteúdo
       branco ter 3:1 (WCAG) até sobre papel de parede branco; no escuro (.dark) fica
       transparente. O blur é do Mutter: força e saturação vêm da atol-bridge (o raio do
       blur() abaixo não chega ao compositor). */
    background-color: alpha(black, 0.4);
    backdrop-filter: blur(24px);
    box-shadow: 0 4px 16px alpha(black, 0.25);
}
.dock.dark { background-color: transparent; }
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
        let dock = gtk::Box::builder().css_classes(["dock"]).build();
        // O CSS do app não recebe o tema (o @media prefers-color-scheme não muda nada);
        // quem sabe é o StyleManager da libadwaita.
        let style = adw::StyleManager::default();
        let widget = dock.clone();
        let sync = move |style: &adw::StyleManager| {
            if style.is_dark() {
                widget.add_css_class("dark");
            } else {
                widget.remove_css_class("dark");
            }
        };
        sync(&style);
        style.connect_dark_notify(sync);
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
