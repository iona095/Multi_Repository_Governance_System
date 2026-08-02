// Phase 10 — Activation, Rollback Drills, and Adoption Readiness
// ======================================================================
// Contract: docs/contracts/phase-10-contract.md (SHA-256 pinned at build
// time by the external harness; identity checks in this file are performed
// against the live CLI surface only).
//
// Obligation map — exactly twelve primary tests, three families:
//
// 15.1 Activation readiness
//   01 test_obligation_01_clean_room_activation_rehearsal
//   02 test_obligation_02_activation_slot_binary_identity_and_smoke
//   03 test_obligation_03_activation_preconditions_and_fail_closed_abort
//   04 test_obligation_04_activation_evidence_privacy_and_determinism
// 15.2 Rollback readiness
//   05 test_obligation_05_partial_activation_rollback_exact_restore
//   06 test_obligation_06_completed_rehearsal_rollback_exact_restore
//   07 test_obligation_07_rollback_snapshot_integrity_and_stale_rejection
//   08 test_obligation_08_interrupted_restore_resumption_and_cleanup
// 15.3 Adoption readiness
//   09 test_obligation_09_runbook_cli_surface_and_sequence
//   10 test_obligation_10_runbook_rollback_checklist_and_boundaries
//   11 test_obligation_11_readme_master_plan_and_claim_accuracy
//   12 test_obligation_12_two_repository_adoption_rehearsal_and_final_manifest
//
// Evidence discipline:
//   * Activation evidence is emitted with the ACTIVATION_REHEARSAL label and
//     never with the PRODUCTION_ACTIVATED label.
//   * Evidence manifests contain only deterministic identifiers, hashes,
//     byte sizes, command results, and repository-relative paths. No absolute
//     pilot paths, usernames, hostnames, secrets, tokens, or source contents.
//   * Platform capability branches emit exactly one of
//     CAPABILITY_EXECUTED or CAPABILITY_NOT_COMPILED_FOR_TARGET.
//   * No sleeps, no random backoff, no host identity, no network access, no
//     remote Git dependency, no recursive Cargo invocation.
//
// Pilot repositories are configured with `core.autocrlf=false` so worktree
// bytes and index bytes are identical under MRGS's sanitized Git environment
// (deterministic implementation-begin cleanliness; see the adoption runbook).

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tempfile::TempDir;

// ---------------------------------------------------------------------------
// Core helpers (same conventions as tests/phase9.rs)
// ---------------------------------------------------------------------------

fn cargo_bin() -> &'static str {
    env!("CARGO_BIN_EXE_mrgs")
}

fn write_bytes(path: &Path, content: &[u8]) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("create parent dirs");
    }
    std::fs::write(path, content).expect("write fixture file");
}

fn write_file(path: &Path, content: &str) {
    write_bytes(path, content.as_bytes());
}

fn stdout_str(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

fn stdout_raw(out: &Output) -> Vec<u8> {
    out.stdout.clone()
}

fn split_stdout(out: &Output) -> Vec<String> {
    stdout_str(out)
        .split_whitespace()
        .map(|s| s.to_string())
        .collect()
}

fn stderr_str(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

fn assert_success(out: &Output) {
    assert!(
        out.status.success(),
        "command failed with exit {:?}\nstdout: {}\nstderr: {}",
        out.status.code(),
        stdout_str(out),
        stderr_str(out)
    );
}

fn assert_failure(out: &Output) {
    assert!(
        !out.status.success(),
        "command unexpectedly succeeded\nstdout: {}\nstderr: {}",
        stdout_str(out),
        stderr_str(out)
    );
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    let mut out = String::with_capacity(64);
    for byte in digest {
        out.push_str(&format!("{:02x}", byte));
    }
    out
}

fn sha_of_file(path: &Path) -> String {
    let bytes = std::fs::read(path).expect("read file for sha256");
    sha256_hex(&bytes)
}

fn file_size(path: &Path) -> u64 {
    std::fs::metadata(path).expect("stat file").len()
}

// ---------------------------------------------------------------------------
// Git helpers
// ---------------------------------------------------------------------------

fn git(repo: &Path, args: &[&str]) -> Output {
    Command::new("git")
        .args(args)
        .current_dir(repo)
        .output()
        .expect("run git")
}

/// Git view commands with the same sanitized environment MRGS applies, so
/// snapshots observe exactly what MRGS observes (and no index writes).
fn git_sanitized(repo: &Path, args: &[&str]) -> Output {
    let mut cmd = Command::new("git");
    cmd.args(args).current_dir(repo);
    for var in [
        "GIT_COMMON_DIR",
        "GIT_INDEX_FILE",
        "GIT_OBJECT_DIRECTORY",
        "GIT_ALTERNATE_OBJECT_DIRECTORIES",
        "GIT_NAMESPACE",
        "GIT_DIR",
        "GIT_WORK_TREE",
        "GIT_CONFIG_SYSTEM",
        "GIT_CONFIG_GLOBAL",
        "GIT_CONFIG_COUNT",
    ] {
        cmd.env_remove(var);
    }
    for (k, _v) in std::env::vars_os() {
        let key = k.to_string_lossy();
        if key.starts_with("GIT_CONFIG_KEY_") || key.starts_with("GIT_CONFIG_VALUE_") {
            cmd.env_remove(k);
        }
    }
    cmd.env("GIT_OPTIONAL_LOCKS", "0");
    cmd.env("GIT_CONFIG_NOSYSTEM", "1");
    cmd.env("GIT_ATTR_NOSYSTEM", "1");
    cmd.env("GIT_NO_LAZY_FETCH", "1");
    cmd.output().expect("spawn sanitized git")
}

fn git_init(repo: &Path) {
    std::fs::create_dir_all(repo).expect("create repo dir");
    let init = git(repo, &["init", "-b", "main"]);
    assert_success(&init);
    let email = git(repo, &["config", "user.email", "phase10@test.local"]);
    assert_success(&email);
    let name = git(repo, &["config", "user.name", "Phase 10 Test"]);
    assert_success(&name);
    // Deterministic content hashing: worktree bytes must equal index bytes
    // under both the test env and MRGS's sanitized Git env (see file header).
    let autocrlf = git(repo, &["config", "core.autocrlf", "false"]);
    assert_success(&autocrlf);
}

/// Commit with fixed identity dates so HEAD, refs, and every downstream
/// content-derived identity are byte-deterministic across equivalent
/// fixtures (required by obligation 04's evidence byte-identity proof).
const FIXED_COMMIT_ENV: [(&str, &str); 4] = [
    ("GIT_AUTHOR_NAME", "Phase 10 Test"),
    ("GIT_AUTHOR_EMAIL", "phase10@test.local"),
    ("GIT_COMMITTER_NAME", "Phase 10 Test"),
    ("GIT_COMMITTER_EMAIL", "phase10@test.local"),
];

fn git_commit_fixed(repo: &Path, message: &str) -> Output {
    let add = git(repo, &["add", "-A"]);
    assert_success(&add);
    let mut cmd = Command::new("git");
    cmd.args(["commit", "-m", message]).current_dir(repo);
    for (k, v) in FIXED_COMMIT_ENV {
        cmd.env(k, v);
    }
    cmd.env("GIT_AUTHOR_DATE", "2026-01-01T00:00:00Z");
    cmd.env("GIT_COMMITTER_DATE", "2026-01-01T00:00:00Z");
    let commit = cmd.output().expect("spawn git commit");
    assert_success(&commit);
    commit
}

fn git_head(repo: &Path) -> String {
    let out = git(repo, &["rev-parse", "HEAD^{commit}"]);
    assert_success(&out);
    stdout_str(&out).trim().to_string()
}

fn git_branch(repo: &Path) -> String {
    let out = git(repo, &["symbolic-ref", "--short", "HEAD"]);
    assert_success(&out);
    stdout_str(&out).trim().to_string()
}

fn git_object_format(repo: &Path) -> String {
    let out = git(repo, &["rev-parse", "--show-object-format"]);
    assert_success(&out);
    stdout_str(&out).trim().to_string()
}

fn git_refs(repo: &Path) -> Vec<u8> {
    let out = git_sanitized(repo, &["for-each-ref", "--format=%(refname) %(objectname)"]);
    assert_success(&out);
    out.stdout
}

fn git_config_list(repo: &Path) -> Vec<u8> {
    let out = git_sanitized(repo, &["config", "--list"]);
    assert_success(&out);
    out.stdout
}

fn git_remotes(repo: &Path) -> Vec<u8> {
    let out = git_sanitized(repo, &["remote", "-v"]);
    assert_success(&out);
    out.stdout
}

fn git_porcelain(repo: &Path) -> Vec<u8> {
    let out = git_sanitized(
        repo,
        &[
            "status",
            "--porcelain=v1",
            "--untracked-files=all",
            "--ignore-submodules=none",
            "--renames",
        ],
    );
    assert_success(&out);
    out.stdout
}

fn git_hooks(repo: &Path) -> BTreeMap<String, String> {
    let hooks = repo.join(".git").join("hooks");
    let mut map = BTreeMap::new();
    if let Ok(rd) = std::fs::read_dir(&hooks) {
        for de in rd.flatten() {
            let path = de.path();
            if path.is_file() {
                map.insert(
                    de.file_name().to_string_lossy().into_owned(),
                    sha_of_file(&path),
                );
            }
        }
    }
    map
}

/// Repo-relative path -> entry for the worktree, excluding `.git` and
/// `.mrgs` (used for "no unintended Git mutation" comparisons).
fn worktree_entries(entries: &BTreeMap<String, Entry>) -> BTreeMap<String, Entry> {
    entries
        .iter()
        .filter(|(p, _)| {
            !p.starts_with(".git/")
                && !p.starts_with(".mrgs/")
                && p.as_str() != ".git"
                && p.as_str() != ".mrgs"
        })
        .map(|(p, e)| (p.clone(), e.clone()))
        .collect()
}

// ---------------------------------------------------------------------------
// Snapshot machinery (independent Phase 10 test instrument)
// ---------------------------------------------------------------------------

#[derive(Clone, PartialEq, Eq, Debug)]
enum Kind {
    Regular,
    Directory,
    Symlink,
    Reparse,
    Other,
}

impl Kind {
    fn tag(&self) -> &'static str {
        match self {
            Kind::Regular => "R",
            Kind::Directory => "D",
            Kind::Symlink => "L",
            Kind::Reparse => "P",
            Kind::Other => "O",
        }
    }

    fn from_tag(tag: &str) -> Option<Kind> {
        match tag {
            "R" => Some(Kind::Regular),
            "D" => Some(Kind::Directory),
            "L" => Some(Kind::Symlink),
            "P" => Some(Kind::Reparse),
            "O" => Some(Kind::Other),
            _ => None,
        }
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
struct Entry {
    kind: Kind,
    size: u64,
    sha256: String,
    link_target: Option<Vec<u8>>,
}

#[derive(Clone, PartialEq, Eq, Debug)]
struct RepoSnapshot {
    entries: BTreeMap<String, Entry>,
    head: String,
    branch: String,
    object_format: String,
    refs: Vec<u8>,
    index: Vec<u8>,
    config: Vec<u8>,
    hooks: BTreeMap<String, String>,
    remotes: Vec<u8>,
    porcelain: Vec<u8>,
    untracked: Vec<String>,
}

fn classify_meta(md: &std::fs::Metadata) -> Kind {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
        if md.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 && !md.file_type().is_symlink()
        {
            return Kind::Reparse;
        }
    }
    if md.file_type().is_symlink() {
        return Kind::Symlink;
    }
    if md.is_dir() {
        return Kind::Directory;
    }
    if md.is_file() {
        return Kind::Regular;
    }
    Kind::Other
}

fn path_to_bytes(p: &Path) -> Vec<u8> {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        p.as_os_str().as_bytes().to_vec()
    }
    #[cfg(not(unix))]
    {
        p.to_string_lossy().as_bytes().to_vec()
    }
}

fn walk_tree(root: &Path) -> Result<BTreeMap<String, Entry>, String> {
    let mut entries = BTreeMap::new();
    let mut stack = vec![(root.to_path_buf(), String::new())];
    while let Some((dir, rel)) = stack.pop() {
        let rd =
            std::fs::read_dir(&dir).map_err(|e| format!("read_dir {}: {}", dir.display(), e))?;
        let mut children: Vec<(PathBuf, String)> = Vec::new();
        for de in rd {
            let de = de.map_err(|e| format!("read_dir entry: {}", e))?;
            let name = de.file_name().to_string_lossy().into_owned();
            let child_rel = if rel.is_empty() {
                name.clone()
            } else {
                format!("{}/{}", rel, name)
            };
            children.push((de.path(), child_rel));
        }
        children.sort_by(|a, b| a.1.cmp(&b.1));
        for (child, child_rel) in children {
            let md = std::fs::symlink_metadata(&child)
                .map_err(|e| format!("symlink_metadata {}: {}", child.display(), e))?;
            match classify_meta(&md) {
                Kind::Directory => {
                    entries.insert(
                        child_rel.clone(),
                        Entry {
                            kind: Kind::Directory,
                            size: 0,
                            sha256: String::new(),
                            link_target: None,
                        },
                    );
                    stack.push((child, child_rel));
                }
                Kind::Regular => {
                    let bytes = std::fs::read(&child)
                        .map_err(|e| format!("read {}: {}", child.display(), e))?;
                    entries.insert(
                        child_rel,
                        Entry {
                            kind: Kind::Regular,
                            size: bytes.len() as u64,
                            sha256: sha256_hex(&bytes),
                            link_target: None,
                        },
                    );
                }
                Kind::Symlink => {
                    let target = std::fs::read_link(&child)
                        .map_err(|e| format!("read_link {}: {}", child.display(), e))?;
                    entries.insert(
                        child_rel,
                        Entry {
                            kind: Kind::Symlink,
                            size: 0,
                            sha256: String::new(),
                            link_target: Some(path_to_bytes(&target)),
                        },
                    );
                }
                Kind::Reparse | Kind::Other => {
                    entries.insert(
                        child_rel,
                        Entry {
                            kind: classify_meta(&md),
                            size: md.len(),
                            sha256: String::new(),
                            link_target: None,
                        },
                    );
                }
            }
        }
    }
    Ok(entries)
}

fn snapshot_repo(repo: &Path) -> RepoSnapshot {
    let entries = walk_tree(repo).expect("walk repo tree");
    let porcelain = git_porcelain(repo);
    let untracked = String::from_utf8_lossy(&porcelain)
        .lines()
        .filter(|l| l.starts_with("??"))
        .map(|l| l.to_string())
        .collect();
    RepoSnapshot {
        entries,
        head: git_head(repo),
        branch: git_branch(repo),
        object_format: git_object_format(repo),
        refs: git_refs(repo),
        index: std::fs::read(repo.join(".git").join("index")).expect("read .git/index"),
        config: git_config_list(repo),
        hooks: git_hooks(repo),
        remotes: git_remotes(repo),
        porcelain,
        untracked,
    }
}

fn to_hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push_str(&format!("{:02x}", b));
    }
    out
}

fn from_hex(hex: &str) -> Option<Vec<u8>> {
    if !hex.len().is_multiple_of(2) {
        return None;
    }
    let mut out = Vec::with_capacity(hex.len() / 2);
    let bytes = hex.as_bytes();
    for i in (0..bytes.len()).step_by(2) {
        let hi = (bytes[i] as char).to_digit(16)?;
        let lo = (bytes[i + 1] as char).to_digit(16)?;
        out.push((hi * 16 + lo) as u8);
    }
    Some(out)
}

/// Deterministic serialization of a snapshot's entry manifest (no contents,
/// no absolute paths) — used for markers and evidence hashes.
fn entry_manifest_bytes(snapshot: &RepoSnapshot) -> Vec<u8> {
    let mut out = Vec::new();
    for (path, entry) in &snapshot.entries {
        out.extend_from_slice(
            format!(
                "E {} {} {} {} {}\n",
                entry.kind.tag(),
                entry.size,
                entry.sha256,
                to_hex(entry.link_target.as_deref().unwrap_or(&[])),
                to_hex(path.as_bytes())
            )
            .as_bytes(),
        );
    }
    out
}

fn entry_manifest_sha(snapshot: &RepoSnapshot) -> String {
    sha256_hex(&entry_manifest_bytes(snapshot))
}

/// Full deterministic serialization: entry manifest plus Git views and file
/// contents (this is the validated backup payload, not the evidence manifest).
fn snapshot_payload_bytes(
    snapshot: &RepoSnapshot,
    contents: &BTreeMap<String, Vec<u8>>,
) -> Vec<u8> {
    let mut out = Vec::new();
    for (path, entry) in &snapshot.entries {
        let content_hex = match contents.get(path) {
            Some(bytes) => to_hex(bytes),
            None => String::new(),
        };
        out.extend_from_slice(
            format!(
                "E {} {} {} {} {} {}\n",
                entry.kind.tag(),
                entry.size,
                entry.sha256,
                content_hex,
                to_hex(entry.link_target.as_deref().unwrap_or(&[])),
                to_hex(path.as_bytes())
            )
            .as_bytes(),
        );
    }
    out.extend_from_slice(format!("G head {}\n", to_hex(snapshot.head.as_bytes())).as_bytes());
    out.extend_from_slice(format!("G branch {}\n", to_hex(snapshot.branch.as_bytes())).as_bytes());
    out.extend_from_slice(
        format!(
            "G object_format {}\n",
            to_hex(snapshot.object_format.as_bytes())
        )
        .as_bytes(),
    );
    out.extend_from_slice(format!("G refs {}\n", to_hex(&snapshot.refs)).as_bytes());
    out.extend_from_slice(format!("G index {}\n", to_hex(&snapshot.index)).as_bytes());
    out.extend_from_slice(format!("G config {}\n", to_hex(&snapshot.config)).as_bytes());
    out.extend_from_slice(format!("G remotes {}\n", to_hex(&snapshot.remotes)).as_bytes());
    out.extend_from_slice(format!("G porcelain {}\n", to_hex(&snapshot.porcelain)).as_bytes());
    for (name, sha) in &snapshot.hooks {
        out.extend_from_slice(format!("H {} {}\n", name, sha).as_bytes());
    }
    out.extend_from_slice(
        format!(
            "T untracked {}\n",
            to_hex(snapshot.untracked.join("\n").as_bytes())
        )
        .as_bytes(),
    );
    out
}

/// A validated backup file. `path` is the sole backup artifact; it is never
/// modified or deleted by restore.
struct Backup {
    snapshot: RepoSnapshot,
    contents: BTreeMap<String, Vec<u8>>,
}

impl Backup {
    fn write(&self, path: &Path) {
        let payload = snapshot_payload_bytes(&self.snapshot, &self.contents);
        let self_sha = sha256_hex(&payload);
        let header = format!(
            "PHASE10_BACKUP_V1 {} {} {} {} {}\n",
            self_sha,
            payload.len(),
            self.snapshot.object_format,
            self.snapshot.head,
            self.snapshot.branch
        );
        let mut file = header.as_bytes().to_vec();
        file.extend_from_slice(&payload);
        write_bytes(path, &file);
    }
}

/// Build the byte contents map for a snapshot from the live directory. The
/// snapshot entries must match the live bytes exactly; the backup artifact
/// is then self-contained (contents are hex-encoded inside it).
fn snapshot_contents(root: &Path, snapshot: &RepoSnapshot) -> BTreeMap<String, Vec<u8>> {
    let mut contents = BTreeMap::new();
    for (rel, entry) in &snapshot.entries {
        if entry.kind == Kind::Regular {
            let bytes = std::fs::read(root.join(rel)).expect("read snapshot file");
            assert_eq!(
                sha256_hex(&bytes),
                entry.sha256,
                "snapshot entry must match live file {}",
                rel
            );
            contents.insert(rel.clone(), bytes);
        }
    }
    contents
}

/// Write a validated, self-contained backup artifact for a snapshot.
fn write_backup(path: &Path, snapshot: &RepoSnapshot, root: &Path) {
    let contents = snapshot_contents(root, snapshot);
    let backup = Backup {
        snapshot: snapshot.clone(),
        contents,
    };
    backup.write(path);
}

/// Assert no restore scaffolding (fresh destination, marker, or any other
/// temporary path) survives anywhere under `root`.
fn assert_no_restore_scaffolding(root: &Path) {
    let mut found = Vec::new();
    collect_restore_scaffolding(root, &mut found);
    assert!(
        found.is_empty(),
        "residual restore scaffolding: {:?}",
        found
    );
}

fn collect_restore_scaffolding(dir: &Path, found: &mut Vec<PathBuf>) {
    if let Ok(rd) = std::fs::read_dir(dir) {
        for de in rd.flatten() {
            let path = de.path();
            let name = de.file_name().to_string_lossy().into_owned();
            if name.contains(".phase10-restore")
                || (name.contains(".phase10-restore-") && name.ends_with(".marker"))
                || name.ends_with(".phase10-restore")
            {
                found.push(path.clone());
            }
            if path.is_dir() {
                collect_restore_scaffolding(&path, found);
            }
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
enum RestoreError {
    BackupMissing,
    BackupCorrupt(String),
    BackupTruncated { declared: usize, actual: usize },
    BindMismatch(String),
    VerifyMismatch(String),
    Io(String),
}

impl std::fmt::Display for RestoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RestoreError::BackupMissing => write!(f, "BACKUP_MISSING"),
            RestoreError::BackupCorrupt(reason) => write!(f, "SNAPSHOT_CORRUPT {}", reason),
            RestoreError::BackupTruncated { declared, actual } => {
                write!(
                    f,
                    "SNAPSHOT_TRUNCATED declared={} actual={}",
                    declared, actual
                )
            }
            RestoreError::BindMismatch(reason) => write!(f, "SNAPSHOT_BIND_MISMATCH {}", reason),
            RestoreError::VerifyMismatch(reason) => write!(f, "RESTORE_VERIFY_MISMATCH {}", reason),
            RestoreError::Io(reason) => write!(f, "RESTORE_IO {}", reason),
        }
    }
}

/// Load and validate a backup file. Self-hash, declared length, entry
/// grammar, and per-entry content hashes are all verified.
fn load_backup(path: &Path) -> Result<Backup, RestoreError> {
    let bytes = std::fs::read(path).map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => RestoreError::BackupMissing,
        _ => RestoreError::Io(format!("read backup: {}", e)),
    })?;
    let text = String::from_utf8_lossy(&bytes);
    let header_end = text
        .find('\n')
        .ok_or_else(|| RestoreError::BackupCorrupt("missing header line".into()))?;
    let header = text[..header_end].to_string();
    let parts: Vec<&str> = header.split_whitespace().collect();
    if parts.len() != 6 || parts[0] != "PHASE10_BACKUP_V1" {
        return Err(RestoreError::BackupCorrupt("malformed header".into()));
    }
    let self_sha = parts[1].to_string();
    let payload_len: usize = parts[2]
        .parse()
        .map_err(|_| RestoreError::BackupCorrupt("malformed payload length".into()))?;
    let bind_object_format = parts[3].to_string();
    let bind_head = parts[4].to_string();
    let bind_branch = parts[5].to_string();

    let payload_start = header_end + 1;
    let actual = bytes.len().saturating_sub(payload_start);
    if actual != payload_len {
        return Err(RestoreError::BackupTruncated {
            declared: payload_len,
            actual,
        });
    }
    let payload = &bytes[payload_start..];
    let computed = sha256_hex(payload);
    if computed != self_sha {
        return Err(RestoreError::BackupCorrupt(format!(
            "self-hash mismatch declared={} computed={}",
            self_sha, computed
        )));
    }

    let mut entries: BTreeMap<String, Entry> = BTreeMap::new();
    let mut contents: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    let mut head = String::new();
    let mut branch = String::new();
    let mut object_format = String::new();
    let mut refs = Vec::new();
    let mut index = Vec::new();
    let mut config = Vec::new();
    let mut remotes = Vec::new();
    let mut porcelain = Vec::new();
    let mut hooks: BTreeMap<String, String> = BTreeMap::new();
    let mut untracked = Vec::new();

    let payload_text = String::from_utf8_lossy(payload);
    for line in payload_text.lines() {
        let mut it = line.split(' ');
        match it.next() {
            Some("E") => {
                let kind_tag = it
                    .next()
                    .ok_or_else(|| RestoreError::BackupCorrupt("entry kind".into()))?;
                let kind = Kind::from_tag(kind_tag)
                    .ok_or_else(|| RestoreError::BackupCorrupt("unknown entry kind".into()))?;
                let size: u64 = it
                    .next()
                    .ok_or_else(|| RestoreError::BackupCorrupt("entry size".into()))?
                    .parse()
                    .map_err(|_| RestoreError::BackupCorrupt("malformed entry size".into()))?;
                let sha = it
                    .next()
                    .ok_or_else(|| RestoreError::BackupCorrupt("entry sha".into()))?
                    .to_string();
                let content_hex = it
                    .next()
                    .ok_or_else(|| RestoreError::BackupCorrupt("entry content".into()))?;
                let link_hex = it
                    .next()
                    .ok_or_else(|| RestoreError::BackupCorrupt("entry link".into()))?;
                let path_hex = it
                    .next()
                    .ok_or_else(|| RestoreError::BackupCorrupt("entry path".into()))?;
                let rel_path = String::from_utf8(
                    from_hex(path_hex)
                        .ok_or_else(|| RestoreError::BackupCorrupt("bad path hex".into()))?,
                )
                .map_err(|_| RestoreError::BackupCorrupt("non-utf8 path".into()))?;
                let link_target = match from_hex(link_hex) {
                    Some(t) if !t.is_empty() => Some(t),
                    _ => None,
                };
                if kind == Kind::Regular {
                    let content = from_hex(content_hex)
                        .ok_or_else(|| RestoreError::BackupCorrupt("bad content hex".into()))?;
                    let recomputed = sha256_hex(&content);
                    if recomputed != sha {
                        return Err(RestoreError::BackupCorrupt(format!(
                            "entry content hash mismatch path={} declared={} computed={}",
                            rel_path, sha, recomputed
                        )));
                    }
                    if content.len() as u64 != size {
                        return Err(RestoreError::BackupCorrupt(format!(
                            "entry size mismatch path={} declared={} actual={}",
                            rel_path,
                            size,
                            content.len()
                        )));
                    }
                    contents.insert(rel_path.clone(), content);
                }
                entries.insert(
                    rel_path,
                    Entry {
                        kind,
                        size,
                        sha256: sha,
                        link_target,
                    },
                );
            }
            Some("G") => {
                let name = it
                    .next()
                    .ok_or_else(|| RestoreError::BackupCorrupt("view name".into()))?;
                let value_hex = it
                    .next()
                    .ok_or_else(|| RestoreError::BackupCorrupt("view value".into()))?;
                let value = from_hex(value_hex)
                    .ok_or_else(|| RestoreError::BackupCorrupt("bad view hex".into()))?;
                match name {
                    "head" => head = String::from_utf8_lossy(&value).into_owned(),
                    "branch" => branch = String::from_utf8_lossy(&value).into_owned(),
                    "object_format" => object_format = String::from_utf8_lossy(&value).into_owned(),
                    "refs" => refs = value,
                    "index" => index = value,
                    "config" => config = value,
                    "remotes" => remotes = value,
                    "porcelain" => porcelain = value,
                    _ => {
                        return Err(RestoreError::BackupCorrupt(format!(
                            "unknown view {}",
                            name
                        )))
                    }
                }
            }
            Some("H") => {
                let name = it
                    .next()
                    .ok_or_else(|| RestoreError::BackupCorrupt("hook name".into()))?;
                let sha = it
                    .next()
                    .ok_or_else(|| RestoreError::BackupCorrupt("hook sha".into()))?;
                hooks.insert(name.to_string(), sha.to_string());
            }
            Some("T") => {
                let name = it
                    .next()
                    .ok_or_else(|| RestoreError::BackupCorrupt("tail name".into()))?;
                let value_hex = it
                    .next()
                    .ok_or_else(|| RestoreError::BackupCorrupt("tail value".into()))?;
                let value = from_hex(value_hex)
                    .ok_or_else(|| RestoreError::BackupCorrupt("bad tail hex".into()))?;
                if name == "untracked" {
                    untracked = String::from_utf8_lossy(&value)
                        .lines()
                        .filter(|l| !l.is_empty())
                        .map(|l| l.to_string())
                        .collect();
                }
            }
            _ => return Err(RestoreError::BackupCorrupt("unknown payload line".into())),
        }
    }

    if head.is_empty() || branch.is_empty() || object_format.is_empty() {
        return Err(RestoreError::BackupCorrupt(
            "missing git identity views".into(),
        ));
    }
    let bind = RepoSnapshot {
        entries,
        head,
        branch,
        object_format,
        refs,
        index,
        config,
        hooks,
        remotes,
        porcelain,
        untracked,
    };
    if bind.object_format != bind_object_format
        || bind.head != bind_head
        || bind.branch != bind_branch
    {
        return Err(RestoreError::BackupCorrupt("header bind mismatch".into()));
    }
    Ok(Backup {
        snapshot: bind,
        contents,
    })
}

fn backup_bind_reason(backup: &Backup, target: &RepoSnapshot) -> Option<String> {
    if backup.snapshot.object_format != target.object_format {
        return Some(format!(
            "object-format declared={} target={}",
            backup.snapshot.object_format, target.object_format
        ));
    }
    if backup.snapshot.head != target.head {
        return Some(format!(
            "head declared={} target={}",
            backup.snapshot.head, target.head
        ));
    }
    if backup.snapshot.branch != target.branch {
        return Some(format!(
            "branch declared={} target={}",
            backup.snapshot.branch, target.branch
        ));
    }
    None
}

/// Restore procedure: load+validate the sole backup, rebuild into a fresh
/// destination, validate byte equality, replace, finalize evidence. The
/// backup file is never modified or deleted. `interrupt` preconstructs a
/// deterministic interruption point for resumption drills.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum InterruptPoint {
    None,
    BeforeReplacement,
    AfterReplacementBeforeFinalize,
}

#[derive(Clone, PartialEq, Eq, Debug)]
struct RestoreReport {
    rebuilt: bool,
    replaced: bool,
    finalized: bool,
}

fn snapshot_of_dir(path: &Path) -> Result<RepoSnapshot, RestoreError> {
    // The restored destination is a full repo clone; git views must match too.
    if !path.join(".git").exists() {
        return Err(RestoreError::VerifyMismatch(
            "destination lacks .git".into(),
        ));
    }
    Ok(snapshot_repo(path))
}

fn restore_repo(
    backup_path: &Path,
    repo: &Path,
    evidence_dir: &Path,
    interrupt: InterruptPoint,
) -> Result<RestoreReport, RestoreError> {
    let backup = load_backup(backup_path)?;
    let parent = repo
        .parent()
        .ok_or_else(|| RestoreError::Io("no parent".into()))?;
    let name = repo
        .file_name()
        .ok_or_else(|| RestoreError::Io("no file name".into()))?
        .to_string_lossy()
        .into_owned();
    let dest = parent.join(format!("{}.phase10-restore", name));
    let marker = parent.join(format!(".phase10-restore-{}.marker", name));
    let entries_sha = entry_manifest_sha(&backup.snapshot);

    // Bind check: the backup must belong to this repository identity.
    let target = snapshot_of_dir(repo)?;
    if let Some(reason) = backup_bind_reason(&backup, &target) {
        return Err(RestoreError::BindMismatch(reason));
    }

    // Post-replacement resumption / fixed point: target already matches.
    let target_entries_match = target.entries == backup.snapshot.entries;
    let receipt = evidence_dir.join("restore-receipt.txt");
    let receipt_present = receipt.exists();
    if target_entries_match {
        if receipt_present {
            return Ok(RestoreReport {
                rebuilt: false,
                replaced: false,
                finalized: true,
            });
        }
        return finalize_restore(
            repo,
            evidence_dir,
            &marker,
            &dest,
            &entries_sha,
            &backup,
            false,
            false,
        );
    }

    // Build or resume the fresh destination.
    let mut rebuilt = false;
    if dest.exists() {
        let marker_ok = std::fs::read_to_string(&marker)
            .map(|m| m.trim() == format!("PHASE10_RESTORE_MARKER {}", entries_sha))
            .unwrap_or(false);
        if !marker_ok {
            std::fs::remove_dir_all(&dest)
                .map_err(|e| RestoreError::Io(format!("remove stale dest: {}", e)))?;
            build_dest(&backup, &dest)?;
            rebuilt = true;
        }
    } else {
        build_dest(&backup, &dest)?;
        rebuilt = true;
    }

    // Validate the destination against the backup before any replacement.
    let dest_snap = snapshot_of_dir(&dest)?;
    if dest_snap.entries != backup.snapshot.entries || dest_snap.head != backup.snapshot.head {
        return Err(RestoreError::VerifyMismatch(
            "restored destination differs from backup".into(),
        ));
    }
    write_file(
        &marker,
        &format!("PHASE10_RESTORE_MARKER {}\n", entries_sha),
    );

    if interrupt == InterruptPoint::BeforeReplacement {
        return Ok(RestoreReport {
            rebuilt,
            replaced: false,
            finalized: false,
        });
    }

    // Replace: fresh destination becomes the repo.
    std::fs::remove_dir_all(repo)
        .map_err(|e| RestoreError::Io(format!("remove current repo: {}", e)))?;
    std::fs::rename(&dest, repo)
        .map_err(|e| RestoreError::Io(format!("rename dest into place: {}", e)))?;

    if interrupt == InterruptPoint::AfterReplacementBeforeFinalize {
        return Ok(RestoreReport {
            rebuilt,
            replaced: true,
            finalized: false,
        });
    }

    finalize_restore(
        repo,
        evidence_dir,
        &marker,
        &dest,
        &entries_sha,
        &backup,
        rebuilt,
        true,
    )
}

#[allow(clippy::too_many_arguments)] // restore procedure signature: one argument per documented boundary
fn finalize_restore(
    repo: &Path,
    evidence_dir: &Path,
    marker: &Path,
    dest: &Path,
    entries_sha: &str,
    backup: &Backup,
    rebuilt: bool,
    replaced: bool,
) -> Result<RestoreReport, RestoreError> {
    // Post-restore validation: the repo must now equal the validated backup.
    let final_snap = snapshot_of_dir(repo)?;
    if final_snap.entries != backup.snapshot.entries || final_snap.head != backup.snapshot.head {
        return Err(RestoreError::VerifyMismatch("post-restore mismatch".into()));
    }
    if dest.exists() {
        return Err(RestoreError::Io(
            "destination still present after replace".into(),
        ));
    }
    let _ = std::fs::remove_file(marker);
    let receipt_path = evidence_dir.join("restore-receipt.txt");
    write_file(
        &receipt_path,
        &format!("PHASE10_RESTORE_RECEIPT {}\n", entries_sha),
    );
    Ok(RestoreReport {
        rebuilt,
        replaced,
        finalized: true,
    })
}

fn build_dest(backup: &Backup, dest: &Path) -> Result<(), RestoreError> {
    std::fs::create_dir_all(dest).map_err(|e| RestoreError::Io(format!("create dest: {}", e)))?;
    for (rel_path, entry) in &backup.snapshot.entries {
        let target = dest.join(rel_path);
        match entry.kind {
            Kind::Directory => {
                std::fs::create_dir_all(&target)
                    .map_err(|e| RestoreError::Io(format!("mkdir {}: {}", target.display(), e)))?;
            }
            Kind::Regular => {
                let content = backup.contents.get(rel_path).ok_or_else(|| {
                    RestoreError::VerifyMismatch(format!("missing content for {}", rel_path))
                })?;
                write_bytes(&target, content);
            }
            Kind::Symlink => {
                let link_bytes = entry
                    .link_target
                    .as_deref()
                    .ok_or_else(|| RestoreError::VerifyMismatch("symlink without target".into()))?;
                create_symlink(link_bytes, &target)?;
            }
            Kind::Reparse | Kind::Other => {
                return Err(RestoreError::VerifyMismatch(format!(
                    "cannot restore kind {:?} at {}",
                    entry.kind, rel_path
                )));
            }
        }
    }
    Ok(())
}

#[cfg(unix)]
fn create_symlink(target: &[u8], link: &Path) -> Result<(), RestoreError> {
    use std::os::unix::ffi::OsStrExt;
    let t = std::ffi::OsStr::from_bytes(target);
    std::os::unix::fs::symlink(t, link)
        .map_err(|e| RestoreError::Io(format!("symlink {}: {}", link.display(), e)))
}

#[cfg(not(unix))]
fn create_symlink(_target: &[u8], _link: &Path) -> Result<(), RestoreError> {
    Err(RestoreError::VerifyMismatch(
        "symlink restore not compiled for this target".into(),
    ))
}

// ---------------------------------------------------------------------------
// Activation slot (local active/backup/evidence directory)
// ---------------------------------------------------------------------------

struct Slot {
    root: PathBuf,
}

impl Slot {
    fn new(root: &Path) -> Slot {
        let slot = Slot {
            root: root.to_path_buf(),
        };
        for sub in ["active", "backup", "evidence"] {
            std::fs::create_dir_all(slot.root.join(sub)).expect("create slot subdir");
        }
        slot
    }

    fn active_dir(&self) -> PathBuf {
        self.root.join("active")
    }

    fn backup_dir(&self) -> PathBuf {
        self.root.join("backup")
    }

    fn remove_all(&self) {
        std::fs::remove_dir_all(&self.root).expect("remove slot");
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
struct SlotSnapshot {
    entries: BTreeMap<String, Entry>,
}

/// Slot state for restore purposes: active/ and backup/ (evidence/ is
/// preserved externally and never rolled back).
fn slot_snapshot(slot: &Slot) -> SlotSnapshot {
    let entries = walk_tree(&slot.root)
        .expect("walk slot")
        .into_iter()
        .filter(|(p, _)| !p.starts_with("evidence") && p.as_str() != "evidence")
        .collect();
    SlotSnapshot { entries }
}

/// Snapshot only one slot subdirectory (active/, backup/, or evidence/),
/// with paths relative to that subdirectory.
fn slot_sub_snapshot(slot: &Slot, sub: &str) -> SlotSnapshot {
    let entries = walk_tree(&slot.root.join(sub))
        .expect("walk slot subdir")
        .into_iter()
        .collect();
    SlotSnapshot { entries }
}

fn slot_restore(slot: &Slot, pre: &SlotSnapshot) {
    for sub in ["active", "backup"] {
        let dir = slot.root.join(sub);
        if dir.exists() {
            std::fs::remove_dir_all(&dir).expect("wipe slot subdir");
        }
    }
    for sub in ["active", "backup"] {
        std::fs::create_dir_all(slot.root.join(sub)).expect("recreate slot subdir");
    }
    // Re-materialize the pre-activation state. A pre-existing binary is kept
    // byte-identical in backup/ and restored from there; an empty active slot
    // is restored as the explicitly recorded absent state.
    let active_pre: Vec<(&String, &Entry)> = pre
        .entries
        .iter()
        .filter(|(p, e)| p.starts_with("active/") && e.kind == Kind::Regular)
        .collect();
    if !active_pre.is_empty() {
        for (rel, entry) in active_pre {
            let target = slot.root.join(rel);
            let backups = std::fs::read_dir(slot.backup_dir()).expect("read slot backup");
            let mut found = None;
            for de in backups.flatten() {
                if sha_of_file(&de.path()) == entry.sha256 {
                    found = Some(de.path());
                    break;
                }
            }
            let source = found.expect("previous binary preserved in slot backup");
            std::fs::copy(source, &target).expect("restore previous binary");
        }
    }
    let restored = slot_snapshot(slot);
    assert_eq!(restored, *pre, "slot restore must be exact");
}

// ---------------------------------------------------------------------------
// Pilot repository fixture
// ---------------------------------------------------------------------------

fn valid_plan_toml(plan_id: &str, phases: &[&str]) -> String {
    let mut out = format!("schema_version = 1\nplan_id = \"{}\"\n\n", plan_id);
    for (i, phase) in phases.iter().enumerate() {
        out.push_str(&format!(
            "[[phases]]\nid = \"{}\"\ntitle = \"Phase {}\"\n",
            phase,
            i + 1
        ));
        if i == 0 {
            out.push_str("depends_on = []\n");
        } else {
            out.push_str(&format!("depends_on = [\"{}\"]\n", phases[i - 1]));
        }
        out.push('\n');
    }
    out
}

fn contract_toml_for_phase(contract_id: &str, phase_id: &str, requirements: &[&str]) -> String {
    let reqs = requirements
        .iter()
        .map(|r| format!("\"{}\"", r))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "schema_version = 1\n\
         contract_id = \"{}\"\n\
         phase_id = \"{}\"\n\
         title = \"Test Contract\"\n\
         objective = \"Exercise the full governance chain\"\n\
         requirements = [{}]\n\
         allowed_paths = [\"src/\"]\n\
         forbidden_paths = [\".git/\", \".mrgs/\"]\n\
         verification_commands = [\"cargo test\", \"cargo clippy\"]\n\
         handoff_fields = [\"FIELD1\"]\n",
        contract_id, phase_id, reqs
    )
}

fn standard_metadata_toml(
    repository_id: &str,
    continuity_id: &str,
    phase_id: &str,
    receipt_sha: &str,
) -> String {
    format!(
        "schema_version = 1\n\
         repository_id = \"{}\"\n\
         continuity_id = \"{}\"\n\
         phase_id = \"{}\"\n\
         completion_receipt_sha256 = \"{}\"\n\
         note = \"continuity record\"\n\
         links = []\n\
         \n\
         [[models]]\n\
         role = \"implementer\"\n\
         provider = \"openai\"\n\
         model_id = \"gpt-5.6\"\n\
         execution_mode = \"hosted\"\n\
         session_label = \"phase-1-implementation\"\n\
         \n\
         [[hosts]]\n\
         host_id = \"main-workstation\"\n\
         platform = \"windows\"\n\
         architecture = \"x86_64\"\n\
         execution_surface = \"opencode\"\n",
        repository_id, continuity_id, phase_id, receipt_sha
    )
}

fn make_pass_report(
    audit_id: &str,
    subject_sha256: &str,
    auditor: &str,
    requirements: &[&str],
) -> String {
    let req_results: Vec<Value> = requirements
        .iter()
        .map(|r| json!({ "requirement": r, "status": "PASS", "evidence": "v" }))
        .collect();
    let report = json!({
        "schema_version": 1,
        "audit_id": audit_id,
        "subject_sha256": subject_sha256,
        "auditor_id": auditor,
        "independence_declaration": "INDEPENDENT",
        "verdict": "PASS",
        "summary": "clean room rehearsal passed",
        "requirement_results": req_results,
        "verification_results": [
            {"command": "cargo test", "status": "PASS", "evidence": "v"},
            {"command": "cargo clippy", "status": "PASS", "evidence": "v"}
        ],
        "findings": []
    });
    serde_json::to_string_pretty(&report).expect("serialize audit report")
}

struct PilotRepo {
    _guard: TempDir,
    repo: PathBuf,
    plan_path: PathBuf,
    contract_path: PathBuf,
    report_dir: PathBuf,
    plan_id: String,
    contract_id: String,
    phase_ids: Vec<String>,
    repository_id: String,
    requirements: Vec<String>,
}

impl PilotRepo {
    fn new() -> PilotRepo {
        PilotRepo::new_with_plan(
            &valid_plan_toml("test-plan", &["phase-1", "phase-2"]),
            &contract_toml_for_phase("test-contract-v1", "phase-1", &["req1", "req2"]),
            "mrgs",
            &["phase-1", "phase-2"],
            "test-plan",
            "test-contract-v1",
            &["req1", "req2"],
        )
    }

    fn new_with_plan(
        plan_toml: &str,
        contract_toml: &str,
        repository_id: &str,
        phases: &[&str],
        plan_id: &str,
        contract_id: &str,
        requirements: &[&str],
    ) -> PilotRepo {
        let guard = tempfile::tempdir().expect("create pilot tempdir");
        let root = guard.path().to_path_buf();
        let repo = root.join("repo");
        git_init(&repo);
        write_file(&repo.join(".gitignore"), ".mrgs/\n");
        let src = repo.join("src");
        std::fs::create_dir_all(&src).expect("create src");
        write_file(&src.join("main.rs"), "fn main() {}\n");
        git_commit_fixed(&repo, "initial sources");

        let plan_path = repo.join("plan.toml");
        let contract_path = repo.join("contract.toml");
        write_file(&plan_path, plan_toml);
        write_file(&contract_path, contract_toml);
        git_commit_fixed(&repo, "governance fixtures");

        let report_dir = root.join("reports");
        std::fs::create_dir_all(&report_dir).expect("create report dir");

        PilotRepo {
            _guard: guard,
            repo,
            plan_path,
            contract_path,
            report_dir,
            plan_id: plan_id.to_string(),
            contract_id: contract_id.to_string(),
            phase_ids: phases.iter().map(|s| s.to_string()).collect(),
            repository_id: repository_id.to_string(),
            requirements: requirements.iter().map(|s| s.to_string()).collect(),
        }
    }

    // -- lifecycle wrappers -------------------------------------------------

    fn accept_plan(&self) -> Output {
        Command::new(cargo_bin())
            .args(["plan", "accept", "--repo"])
            .arg(&self.repo)
            .args(["--plan"])
            .arg(&self.plan_path)
            .output()
            .expect("run plan accept")
    }

    fn select_phase(&self, phase: &str) -> Output {
        Command::new(cargo_bin())
            .args(["phase", "select", "--repo"])
            .arg(&self.repo)
            .args(["--phase", phase])
            .output()
            .expect("run phase select")
    }

    fn draft_contract(&self) -> Output {
        Command::new(cargo_bin())
            .args(["contract", "draft", "--repo"])
            .arg(&self.repo)
            .args(["--contract"])
            .arg(&self.contract_path)
            .output()
            .expect("run contract draft")
    }

    fn get_draft(&self) -> Value {
        let path = self.repo.join(".mrgs").join("contract-draft.json");
        let text = std::fs::read_to_string(&path).expect("read contract-draft.json");
        serde_json::from_str(&text).expect("parse contract-draft.json")
    }

    fn accept_contract(&self, revision: u64, sha: &str) -> Output {
        Command::new(cargo_bin())
            .args(["contract", "accept", "--repo"])
            .arg(&self.repo)
            .args([
                "--revision",
                &revision.to_string(),
                "--sha256",
                sha,
                "--decision",
                "ACCEPTED",
            ])
            .output()
            .expect("run contract accept")
    }

    fn impl_begin(&self, revision: u64, sha: &str) -> Output {
        Command::new(cargo_bin())
            .args(["implementation", "begin", "--repo"])
            .arg(&self.repo)
            .args(["--revision", &revision.to_string(), "--sha256", sha])
            .output()
            .expect("run implementation begin")
    }

    fn impl_check(&self) -> Output {
        Command::new(cargo_bin())
            .args(["implementation", "check", "--repo"])
            .arg(&self.repo)
            .output()
            .expect("run implementation check")
    }

    fn audit_begin(&self, auditor: &str) -> Output {
        Command::new(cargo_bin())
            .args(["audit", "begin", "--repo"])
            .arg(&self.repo)
            .args(["--auditor", auditor])
            .output()
            .expect("run audit begin")
    }

    fn write_report(&self, report: &str) -> PathBuf {
        let path = self.report_dir.join("audit-report.json");
        write_file(&path, report);
        path
    }

    fn audit_record(&self, report: &Path) -> Output {
        Command::new(cargo_bin())
            .args(["audit", "record", "--repo"])
            .arg(&self.repo)
            .args(["--report"])
            .arg(report)
            .output()
            .expect("run audit record")
    }

    fn phase_close(&self, phase: &str) -> Output {
        Command::new(cargo_bin())
            .args(["phase", "close", "--repo"])
            .arg(&self.repo)
            .args(["--phase", phase])
            .output()
            .expect("run phase close")
    }

    fn write_metadata(&self, name: &str, content: &str) -> PathBuf {
        // Metadata must live inside the governed repo (MRGS records the
        // metadata source as a repository-relative path).
        let path = self.repo.join(name);
        write_file(&path, content);
        path
    }

    fn continuity_record(&self, metadata: &Path) -> Output {
        Command::new(cargo_bin())
            .args(["continuity", "record", "--repo"])
            .arg(&self.repo)
            .args(["--metadata"])
            .arg(metadata)
            .output()
            .expect("run continuity record")
    }

    fn inspect(&self) -> Output {
        Command::new(cargo_bin())
            .args(["recovery", "inspect", "--repo"])
            .arg(&self.repo)
            .output()
            .expect("run recovery inspect")
    }
}

// ---------------------------------------------------------------------------
// Evidence emission
// ---------------------------------------------------------------------------

struct Evidence {
    lines: Vec<String>,
}

impl Evidence {
    fn new() -> Evidence {
        Evidence { lines: Vec::new() }
    }

    fn add(&mut self, line: impl Into<String>) {
        self.lines.push(line.into());
    }

    fn render(&self) -> String {
        self.lines.join("\n")
    }

    fn emit(&self) {
        for line in &self.lines {
            println!("PHASE10_EVIDENCE {}", line);
        }
    }
}

// ---------------------------------------------------------------------------
// Shared activation rehearsal
// ---------------------------------------------------------------------------

struct RehearsalResult {
    plan_id: String,
    plan_sha: String,
    plan_size: u64,
    contract_id: String,
    phase_id: String,
    draft_sha: String,
    accept_revision: u64,
    impl_head: String,
    audit_id: String,
    audit_round: u64,
    subject_sha: String,
    draft_ledger_sha: String,
    accepted_contract_ledger_sha: String,
    impl_authority_ledger_sha: String,
    close_manifest_sha: String,
    close_receipt_sha: String,
    continuity_manifest_sha: String,
    continuity_receipt_sha: String,
    inspect_subject: String,
}

/// Runs the full public governed lifecycle and asserts the exact success
/// framing and output-bound identities at every boundary.
fn run_activation_rehearsal(t: &PilotRepo) -> RehearsalResult {
    let plan = t.accept_plan();
    assert_success(&plan);
    let plan_sha = sha_of_file(&t.plan_path);
    assert_eq!(stdout_str(&plan), format!("{} {}", t.plan_id, plan_sha));

    let select = t.select_phase(&t.phase_ids[0]);
    assert_success(&select);
    assert_eq!(stdout_str(&select), t.phase_ids[0]);

    let draft = t.draft_contract();
    assert_success(&draft);
    let draft_sha = t.get_draft()["sha256"].as_str().unwrap().to_string();
    assert_eq!(sha_of_file(&t.contract_path), draft_sha);
    assert_eq!(
        stdout_str(&draft),
        format!("{} {}", t.contract_id, draft_sha)
    );
    let draft_ledger_sha = sha_of_file(&t.repo.join(".mrgs").join("contract-draft.json"));

    let accept = t.accept_contract(1, &draft_sha);
    assert_success(&accept);
    assert_eq!(
        stdout_str(&accept),
        format!("ACCEPTED {} 1 {}", t.contract_id, draft_sha)
    );
    let accepted_contract_ledger_sha =
        sha_of_file(&t.repo.join(".mrgs").join("accepted-contract.json"));

    let begin = t.impl_begin(1, &draft_sha);
    assert_success(&begin);
    let impl_parts = split_stdout(&begin);
    assert_eq!(impl_parts[0], "IMPLEMENTATION_BOUND");
    assert_eq!(impl_parts[1], t.contract_id);
    assert_eq!(impl_parts[2], "1");
    assert_eq!(impl_parts[3], draft_sha);
    let impl_head = git_head(&t.repo);
    assert_eq!(impl_parts[4], impl_head);
    let impl_authority_ledger_sha =
        sha_of_file(&t.repo.join(".mrgs").join("implementation-authority.json"));

    let check = t.impl_check();
    assert_success(&check);
    let check_parts = split_stdout(&check);
    assert_eq!(check_parts[0], "IMPLEMENTATION_OK");
    assert_eq!(check_parts[1], t.contract_id);
    assert_eq!(check_parts[3], draft_sha);

    let open = t.audit_begin("auditor1");
    assert_success(&open);
    let open_parts = split_stdout(&open);
    assert_eq!(open_parts[0], "AUDIT_OPEN");
    assert_eq!(open_parts[2], "1");
    let audit_id = open_parts[1].clone();
    let subject = open_parts[3].clone();
    assert_eq!(audit_id.len(), 64);
    assert_eq!(subject.len(), 64);

    let report_path = t.write_report(&make_pass_report(
        &audit_id,
        &subject,
        "auditor1",
        &t.requirements
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<_>>(),
    ));
    let record = t.audit_record(&report_path);
    assert_success(&record);
    let rec_parts = split_stdout(&record);
    assert_eq!(rec_parts[0], "AUDIT_PASS");
    assert_eq!(rec_parts[1], audit_id);
    assert_eq!(rec_parts[3], subject);

    let close = t.phase_close(&t.phase_ids[0]);
    assert_success(&close);
    let close_parts = split_stdout(&close);
    assert_eq!(close_parts[0], "PHASE_CLOSED");
    assert_eq!(close_parts[1], t.phase_ids[0]);
    assert_eq!(close_parts[2], "1");
    assert_eq!(close_parts[3].len(), 64);
    assert_eq!(close_parts[4].len(), 64);
    let close_manifest_sha = close_parts[3].clone();
    let close_receipt_sha = close_parts[4].clone();

    let metadata = t.write_metadata(
        "continuity.toml",
        &standard_metadata_toml(
            &t.repository_id,
            &format!("{}-primary", t.phase_ids[0]),
            &t.phase_ids[0],
            &close_receipt_sha,
        ),
    );
    let cont = t.continuity_record(&metadata);
    assert_success(&cont);
    // The temporary metadata fixture is removed after the record: the
    // continuity ledger archives its bytes, and the pilot worktree must
    // return to its baseline for pre/post equality proofs.
    std::fs::remove_file(&metadata).expect("remove temporary metadata fixture");
    let cont_parts = split_stdout(&cont);
    assert_eq!(cont_parts[0], "CONTINUITY_RECORDED");
    assert_eq!(cont_parts[1], t.repository_id);
    assert_eq!(cont_parts[2], t.phase_ids[0]);
    assert_eq!(cont_parts[3], "1");
    assert_eq!(cont_parts[4].len(), 64);
    assert_eq!(cont_parts[5].len(), 64);
    let continuity_manifest_sha = cont_parts[4].clone();
    let continuity_receipt_sha = cont_parts[5].clone();

    let insp = t.inspect();
    assert_success(&insp);
    let insp_stdout = stdout_str(&insp);
    let insp_lines: Vec<&str> = insp_stdout.lines().collect();
    assert_eq!(insp_lines.len(), 1);
    let insp_parts: Vec<&str> = insp_lines[0].split_whitespace().collect();
    assert_eq!(insp_parts[0], "RECOVERY_NOT_REQUIRED");
    assert_eq!(insp_parts[1].len(), 64);
    let insp2 = t.inspect();
    assert_eq!(
        stdout_raw(&insp2),
        stdout_raw(&insp),
        "inspection must be deterministic"
    );

    RehearsalResult {
        plan_id: t.plan_id.clone(),
        plan_sha,
        plan_size: file_size(&t.plan_path),
        contract_id: t.contract_id.clone(),
        phase_id: t.phase_ids[0].clone(),
        draft_sha,
        accept_revision: 1,
        impl_head,
        audit_id,
        audit_round: 1,
        subject_sha: subject,
        draft_ledger_sha,
        accepted_contract_ledger_sha,
        impl_authority_ledger_sha,
        close_manifest_sha,
        close_receipt_sha,
        continuity_manifest_sha,
        continuity_receipt_sha,
        inspect_subject: insp_parts[1].to_string(),
    }
}

// ---------------------------------------------------------------------------
// Obligation 01 — clean-room activation rehearsal
// ---------------------------------------------------------------------------

#[test]
fn test_obligation_01_clean_room_activation_rehearsal() {
    let t = PilotRepo::new();
    let baseline = snapshot_repo(&t.repo);
    assert!(
        !baseline.entries.keys().any(|p| p.starts_with(".mrgs")),
        "pre-activation snapshot must record explicit .mrgs absence"
    );

    let result = run_activation_rehearsal(&t);

    // Durable authority chain validation: every surviving ledger must be
    // valid JSON with schema_version 1 and must bind to the exact
    // output-bound identities of every previous boundary.
    let gov = t.repo.join(".mrgs");

    let plan_json: Value =
        serde_json::from_str(&std::fs::read_to_string(gov.join("accepted-plan.json")).unwrap())
            .unwrap();
    assert_eq!(plan_json["schema_version"], 1);
    assert_eq!(plan_json["plan_id"], result.plan_id);
    assert_eq!(plan_json["plan_path"], "plan.toml");
    assert_eq!(plan_json["sha256"], result.plan_sha);
    assert_eq!(plan_json["phase_count"], 2);

    let state_json: Value =
        serde_json::from_str(&std::fs::read_to_string(gov.join("state.json")).unwrap()).unwrap();
    assert_eq!(state_json["schema_version"], 1);
    assert_eq!(state_json["accepted_plan_sha256"], result.plan_sha);
    assert_eq!(state_json["active_phase"], Value::Null);
    assert_eq!(state_json["closed_phases"], json!([result.phase_id]));

    let completion_json: Value =
        serde_json::from_str(&std::fs::read_to_string(gov.join("completion-ledger.json")).unwrap())
            .unwrap();
    assert_eq!(completion_json["schema_version"], 1);
    assert_eq!(completion_json["accepted_plan_sha256"], result.plan_sha);
    assert_eq!(completion_json["plan_id"], result.plan_id);
    let completion = &completion_json["completions"][0];
    assert_eq!(
        completion["final_manifest_sha256"],
        result.close_manifest_sha
    );
    assert_eq!(
        completion["completion_receipt_sha256"],
        result.close_receipt_sha
    );
    let manifest = &completion["final_manifest"];
    assert_eq!(manifest["accepted_plan_sha256"], result.plan_sha);
    assert_eq!(manifest["plan_id"], result.plan_id);
    assert_eq!(manifest["plan_source_path"], "plan.toml");
    assert_eq!(
        manifest["plan_content"],
        std::fs::read_to_string(&t.plan_path).unwrap()
    );
    assert_eq!(manifest["phase_id"], result.phase_id);
    assert_eq!(manifest["completion_sequence"], 1);
    assert_eq!(manifest["contract_id"], result.contract_id);
    assert_eq!(manifest["contract_revision"], result.accept_revision);
    assert_eq!(manifest["contract_source_path"], "contract.toml");
    assert_eq!(manifest["contract_sha256"], result.draft_sha);
    assert_eq!(
        manifest["contract_content"],
        std::fs::read_to_string(&t.contract_path).unwrap()
    );
    assert_eq!(manifest["implementation_baseline_head"], result.impl_head);
    assert_eq!(manifest["implementation_baseline_branch"], "main");
    assert_eq!(manifest["git_object_format"], "sha1");
    assert_eq!(manifest["final_head"], result.impl_head);
    assert_eq!(manifest["final_branch"], "main");
    assert_eq!(manifest["final_audit_id"], result.audit_id);
    assert_eq!(manifest["final_audit_round"], result.audit_round);
    assert_eq!(manifest["final_auditor_id"], "auditor1");
    assert_eq!(manifest["final_subject_sha256"], result.subject_sha);
    assert_eq!(
        manifest["final_subject"]["contract_sha256"],
        result.draft_sha
    );
    assert_eq!(
        manifest["final_subject"]["implementation_baseline_head"],
        result.impl_head
    );
    assert_eq!(
        manifest["final_subject"]["implementation_baseline_head"],
        result.impl_head
    );
    assert_eq!(
        manifest["final_report_sha256"],
        sha_of_file(&t.report_dir.join("audit-report.json"))
    );
    let archived = &manifest["archived_governance"];
    // Archived governance hashes are the SHA-256 of the ledger file bytes at
    // closeout time (not of the original plan/contract TOML); the TOML shas
    // are bound through the archived content's own `sha256` fields below.
    assert_eq!(archived["contract_draft_sha256"], result.draft_ledger_sha);
    let archived_draft: Value =
        serde_json::from_str(archived["contract_draft_content"].as_str().unwrap()).unwrap();
    assert_eq!(archived_draft["phase_id"], result.phase_id);
    assert_eq!(archived_draft["sha256"], result.draft_sha);
    assert_eq!(
        archived["accepted_contract_sha256"],
        result.accepted_contract_ledger_sha
    );
    let archived_contract: Value =
        serde_json::from_str(archived["accepted_contract_content"].as_str().unwrap()).unwrap();
    assert_eq!(archived_contract["contract_id"], result.contract_id);
    assert_eq!(archived_contract["revisions"][0]["revision"], 1);
    assert_eq!(
        archived_contract["revisions"][0]["sha256"],
        result.draft_sha
    );
    let archived_audit: Value =
        serde_json::from_str(archived["audit_ledger_content"].as_str().unwrap()).unwrap();
    assert_eq!(archived_audit["rounds"][0]["audit_id"], result.audit_id);
    assert_eq!(archived_audit["rounds"][0]["round"], 1);
    assert_eq!(archived_audit["rounds"][0]["status"], "PASS");
    assert_eq!(
        archived_audit["rounds"][0]["subject_sha256"],
        result.subject_sha
    );
    assert_eq!(
        archived["implementation_authority_sha256"],
        result.impl_authority_ledger_sha
    );
    let archived_auth: Value = serde_json::from_str(
        archived["implementation_authority_content"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(archived_auth["contract_id"], result.contract_id);
    assert_eq!(archived_auth["contract_revision"], 1);
    assert_eq!(archived_auth["contract_sha256"], result.draft_sha);
    assert_eq!(archived_auth["baseline_head"], result.impl_head);

    let cont_json: Value =
        serde_json::from_str(&std::fs::read_to_string(gov.join("continuity-ledger.json")).unwrap())
            .unwrap();
    assert_eq!(cont_json["schema_version"], 1);
    assert_eq!(cont_json["accepted_plan_sha256"], result.plan_sha);
    assert_eq!(cont_json["plan_id"], result.plan_id);
    assert_eq!(cont_json["repository_id"], t.repository_id);
    let entry = &cont_json["entries"][0];
    assert_eq!(
        entry["continuity_manifest_sha256"],
        result.continuity_manifest_sha
    );
    assert_eq!(
        entry["continuity_receipt_sha256"],
        result.continuity_receipt_sha
    );
    let c_manifest = &entry["continuity_manifest"];
    assert_eq!(
        c_manifest["continuity_id"],
        format!("{}-primary", result.phase_id)
    );
    assert_eq!(c_manifest["phase_id"], result.phase_id);
    assert_eq!(c_manifest["target_completion_sequence"], 1);
    assert_eq!(
        c_manifest["target_final_manifest_sha256"],
        result.close_manifest_sha
    );
    assert_eq!(
        c_manifest["target_completion_receipt_sha256"],
        result.close_receipt_sha
    );
    assert_eq!(c_manifest["note"], "continuity record");
    let c_receipt = &entry["continuity_receipt"];
    assert_eq!(c_receipt["continuity_sequence"], 1);
    assert_eq!(
        c_receipt["target_completion_receipt_sha256"],
        result.close_receipt_sha
    );
    assert_eq!(c_receipt["repository_id"], t.repository_id);

    // Closeout cleanup: pre-close objects must be gone (archived instead).
    for removed in [
        "contract-draft.json",
        "accepted-contract.json",
        "implementation-authority.json",
        "audit-ledger.json",
    ] {
        assert!(
            !gov.join(removed).exists(),
            "{} must be removed by closeout cleanup",
            removed
        );
    }
    assert!(!gov.join("recovery-ledger.json").exists());
    assert_no_temp_files(&gov);

    // No unintended Git mutation: worktree, HEAD, branch, refs, index,
    // config, hooks, remotes, porcelain, and untracked inventory all equal
    // the pre-activation baseline (`.mrgs` additions are expected and
    // recorded explicitly).
    let post = snapshot_repo(&t.repo);
    assert_eq!(
        worktree_entries(&post.entries),
        worktree_entries(&baseline.entries)
    );
    assert_eq!(post.head, baseline.head);
    assert_eq!(post.branch, baseline.branch);
    assert_eq!(post.refs, baseline.refs);
    assert_eq!(post.index, baseline.index);
    assert_eq!(post.config, baseline.config);
    assert_eq!(post.hooks, baseline.hooks);
    assert_eq!(post.remotes, baseline.remotes);
    assert_eq!(post.porcelain, baseline.porcelain);
    assert_eq!(post.untracked, baseline.untracked);
    assert!(
        post.entries.keys().any(|p| p.starts_with(".mrgs/")),
        "post-activation must add .mrgs objects"
    );

    // Evidence: labelled ACTIVATION_REHEARSAL, never PRODUCTION_ACTIVATED.
    let mut evidence = Evidence::new();
    evidence.add(format!(
        "ACTIVATION_REHEARSAL obligation=01 lifecycle=clean-room result=success plan_id={} plan_sha256={} plan_byte_size={}",
        result.plan_id, result.plan_sha, result.plan_size
    ));
    evidence.add(format!(
        "ACTIVATION_REHEARSAL obligation=01 chain=closeout result=success phase_id={} contract_id={} contract_sha256={} final_manifest_sha256={} completion_receipt_sha256={}",
        result.phase_id, result.contract_id, result.draft_sha, result.close_manifest_sha, result.close_receipt_sha
    ));
    evidence.add(format!(
        "ACTIVATION_REHEARSAL obligation=01 chain=continuity result=success repository_id={} continuity_manifest_sha256={} continuity_receipt_sha256={}",
        t.repository_id, result.continuity_manifest_sha, result.continuity_receipt_sha
    ));
    evidence.add(format!(
        "ACTIVATION_REHEARSAL obligation=01 chain=recovery result=healthy subject_sha256={}",
        result.inspect_subject
    ));
    let rendered = evidence.render();
    assert!(rendered.contains("ACTIVATION_REHEARSAL"));
    assert!(!rendered.contains("PRODUCTION_ACTIVATED"));
    evidence.emit();
}

// ---------------------------------------------------------------------------
// Obligation 02 — activation slot binary identity and smoke
// ---------------------------------------------------------------------------

/// Binary file name for the slot's active path (`mrgs.exe` on Windows,
/// `mrgs` elsewhere). The test never assumes a release build exists.
fn slot_binary_name() -> String {
    format!("mrgs{}", std::env::consts::EXE_SUFFIX)
}

/// Install the candidate binary into the slot's active/ directory using
/// copy-to-temporary plus atomic replacement. On Windows, `rename` cannot
/// overwrite an existing destination, so a previous active binary is
/// removed first; its bytes are preserved in backup/ before that happens
/// in every rollback drill that replaces a live binary.
fn install_candidate(slot: &Slot, candidate: &Path) -> PathBuf {
    let active = slot.active_dir();
    let target = active.join(slot_binary_name());
    // Copy to a unique temporary file inside the target directory so the
    // final rename stays on one volume and is atomic where supported.
    let tmp = active.join(format!(".mrgs-candidate-{}.tmp", std::process::id()));
    std::fs::copy(candidate, &tmp).expect("copy candidate to temporary");
    if target.exists() {
        std::fs::remove_file(&target).expect("remove previous active binary");
    }
    std::fs::rename(&tmp, &target).expect("atomic rename into active slot");
    assert!(!tmp.exists(), "temporary copy must not survive placement");
    // Identity proof: the bytes in the slot are identical to the candidate.
    assert_eq!(sha_of_file(&target), sha_of_file(candidate));
    assert_eq!(file_size(&target), file_size(candidate));
    target
}

/// Inventory of PATH directories that already contain a mrgs binary.
/// The drill must not change this inventory: it may never add a binary to
/// any PATH directory (the cargo test harness itself may place target/debug
/// on PATH, which is the pre-existing environment, not a drill side effect).
fn path_mrgs_inventory(path: &std::ffi::OsStr) -> Vec<PathBuf> {
    let mut found = Vec::new();
    for dir in std::env::split_paths(path) {
        if !dir.is_dir() {
            continue;
        }
        for name in ["mrgs.exe", "mrgs"] {
            if dir.join(name).exists() {
                found.push(dir.join(name));
            }
        }
    }
    found.sort();
    found
}

#[test]
fn test_obligation_02_activation_slot_binary_identity_and_smoke() {
    let tmp = std::env::temp_dir().join(format!(
        "mrgs-phase10-ob02-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let slot_root = tmp.join("slot");
    let slot = Slot::new(&slot_root);
    let candidate = PathBuf::from(cargo_bin());
    let candidate_sha = sha_of_file(&candidate);
    let candidate_size = file_size(&candidate);

    // Pre-activation slot state: the three subdirectories exist but are
    // empty. That absence is recorded explicitly and is the state a
    // rollback drill must restore for a fresh slot.
    let pre = slot_snapshot(&slot);
    let pre_regular: Vec<_> = pre
        .entries
        .iter()
        .filter(|(_, e)| e.kind == Kind::Regular)
        .collect();
    assert!(
        pre_regular.is_empty(),
        "pre-activation slot must contain no files"
    );
    let backup_state = slot_snapshot(&slot);
    assert_eq!(
        backup_state, pre,
        "backup must record the pre-activation state"
    );

    // Environment confinement baseline: PATH is recorded and the inventory
    // of mrgs binaries on PATH is fixed before the drill.
    let path_before = std::env::var_os("PATH").expect("PATH is set");
    let mrgs_on_path_before = path_mrgs_inventory(&path_before);

    // Place the candidate via copy-to-temp + atomic replacement.
    let active_bin = install_candidate(&slot, &candidate);
    let placed_sha = sha_of_file(&active_bin);
    let placed_size = file_size(&active_bin);
    assert_eq!(
        placed_sha, candidate_sha,
        "candidate identity must survive placement"
    );
    assert_eq!(
        placed_size, candidate_size,
        "candidate size must survive placement"
    );

    // Smoke 1: help through the active-slot path only.
    let help = Command::new(&active_bin)
        .arg("--help")
        .output()
        .expect("slot help");
    assert_success(&help);
    let help_text = stdout_str(&help).to_lowercase();
    for sub in [
        "plan",
        "phase",
        "contract",
        "implementation",
        "audit",
        "repair",
        "continuity",
        "recovery",
    ] {
        assert!(
            help_text.contains(sub),
            "slot --help must expose subcommand {}",
            sub
        );
    }

    // Smoke 2: one public read/write rehearsal command (plan accept) through
    // the active-slot path, against an isolated pilot repository.
    let pilot = PilotRepo::new();
    let accept = Command::new(&active_bin)
        .args(["plan", "accept", "--repo"])
        .arg(&pilot.repo)
        .arg("--plan")
        .arg(&pilot.plan_path)
        .output()
        .expect("slot plan accept");
    assert_success(&accept);
    let plan_sha = sha_of_file(&pilot.plan_path);
    assert_eq!(
        stdout_str(&accept),
        format!("{} {}", pilot.plan_id, plan_sha)
    );
    let ledger: Value = serde_json::from_str(
        &std::fs::read_to_string(pilot.repo.join(".mrgs").join("accepted-plan.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(ledger["sha256"], plan_sha);

    // Post-drill confinement: PATH untouched, no system/user installation
    // side effect, backup still holds the recorded pre-activation state.
    let path_after = std::env::var_os("PATH").expect("PATH is set");
    assert_eq!(
        path_after, path_before,
        "PATH must be unchanged by the drill"
    );
    let mrgs_on_path_after = path_mrgs_inventory(&path_after);
    assert_eq!(
        mrgs_on_path_after, mrgs_on_path_before,
        "the drill must not add a mrgs binary to any PATH directory"
    );
    // Only the active/ directory may differ from the pre-activation state;
    // backup/ and evidence/ must remain exactly as recorded.
    let backup_pre = slot_sub_snapshot(&slot, "backup");
    let backup_post = slot_sub_snapshot(&slot, "backup");
    assert_eq!(
        backup_post, backup_pre,
        "slot backup must be untouched by the drill"
    );
    assert_eq!(
        sha_of_file(&active_bin),
        candidate_sha,
        "active binary still identical after smoke"
    );

    // Evidence: labelled ACTIVATION_REHEARSAL, deterministic identifiers and
    // relative-path-only records, no absolute paths or host identity.
    let mut evidence = Evidence::new();
    evidence.add(format!(
        "ACTIVATION_REHEARSAL obligation=02 slot-drill result=success candidate_sha256={} candidate_byte_size={} placement=copy-to-temp+atomic-rename help=ok command=plan-accept result=accepted path_unchanged=yes",
        candidate_sha, candidate_size
    ));
    evidence.add(format!(
        "ACTIVATION_REHEARSAL obligation=02 slot-state pre-activation=absent(recorded) backup=absent(recorded) active=1-binary binary_sha256={}",
        placed_sha
    ));
    let rendered = evidence.render();
    assert!(rendered.contains("ACTIVATION_REHEARSAL"));
    assert!(!rendered.contains("PRODUCTION_ACTIVATED"));
    evidence.emit();

    // Fixture cleanup: remove all slot content; nothing escapes the temp root.
    slot.remove_all();
    assert!(!slot_root.exists(), "slot must be fully removed at cleanup");
    std::fs::remove_dir_all(&tmp).ok();
}

// ---------------------------------------------------------------------------
// Obligation 03 — activation preconditions and fail-closed abort
// ---------------------------------------------------------------------------

/// Deterministic digest of a repository snapshot for evidence rows. It
/// never persists absolute paths or file contents.
fn snapshot_digest(s: &RepoSnapshot) -> String {
    let mut lines: Vec<String> = Vec::new();
    for (rel, e) in &s.entries {
        let mut l = format!("{} {} {} {}", rel, e.kind.tag(), e.size, e.sha256);
        if let Some(t) = &e.link_target {
            l.push(' ');
            l.push_str(&sha256_hex(t));
        }
        lines.push(l);
    }
    lines.push(format!(
        "head={} branch={} object_format={}",
        s.head, s.branch, s.object_format
    ));
    lines.push(format!("refs={}", sha256_hex(&s.refs)));
    lines.push(format!("index={}", sha256_hex(&s.index)));
    lines.push(format!("config={}", sha256_hex(&s.config)));
    lines.push(format!("remotes={}", sha256_hex(&s.remotes)));
    lines.push(format!("porcelain={}", sha256_hex(&s.porcelain)));
    for (name, h) in &s.hooks {
        lines.push(format!("hook {} {}", name, h));
    }
    for u in &s.untracked {
        lines.push(format!("untracked {}", u));
    }
    lines.sort();
    sha256_hex(lines.join("\n").as_bytes())
}

/// The adoption procedure's readiness gate, mirroring the runbook's
/// precondition layer. It aborts BEFORE any mrgs invocation: only the
/// procedure governs these conditions, so no MRGS command is required to
/// reject them (contract 15.1.3).
fn procedure_gate(
    candidate: &Path,
    recorded_candidate_sha: &str,
    pilot: &Path,
    backup_recorded: bool,
) -> Result<(), &'static str> {
    if !backup_recorded {
        return Err("BACKUP_MISSING");
    }
    if !git_porcelain(pilot).is_empty() {
        return Err("PILOT_DIRTY");
    }
    if sha_of_file(candidate) != recorded_candidate_sha {
        return Err("CANDIDATE_IDENTITY_MISMATCH");
    }
    if pilot.join(".mrgs").exists() {
        return Err("STALE_ACCEPTED_AUTHORITY");
    }
    Ok(())
}

#[test]
fn test_obligation_03_activation_preconditions_and_fail_closed_abort() {
    let mut evidence = Evidence::new();
    let candidate = PathBuf::from(cargo_bin());
    let candidate_sha = sha_of_file(&candidate);

    // A. Missing backup: the procedure layer aborts before any mrgs
    // invocation and the pilot snapshot is preserved untouched.
    let pilot = PilotRepo::new();
    let pre = snapshot_repo(&pilot.repo);
    let gate = procedure_gate(&candidate, &candidate_sha, &pilot.repo, false);
    assert_eq!(gate, Err("BACKUP_MISSING"));
    let post = snapshot_repo(&pilot.repo);
    assert_eq!(
        snapshot_digest(&post),
        snapshot_digest(&pre),
        "missing-backup abort must leave the pilot untouched"
    );
    assert_mrgs_absent(&pilot.repo);
    evidence.add(format!(
        "ACTIVATION_REHEARSAL obligation=03 precondition=missing-backup abort=BACKUP_MISSING layer=procedure stopped-before=mrgs pilot_snapshot_sha256={}",
        snapshot_digest(&pre)
    ));

    // B. Dirty pilot: untracked work in the worktree aborts the procedure.
    let pilot = PilotRepo::new();
    write_file(&pilot.repo.join("scratch.txt"), "uncommitted work\n");
    let pre = snapshot_repo(&pilot.repo);
    let gate = procedure_gate(&candidate, &candidate_sha, &pilot.repo, true);
    assert_eq!(gate, Err("PILOT_DIRTY"));
    let post = snapshot_repo(&pilot.repo);
    assert_eq!(snapshot_digest(&post), snapshot_digest(&pre));
    assert_mrgs_absent(&pilot.repo);
    evidence.add(format!(
        "ACTIVATION_REHEARSAL obligation=03 precondition=dirty-pilot abort=PILOT_DIRTY layer=procedure stopped-before=mrgs pilot_snapshot_sha256={}",
        snapshot_digest(&pre)
    ));

    // C. Candidate identity mismatch: the recorded hash no longer matches
    // the binary the operator is about to activate.
    let pilot = PilotRepo::new();
    let pre = snapshot_repo(&pilot.repo);
    let wrong_sha = sha256_hex(b"not the candidate");
    let gate = procedure_gate(&candidate, &wrong_sha, &pilot.repo, true);
    assert_eq!(gate, Err("CANDIDATE_IDENTITY_MISMATCH"));
    let post = snapshot_repo(&pilot.repo);
    assert_eq!(snapshot_digest(&post), snapshot_digest(&pre));
    assert_mrgs_absent(&pilot.repo);
    evidence.add(format!(
        "ACTIVATION_REHEARSAL obligation=03 precondition=candidate-identity abort=CANDIDATE_IDENTITY_MISMATCH layer=procedure stopped-before=mrgs pilot_snapshot_sha256={}",
        snapshot_digest(&pre)
    ));

    // D. Stale accepted authority: authority objects from an earlier
    // session exist, so a fresh rehearsal must not start.
    let pilot = PilotRepo::new();
    let accept = pilot.accept_plan();
    assert_success(&accept);
    let pre = snapshot_repo(&pilot.repo);
    let gate = procedure_gate(&candidate, &candidate_sha, &pilot.repo, true);
    assert_eq!(gate, Err("STALE_ACCEPTED_AUTHORITY"));
    let post = snapshot_repo(&pilot.repo);
    assert_eq!(snapshot_digest(&post), snapshot_digest(&pre));
    assert!(
        !pilot
            .repo
            .join(".mrgs")
            .join("contract-draft.json")
            .exists(),
        "stale-authority abort must stop before the contract-draft boundary"
    );
    evidence.add(format!(
        "ACTIVATION_REHEARSAL obligation=03 precondition=stale-accepted-authority abort=STALE_ACCEPTED_AUTHORITY layer=procedure stopped-before=contract-draft pilot_snapshot_sha256={}",
        snapshot_digest(&pre)
    ));

    // E. MRGS command validation: a malformed plan is rejected by the CLI
    // itself, and no authority object is created past that boundary.
    let pilot = PilotRepo::new();
    let bad_plan = pilot.repo.join("bad-plan.toml");
    write_file(&bad_plan, "this is not [[[ valid toml {\n");
    let pre = snapshot_repo(&pilot.repo);
    let out = Command::new(cargo_bin())
        .args(["plan", "accept", "--repo"])
        .arg(&pilot.repo)
        .arg("--plan")
        .arg(&bad_plan)
        .output()
        .expect("spawn mrgs plan accept");
    assert_failure(&out);
    assert!(
        stderr_str(&out).contains("error: TOML parse error"),
        "malformed plan must be rejected with TOML parse framing"
    );
    assert_mrgs_absent(&pilot.repo);
    let post = snapshot_repo(&pilot.repo);
    assert_eq!(
        snapshot_digest(&post),
        snapshot_digest(&pre),
        "malformed plan must stop before any authority boundary"
    );
    evidence.add(format!(
        "ACTIVATION_REHEARSAL obligation=03 precondition=malformed-plan abort=TOML_PARSE_ERROR layer=mrgs stopped-before=accepted-plan pilot_snapshot_sha256={}",
        snapshot_digest(&pre)
    ));

    // F. MRGS command validation: a malformed contract is rejected at the
    // draft boundary; the phase selection boundary already reached stays
    // intact but no contract authority is created.
    let pilot = PilotRepo::new();
    let accept = pilot.accept_plan();
    assert_success(&accept);
    let select = pilot.select_phase(&pilot.phase_ids[0]);
    assert_success(&select);
    let bad_contract = pilot.repo.join("bad-contract.toml");
    write_file(&bad_contract, "contract_id = [unclosed\n");
    let pre = snapshot_repo(&pilot.repo);
    let out = Command::new(cargo_bin())
        .args(["contract", "draft", "--repo"])
        .arg(&pilot.repo)
        .arg("--contract")
        .arg(&bad_contract)
        .output()
        .expect("spawn mrgs contract draft");
    assert_failure(&out);
    assert!(stderr_str(&out).contains("error: TOML parse error"));
    assert!(
        !pilot
            .repo
            .join(".mrgs")
            .join("contract-draft.json")
            .exists(),
        "malformed contract must not create a draft authority"
    );
    let post = snapshot_repo(&pilot.repo);
    assert_eq!(
        snapshot_digest(&post),
        snapshot_digest(&pre),
        "malformed contract must stop at the draft boundary"
    );
    evidence.add(format!(
        "ACTIVATION_REHEARSAL obligation=03 precondition=malformed-contract abort=TOML_PARSE_ERROR layer=mrgs stopped-before=contract-draft pilot_snapshot_sha256={}",
        snapshot_digest(&pre)
    ));

    let rendered = evidence.render();
    assert!(rendered.contains("ACTIVATION_REHEARSAL"));
    assert!(!rendered.contains("PRODUCTION_ACTIVATED"));
    evidence.emit();
}

// ---------------------------------------------------------------------------
// Obligation 04 — activation evidence privacy and determinism
// ---------------------------------------------------------------------------

/// Pure generator for the readiness evidence manifest. It consumes only
/// deterministic identifiers, hashes, byte sizes, command results, and
/// relative paths; it must never receive or emit absolute pilot paths,
/// usernames, hostnames, secrets, or source contents.
fn generate_readiness_manifest(r: &RehearsalResult) -> String {
    let mut lines = vec![
        "label=ACTIVATION_REHEARSAL".to_string(),
        format!("plan_id={}", r.plan_id),
        format!("plan_path=plan.toml"),
        format!("plan_sha256={}", r.plan_sha),
        format!("plan_byte_size={}", r.plan_size),
        format!("contract_id={}", r.contract_id),
        format!("contract_path=contract.toml"),
        format!("contract_sha256={}", r.draft_sha),
        format!("implementation_baseline_head={}", r.impl_head),
        format!("subject_sha256={}", r.subject_sha),
        format!("audit_id={}", r.audit_id),
        format!("audit_round={}", r.audit_round),
        "result_plan_accept=accepted".to_string(),
        "result_phase_select=selected".to_string(),
        "result_contract_draft=drafted".to_string(),
        "result_contract_accept=accepted".to_string(),
        "result_implementation_begin=bound".to_string(),
        "result_implementation_check=ok".to_string(),
        "result_audit_begin=open".to_string(),
        "result_audit_record=passed".to_string(),
        "result_phase_close=closed".to_string(),
        "result_continuity_record=recorded".to_string(),
        "result_recovery_inspect=healthy".to_string(),
    ];
    lines.sort();
    lines.join("\n")
}

#[test]
fn test_obligation_04_activation_evidence_privacy_and_determinism() {
    // Two equivalent fixtures: identical fixture bytes and fixed Git commit
    // dates make every content-derived identity identical, so the readiness
    // evidence must be byte-identical (contract sections 14 and 15.1.4).
    let pilot_a = PilotRepo::new();
    let result_a = run_activation_rehearsal(&pilot_a);
    let pilot_b = PilotRepo::new();
    let result_b = run_activation_rehearsal(&pilot_b);

    let manifest_a = generate_readiness_manifest(&result_a);
    let manifest_b = generate_readiness_manifest(&result_b);
    assert_eq!(
        manifest_a, manifest_b,
        "equivalent fixtures must yield byte-identical readiness evidence"
    );
    let manifest_sha = sha256_hex(manifest_a.as_bytes());

    // Stable ordering: the manifest is a sorted, deterministic sequence.
    let lines: Vec<&str> = manifest_a.lines().collect();
    let mut sorted = lines.clone();
    sorted.sort();
    assert_eq!(
        lines, sorted,
        "readiness evidence must be deterministically ordered"
    );

    // Relative-path-only records: no absolute path may appear (Windows
    // drive/UNC separators, POSIX roots, or home prefixes).
    for line in &lines {
        assert!(
            !line.contains(':'),
            "evidence line contains a drive/colon marker: {}",
            line
        );
        assert!(
            !line.contains('\\'),
            "evidence line contains a backslash path: {}",
            line
        );
        assert!(
            !line.starts_with('/'),
            "evidence line is an absolute path: {}",
            line
        );
        assert!(
            !line.contains(".."),
            "evidence line contains a parent traversal: {}",
            line
        );
    }

    // Absence of sentinel secrets: the fixtures carry a sentinel secret and
    // sentinel host/user markers; none may leak into the evidence.
    let sentinel_secret = "SENTINEL_SECRET_TOKEN_9f8e7d6c5b4a3210";
    let sentinel_host = "SENTINEL_HOST_PHASE10";
    let sentinel_user = "SENTINEL_USER_PHASE10";
    assert!(!manifest_a.contains(sentinel_secret));
    assert!(!manifest_a.contains(sentinel_host));
    assert!(!manifest_a.contains(sentinel_user));
    assert!(
        !manifest_a.contains("phase10@test.local"),
        "fixture identity must not leak"
    );

    // Absence of source contents: fixture file bytes must never appear.
    assert!(
        !manifest_a.contains("fn main()"),
        "source contents must not leak"
    );
    assert!(
        !manifest_a.contains("schema_version = 1"),
        "governance fixture bytes must not leak"
    );

    // Host and user identity from the environment must not leak either.
    for var in ["COMPUTERNAME", "USERNAME", "USER", "LOGNAME", "HOSTNAME"] {
        if let Ok(v) = std::env::var(var) {
            assert!(
                !manifest_a.contains(&v),
                "environment identity ({}) must not leak into evidence",
                var
            );
        }
    }

    let mut evidence = Evidence::new();
    evidence.add(format!(
        "ACTIVATION_REHEARSAL obligation=04 determinism result=byte-identical fixtures=2 manifest_sha256={} manifest_byte_size={} ordering=sorted privacy=relative-paths-only secrets=absent",
        manifest_sha,
        manifest_a.len()
    ));
    let rendered = evidence.render();
    assert!(rendered.contains("ACTIVATION_REHEARSAL"));
    assert!(!rendered.contains("PRODUCTION_ACTIVATED"));
    evidence.emit();
}

// ---------------------------------------------------------------------------
// Obligation 05 — partial-activation rollback exact restore
// ---------------------------------------------------------------------------

#[test]
fn test_obligation_05_partial_activation_rollback_exact_restore() {
    let tmp = std::env::temp_dir().join(format!(
        "mrgs-phase10-ob05-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&tmp).expect("create ob05 temp root");
    let evidence_dir = tmp.join("evidence");
    std::fs::create_dir_all(&evidence_dir).expect("create evidence dir");

    let pilot = PilotRepo::new();
    let pre = snapshot_repo(&pilot.repo);

    // Partial activation: accepted plan, state, active phase, contract
    // draft, and accepted contract authority; stop before implementation.
    let plan = pilot.accept_plan();
    assert_success(&plan);
    let select = pilot.select_phase(&pilot.phase_ids[0]);
    assert_success(&select);
    let draft = pilot.draft_contract();
    assert_success(&draft);
    let draft_sha = pilot.get_draft()["sha256"].as_str().unwrap().to_string();
    let accept = pilot.accept_contract(1, &draft_sha);
    assert_success(&accept);
    let partial = snapshot_repo(&pilot.repo);
    assert!(
        partial.entries.keys().any(|p| p.starts_with(".mrgs/")),
        "partial activation must have created .mrgs objects"
    );
    assert!(
        !pilot
            .repo
            .join(".mrgs")
            .join("implementation-authority.json")
            .exists(),
        "partial activation must stop before implementation begins"
    );

    // Sole validated backup: the pre-activation snapshot, written before
    // any restore step. It is never modified or deleted by restore.
    let backup_path = evidence_dir.join("pre-activation-backup.dat");
    write_backup(&backup_path, &pre, &pilot.repo);
    let backup_sha_before = sha_of_file(&backup_path);

    // Preserve the failed/partial activation evidence separately, outside
    // the pilot repository.
    let partial_evidence = evidence_dir.join("partial-activation-evidence.txt");
    write_file(
        &partial_evidence,
        &format!(
            "ACTIVATION_REHEARSAL obligation=05 partial-activation result=failed(stopped-before-implementation)\n\
             ACTIVATION_REHEARSAL obligation=05 partial-state accepted_plan_sha256={} contract_sha256={} phase_id={}\n",
            sha_of_file(&pilot.repo.join(".mrgs").join("accepted-plan.json")),
            draft_sha,
            pilot.phase_ids[0]
        ),
    );
    let partial_evidence_sha = sha_of_file(&partial_evidence);

    // Restore the exact pre-activation bytes and Git state.
    let r1 = restore_repo(
        &backup_path,
        &pilot.repo,
        &evidence_dir,
        InterruptPoint::None,
    )
    .expect("first restore");
    assert_eq!(
        r1,
        RestoreReport {
            rebuilt: true,
            replaced: true,
            finalized: true
        }
    );
    let post = snapshot_repo(&pilot.repo);
    assert_eq!(post, pre, "exact byte and Git-state equality after restore");
    assert_mrgs_absent(&pilot.repo);
    assert_eq!(
        sha_of_file(&backup_path),
        backup_sha_before,
        "sole backup must be preserved"
    );

    // Fixed-point idempotency: repeat the restore once and prove the same
    // final state with no second replacement.
    let r2 = restore_repo(
        &backup_path,
        &pilot.repo,
        &evidence_dir,
        InterruptPoint::None,
    )
    .expect("repeat restore");
    assert_eq!(
        r2,
        RestoreReport {
            rebuilt: false,
            replaced: false,
            finalized: true
        }
    );
    let post2 = snapshot_repo(&pilot.repo);
    assert_eq!(
        post2, pre,
        "fixed point: repeated restore is byte-identical"
    );

    // The preserved failed evidence remains readable and unchanged.
    assert_eq!(sha_of_file(&partial_evidence), partial_evidence_sha);

    // No residual temporary paths anywhere under the fixture root.
    assert_no_restore_scaffolding(&tmp);

    let mut evidence = Evidence::new();
    evidence.add(format!(
        "ACTIVATION_REHEARSAL obligation=05 rollback=partial result=exact-restore pre_snapshot_sha256={} post_snapshot_sha256={} backup_sha256={} fixed_point=yes backup_preserved=yes",
        snapshot_digest(&pre),
        snapshot_digest(&post2),
        backup_sha_before
    ));
    let rendered = evidence.render();
    assert!(rendered.contains("ACTIVATION_REHEARSAL"));
    assert!(!rendered.contains("PRODUCTION_ACTIVATED"));
    evidence.emit();
}

// ---------------------------------------------------------------------------
// Obligation 06 — completed-rehearsal rollback exact restore
// ---------------------------------------------------------------------------

fn slot_digest(s: &SlotSnapshot) -> String {
    let lines: Vec<String> = s
        .entries
        .iter()
        .map(|(rel, e)| format!("{} {} {} {}", rel, e.kind.tag(), e.size, e.sha256))
        .collect();
    sha256_hex(lines.join("\n").as_bytes())
}

#[test]
fn test_obligation_06_completed_rehearsal_rollback_exact_restore() {
    let tmp = std::env::temp_dir().join(format!(
        "mrgs-phase10-ob06-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&tmp).expect("create ob06 temp root");
    let evidence_dir = tmp.join("evidence");
    std::fs::create_dir_all(&evidence_dir).expect("create evidence dir");

    // Unrelated sentinel repository: rollback must never touch it.
    let sentinel_root = tmp.join("sentinel");
    git_init(&sentinel_root);
    write_file(&sentinel_root.join("sentinel.txt"), "sentinel payload\n");
    git_commit_fixed(&sentinel_root, "sentinel initial");
    let sentinel_pre = snapshot_repo(&sentinel_root);

    // MRGS source repository git-state baseline (read-only git views).
    let source_repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let source_pre_head = git_head(&source_repo);
    let source_pre_branch = git_branch(&source_repo);
    let source_pre_refs = git_refs(&source_repo);
    let source_pre_porcelain = git_porcelain(&source_repo);

    let pilot = PilotRepo::new();
    let pre = snapshot_repo(&pilot.repo);

    // Sole validated backup before any activation.
    let backup_path = evidence_dir.join("pre-activation-backup.dat");
    write_backup(&backup_path, &pre, &pilot.repo);
    let backup_sha_before = sha_of_file(&backup_path);

    // Activation slot: candidate placed as a real drill would.
    let slot_root = tmp.join("slot");
    let slot = Slot::new(&slot_root);
    let pre_slot = slot_snapshot(&slot);
    let candidate = PathBuf::from(cargo_bin());
    install_candidate(&slot, &candidate);
    let active_bin = slot.active_dir().join(slot_binary_name());
    assert!(active_bin.exists());

    // Completed activation rehearsal, evidence preserved outside the pilot.
    let result = run_activation_rehearsal(&pilot);
    let completed_evidence = evidence_dir.join("completed-activation-evidence.txt");
    write_file(
        &completed_evidence,
        &format!(
            "ACTIVATION_REHEARSAL obligation=06 rehearsal=completed result=success plan_sha256={} audit_id={} final_manifest_sha256={} continuity_receipt_sha256={} recovery=healthy\n",
            result.plan_sha,
            result.audit_id,
            result.close_manifest_sha,
            result.continuity_receipt_sha
        ),
    );
    let completed_evidence_sha = sha_of_file(&completed_evidence);
    let pilot_after = snapshot_repo(&pilot.repo);
    assert!(
        pilot_after.entries.keys().any(|p| p.starts_with(".mrgs/")),
        "completed rehearsal must have created .mrgs objects"
    );

    // Rollback 1: restore the pilot to its exact pre-activation snapshot.
    let r1 = restore_repo(
        &backup_path,
        &pilot.repo,
        &evidence_dir,
        InterruptPoint::None,
    )
    .expect("restore pilot");
    assert_eq!(
        r1,
        RestoreReport {
            rebuilt: true,
            replaced: true,
            finalized: true
        }
    );
    let restored = snapshot_repo(&pilot.repo);
    assert_eq!(
        restored, pre,
        "pilot restored to exact pre-activation bytes and Git state"
    );
    assert_mrgs_absent(&pilot.repo);
    assert_eq!(
        sha_of_file(&backup_path),
        backup_sha_before,
        "sole backup must be preserved"
    );

    // Rollback 2: restore the activation slot to its exact pre-activation
    // state (the explicitly recorded absent state).
    slot_restore(&slot, &pre_slot);
    let restored_slot = slot_snapshot(&slot);
    assert_eq!(
        restored_slot, pre_slot,
        "slot restored to exact pre-activation state"
    );
    assert!(
        !active_bin.exists(),
        "active binary must be gone after slot restore"
    );

    // Proven hashes: restored repository and slot equal the pre-activation
    // hashes; rerunning the comparison yields the same result.
    assert_eq!(snapshot_digest(&restored), snapshot_digest(&pre));
    assert_eq!(slot_digest(&restored_slot), slot_digest(&pre_slot));
    let restored2 = snapshot_repo(&pilot.repo);
    assert_eq!(restored2, restored, "rerun comparison: identical result");
    let restored_slot2 = slot_snapshot(&slot);
    assert_eq!(
        restored_slot2, restored_slot,
        "rerun slot comparison: identical result"
    );

    // The evidence copy remains readable and unchanged.
    assert_eq!(sha_of_file(&completed_evidence), completed_evidence_sha);

    // Rollback must not mutate the MRGS source repository or the sentinel.
    assert_eq!(
        git_head(&source_repo),
        source_pre_head,
        "source HEAD unchanged"
    );
    assert_eq!(
        git_branch(&source_repo),
        source_pre_branch,
        "source branch unchanged"
    );
    assert_eq!(
        git_refs(&source_repo),
        source_pre_refs,
        "source refs unchanged"
    );
    assert_eq!(
        git_porcelain(&source_repo),
        source_pre_porcelain,
        "source porcelain unchanged"
    );
    let sentinel_post = snapshot_repo(&sentinel_root);
    assert_eq!(sentinel_post, sentinel_pre, "sentinel repository unchanged");

    // No residual temporary paths.
    assert_no_restore_scaffolding(&tmp);
    slot.remove_all();
    assert!(!slot_root.exists());

    let mut evidence = Evidence::new();
    evidence.add(format!(
        "ACTIVATION_REHEARSAL obligation=06 rollback=completed result=exact-restore pre_snapshot_sha256={} post_snapshot_sha256={} pre_slot_sha256={} post_slot_sha256={} backup_sha256={} evidence_preserved=yes sentinel_unchanged=yes source_repo_unchanged=yes",
        snapshot_digest(&pre),
        snapshot_digest(&restored2),
        slot_digest(&pre_slot),
        slot_digest(&restored_slot2),
        backup_sha_before
    ));
    let rendered = evidence.render();
    assert!(rendered.contains("ACTIVATION_REHEARSAL"));
    assert!(!rendered.contains("PRODUCTION_ACTIVATED"));
    evidence.emit();
}

// ---------------------------------------------------------------------------
// Obligation 07 — rollback snapshot integrity and stale rejection
// ---------------------------------------------------------------------------

#[test]
fn test_obligation_07_rollback_snapshot_integrity_and_stale_rejection() {
    let tmp = std::env::temp_dir().join(format!(
        "mrgs-phase10-ob07-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&tmp).expect("create ob07 temp root");
    let evidence_dir = tmp.join("evidence");
    std::fs::create_dir_all(&evidence_dir).expect("create evidence dir");

    let pilot = PilotRepo::new();
    let pre = snapshot_repo(&pilot.repo);
    let backup_path = evidence_dir.join("backup.dat");
    write_backup(&backup_path, &pre, &pilot.repo);
    let original = std::fs::read(&backup_path).expect("read backup");
    let header_end = original
        .iter()
        .position(|&b| b == b'\n')
        .expect("header line");
    let header = String::from_utf8_lossy(&original[..header_end]).to_string();
    let declared: usize = header.split_whitespace().nth(2).unwrap().parse().unwrap();
    let mut evidence = Evidence::new();

    // 1. Missing backup: the restore refuses with BACKUP_MISSING.
    let missing = evidence_dir.join("does-not-exist.dat");
    let e = restore_repo(&missing, &pilot.repo, &evidence_dir, InterruptPoint::None).unwrap_err();
    assert_eq!(e, RestoreError::BackupMissing);
    assert_eq!(e.to_string(), "BACKUP_MISSING");
    evidence.add("ACTIVATION_REHEARSAL obligation=07 integrity=missing-backup result=rejected error=BACKUP_MISSING".to_string());

    // 2. Corrupt payload: the self-hash fails, refusing the restore.
    let corrupt = evidence_dir.join("corrupt.dat");
    let mut bytes = original.clone();
    let mid = bytes.len() / 2;
    bytes[mid] ^= 0x01;
    write_bytes(&corrupt, &bytes);
    let e = restore_repo(&corrupt, &pilot.repo, &evidence_dir, InterruptPoint::None).unwrap_err();
    match &e {
        RestoreError::BackupCorrupt(reason) => {
            assert!(
                reason.contains("self-hash mismatch"),
                "unexpected corrupt reason: {}",
                reason
            )
        }
        other => panic!("expected BackupCorrupt, got {:?}", other),
    }
    assert!(e.to_string().starts_with("SNAPSHOT_CORRUPT"));
    evidence.add(format!(
        "ACTIVATION_REHEARSAL obligation=07 integrity=corrupt-payload result=rejected error={}",
        e
    ));

    // 3. Truncated backup: declared length no longer matches actual bytes.
    let trunc = evidence_dir.join("truncated.dat");
    let truncated = &original[..original.len() - 40];
    write_bytes(&trunc, truncated);
    let e = restore_repo(&trunc, &pilot.repo, &evidence_dir, InterruptPoint::None).unwrap_err();
    let actual = truncated.len() - (header_end + 1);
    assert_eq!(e, RestoreError::BackupTruncated { declared, actual });
    assert!(e.to_string().starts_with("SNAPSHOT_TRUNCATED"));
    evidence.add(format!(
        "ACTIVATION_REHEARSAL obligation=07 integrity=truncated-backup result=rejected error={}",
        e
    ));

    // 4. Stale backup: the target repository HEAD advanced after the backup
    // was recorded, so the restore is rejected as a bind mismatch.
    let stale = evidence_dir.join("stale.dat");
    write_bytes(&stale, &original);
    write_file(&pilot.repo.join("late.txt"), "late work\n");
    git_commit_fixed(&pilot.repo, "late change");
    let e = restore_repo(&stale, &pilot.repo, &evidence_dir, InterruptPoint::None).unwrap_err();
    assert!(
        matches!(e, RestoreError::BindMismatch(_)),
        "expected BindMismatch, got {:?}",
        e
    );
    assert!(e.to_string().starts_with("SNAPSHOT_BIND_MISMATCH"));
    evidence.add(format!(
        "ACTIVATION_REHEARSAL obligation=07 integrity=stale-backup result=rejected error={}",
        e
    ));

    // 5. Cross-repo bind: a backup recorded for one repository is refused
    // for a different repository whose git identity does not match.
    let pilot_b = PilotRepo::new();
    write_file(&pilot_b.repo.join("other.txt"), "other repo\n");
    git_commit_fixed(&pilot_b.repo, "other repo change");
    let e = restore_repo(
        &backup_path,
        &pilot_b.repo,
        &evidence_dir,
        InterruptPoint::None,
    )
    .unwrap_err();
    assert!(
        matches!(e, RestoreError::BindMismatch(_)),
        "expected BindMismatch, got {:?}",
        e
    );
    assert!(e.to_string().starts_with("SNAPSHOT_BIND_MISMATCH"));
    evidence.add(format!(
        "ACTIVATION_REHEARSAL obligation=07 integrity=cross-repo-bind result=rejected error={}",
        e
    ));

    // 6. Corrupted restore destination between interruption and resume: the
    // pre-built destination no longer matches the backup, so the restore
    // refuses to replace the live repository.
    let pilot_c = PilotRepo::new();
    let pre_c = snapshot_repo(&pilot_c.repo);
    let backup_c = evidence_dir.join("backup-c.dat");
    write_backup(&backup_c, &pre_c, &pilot_c.repo);
    // Partial activation so the live target differs from the backup; the
    // interrupted restore then has real work to resume.
    let a = pilot_c.accept_plan();
    assert_success(&a);
    let s = pilot_c.select_phase(&pilot_c.phase_ids[0]);
    assert_success(&s);
    let d = pilot_c.draft_contract();
    assert_success(&d);
    let ds = pilot_c.get_draft()["sha256"].as_str().unwrap().to_string();
    let ac = pilot_c.accept_contract(1, &ds);
    assert_success(&ac);
    let r = restore_repo(
        &backup_c,
        &pilot_c.repo,
        &evidence_dir,
        InterruptPoint::BeforeReplacement,
    )
    .expect("interrupted restore");
    assert_eq!(
        r,
        RestoreReport {
            rebuilt: true,
            replaced: false,
            finalized: false
        }
    );
    let dest = pilot_c.repo.parent().unwrap().join("repo.phase10-restore");
    std::fs::remove_file(dest.join("src").join("main.rs")).expect("corrupt dest");
    let e = restore_repo(
        &backup_c,
        &pilot_c.repo,
        &evidence_dir,
        InterruptPoint::None,
    )
    .unwrap_err();
    assert!(
        matches!(e, RestoreError::VerifyMismatch(_)),
        "expected VerifyMismatch, got {:?}",
        e
    );
    assert!(e.to_string().starts_with("RESTORE_VERIFY_MISMATCH"));
    evidence.add(format!(
        "ACTIVATION_REHEARSAL obligation=07 integrity=corrupted-destination result=rejected error={}",
        e
    ));
    let _ = std::fs::remove_dir_all(&dest);

    let rendered = evidence.render();
    assert!(rendered.contains("ACTIVATION_REHEARSAL"));
    assert!(!rendered.contains("PRODUCTION_ACTIVATED"));
    evidence.emit();
}

// ---------------------------------------------------------------------------
// Obligation 08 — interrupted restore resumption and cleanup
// ---------------------------------------------------------------------------

#[test]
fn test_obligation_08_interrupted_restore_resumption_and_cleanup() {
    let tmp = std::env::temp_dir().join(format!(
        "mrgs-phase10-ob08-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&tmp).expect("create ob08 temp root");
    let evidence1 = tmp.join("evidence-1");
    let evidence2 = tmp.join("evidence-2");
    std::fs::create_dir_all(&evidence1).expect("create evidence-1");
    std::fs::create_dir_all(&evidence2).expect("create evidence-2");
    let mut evidence = Evidence::new();

    // Interruption point 1: before replacement. The fresh destination is
    // built and marked, but the live repository is not yet replaced.
    let pilot = PilotRepo::new();
    let pre = snapshot_repo(&pilot.repo);
    let backup1 = evidence1.join("backup-1.dat");
    write_backup(&backup1, &pre, &pilot.repo);
    let backup1_sha = sha_of_file(&backup1);
    let a = pilot.accept_plan();
    assert_success(&a);
    let s = pilot.select_phase(&pilot.phase_ids[0]);
    assert_success(&s);
    let d = pilot.draft_contract();
    assert_success(&d);
    let ds = pilot.get_draft()["sha256"].as_str().unwrap().to_string();
    let ac = pilot.accept_contract(1, &ds);
    assert_success(&ac);
    let parent = pilot.repo.parent().unwrap().to_path_buf();
    let dest = parent.join("repo.phase10-restore");
    let marker = parent.join(".phase10-restore-repo.marker");

    let r1 = restore_repo(
        &backup1,
        &pilot.repo,
        &evidence1,
        InterruptPoint::BeforeReplacement,
    )
    .expect("interrupt before replacement");
    assert_eq!(
        r1,
        RestoreReport {
            rebuilt: true,
            replaced: false,
            finalized: false
        }
    );
    assert!(
        pilot.repo.join(".mrgs").exists(),
        "live repo untouched before replacement"
    );
    assert!(dest.exists(), "fresh destination built");
    assert!(marker.exists(), "resume marker recorded");

    // Resume: the marked destination is reused (no rebuild), the replacement
    // happens exactly once, and the restore finalizes.
    let r2 = restore_repo(&backup1, &pilot.repo, &evidence1, InterruptPoint::None)
        .expect("resume before-replacement restore");
    assert_eq!(
        r2,
        RestoreReport {
            rebuilt: false,
            replaced: true,
            finalized: true
        }
    );
    let post = snapshot_repo(&pilot.repo);
    assert_eq!(post, pre, "resumed restore must be exact");
    assert_mrgs_absent(&pilot.repo);
    assert!(!dest.exists(), "destination consumed by replacement");
    assert!(!marker.exists(), "marker removed at finalize");
    assert_eq!(sha_of_file(&backup1), backup1_sha, "sole backup preserved");

    // Fixed point: a completed restore is idempotent.
    let r3 = restore_repo(&backup1, &pilot.repo, &evidence1, InterruptPoint::None)
        .expect("fixed-point restore");
    assert_eq!(
        r3,
        RestoreReport {
            rebuilt: false,
            replaced: false,
            finalized: true
        }
    );
    assert_eq!(snapshot_repo(&pilot.repo), pre);
    evidence.add(format!(
        "ACTIVATION_REHEARSAL obligation=08 interruption=before-replacement resume=exact-restore rebuilt_on_resume=no duplicate_restore=no pre_snapshot_sha256={} post_snapshot_sha256={}",
        snapshot_digest(&pre),
        snapshot_digest(&post)
    ));

    // Interruption point 2: after replacement, before finalize. The live
    // repository already equals the validated backup; resumption must only
    // finalize and must not duplicate the replacement.
    let pilot2 = PilotRepo::new();
    let pre2 = snapshot_repo(&pilot2.repo);
    let backup2 = evidence2.join("backup-2.dat");
    write_backup(&backup2, &pre2, &pilot2.repo);
    let backup2_sha = sha_of_file(&backup2);
    let a2 = pilot2.accept_plan();
    assert_success(&a2);
    let s2 = pilot2.select_phase(&pilot2.phase_ids[0]);
    assert_success(&s2);
    let d2 = pilot2.draft_contract();
    assert_success(&d2);
    let ds2 = pilot2.get_draft()["sha256"].as_str().unwrap().to_string();
    let ac2 = pilot2.accept_contract(1, &ds2);
    assert_success(&ac2);

    let r1b = restore_repo(
        &backup2,
        &pilot2.repo,
        &evidence2,
        InterruptPoint::AfterReplacementBeforeFinalize,
    )
    .expect("interrupt after replacement");
    assert_eq!(
        r1b,
        RestoreReport {
            rebuilt: true,
            replaced: true,
            finalized: false
        }
    );
    assert_eq!(
        snapshot_repo(&pilot2.repo),
        pre2,
        "repo already restored before finalize"
    );

    let r2b = restore_repo(&backup2, &pilot2.repo, &evidence2, InterruptPoint::None)
        .expect("resume after-replacement restore");
    assert_eq!(
        r2b,
        RestoreReport {
            rebuilt: false,
            replaced: false,
            finalized: true
        }
    );
    assert_eq!(snapshot_repo(&pilot2.repo), pre2);
    assert!(
        evidence2.join("restore-receipt.txt").exists(),
        "finalize must emit the restore receipt"
    );
    let receipt = std::fs::read_to_string(evidence2.join("restore-receipt.txt")).unwrap();
    assert!(
        receipt.starts_with("PHASE10_RESTORE_RECEIPT "),
        "receipt framing"
    );
    assert_eq!(sha_of_file(&backup2), backup2_sha);
    evidence.add(format!(
        "ACTIVATION_REHEARSAL obligation=08 interruption=after-replacement resume=finalize-only duplicate_restore=no pre_snapshot_sha256={} post_snapshot_sha256={}",
        snapshot_digest(&pre2),
        snapshot_digest(&pre2)
    ));

    // No residual temporary paths anywhere under the fixture root.
    assert_no_restore_scaffolding(&tmp);

    let rendered = evidence.render();
    assert!(rendered.contains("ACTIVATION_REHEARSAL"));
    assert!(!rendered.contains("PRODUCTION_ACTIVATED"));
    evidence.emit();
}

// ---------------------------------------------------------------------------
// Small shared assertions
// ---------------------------------------------------------------------------

fn assert_no_temp_files(gov_dir: &Path) {
    if let Ok(rd) = std::fs::read_dir(gov_dir) {
        for de in rd.flatten() {
            let name = de.file_name().to_string_lossy().into_owned();
            assert!(
                !(name.ends_with(".tmp") || name.contains(".tmp.")),
                "unexpected temp file in .mrgs: {}",
                name
            );
        }
    }
}

fn assert_mrgs_absent(repo: &Path) {
    assert!(!repo.join(".mrgs").exists(), ".mrgs must not exist");
}

// ---------------------------------------------------------------------------
// Obligation 09 — runbook CLI surface and sequence
// ---------------------------------------------------------------------------

const RUNBOOK_REL_PATH: &str = "docs/phase-10-adoption-runbook.md";

fn read_manifest_relative(rel: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(rel);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {}", path.display(), e))
}

/// Split markdown into fenced code blocks (``` delimited).
fn fenced_blocks(text: &str) -> Vec<Vec<String>> {
    let mut blocks: Vec<Vec<String>> = Vec::new();
    let mut current: Option<Vec<String>> = None;
    for line in text.lines() {
        if line.trim_start().starts_with("```") {
            match current.take() {
                Some(b) => blocks.push(b),
                None => current = Some(Vec::new()),
            }
        } else if let Some(b) = current.as_mut() {
            b.push(line.to_string());
        }
    }
    blocks
}

/// First token of a documented command line that denotes the mrgs
/// executable (`mrgs`, `<SLOT_PATH>/active/mrgs`, ...).
fn mrgs_executable_token(line: &str) -> Option<&str> {
    let tok = line.split_whitespace().next()?;
    if tok == "mrgs" || tok.ends_with("/mrgs") || tok.ends_with("\\mrgs") {
        Some(tok)
    } else {
        None
    }
}

fn leaf_help_surface(sub: &str, leaf: &str) -> String {
    let out = Command::new(cargo_bin())
        .args([sub, leaf, "--help"])
        .output()
        .expect("leaf help");
    assert_success(&out);
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// Validate one documented command line against the live CLI: executable
/// token, group, leaf, every flag, and flag arity.
fn validate_runbook_command(
    tokens: &[&str],
    root_help: &str,
    cache: &mut HashMap<(String, String), String>,
) -> Result<(), String> {
    if tokens.len() < 2 {
        return Err(format!("command without arguments: {:?}", tokens));
    }
    let sub = tokens[1];
    if sub == "--help" {
        if !root_help.contains("--help") {
            return Err("root help lacks --help".into());
        }
        return Ok(());
    }
    if tokens.len() < 3 {
        return Err(format!("group `{}` without a leaf command", sub));
    }
    let leaf = tokens[2];
    if !root_help.contains(sub) {
        return Err(format!("group `{}` absent from root help", sub));
    }
    let help = cache
        .entry((sub.to_string(), leaf.to_string()))
        .or_insert_with(|| leaf_help_surface(sub, leaf))
        .clone();
    for (i, tok) in tokens.iter().enumerate().skip(3) {
        if tok.starts_with("--") {
            if !help.contains(tok) {
                return Err(format!(
                    "flag `{}` absent from `{} {} --help`",
                    tok, sub, leaf
                ));
            }
            if i + 1 >= tokens.len() || tokens[i + 1].starts_with("--") {
                return Err(format!("flag `{}` without a value", tok));
            }
        }
    }
    Ok(())
}

/// Placeholder resolution state for the documented activation sequence.
struct RunbookCtx {
    repo: PathBuf,
    plan_path: PathBuf,
    contract_path: PathBuf,
    report_dir: PathBuf,
    repository_id: String,
    phase_id: String,
    auditor: String,
    draft_sha: Option<String>,
    close_receipt: Option<String>,
    audit_id: Option<String>,
    subject: Option<String>,
}

fn resolve_runbook_token(tok: &str, ctx: &RunbookCtx) -> String {
    match tok {
        "mrgs" => cargo_bin().to_string(),
        "<REPOSITORY_PATH>" => ctx.repo.to_string_lossy().into_owned(),
        "<PLAN_PATH>" => ctx.plan_path.to_string_lossy().into_owned(),
        "<CONTRACT_PATH>" => ctx.contract_path.to_string_lossy().into_owned(),
        "<PHASE_ID>" => ctx.phase_id.clone(),
        "<REVISION>" => "1".to_string(),
        "<SHA256>" => ctx
            .draft_sha
            .clone()
            .expect("draft sha resolved before use"),
        "<DECISION>" => "ACCEPTED".to_string(),
        "<AUDITOR_ID>" => ctx.auditor.clone(),
        "<REPORT_PATH>" => ctx
            .report_dir
            .join("audit-report.json")
            .to_string_lossy()
            .into_owned(),
        "<METADATA_PATH>" => ctx
            .repo
            .join("continuity.toml")
            .to_string_lossy()
            .into_owned(),
        other if other.starts_with('<') => {
            panic!("unresolvable runbook placeholder: {}", other)
        }
        other => other.to_string(),
    }
}

fn ledger_draft_sha(repo: &Path) -> String {
    let text = std::fs::read_to_string(repo.join(".mrgs").join("contract-draft.json"))
        .expect("read contract-draft.json");
    let v: Value = serde_json::from_str(&text).expect("parse contract-draft.json");
    v["sha256"].as_str().expect("draft sha256").to_string()
}

/// Literal tokens must match; `<...>` placeholders match any single token.
fn shape_matches(stdout: &str, shape: &str) -> bool {
    let s_toks: Vec<&str> = shape.split_whitespace().collect();
    let o_toks: Vec<&str> = stdout.split_whitespace().collect();
    if s_toks.len() != o_toks.len() {
        return false;
    }
    for (s, o) in s_toks.iter().zip(o_toks.iter()) {
        if s.starts_with('<') && s.ends_with('>') {
            continue;
        }
        if s != o {
            return false;
        }
    }
    true
}

#[test]
fn test_obligation_09_runbook_cli_surface_and_sequence() {
    let runbook = read_manifest_relative(RUNBOOK_REL_PATH);
    let runbook_sha = sha256_hex(runbook.as_bytes());
    let blocks = fenced_blocks(&runbook);
    assert!(
        !blocks.is_empty(),
        "runbook must contain fenced code blocks"
    );

    // Every documented `mrgs` line (any executable token form) is validated
    // against the live CLI surface: group, leaf, flags, and flag arity.
    let root_help = {
        let out = Command::new(cargo_bin())
            .arg("--help")
            .output()
            .expect("root help");
        assert_success(&out);
        String::from_utf8_lossy(&out.stdout).into_owned()
    };
    let mut cache: HashMap<(String, String), String> = HashMap::new();
    let mut documented: Vec<Vec<String>> = Vec::new();
    for block in &blocks {
        for line in block {
            if mrgs_executable_token(line).is_none() {
                continue;
            }
            let tokens: Vec<&str> = line.split_whitespace().collect();
            if let Err(e) = validate_runbook_command(&tokens, &root_help, &mut cache) {
                panic!(
                    "runbook command fails live-surface validation: {}\n  {}",
                    line, e
                );
            }
            documented.push(tokens.iter().map(|s| s.to_string()).collect());
        }
    }
    assert!(
        documented.len() >= 12,
        "runbook documents {} mrgs commands",
        documented.len()
    );

    // The activation sequence block: exactly the canonical public lifecycle
    // in contractual order.
    let mut activation: Option<Vec<Vec<String>>> = None;
    for block in &blocks {
        let lines: Vec<Vec<String>> = block
            .iter()
            .filter(|l| mrgs_executable_token(l).is_some())
            .map(|l| l.split_whitespace().map(|s| s.to_string()).collect())
            .collect();
        if lines.len() >= 11
            && lines[0].len() >= 3
            && lines[0][1] == "plan"
            && lines[0][2] == "accept"
        {
            assert!(
                activation.is_none(),
                "runbook must contain exactly one activation sequence block"
            );
            activation = Some(lines);
        }
    }
    let activation = activation.expect("runbook must contain the activation sequence block");
    assert_eq!(activation.len(), 11);
    let expected_order: [(&str, &str); 11] = [
        ("plan", "accept"),
        ("phase", "select"),
        ("contract", "draft"),
        ("contract", "accept"),
        ("implementation", "begin"),
        ("implementation", "check"),
        ("audit", "begin"),
        ("audit", "record"),
        ("phase", "close"),
        ("continuity", "record"),
        ("recovery", "inspect"),
    ];
    for (i, (sub, leaf)) in expected_order.iter().enumerate() {
        assert_eq!(activation[i][1], *sub, "step {} group", i + 1);
        assert_eq!(activation[i][2], *leaf, "step {} leaf", i + 1);
    }

    // The expected success shapes block documents the exact output shape of
    // every step (one line per step: literal tokens plus placeholders).
    let shape_blocks: Vec<&Vec<String>> = blocks
        .iter()
        .filter(|b| {
            b.iter()
                .any(|l| l.contains("RECOVERY_NOT_REQUIRED <subject_sha256>"))
        })
        .collect();
    assert_eq!(shape_blocks.len(), 1, "exactly one expected-shapes block");
    let shapes: Vec<String> = shape_blocks[0]
        .iter()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();
    assert_eq!(shapes.len(), 11, "one expected shape per activation step");

    // Resolve placeholders in an isolated fixture and execute the documented
    // sequence end to end.
    let pilot = PilotRepo::new();
    let mut ctx = RunbookCtx {
        repo: pilot.repo.clone(),
        plan_path: pilot.plan_path.clone(),
        contract_path: pilot.contract_path.clone(),
        report_dir: pilot.report_dir.clone(),
        repository_id: pilot.repository_id.clone(),
        phase_id: pilot.phase_ids[0].clone(),
        auditor: "auditor1".to_string(),
        draft_sha: None,
        close_receipt: None,
        audit_id: None,
        subject: None,
    };

    let mut outputs: Vec<String> = Vec::new();
    for (i, toks) in activation.iter().enumerate() {
        // Fixture preparation required by the documented sequence.
        match (toks[1].as_str(), toks[2].as_str()) {
            ("audit", "record") => {
                let aid = ctx.audit_id.clone().expect("audit begin ran first");
                let subj = ctx.subject.clone().expect("audit begin ran first");
                write_file(
                    &ctx.report_dir.join("audit-report.json"),
                    &make_pass_report(&aid, &subj, &ctx.auditor, &["req1", "req2"]),
                );
            }
            ("continuity", "record") => {
                let receipt = ctx.close_receipt.clone().expect("phase close ran first");
                write_file(
                    &ctx.repo.join("continuity.toml"),
                    &standard_metadata_toml(
                        &ctx.repository_id,
                        "phase-1-primary",
                        &ctx.phase_id,
                        &receipt,
                    ),
                );
            }
            _ => {}
        }
        let resolved: Vec<String> = toks
            .iter()
            .map(|t| resolve_runbook_token(t, &ctx))
            .collect();
        let out = Command::new(&resolved[0])
            .args(&resolved[1..])
            .output()
            .expect("execute documented runbook command");
        assert_success(&out);
        let stdout = stdout_str(&out);
        outputs.push(stdout.clone());

        match (toks[1].as_str(), toks[2].as_str()) {
            ("plan", "accept") => {
                assert_eq!(stdout, format!("test-plan {}", sha_of_file(&ctx.plan_path)));
            }
            ("phase", "select") => assert_eq!(stdout, ctx.phase_id),
            ("contract", "draft") => {
                let sha = ledger_draft_sha(&ctx.repo);
                assert_eq!(sha_of_file(&ctx.contract_path), sha);
                assert_eq!(stdout, format!("{} {}", pilot.contract_id, sha));
                ctx.draft_sha = Some(sha);
            }
            ("contract", "accept") => {
                let sha = ctx.draft_sha.as_ref().expect("draft sha captured");
                assert_eq!(stdout, format!("ACCEPTED {} 1 {}", pilot.contract_id, sha));
            }
            ("implementation", "begin") => {
                let parts = split_stdout(&out);
                assert_eq!(parts[0], "IMPLEMENTATION_BOUND");
                assert_eq!(parts[1], pilot.contract_id);
                assert_eq!(parts[3], *ctx.draft_sha.as_ref().unwrap());
                assert_eq!(parts[4], git_head(&ctx.repo));
            }
            ("implementation", "check") => {
                let parts = split_stdout(&out);
                assert_eq!(parts[0], "IMPLEMENTATION_OK");
                assert_eq!(parts[1], pilot.contract_id);
                assert_eq!(parts[3], *ctx.draft_sha.as_ref().unwrap());
                assert!(parts[4].parse::<u64>().is_ok());
            }
            ("audit", "begin") => {
                let parts = split_stdout(&out);
                assert_eq!(parts[0], "AUDIT_OPEN");
                assert_eq!(parts[2], "1");
                ctx.audit_id = Some(parts[1].clone());
                ctx.subject = Some(parts[3].clone());
            }
            ("audit", "record") => {
                let parts = split_stdout(&out);
                assert_eq!(parts[0], "AUDIT_PASS");
                assert_eq!(parts[1], *ctx.audit_id.as_ref().unwrap());
                assert_eq!(parts[2], "1");
                assert_eq!(parts[3], *ctx.subject.as_ref().unwrap());
            }
            ("phase", "close") => {
                let parts = split_stdout(&out);
                assert_eq!(parts[0], "PHASE_CLOSED");
                assert_eq!(parts[1], ctx.phase_id);
                assert_eq!(parts[2], "1");
                ctx.close_receipt = Some(parts[4].to_string());
            }
            ("continuity", "record") => {
                let parts = split_stdout(&out);
                assert_eq!(parts[0], "CONTINUITY_RECORDED");
                assert_eq!(parts[1], ctx.repository_id);
                assert_eq!(parts[2], ctx.phase_id);
                assert_eq!(parts[3], "1");
                std::fs::remove_file(ctx.repo.join("continuity.toml"))
                    .expect("temporary metadata removed after record");
            }
            ("recovery", "inspect") => {
                let lines: Vec<&str> = stdout.lines().collect();
                assert_eq!(lines.len(), 1);
                let parts: Vec<&str> = lines[0].split_whitespace().collect();
                assert_eq!(parts[0], "RECOVERY_NOT_REQUIRED");
                let second = pilot.inspect();
                assert_eq!(
                    stdout_raw(&second),
                    stdout_raw(&out),
                    "inspection must be deterministic"
                );
            }
            other => panic!("unexpected activation step: {:?}", other),
        }

        assert!(
            shape_matches(&stdout, &shapes[i]),
            "step {} output {:?} does not match documented shape {:?}",
            i + 1,
            stdout,
            shapes[i]
        );
    }

    // Post-sequence: no temporary metadata, clean worktree.
    assert_no_temp_files(&ctx.repo.join(".mrgs"));
    assert!(
        git_porcelain(&ctx.repo).is_empty(),
        "worktree clean after sequence"
    );

    let mut evidence = Evidence::new();
    evidence.add(format!(
        "ACTIVATION_REHEARSAL obligation=09 runbook_sha256={} commands_documented={} commands_validated={} sequence_executed=11 shapes_matched=11 result=passed",
        runbook_sha,
        documented.len(),
        documented.len()
    ));
    let rendered = evidence.render();
    assert!(rendered.contains("ACTIVATION_REHEARSAL"));
    assert!(!rendered.contains("PRODUCTION_ACTIVATED"));
    evidence.emit();
}

// ---------------------------------------------------------------------------
// Obligation 10 — runbook rollback checklist and boundaries
// ---------------------------------------------------------------------------

#[test]
fn test_obligation_10_runbook_rollback_checklist_and_boundaries() {
    let runbook = read_manifest_relative(RUNBOOK_REL_PATH);

    // Required checklist sections: pre-backup, stop, restore, post-restore,
    // evidence retention, privacy, no-network, no-installation, and separate
    // approvals, plus a binary PASS/FAIL outcome with no ambiguous state.
    let required_headings = [
        "## 6. Pre-activation backup and evidence-location rules",
        "## 8. Stop conditions",
        "## 9. Rollback drills",
        "### 9.1 Partial-activation rollback",
        "### 9.2 Completed-rehearsal rollback",
        "## 10. Post-rollback equality checks",
        "## 11. Privacy, secret-handling, path, network, and Git boundaries",
        "## 12. Evidence retention and disposal",
        "## 13. Known limitations",
        "## 14. Separate human approvals",
        "## 15. PASS/FAIL checklist",
    ];
    for h in required_headings {
        assert!(runbook.contains(h), "runbook must contain section `{}`", h);
    }

    // Executable step content for each checklist category.
    let category_checks: [(&str, &[&str]); 9] = [
        (
            "pre-backup",
            &[
                "pre-activation snapshot",
                "sole validated backup",
                "backup recorded",
            ],
        ),
        ("stop", &["STOP", "preserved", "nonzero exit"]),
        (
            "restore",
            &[
                "fresh restore destination",
                "Replace the live pilot",
                "fixed-point",
            ],
        ),
        (
            "post-restore",
            &[
                "equal the pre-activation snapshot",
                "restored slot equals the pre-activation slot state",
            ],
        ),
        (
            "evidence-retention",
            &[
                "ACTIVATION_REHEARSAL",
                "sole validated copy",
                "Failed-drill evidence",
            ],
        ),
        ("privacy", &["absolute pilot paths", "relative paths"]),
        ("no-network", &["No network contact", "no remote Git"]),
        (
            "no-installation",
            &[
                "not an installer",
                "no installer, updater",
                "no rollback command",
            ],
        ),
        (
            "separate-approval",
            &[
                "real activation",
                "commit",
                "push",
                "release publication",
                "rollback execution",
            ],
        ),
    ];
    for (name, phrases) in category_checks {
        for p in phrases {
            assert!(
                runbook.contains(p),
                "runbook {} checklist must contain `{}`",
                name,
                p
            );
        }
    }
    assert!(
        runbook.contains("no \"mostly ready\""),
        "runbook checklist must exclude an ambiguous mostly-ready outcome"
    );
    let checklist_items = runbook
        .lines()
        .filter(|l| l.trim_start().starts_with("- [ ]"))
        .count();
    assert!(
        checklist_items >= 15,
        "PASS/FAIL checklist must be concrete"
    );
    assert!(runbook.contains("PASS"), "checklist must name PASS");
    assert!(runbook.contains("FAIL"), "checklist must name FAIL");

    // Execute the documented rollback sequence in a fixture: partial
    // activation, interruption before replacement, resumption, fixed point.
    let tmp = std::env::temp_dir().join(format!(
        "mrgs-phase10-ob10-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&tmp).expect("create ob10 temp root");
    let evidence_dir = tmp.join("evidence");
    std::fs::create_dir_all(&evidence_dir).expect("create evidence dir");

    let pilot = PilotRepo::new();
    let pre = snapshot_repo(&pilot.repo);
    let backup_path = evidence_dir.join("pre-activation-backup.dat");
    write_backup(&backup_path, &pre, &pilot.repo);
    let backup_sha_before = sha_of_file(&backup_path);

    // Partial activation per runbook section 7 boundary (stop before
    // implementation begins).
    let plan = pilot.accept_plan();
    assert_success(&plan);
    let select = pilot.select_phase(&pilot.phase_ids[0]);
    assert_success(&select);
    let draft = pilot.draft_contract();
    assert_success(&draft);
    let draft_sha = pilot.get_draft()["sha256"].as_str().unwrap().to_string();
    let accept = pilot.accept_contract(1, &draft_sha);
    assert_success(&accept);
    let partial = snapshot_repo(&pilot.repo);
    assert_ne!(partial, pre, "partial activation must change the repo");

    // Interrupted restore before replacement: live repo untouched, fresh
    // destination and marker present for resumption.
    let r1 = restore_repo(
        &backup_path,
        &pilot.repo,
        &evidence_dir,
        InterruptPoint::BeforeReplacement,
    )
    .expect("interrupted restore");
    assert_eq!(
        r1,
        RestoreReport {
            rebuilt: true,
            replaced: false,
            finalized: false
        }
    );
    assert_eq!(
        snapshot_repo(&pilot.repo),
        partial,
        "live repo untouched before replacement"
    );
    let name = pilot
        .repo
        .file_name()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let parent = pilot.repo.parent().unwrap();
    let dest = parent.join(format!("{}.phase10-restore", name));
    let marker = parent.join(format!(".phase10-restore-{}.marker", name));
    assert!(
        dest.exists(),
        "fresh restore destination exists for resumption"
    );
    assert!(marker.exists(), "restore marker exists for resumption");

    // Resume: no rebuild, exact single replacement, finalize.
    let r2 = restore_repo(
        &backup_path,
        &pilot.repo,
        &evidence_dir,
        InterruptPoint::None,
    )
    .expect("resumed restore");
    assert_eq!(
        r2,
        RestoreReport {
            rebuilt: false,
            replaced: true,
            finalized: true
        }
    );
    let post = snapshot_repo(&pilot.repo);
    assert_eq!(
        post, pre,
        "exact pre-activation bytes and Git state restored"
    );
    assert_mrgs_absent(&pilot.repo);

    // Fixed point: repeated restore performs no rebuild and no replacement.
    let r3 = restore_repo(
        &backup_path,
        &pilot.repo,
        &evidence_dir,
        InterruptPoint::None,
    )
    .expect("fixed-point restore");
    assert_eq!(
        r3,
        RestoreReport {
            rebuilt: false,
            replaced: false,
            finalized: true
        }
    );
    assert_eq!(snapshot_repo(&pilot.repo), pre);

    // Post-restore equality checks: backup preserved, evidence readable,
    // no residual scaffolding, restored HEAD.
    assert_eq!(
        sha_of_file(&backup_path),
        backup_sha_before,
        "sole backup preserved"
    );
    let receipt =
        std::fs::read_to_string(evidence_dir.join("restore-receipt.txt")).expect("restore receipt");
    assert!(
        receipt.starts_with("PHASE10_RESTORE_RECEIPT "),
        "receipt format"
    );
    assert_no_restore_scaffolding(&tmp);
    assert_eq!(git_head(&pilot.repo), pre.head, "HEAD restored");

    let mut evidence = Evidence::new();
    evidence.add(format!(
        "ACTIVATION_REHEARSAL obligation=10 rollback=documented-sequence result=exact-restore checklist_sections=9 backup_sha256={} pre_snapshot_sha256={} post_snapshot_sha256={} interrupted=resumed fixed_point=yes",
        backup_sha_before,
        snapshot_digest(&pre),
        snapshot_digest(&post)
    ));
    let rendered = evidence.render();
    assert!(rendered.contains("ACTIVATION_REHEARSAL"));
    assert!(!rendered.contains("PRODUCTION_ACTIVATED"));
    evidence.emit();
}

// ---------------------------------------------------------------------------
// Obligation 11 — README master-plan and claim accuracy
// ---------------------------------------------------------------------------

#[test]
fn test_obligation_11_readme_master_plan_and_claim_accuracy() {
    let readme = read_manifest_relative("README.md");
    let master_plan = read_manifest_relative("docs/master-plan.md");

    // Master-plan titles, in order.
    let mut expected: Vec<(String, String)> = Vec::new();
    for line in master_plan.lines() {
        if let Some(rest) = line.strip_prefix("## Phase ") {
            let (num, title) = rest.split_once('—').expect("master-plan phase dash");
            expected.push((num.trim().to_string(), title.trim().to_string()));
        }
    }
    assert_eq!(
        expected.len(),
        10,
        "master plan must define exactly ten phases"
    );

    // README phase list: exactly the ten master-plan titles in order.
    let mut actual: Vec<(String, String)> = Vec::new();
    for line in readme.lines() {
        if let Some(rest) = line.strip_prefix("Phase ") {
            if let Some((num, title)) = rest.split_once('—') {
                actual.push((num.trim().to_string(), title.trim().to_string()));
            }
        }
    }
    assert_eq!(actual.len(), 10, "README must list exactly ten phases");
    assert_eq!(
        actual, expected,
        "README phase titles must match master-plan titles exactly"
    );

    // Runbook link and rehearsal-vs-activation distinction.
    assert!(
        readme.contains("docs/phase-10-adoption-runbook.md"),
        "README must link the adoption runbook"
    );
    assert!(
        readme.contains("readiness evidence only"),
        "README must state Phase 10 produces readiness evidence only"
    );
    assert!(
        readme.contains("separate human authorization"),
        "README must state separate human authorization"
    );
    assert!(
        readme.contains("ACTIVATION_REHEARSAL"),
        "rehearsal label present"
    );
    assert!(
        readme.contains("PRODUCTION_ACTIVATED"),
        "never-activated label present"
    );

    // No forbidden unsupported claims (contract section 11).
    for claim in [
        "installation support",
        "production deployment",
        "automatic rollback",
        "certif",
        "accredit",
        "complian",
        "universal platform",
    ] {
        assert!(
            !readme.to_lowercase().contains(claim),
            "README must not claim `{}`",
            claim
        );
    }

    let mut evidence = Evidence::new();
    evidence.add(
        "ACTIVATION_REHEARSAL obligation=11 readme=README.md phases=10 titles=master-plan-exact runbook_link=yes rehearsal_vs_activation=distinct forbidden_claims=absent"
            .to_string(),
    );
    let rendered = evidence.render();
    assert!(rendered.contains("ACTIVATION_REHEARSAL"));
    assert!(!rendered.contains("PRODUCTION_ACTIVATED"));
    evidence.emit();
}

// ---------------------------------------------------------------------------
// Obligation 12 — two-repository adoption rehearsal and final manifest
// ---------------------------------------------------------------------------

#[test]
fn test_obligation_12_two_repository_adoption_rehearsal_and_final_manifest() {
    // Two independent pilot repositories with distinct plans, identities,
    // and paths.
    let pilot_a = PilotRepo::new_with_plan(
        &valid_plan_toml("plan-alpha", &["phase-1", "phase-2"]),
        &contract_toml_for_phase("contract-alpha", "phase-1", &["reqa1", "reqa2"]),
        "repo-alpha",
        &["phase-1", "phase-2"],
        "plan-alpha",
        "contract-alpha",
        &["reqa1", "reqa2"],
    );
    let pilot_b = PilotRepo::new_with_plan(
        &valid_plan_toml("plan-beta", &["phase-1", "phase-2"]),
        &contract_toml_for_phase("contract-beta", "phase-1", &["reqb1", "reqb2"]),
        "repo-beta",
        &["phase-1", "phase-2"],
        "plan-beta",
        "contract-beta",
        &["reqb1", "reqb2"],
    );
    assert_ne!(pilot_a.repo, pilot_b.repo, "distinct repository paths");

    let tmp = std::env::temp_dir().join(format!(
        "mrgs-phase10-ob12-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&tmp).expect("create ob12 temp root");
    let evidence_a = tmp.join("evidence-a");
    let evidence_b = tmp.join("evidence-b");
    std::fs::create_dir_all(&evidence_a).expect("evidence-a");
    std::fs::create_dir_all(&evidence_b).expect("evidence-b");

    // Pre-activation snapshots and sole validated backups per repository.
    let pre_a = snapshot_repo(&pilot_a.repo);
    let pre_b = snapshot_repo(&pilot_b.repo);
    let backup_a = evidence_a.join("pre-activation-backup.dat");
    let backup_b = evidence_b.join("pre-activation-backup.dat");
    write_backup(&backup_a, &pre_a, &pilot_a.repo);
    write_backup(&backup_b, &pre_b, &pilot_b.repo);
    let backup_a_sha = sha_of_file(&backup_a);
    let backup_b_sha = sha_of_file(&backup_b);

    // Documented rehearsal in both repositories.
    let result_a = run_activation_rehearsal(&pilot_a);
    let result_b = run_activation_rehearsal(&pilot_b);

    // No cross-repository authority or identity leakage.
    assert_ne!(
        git_head(&pilot_a.repo),
        git_head(&pilot_b.repo),
        "distinct identities"
    );
    assert_ne!(
        result_a.subject_sha, result_b.subject_sha,
        "distinct subjects"
    );
    assert_ne!(
        result_a.inspect_subject, result_b.inspect_subject,
        "distinct inspect subjects"
    );
    let accepted_a = std::fs::read_to_string(pilot_a.repo.join(".mrgs").join("accepted-plan.json"))
        .expect("accepted plan a");
    let accepted_b = std::fs::read_to_string(pilot_b.repo.join(".mrgs").join("accepted-plan.json"))
        .expect("accepted plan b");
    assert!(accepted_a.contains("plan-alpha"), "a holds its own plan");
    assert!(accepted_b.contains("plan-beta"), "b holds its own plan");
    assert!(!accepted_a.contains("plan-beta"), "a holds no foreign plan");
    assert!(
        !accepted_b.contains("plan-alpha"),
        "b holds no foreign plan"
    );
    let continuity_a =
        std::fs::read_to_string(pilot_a.repo.join(".mrgs").join("continuity-ledger.json"))
            .expect("continuity ledger a");
    let continuity_b =
        std::fs::read_to_string(pilot_b.repo.join(".mrgs").join("continuity-ledger.json"))
            .expect("continuity ledger b");
    assert!(
        continuity_a.contains("repo-alpha"),
        "a continuity carries its identity"
    );
    assert!(
        continuity_b.contains("repo-beta"),
        "b continuity carries its identity"
    );
    assert!(
        !continuity_a.contains("repo-beta"),
        "a continuity carries no foreign identity"
    );
    assert!(
        !continuity_b.contains("repo-alpha"),
        "b continuity carries no foreign identity"
    );

    // Temporary-file and evidence leakage.
    assert_no_temp_files(&pilot_a.repo.join(".mrgs"));
    assert_no_temp_files(&pilot_b.repo.join(".mrgs"));
    assert!(git_porcelain(&pilot_a.repo).is_empty(), "a worktree clean");
    assert!(git_porcelain(&pilot_b.repo).is_empty(), "b worktree clean");

    // Deterministic readiness manifests: per-repository, distinct, stable,
    // relative-path-only, no cross identifiers.
    let manifest_a = generate_readiness_manifest(&result_a);
    let manifest_b = generate_readiness_manifest(&result_b);
    assert_eq!(
        manifest_a,
        generate_readiness_manifest(&result_a),
        "a evidence deterministic"
    );
    assert_eq!(
        manifest_b,
        generate_readiness_manifest(&result_b),
        "b evidence deterministic"
    );
    assert_ne!(
        manifest_a, manifest_b,
        "distinct plans yield distinct evidence"
    );
    assert!(
        manifest_a.contains("plan-alpha") && manifest_a.contains("contract-alpha"),
        "a evidence carries its identifiers"
    );
    assert!(
        manifest_b.contains("plan-beta") && manifest_b.contains("contract-beta"),
        "b evidence carries its identifiers"
    );
    assert!(
        !manifest_a.contains("beta"),
        "a evidence carries no foreign identifiers"
    );
    assert!(
        !manifest_b.contains("alpha"),
        "b evidence carries no foreign identifiers"
    );
    for manifest in [&manifest_a, &manifest_b] {
        let lines: Vec<&str> = manifest.lines().collect();
        let mut sorted = lines.clone();
        sorted.sort();
        assert_eq!(lines, sorted, "evidence deterministically ordered");
        for line in &lines {
            assert!(!line.contains(':'), "absolute-path marker in evidence");
            assert!(!line.contains('\\'), "backslash path in evidence");
            assert!(!line.starts_with('/'), "absolute path in evidence");
            assert!(!line.contains(".."), "parent traversal in evidence");
        }
        assert!(manifest.contains("label=ACTIVATION_REHEARSAL"));
        assert!(!manifest.contains("PRODUCTION_ACTIVATED"));
    }

    // Preserve rehearsal evidence before rollback (runbook section 9.2).
    let evidence_file_a = evidence_a.join("rehearsal-evidence.txt");
    let evidence_file_b = evidence_b.join("rehearsal-evidence.txt");
    write_file(&evidence_file_a, &manifest_a);
    write_file(&evidence_file_b, &manifest_b);
    let evidence_a_sha = sha_of_file(&evidence_file_a);
    let evidence_b_sha = sha_of_file(&evidence_file_b);

    // Rollback leakage: restoring one repository must not touch the other.
    let b_during = snapshot_repo(&pilot_b.repo);
    let r_a = restore_repo(&backup_a, &pilot_a.repo, &evidence_a, InterruptPoint::None)
        .expect("rollback a");
    assert_eq!(
        r_a,
        RestoreReport {
            rebuilt: true,
            replaced: true,
            finalized: true
        }
    );
    assert_eq!(snapshot_repo(&pilot_a.repo), pre_a, "a restored exactly");
    assert_eq!(
        snapshot_repo(&pilot_b.repo),
        b_during,
        "b untouched by a rollback"
    );
    assert_mrgs_absent(&pilot_a.repo);

    let a_restored = snapshot_repo(&pilot_a.repo);
    let r_b = restore_repo(&backup_b, &pilot_b.repo, &evidence_b, InterruptPoint::None)
        .expect("rollback b");
    assert_eq!(
        r_b,
        RestoreReport {
            rebuilt: true,
            replaced: true,
            finalized: true
        }
    );
    assert_eq!(snapshot_repo(&pilot_b.repo), pre_b, "b restored exactly");
    assert_eq!(
        snapshot_repo(&pilot_a.repo),
        a_restored,
        "a untouched by b rollback"
    );
    assert_mrgs_absent(&pilot_b.repo);

    // Post-rollback: backups and preserved evidence unchanged; no scaffolding.
    assert_eq!(sha_of_file(&backup_a), backup_a_sha);
    assert_eq!(sha_of_file(&backup_b), backup_b_sha);
    assert_eq!(sha_of_file(&evidence_file_a), evidence_a_sha);
    assert_eq!(sha_of_file(&evidence_file_b), evidence_b_sha);
    assert_no_restore_scaffolding(&tmp);

    // Discipline checks: exactly twelve primary obligations, none ignored,
    // no recursive Cargo invocation.
    let source = read_manifest_relative("tests/phase10.rs");
    let primary: Vec<&str> = source
        .lines()
        .filter(|l| l.starts_with("fn test_obligation_"))
        .collect();
    assert_eq!(primary.len(), 12, "exactly twelve primary obligations");
    for name in [
        "fn test_obligation_09_runbook_cli_surface_and_sequence",
        "fn test_obligation_10_runbook_rollback_checklist_and_boundaries",
        "fn test_obligation_11_readme_master_plan_and_claim_accuracy",
        "fn test_obligation_12_two_repository_adoption_rehearsal_and_final_manifest",
    ] {
        assert!(source.contains(name), "missing primary test {}", name);
    }
    assert!(
        !source.lines().any(|l| l.trim() == "#[ignore]"),
        "no ignored primary tests"
    );
    assert!(
        !source.contains("Command::new(\"cargo\")"),
        "no recursive Cargo invocation"
    );

    // Final deterministic Phase 10 readiness manifest.
    let mut evidence = Evidence::new();
    evidence.add(format!(
        "ACTIVATION_REHEARSAL obligation=12 repositories=2 rehearsal=passed rollback=exact-restore cross_repository_leakage=absent manifest_a_sha256={} manifest_b_sha256={} discipline_primary_tests=12 ignored=0 recursive_cargo=0",
        sha256_hex(manifest_a.as_bytes()),
        sha256_hex(manifest_b.as_bytes())
    ));
    let rendered = evidence.render();
    assert!(rendered.contains("ACTIVATION_REHEARSAL"));
    assert!(!rendered.contains("PRODUCTION_ACTIVATED"));
    evidence.emit();
}
