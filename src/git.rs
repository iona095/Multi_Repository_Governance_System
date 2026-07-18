use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

#[derive(Debug)]
pub struct GitOutput {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub status: std::process::ExitStatus,
}

/// Forbidden inherited Git environment variables that must never reach a child.
/// These are removed explicitly in addition to `env_clear` so the production
/// path can prove their absence.
const FORBIDDEN_GIT_VARS: &[&str] = &[
    "GIT_CONFIG_PARAMETERS",
    "GIT_SHALLOW_FILE",
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_COMMON_DIR",
    "GIT_INDEX_FILE",
    "GIT_OBJECT_DIRECTORY",
    "GIT_ALTERNATE_OBJECT_DIRECTORIES",
    "GIT_NAMESPACE",
    "GIT_PREFIX",
    "GIT_QUARANTINE_PATH",
    "GIT_CONFIG_SYSTEM",
    "GIT_CONFIG_GLOBAL",
    "GIT_CONFIG_COUNT",
];

#[cfg(windows)]
const MIN_OS_VARS: &[&str] = &[
    "PATH",
    "SYSTEMROOT",
    "WINDIR",
    "COMSPEC",
    "USERPROFILE",
    "TEMP",
    "TMP",
];

#[cfg(not(windows))]
const MIN_OS_VARS: &[&str] = &["PATH", "HOME", "TMPDIR"];

/// Resolve the absolute path of the `git` executable from the current
/// process search path. Cached for the lifetime of the process so every child
/// uses the identical resolved binary.
fn resolved_git() -> &'static PathBuf {
    static GIT_PATH: OnceLock<PathBuf> = OnceLock::new();
    GIT_PATH.get_or_init(|| {
        if let Some(p) = search_git_on_path() {
            return p;
        }
        // Fall back to the bare name; the OS loader will resolve it.
        PathBuf::from("git")
    })
}

fn search_git_on_path() -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    let exe_names: &[&str] = if cfg!(windows) {
        &["git.exe", "git.cmd", "git"]
    } else {
        &["git"]
    };
    for dir in std::env::split_paths(&path_var) {
        for name in exe_names {
            let candidate = dir.join(name);
            if candidate.is_file() {
                if let Ok(canon) = std::fs::canonicalize(&candidate) {
                    return Some(canon);
                }
                return Some(candidate);
            }
        }
    }
    None
}

pub struct GitRunner {
    repo: PathBuf,
}

impl GitRunner {
    pub fn new(repo: &Path) -> Self {
        Self {
            repo: repo.to_path_buf(),
        }
    }

    fn build_cmd(&self) -> Command {
        let mut cmd = Command::new(resolved_git());
        cmd.stdin(std::process::Stdio::null());

        cmd.arg("--no-replace-objects");
        cmd.arg("--no-lazy-fetch");
        cmd.arg("--literal-pathspecs");
        cmd.arg("-c");
        cmd.arg("core.fsmonitor=false");
        cmd.arg("-c");
        cmd.arg("core.untrackedCache=false");
        cmd.arg("-c");
        cmd.arg("diff.external=");
        cmd.arg("-C");
        cmd.arg(&self.repo);

        // Remove every inherited environment value first.
        cmd.env_clear();

        // Explicitly delete the forbidden Git variables so their absence is
        // provable even if `env_clear` semantics ever change.
        for var in FORBIDDEN_GIT_VARS {
            cmd.env_remove(var);
        }
        // Remove any inherited GIT_CONFIG_KEY_*/GIT_CONFIG_VALUE_* variables.
        for (k, _v) in std::env::vars_os() {
            let key = k.to_string_lossy();
            if key.starts_with("GIT_CONFIG_KEY_") || key.starts_with("GIT_CONFIG_VALUE_") {
                cmd.env_remove(&k);
            }
        }

        // Restore only the minimum documented operating-system variables
        // required to start the resolved Git executable on this target.
        for var in MIN_OS_VARS {
            if let Ok(val) = std::env::var(var) {
                cmd.env(var, val);
            }
        }

        // Contract-required Git controls, set exactly.
        cmd.env("GIT_OPTIONAL_LOCKS", "0");
        cmd.env("GIT_CONFIG_NOSYSTEM", "1");
        cmd.env("GIT_ATTR_NOSYSTEM", "1");
        cmd.env("GIT_NO_LAZY_FETCH", "1");

        cmd
    }

    pub fn run<I, S>(&self, args: I) -> Result<GitOutput, crate::error::Error>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let mut cmd = self.build_cmd();
        cmd.args(args);
        let output = cmd
            .output()
            .map_err(|e| crate::error::Error::GitCommandFailed(format!("spawn failed: {}", e)))?;
        Ok(GitOutput {
            stdout: output.stdout,
            stderr: output.stderr,
            status: output.status,
        })
    }

    pub fn run_stdout_string<I, S>(&self, args: I) -> Result<String, crate::error::Error>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let out = self.run(args)?;
        if !out.status.success() {
            return Err(crate::error::Error::GitCommandFailed(
                "git command failed".into(),
            ));
        }
        String::from_utf8(out.stdout)
            .map_err(|_| crate::error::Error::GitCommandFailed("non-UTF-8 output".into()))
    }

    pub fn repo_path(&self) -> &Path {
        &self.repo
    }
}
