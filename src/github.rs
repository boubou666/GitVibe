use std::{
    path::Path,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Clone, serde::Deserialize)]
pub struct PullRequest {
    pub number: u64,
    pub title: String,
    #[serde(rename = "headRefName")]
    pub head: String,
    #[serde(rename = "baseRefName")]
    pub base: String,
    pub state: String,
    pub url: String,
}

fn gh(repo: &Path, args: &[&str]) -> Result<String, String> {
    let mut command = Command::new("gh");
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000);
    }
    let output = command
        .current_dir(repo)
        .args(args)
        .env("GH_PROMPT_DISABLED", "1")
        .output()
        .map_err(|error| format!("Could not start GitHub CLI: {error}. Install and sign in to gh to use pull requests."))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_owned())
    }
}

pub fn list(repo: &Path) -> Result<Vec<PullRequest>, String> {
    let text = gh(
        repo,
        &[
            "pr",
            "list",
            "--state",
            "all",
            "--limit",
            "50",
            "--json",
            "number,title,headRefName,baseRefName,state,url",
        ],
    )?;
    serde_json::from_str(&text)
        .map_err(|error| format!("Could not read GitHub pull requests: {error}"))
}

pub fn create(
    repo: &Path,
    head: &str,
    base: &str,
    title: &str,
    body: &str,
) -> Result<String, String> {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_nanos();
    let body_file = std::env::temp_dir().join(format!(
        "gitvibe-pr-body-{}-{unique}.md",
        std::process::id()
    ));
    std::fs::write(&body_file, body).map_err(|error| error.to_string())?;
    let result = gh(
        repo,
        &[
            "pr",
            "create",
            "--head",
            head,
            "--base",
            base,
            "--title",
            title,
            "--body-file",
            &body_file.to_string_lossy(),
        ],
    );
    let _ = std::fs::remove_file(body_file);
    result
}

#[cfg(test)]
mod tests {
    use super::PullRequest;

    #[test]
    fn reads_pull_request_list() {
        let data = r#"[{"number":12,"title":"Fix graph","headRefName":"feature/graph","baseRefName":"main","state":"OPEN","url":"https://github.com/example/repo/pull/12"}]"#;
        let requests: Vec<PullRequest> = serde_json::from_str(data).unwrap();
        assert_eq!(requests[0].number, 12);
        assert_eq!(requests[0].head, "feature/graph");
        assert_eq!(requests[0].base, "main");
    }
}
