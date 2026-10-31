// Cliente D-Bus da atol-bridge, a extensão do GNOME Shell que conhece as janelas.
use gtk::glib::{self, variant::ToVariant};
use gtk::gio;

const NAME: &str = "io.github.augustcaio.Atol.Bridge";
const PATH: &str = "/io/github/augustcaio/Atol/Bridge";

/// App aberto, como o Shell o vê.
#[derive(Clone, Debug, PartialEq)]
pub struct RunningApp {
    pub id: String,
    pub focused: bool,
    pub urgent: bool,
}

/// Chama um método da ponte sem esperar resposta; uma falha só vai para o log.
pub fn call(conn: &gio::DBusConnection, method: &'static str, args: impl ToVariant) {
    conn.call(
        Some(NAME),
        PATH,
        NAME,
        method,
        Some(&args.to_variant()),
        None,
        gio::DBusCallFlags::NONE,
        -1,
        gio::Cancellable::NONE,
        move |res| {
            if let Err(e) = res {
                glib::g_warning!("atol", "ponte: {method} falhou: {e}");
            }
        },
    );
}

/// Chama `on_apps` com os apps abertos sempre que a ponte aparece, avisa mudança ou some.
/// A assinatura do sinal vale enquanto o retorno existir.
pub fn watch(
    conn: &gio::DBusConnection,
    on_apps: impl Fn(Vec<RunningApp>) + Clone + 'static,
) -> gio::SignalSubscription {
    let fetch = {
        let on_apps = on_apps.clone();
        move |conn: &gio::DBusConnection| {
            let on_apps = on_apps.clone();
            conn.call(
                Some(NAME),
                PATH,
                NAME,
                "GetApps",
                None,
                Some(glib::VariantTy::new("(a(sbb))").unwrap()),
                gio::DBusCallFlags::NONE,
                -1,
                gio::Cancellable::NONE,
                move |res| match res {
                    Ok(v) => on_apps(parse_apps(&v)),
                    Err(e) => glib::g_warning!("atol", "ponte: GetApps falhou: {e}"),
                },
            );
        }
    };

    let on_signal = fetch.clone();
    let subscription = conn.subscribe_to_signal(
        None,
        Some(NAME),
        Some("AppsChanged"),
        Some(PATH),
        None,
        gio::DBusSignalFlags::NONE,
        move |signal| on_signal(signal.connection),
    );
    // A ponte some quando a extensão é desligada (tela bloqueada, por exemplo).
    gio::bus_watch_name_on_connection(
        conn,
        NAME,
        gio::BusNameWatcherFlags::NONE,
        move |conn, _, _| fetch(&conn),
        move |_, _| on_apps(Vec::new()),
    );
    subscription
}

fn parse_apps(reply: &glib::Variant) -> Vec<RunningApp> {
    let (apps,): (Vec<(String, bool, bool)>,) = reply.get().unwrap_or_default();
    apps.into_iter()
        .map(|(id, focused, urgent)| RunningApp { id, focused, urgent })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_bridge_reply() {
        let reply = (vec![("org.gnome.Console.desktop", true, false)],).to_variant();
        assert_eq!(
            parse_apps(&reply),
            [RunningApp { id: "org.gnome.Console.desktop".into(), focused: true, urgent: false }]
        );
    }

    #[test]
    fn wrong_reply_type_gives_no_apps() {
        assert!(parse_apps(&("x",).to_variant()).is_empty());
    }
}
