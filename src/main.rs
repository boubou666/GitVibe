#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod app;
mod external;
mod git;
mod github;
mod logging;
mod profiles;
mod syntax;
mod updates;

fn main() -> eframe::Result {
    if let Some(code) = git::run_helper_command() {
        std::process::exit(code);
    }
    logging::init();
    #[cfg(target_os = "windows")]
    dark_titlebar::enable();
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("GitVibe")
            .with_inner_size([1480.0, 920.0])
            .with_min_inner_size([1120.0, 700.0])
            .with_icon(icon()),
        ..Default::default()
    };
    let result = eframe::run_native(
        "GitVibe",
        options,
        Box::new(|cc| Ok(Box::new(app::GitVibe::new(cc)))),
    );
    if let Err(error) = &result {
        logging::error("application", &error.to_string());
    }
    result
}

#[cfg(target_os = "windows")]
#[allow(non_snake_case)]
mod dark_titlebar {
    use std::{ffi::c_void, thread, time::Duration};

    type Hwnd = *mut c_void;

    #[link(name = "user32")]
    unsafe extern "system" {
        fn EnumWindows(callback: extern "system" fn(Hwnd, isize) -> i32, param: isize) -> i32;
        fn GetWindowThreadProcessId(window: Hwnd, process_id: *mut u32) -> u32;
        fn IsWindowVisible(window: Hwnd) -> i32;
        fn GetWindowTextW(window: Hwnd, text: *mut u16, max_count: i32) -> i32;
    }

    #[link(name = "dwmapi")]
    unsafe extern "system" {
        fn DwmSetWindowAttribute(
            window: Hwnd,
            attribute: u32,
            value: *const c_void,
            size: u32,
        ) -> i32;
    }

    struct Search {
        process_id: u32,
        found: bool,
    }

    extern "system" fn darken(window: Hwnd, param: isize) -> i32 {
        let search = unsafe { &mut *(param as *mut Search) };
        let mut process_id = 0;
        unsafe { GetWindowThreadProcessId(window, &mut process_id) };
        if process_id != search.process_id {
            return 1;
        }
        if unsafe { IsWindowVisible(window) } == 0 {
            return 1;
        }
        let mut title = [0_u16; 64];
        let length = unsafe { GetWindowTextW(window, title.as_mut_ptr(), title.len() as i32) };
        if length == 0 || String::from_utf16_lossy(&title[..length as usize]) != "GitVibe" {
            return 1;
        }
        let dark = 1_i32;
        let value = &dark as *const i32 as *const c_void;
        let size = std::mem::size_of::<i32>() as u32;
        let result = unsafe { DwmSetWindowAttribute(window, 20, value, size) };
        let result = if result != 0 {
            unsafe { DwmSetWindowAttribute(window, 19, value, size) }
        } else {
            result
        };
        search.found = result == 0;
        0
    }

    pub fn enable() {
        thread::spawn(|| {
            for _ in 0..40 {
                let mut search = Search {
                    process_id: std::process::id(),
                    found: false,
                };
                unsafe { EnumWindows(darken, &mut search as *mut Search as isize) };
                if search.found {
                    return;
                }
                thread::sleep(Duration::from_millis(100));
            }
        });
    }
}

fn icon() -> eframe::egui::IconData {
    eframe::icon_data::from_png_bytes(include_bytes!("../assets/gitvibe-icon.png"))
        .expect("bundled GitVibe icon is a valid PNG")
}
