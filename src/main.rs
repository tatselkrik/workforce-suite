#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
fn main() -> hris_desktop::desktop::AppResult {
    hris_desktop::desktop::run(hris_desktop::desktop::Application::hris())
}
