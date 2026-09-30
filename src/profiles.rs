use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Profile {
    pub label: String,
    pub name: String,
    pub email: String,
}

impl Profile {
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.label.trim().is_empty() || self.label.contains(['\n', '\r']) {
            return Err("Enter a profile name");
        }
        if self.name.trim().is_empty() || self.name.contains(['\n', '\r', '<', '>']) {
            return Err("Enter a valid commit author name");
        }
        let email = self.email.trim();
        if !email.split_once('@').is_some_and(|(local, domain)| {
            !local.is_empty()
                && domain.contains('.')
                && !domain.ends_with('.')
                && !domain.contains('@')
        }) || email.contains(['\n', '\r', '<', '>', ' ', '\t'])
        {
            return Err("Enter a valid commit author email");
        }
        Ok(())
    }

    pub fn normalized(mut self) -> Self {
        self.label = self.label.trim().to_owned();
        self.name = self.name.trim().to_owned();
        self.email = self.email.trim().to_owned();
        self
    }

    pub fn scoped_args(&self, args: Vec<String>) -> Vec<String> {
        let mut command = vec![
            "-c".into(),
            format!("user.name={}", self.name),
            "-c".into(),
            format!("user.email={}", self.email),
        ];
        command.extend(args);
        command
    }

    pub fn commit_args(&self, message: String) -> Vec<String> {
        self.scoped_args(vec!["commit".into(), "-m".into(), message])
    }
}

#[cfg(test)]
mod tests {
    use super::Profile;
    use crate::git;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn validates_identity_and_builds_scoped_commit_args() {
        let profile = Profile {
            label: " Work ".into(),
            name: " Ada Lovelace ".into(),
            email: " ada@example.com ".into(),
        }
        .normalized();
        assert_eq!(profile.label, "Work");
        assert!(profile.validate().is_ok());
        assert_eq!(
            profile.commit_args("A change".into()),
            [
                "-c",
                "user.name=Ada Lovelace",
                "-c",
                "user.email=ada@example.com",
                "commit",
                "-m",
                "A change"
            ]
        );
        for email in ["broken", "a@", "a@b", "a b@example.com", "a@b@c.com"] {
            assert!(
                Profile {
                    email: email.into(),
                    ..profile.clone()
                }
                .validate()
                .is_err()
            );
        }
    }

    #[test]
    fn commit_uses_active_profile_without_changing_repository_config() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("gitvibe-profile-{}-{unique}", std::process::id()));
        let repo = git::init(&path).unwrap();
        git::run(&repo, &["config", "commit.gpgsign", "false"]).unwrap();
        std::fs::write(repo.join("sample.txt"), "sample\n").unwrap();
        git::run(&repo, &["add", "sample.txt"]).unwrap();
        let profile = Profile {
            label: "Work".into(),
            name: "Profile Author".into(),
            email: "profile@example.com".into(),
        };
        git::run_owned(&repo, &profile.commit_args("Profile commit".into())).unwrap();
        assert_eq!(
            git::run(&repo, &["log", "-1", "--format=%an <%ae> | %cn <%ce>"]).unwrap(),
            "Profile Author <profile@example.com> | Profile Author <profile@example.com>"
        );
        assert!(git::run(&repo, &["config", "--local", "--get", "user.name"]).is_err());
        std::fs::remove_dir_all(path).unwrap();
    }
}
