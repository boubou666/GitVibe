use std::path::{Component, Path, PathBuf};

pub fn edit_file(repo: &Path, relative: &str) -> Result<(), String> {
    let path = resolve_file(repo, relative)?;
    open_default(&path)
}

pub fn show_in_folder(repo: &Path, relative: &str) -> Result<(), String> {
    let path = resolve_file(repo, relative)?;
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer.exe")
            .arg(format!("/select,{}", path.display()))
            .spawn()
            .map_err(|error| format!("Could not show this file in Explorer: {error}"))?;
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg("-R")
            .arg(&path)
            .spawn()
            .map_err(|error| format!("Could not show this file in Finder: {error}"))?;
    }
    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    {
        std::process::Command::new("xdg-open")
            .arg(path.parent().expect("resolved file has a parent"))
            .spawn()
            .map_err(|error| format!("Could not show this file in the file manager: {error}"))?;
    }
    Ok(())
}

fn resolve_file(repo: &Path, relative: &str) -> Result<PathBuf, String> {
    let relative = Path::new(relative);
    if relative.is_absolute()
        || relative.components().any(|part| {
            matches!(
                part,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err("The selected file is outside this repository".into());
    }
    let root = repo
        .canonicalize()
        .map_err(|error| format!("Could not locate repository: {error}"))?;
    let file = repo
        .join(relative)
        .canonicalize()
        .map_err(|error| format!("Could not open this working file: {error}"))?;
    if !file.starts_with(root) || !file.is_file() {
        return Err("The selected working file is unavailable or outside this repository".into());
    }
    Ok(file)
}

#[cfg(target_os = "windows")]
fn open_default(path: &Path) -> Result<(), String> {
    use std::{ffi::c_void, os::windows::ffi::OsStrExt};

    #[link(name = "shell32")]
    unsafe extern "system" {
        fn ShellExecuteW(
            window: *mut c_void,
            operation: *const u16,
            file: *const u16,
            parameters: *const u16,
            directory: *const u16,
            show: i32,
        ) -> *mut c_void;
    }

    let operation: Vec<u16> = "open".encode_utf16().chain(std::iter::once(0)).collect();
    let file: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let result = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            operation.as_ptr(),
            file.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            1,
        )
    } as usize;
    if result <= 32 {
        Err(format!(
            "Could not open this file in its default app (Windows error {result})"
        ))
    } else {
        Ok(())
    }
}

#[cfg(not(target_os = "windows"))]
fn open_default(path: &Path) -> Result<(), String> {
    use std::process::Command;

    #[cfg(target_os = "macos")]
    let opener = "open";
    #[cfg(not(target_os = "macos"))]
    let opener = "xdg-open";
    Command::new(opener)
        .arg(path)
        .spawn()
        .map_err(|error| format!("Could not open this file in its default app: {error}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::resolve_file;
    use std::path::Path;

    #[test]
    fn refuses_paths_outside_repository() {
        assert!(resolve_file(Path::new("."), "../secret").is_err());
        assert!(resolve_file(Path::new("."), "C:/other/file").is_err());
    }
}
