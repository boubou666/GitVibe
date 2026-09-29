use std::{
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
    process::Command,
    time::Duration,
};

use semver::Version;
use serde::Deserialize;
use sha2::{Digest, Sha256};

const LATEST_RELEASE: &str = "https://api.github.com/repos/boubou666/GitVibe/releases/latest";
const MAX_DOWNLOAD_SIZE: u64 = 512 * 1024 * 1024;

#[derive(Clone)]
pub struct Release {
    pub version: String,
    pub page_url: String,
    pub notes: String,
    pub asset: Asset,
}

#[derive(Clone)]
pub struct Asset {
    name: String,
    url: String,
    sha256: String,
    size: u64,
}

pub enum Check {
    UpToDate(String),
    Available(Release),
}

#[derive(Deserialize)]
struct ApiRelease {
    tag_name: String,
    html_url: String,
    body: Option<String>,
    assets: Vec<ApiAsset>,
}

#[derive(Deserialize)]
struct ApiAsset {
    name: String,
    browser_download_url: String,
    digest: Option<String>,
    size: u64,
}

fn agent(timeout: Duration) -> ureq::Agent {
    let config = ureq::Agent::config_builder()
        .https_only(true)
        .timeout_global(Some(timeout))
        .build();
    ureq::Agent::new_with_config(config)
}

pub fn check_latest() -> Result<Check, String> {
    let mut response = agent(Duration::from_secs(20))
        .get(LATEST_RELEASE)
        .header("User-Agent", concat!("GitVibe/", env!("CARGO_PKG_VERSION")))
        .header("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| format!("Could not reach GitHub Releases: {e}"))?;
    let body = response
        .body_mut()
        .read_to_string()
        .map_err(|e| format!("Could not read release information: {e}"))?;
    parse_release(&body, env!("CARGO_PKG_VERSION"))
}

fn parse_release(json: &str, current: &str) -> Result<Check, String> {
    let raw: ApiRelease = serde_json::from_str(json)
        .map_err(|e| format!("GitHub returned invalid release information: {e}"))?;
    let current_version = Version::parse(current).map_err(|e| e.to_string())?;
    let latest_version = Version::parse(raw.tag_name.strip_prefix('v').unwrap_or(&raw.tag_name))
        .map_err(|e| format!("Latest release has an invalid version: {e}"))?;
    if latest_version <= current_version {
        return Ok(Check::UpToDate(raw.tag_name));
    }
    let expected_name =
        platform_asset_name().ok_or("No update package exists for this platform")?;
    let raw_asset = raw
        .assets
        .into_iter()
        .find(|asset| asset.name == expected_name)
        .ok_or_else(|| format!("Release {} has no {expected_name} package", raw.tag_name))?;
    let expected_url = format!(
        "https://github.com/boubou666/GitVibe/releases/download/{}/{expected_name}",
        raw.tag_name
    );
    if raw_asset.browser_download_url != expected_url {
        return Err("The release download URL did not match GitVibe's repository".into());
    }
    if !(1..=MAX_DOWNLOAD_SIZE).contains(&raw_asset.size) {
        return Err("The release package size is invalid".into());
    }
    let sha256 = raw_asset
        .digest
        .as_deref()
        .and_then(|digest| digest.strip_prefix("sha256:"))
        .filter(|digest| digest.len() == 64 && digest.bytes().all(|b| b.is_ascii_hexdigit()))
        .ok_or("The release package has no valid SHA-256 digest")?
        .to_ascii_lowercase();
    if raw.html_url
        != format!(
            "https://github.com/boubou666/GitVibe/releases/tag/{}",
            raw.tag_name
        )
    {
        return Err("The release page URL did not match GitVibe's repository".into());
    }
    Ok(Check::Available(Release {
        version: raw.tag_name,
        page_url: raw.html_url,
        notes: raw.body.unwrap_or_default(),
        asset: Asset {
            name: raw_asset.name,
            url: raw_asset.browser_download_url,
            sha256,
            size: raw_asset.size,
        },
    }))
}

fn platform_asset_name() -> Option<&'static str> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("windows", "x86_64") => Some("GitVibe-windows-X64-setup.exe"),
        ("linux", "x86_64") => Some("GitVibe-linux-X64.tar.gz"),
        ("macos", "aarch64") => Some("GitVibe-macos-ARM64.zip"),
        ("macos", "x86_64") => Some("GitVibe-macos-X64.zip"),
        _ => None,
    }
}

fn download_directory(version: &str) -> PathBuf {
    #[cfg(target_os = "windows")]
    let base = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("GitVibe")
        .join("Updates");
    #[cfg(not(target_os = "windows"))]
    let base = std::env::var_os("HOME")
        .map(PathBuf::from)
        .map(|home| home.join("Downloads"))
        .filter(|downloads| downloads.is_dir())
        .unwrap_or_else(std::env::temp_dir)
        .join("GitVibe");
    base.join(version)
}

fn sha256_file(path: &Path) -> Result<String, String> {
    let mut file = File::open(path).map_err(|e| e.to_string())?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let count = file.read(&mut buffer).map_err(|e| e.to_string())?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

pub fn download(release: &Release) -> Result<PathBuf, String> {
    let directory = download_directory(&release.version);
    fs::create_dir_all(&directory).map_err(|e| format!("Could not create update folder: {e}"))?;
    let destination = directory.join(&release.asset.name);
    if destination.is_file()
        && fs::metadata(&destination).map_err(|e| e.to_string())?.len() == release.asset.size
        && sha256_file(&destination)? == release.asset.sha256
    {
        return Ok(destination);
    }
    let partial = directory.join(format!(
        "{}.{}.part",
        release.asset.name,
        std::process::id()
    ));
    let result = (|| -> Result<(), String> {
        let mut response = agent(Duration::from_secs(300))
            .get(&release.asset.url)
            .header("User-Agent", concat!("GitVibe/", env!("CARGO_PKG_VERSION")))
            .call()
            .map_err(|e| format!("Could not download update: {e}"))?;
        let mut reader = response.body_mut().as_reader();
        let mut file = File::create(&partial).map_err(|e| e.to_string())?;
        let mut hasher = Sha256::new();
        let mut size = 0u64;
        let mut buffer = [0u8; 64 * 1024];
        loop {
            let count = reader.read(&mut buffer).map_err(|e| e.to_string())?;
            if count == 0 {
                break;
            }
            size += count as u64;
            if size > release.asset.size || size > MAX_DOWNLOAD_SIZE {
                return Err("Downloaded package is larger than expected".into());
            }
            file.write_all(&buffer[..count])
                .map_err(|e| e.to_string())?;
            hasher.update(&buffer[..count]);
        }
        file.sync_all().map_err(|e| e.to_string())?;
        if size != release.asset.size || format!("{:x}", hasher.finalize()) != release.asset.sha256
        {
            return Err("Downloaded package failed SHA-256 verification".into());
        }
        if destination.exists() {
            fs::remove_file(&destination).map_err(|e| e.to_string())?;
        }
        fs::rename(&partial, &destination).map_err(|e| e.to_string())?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&partial);
    }
    result.map(|()| destination)
}

pub fn open_download(path: &Path) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    let mut command = Command::new(path);
    #[cfg(target_os = "macos")]
    let mut command = {
        let mut command = Command::new("open");
        command.arg(path);
        command
    };
    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    let mut command = {
        let mut command = Command::new("xdg-open");
        command.arg(path);
        command
    };
    command
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("Could not open downloaded update: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_new_release_and_rejects_untrusted_asset() {
        let name = platform_asset_name().unwrap();
        let json = format!(
            r#"{{"tag_name":"v0.5.0","html_url":"https://github.com/boubou666/GitVibe/releases/tag/v0.5.0","body":"Notes","assets":[{{"name":"{name}","browser_download_url":"https://github.com/boubou666/GitVibe/releases/download/v0.5.0/{name}","digest":"sha256:{}","size":123}}]}}"#,
            "a".repeat(64)
        );
        assert!(matches!(
            parse_release(&json, "0.4.0"),
            Ok(Check::Available(_))
        ));
        assert!(matches!(
            parse_release(&json, "0.5.0"),
            Ok(Check::UpToDate(_))
        ));
        assert!(parse_release(&json.replace("sha256:", "sha1:"), "0.4.0").is_err());
        assert!(
            parse_release(
                &json.replace("github.com/boubou666", "evil.example/boubou666"),
                "0.4.0"
            )
            .is_err()
        );
    }

    #[test]
    fn hashes_downloaded_files() {
        let path = std::env::temp_dir().join(format!("gitvibe-hash-{}", std::process::id()));
        fs::write(&path, b"abc").unwrap();
        assert_eq!(
            sha256_file(&path).unwrap(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        fs::remove_file(path).unwrap();
    }

    #[test]
    #[ignore = "requires access to GitHub Releases"]
    fn reads_public_release() {
        assert!(matches!(
            check_latest().unwrap(),
            Check::UpToDate(_) | Check::Available(_)
        ));
    }
}
