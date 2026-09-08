use neuradex::{core, ui};
use neuradex::ui::MainWindow;
use adw::prelude::*;
use adw::Application;

const APP_ID: &str = "io.github.bazinfla.NeuraDex";

fn main() -> glib::ExitCode {
    // Initialize tracing subscriber for structured logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "neuradex=info".into()),
        )
        .init();

    tracing::info!("Starting NeuraDex v{}", env!("CARGO_PKG_VERSION"));

    // Initialize Tokio runtime and enter context
    let rt = core::runtime();
    let _guard = rt.enter();

    // Initialize Libadwaita
    adw::init().expect("Failed to initialize Libadwaita");

    // Register icon search paths for GNOME dock and UI
    if let Some(display) = gtk4::gdk::Display::default() {
        let icon_theme = gtk4::IconTheme::for_display(&display);
        icon_theme.add_search_path("data/icons");
        icon_theme.add_search_path("data/icons/models");
        icon_theme.add_search_path("neuradex/data/icons");
        icon_theme.add_search_path("neuradex/data/icons/models");
        if let Ok(home) = std::env::var("HOME") {
            icon_theme.add_search_path(format!("{}/.local/share/icons", home));
            icon_theme.add_search_path(format!("{}/.local/share/icons/hicolor", home));
            icon_theme.add_search_path(format!("{}/.local/share/icons/hicolor/scalable/apps", home));
        }
        icon_theme.add_search_path("/usr/share/icons/hicolor/scalable/apps");
    }

    gtk4::Window::set_default_icon_name(APP_ID);

    let app = Application::builder()
        .application_id(APP_ID)
        .build();

    app.connect_activate(|app| {
        // Load custom CSS design system
        ui::style::load_custom_styles();

        let window = MainWindow::new(app);
        window.present();
    });

    app.run()
}
