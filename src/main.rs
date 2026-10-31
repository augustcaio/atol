use adw::prelude::*;

const APP_ID: &str = "io.github.augustcaio.Atol.Dock";

fn main() -> gtk::glib::ExitCode {
    adw::Application::builder()
        .application_id(APP_ID)
        .build()
        .run()
}
