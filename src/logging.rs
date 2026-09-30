use std::{
    backtrace::Backtrace,
    env,
    fs::{self, OpenOptions},
    io::Write,
    path::PathBuf,
    process::Command,
    sync::{Mutex, OnceLock},
};

const MAX_LOG_BYTES: u64 = 2 * 1024 * 1024;
static LOG_PATH: OnceLock<PathBuf> = OnceLock::new();
static LOG_LOCK: Mutex<()> = Mutex::new(());

pub fn path() -> &'static PathBuf {
    LOG_PATH.get_or_init(|| {
        let base = platform_log_dir().unwrap_or_else(|| env::temp_dir().join("GitVibe"));
        let directory = if fs::create_dir_all(&base).is_ok() {
            base
        } else {
            let fallback = env::temp_dir().join("GitVibe");
            let _ = fs::create_dir_all(&fallback);
            fallback
        };
        directory.join("gitvibe.log")
    })
}

fn platform_log_dir() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        env::var_os("LOCALAPPDATA")
            .or_else(|| env::var_os("APPDATA"))
            .map(|root| PathBuf::from(root).join("GitVibe/logs"))
    }
    #[cfg(target_os = "macos")]
    {
        env::var_os("HOME").map(|home| PathBuf::from(home).join("Library/Logs/GitVibe"))
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        env::var_os("XDG_STATE_HOME")
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
            .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/state")))
            .map(|root| root.join("gitvibe"))
    }
}

pub fn init() {
    let previous_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic| {
        record("PANIC", &format!("{panic}\n{}", Backtrace::force_capture()));
        previous_hook(panic);
    }));
    record(
        "INFO",
        &format!(
            "GitVibe {} started on {} {}",
            env!("CARGO_PKG_VERSION"),
            env::consts::OS,
            env::consts::ARCH
        ),
    );
}

pub fn error(source: &str, message: &str) {
    record("ERROR", &format!("{source}: {message}"));
}

fn record(level: &str, message: &str) {
    let Ok(_guard) = LOG_LOCK.lock() else {
        return;
    };
    let file = path();
    if fs::metadata(file).is_ok_and(|meta| meta.len() >= MAX_LOG_BYTES) {
        let previous = file.with_file_name("gitvibe.previous.log");
        let _ = fs::remove_file(&previous);
        let _ = fs::rename(file, previous);
    }
    let timestamp = time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| "unknown-time".into());
    if let Ok(mut output) = OpenOptions::new().create(true).append(true).open(file) {
        let message = sanitize(message);
        let _ = writeln!(output, "[{timestamp}] {level} {message}");
    }
}

fn sanitize(message: &str) -> String {
    let mut redacted = String::with_capacity(message.len().min(8192));
    let mut rest = message;
    while let Some((start, scheme_len)) = ["https://", "http://", "ssh://"]
        .iter()
        .filter_map(|scheme| rest.find(scheme).map(|position| (position, scheme.len())))
        .min_by_key(|(position, _)| *position)
    {
        redacted.push_str(&rest[..start + scheme_len]);
        rest = &rest[start + scheme_len..];
        let authority_end = rest
            .find(['/', '?', '#', ' ', '\n', '\r'])
            .unwrap_or(rest.len());
        if let Some(at) = rest[..authority_end].find('@') {
            redacted.push_str("[redacted]@");
            rest = &rest[at + 1..];
        }
    }
    redacted.push_str(rest);
    redacted.chars().take(8192).collect()
}

pub fn open_folder() -> Result<(), String> {
    let folder = path().parent().ok_or("Log folder is unavailable")?;
    #[cfg(target_os = "windows")]
    let mut command = Command::new("explorer.exe");
    #[cfg(target_os = "macos")]
    let mut command = Command::new("open");
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let mut command = Command::new("xdg-open");
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000);
    }
    command
        .arg(folder)
        .spawn()
        .map_err(|error| format!("Could not open log folder: {error}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::sanitize;

    #[test]
    fn hides_credentials_in_remote_urls() {
        assert_eq!(
            sanitize("fatal: https://name:secret@example.com/repo and ssh://user@host/path"),
            "fatal: https://[redacted]@example.com/repo and ssh://[redacted]@host/path"
        );
    }
}
