#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() -> eframe::Result {
    #[cfg(target_os = "windows")]
    {
        let app_id: Vec<u16> = hris_desktop::APP_ID.encode_utf16().chain(Some(0)).collect();
        // SAFETY: app_id is a valid, null-terminated UTF-16 buffer for this call.
        unsafe {
            windows_sys::Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID(app_id.as_ptr());
        }
    }
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_app_id(hris_desktop::APP_ID)
            .with_inner_size([1320.0, 880.0])
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
