use adw::prelude::*;

mod bridge;
mod dock;

const APP_ID: &str = "io.github.augustcaio.Atol.Dock";

// A janela é transparente; só a pílula da dock tem fundo. A margem afasta a pílula
// da borda da tela e dá espaço para a sombra. O backdrop-filter vira blur do
// compositor (protocolo ext-background-effect do Mutter 51).
// Medidas em px inteiros, como o Dash do GNOME: em rem (1rem ≈ 14.7px) tudo caía em
// fração de pixel e o ponto de app aberto saía borrado e torto.
//
// Grade: ícone 32 + 8 de padding = hover 48. Recuo de 4 em volta de cada hover, até a
// borda e entre vizinhos (na lateral, 2 de padding + 2 de margem do item): pílula de 56.
// O recuo precisa ser par: entre vizinhos ele é a soma das duas margens.
const CSS: &str = "
window.atol { background: none; }
.dock {
    padding: 4px 2px;
    /* espaço para a sombra em volta; a pílula flutua acima da borda da tela */
    margin: 12px 16px 16px;
    /* Liquid Glass da Apple: cantos quase de cápsula (~40% da altura de 56) */
    border-radius: 22px;
    /* Vidro sem brilho no miolo, só o papel de parede desfocado. No tema claro ele escurece
       o bastante para ponto e grade brancos terem 3:1 (WCAG) até sobre papel de parede branco;
       no escuro (.dark) fica transparente.
       O blur é do Mutter: força e saturação vêm da atol-bridge (o raio do blur() abaixo
       não chega ao compositor). Refração, o protocolo não oferece. */
    background-color: alpha(black, 0.4);
    backdrop-filter: blur(24px);
    /* A espessura do vidro vem só das bordas: filete especular em cima, reflexo embaixo
       e nas laterais, contorno fino para fundos claros e sombra suave. Nada no miolo. */
    box-shadow:
        inset 0 1px 0 alpha(white, 0.45),
        inset 0 -1px 0 alpha(white, 0.15),
        inset 1px 0 0 alpha(white, 0.1),
        inset -1px 0 0 alpha(white, 0.1),
        0 0 0 1px alpha(black, 0.12),
        0 4px 16px alpha(black, 0.25);
}
.dock.dark { background-color: transparent; }
/* Espaço entre itens como margem, e não border-spacing: assim a vaga do arrasto
   abre de 0 sem pulo. */
.dock > overlay, .dock > button { margin: 0 2px; }
/* Hovers como bonecas russas: todos do mesmo tamanho e raio, com o mesmo recuo. */
.dock button.app {
    padding: 8px;
    border-radius: 12px;
    background: none;
    box-shadow: none;
}
/* Nas pontas, o lado que encosta na borda segue a curva da pílula: raio dela menos o
   recuo (22 − 4). Primeiro app à esquerda; botão de aplicativos à direita. */
.dock > overlay:first-child button.app {
    border-top-left-radius: 18px;
    border-bottom-left-radius: 18px;
}
.dock > button.app:last-child {
    border-top-right-radius: 18px;
    border-bottom-right-radius: 18px;
}
.dock button.app:hover { background-color: alpha(white, 0.1); }
.dock button.app:active { background-color: alpha(white, 0.18); }
.dock button.app > image { -gtk-icon-size: 32px; }
/* Item sendo arrastado: fica no lugar, encolhe e apaga, como o scaleAndFade do GNOME. */
.dock > overlay { transition: opacity 250ms ease-out, transform 250ms ease-out; }
.dock > overlay.dragging { opacity: 0; transform: scale(0.5); }
.dock > separator {
    margin: 12px 2px;
    background-color: alpha(white, 0.2);
}
/* Um ponto redondo de 4px por app aberto: mais forte no app em foco, na cor de destaque se
   pede atenção. Sempre claro: no tema claro o vidro escurece, e no escuro o tema já é
   escuro. Fica no padding de baixo do hover (8px): 2px até o ícone, 2px até a borda. */
.indicator {
    min-width: 4px;
    min-height: 4px;
    margin-bottom: 2px;
    border-radius: 2px;
    background-color: alpha(white, 0.6);
}
.indicator.focused { background-color: white; }
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
        // O CSS do app não recebe o tema (o @media prefers-color-scheme não muda nada);
        // quem sabe é o StyleManager da libadwaita.
        let style = adw::StyleManager::default();
        let widget = dock.widget.clone();
        let sync = move |style: &adw::StyleManager| {
            if style.is_dark() {
                widget.add_css_class("dark");
            } else {
                widget.remove_css_class("dark");
            }
        };
        sync(&style);
        style.connect_dark_notify(sync);
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
