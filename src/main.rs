mod app;
mod git;
mod github;
mod updates;

fn main() -> eframe::Result {
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
    eframe::run_native(
        "GitVibe",
        options,
        Box::new(|cc| Ok(Box::new(app::GitVibe::new(cc)))),
    )
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
    const SIZE: usize = 64;
    let mut rgba = vec![0u8; SIZE * SIZE * 4];
    for y in 0..SIZE {
        for x in 0..SIZE {
            let rounded =
                (x < 10 && y < 10 && (x as i32 - 10).pow(2) + (y as i32 - 10).pow(2) > 100)
                    || (x > 53 && y < 10 && (x as i32 - 53).pow(2) + (y as i32 - 10).pow(2) > 100)
                    || (x < 10 && y > 53 && (x as i32 - 10).pow(2) + (y as i32 - 53).pow(2) > 100)
                    || (x > 53 && y > 53 && (x as i32 - 53).pow(2) + (y as i32 - 53).pow(2) > 100);
            if rounded {
                continue;
            }
            let mut color = [21, 36, 48, 255];
            let main_line = (x as i32 - 23).abs() <= 3 && (14..=50).contains(&y);
            let branch_line = (29..=43).contains(&x) && (y as i32 - (x as i32 - 8)).abs() <= 3;
            let node = |cx: i32, cy: i32| (x as i32 - cx).pow(2) + (y as i32 - cy).pow(2) <= 47;
            if main_line || node(23, 14) || node(23, 50) {
                color = [80, 201, 189, 255];
            }
            if branch_line || node(44, 36) {
                color = [245, 165, 94, 255];
            }
            rgba[(y * SIZE + x) * 4..(y * SIZE + x + 1) * 4].copy_from_slice(&color);
        }
    }
    eframe::egui::IconData {
        rgba,
        width: SIZE as u32,
        height: SIZE as u32,
    }
}
