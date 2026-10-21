#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() -> eframe::Result {
    if std::env::args().nth(1).as_deref() == Some("--apply-update") {
        if let Some(path) = std::env::args_os().nth(2) {
            if let Err(error) = hris_desktop::updater::apply_plan(std::path::Path::new(&path)) {
                // No credentials are in an update plan or its local failure diagnostics.
                eprintln!("HRIS update failed: {error}");
                std::process::exit(1);
            }
        } else {
            std::process::exit(1);
        }
        return Ok(());
    }
    #[cfg(target_os = "windows")]
    {
        let app_id: Vec<u16> = hris_desktop::APP_ID.encode_utf16().chain(Some(0)).collect();
        // SAFETY: app_id is a valid, null-terminated UTF-16 buffer for this call.
        unsafe {
            windows_sys::Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID(app_id.as_ptr());
        }
    }
    let window_size = [1320.0, 880.0];
    #[cfg(feature = "qa-screenshot")]
    let window_size = if std::env::var("HRIS_PREVIEW_SIZE").as_deref() == Ok("compact") {
        [1000.0, 720.0]
    } else {
        window_size
    };
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_app_id(hris_desktop::APP_ID)
            .with_inner_size(window_size)
            .with_min_inner_size([1000.0, 720.0]),
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };
    eframe::run_native(
        "HRIS Desktop",
        options,
        Box::new(|cc| Ok(Box::new(hris_desktop::ui::HrisApp::new(cc)))),
    )
}
