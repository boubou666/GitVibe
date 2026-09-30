#[cfg(all(target_os = "windows", target_env = "msvc"))]
fn main() {
    use std::{env, fs, path::PathBuf, process::Command};

    println!("cargo:rerun-if-changed=assets/gitvibe.ico");
    let output =
        PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR is set by Cargo")).join("gitvibe.res");
    let architecture = match env::var("CARGO_CFG_TARGET_ARCH").as_deref() {
        Ok("aarch64") => "arm64",
        Ok("x86") => "x86",
        _ => "x64",
    };
    let kit_root = PathBuf::from(env::var_os("ProgramFiles(x86)").expect("Windows SDK location"))
        .join("Windows Kits/10/bin");
    let mut sdk_versions: Vec<_> = fs::read_dir(&kit_root)
        .expect("Windows SDK is required for the app icon")
        .filter_map(Result::ok)
        .filter(|entry| entry.path().join(architecture).join("rc.exe").is_file())
        .collect();
    sdk_versions.sort_by_key(|entry| entry.file_name());
    let rc = sdk_versions
        .last()
        .expect("Windows SDK rc.exe is required for the app icon")
        .path()
        .join(architecture)
        .join("rc.exe");
    let status = Command::new(rc)
        .arg(format!("/fo{}", output.display()))
        .arg("assets/gitvibe.rc")
        .status()
        .expect("launch Windows resource compiler");
    assert!(status.success(), "Windows resource compiler failed");
    println!("cargo:rustc-link-arg-bins={}", output.display());
}

#[cfg(not(all(target_os = "windows", target_env = "msvc")))]
fn main() {}
