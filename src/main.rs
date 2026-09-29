mod app;
mod git;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("GitVibe")
            .with_inner_size([1420.0, 900.0])
            .with_min_inner_size([900.0, 600.0])
            .with_icon(icon()),
        ..Default::default()
    };
    eframe::run_native(
        "GitVibe",
        options,
        Box::new(|cc| Ok(Box::new(app::GitVibe::new(cc)))),
    )
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
