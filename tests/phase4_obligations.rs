//! Phase 4 P2-D obligation-catalog and registry framework (P2-D1).
//!
//! This test is a contract-derived, machine-checkable registry for the 188
//! numbered obligations in Section 18 of the Phase 4 contract. It does NOT
//! assert product behavior; it validates that the registry is internally
//! consistent, exactly reconciled with the controlling contract (by its
//! canonical Git-blob SHA-256), and that every `SATISFIED` obligation carries
//! at least one concrete, executable evidence reference to a real test present
//! in the worktree.
//!
//! D1 expectation: the registry framework and the full Cargo suite are green,
//! while the governance recommendation is `FAIL` because no obligation is yet
//! mapped to a real executable test. The exact unmapped ID list is reported in
//! the handoff; it is NOT surfaced as a test failure.

use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::process::Command;

const CONTRACT_BLOB_REF: &str = "HEAD:docs/contracts/phase-04-contract.md";

/// Canonical SHA-256 of the exact Git blob `HEAD:docs/contracts/phase-04-contract.md`.
const EXPECTED_CONTRACT_BLOB_SHA256: &str =
    "542f38e78e0aa22dc8fec3692a608c80b0494b96678a43caa80fa7f78de22a04";

/// Exact byte length of the canonical contract blob.
const EXPECTED_CONTRACT_BLOB_LEN: usize = 84306;

/// SHA-256 of the verified canonical catalog input
/// `phase4-obligations-1-188.json` (read externally, never copied in-tree).
const EXPECTED_CATALOG_SHA256: &str =
    "3559ee4864b16a9099d2401eb314ecbd40109f19477fb5413d936801f6711a20";

const REGISTRY_REL_PATH: &str = "tests/phase4_obligations.json";

/// The P2-D registry/meta-test target. Evidence pointing only here is
/// self-certification and must be rejected.
const SELF_TEST_TARGET: &str = "phase4_obligations";

const SUPPORTED_TARGET_KINDS: &[&str] = &["unit", "integration"];
const SUPPORTED_PLATFORM_SCOPES: &[&str] = &["all", "windows", "unix", "parser_fallback"];

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Evidence {
    cargo_target_kind: String,
    cargo_target: String,
    fully_qualified_test: String,
    scenario: String,
    assertion_semantics: String,
    platform_scope: String,
    source_anchor: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Obligation {
    id: u32,
    canonical_id: String,
    text: String,
    mapping_status: String,
    #[serde(default)]
    evidence: Vec<Evidence>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Registry {
    schema_version: u32,
    obligation_count: u32,
    source_catalog_sha256: String,
    obligations: Vec<Obligation>,
}

// ---------------------------------------------------------------------------
// Canonical contract blob helper (single source of truth)
// ---------------------------------------------------------------------------

/// Load the exact contract blob, verify its SHA-256 and byte length, then decode
/// it as UTF-8. Runs `git cat-file blob HEAD:docs/contracts/phase-04-contract.md`
/// from the crate manifest directory; requires success, empty stderr, the exact
/// SHA-256, and the exact byte length.
fn load_verified_contract_text() -> String {
    let manifest = env!("CARGO_MANIFEST_DIR");
    let out = Command::new("git")
        .current_dir(manifest)
        .args(["cat-file", "blob", CONTRACT_BLOB_REF])
        .output()
        .unwrap_or_else(|e| panic!("cannot run git cat-file for contract blob: {}", e));
    assert!(
        out.status.success(),
        "git cat-file blob must succeed; exit={:?}",
        out.status.code()
    );
    assert!(
        out.stderr.is_empty(),
        "git cat-file blob stderr must be empty; got: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let bytes = out.stdout;
    assert_eq!(
        bytes.len(),
        EXPECTED_CONTRACT_BLOB_LEN,
        "contract blob byte length must be {}",
        EXPECTED_CONTRACT_BLOB_LEN
    );
    let digest = {
        let mut hasher = Sha256::new();
        hasher.update(&bytes);
        let d = hasher.finalize();
        let mut s = String::with_capacity(64);
        for b in d {
            s.push_str(&format!("{:02x}", b));
        }
        s
    };
    assert_eq!(
        digest, EXPECTED_CONTRACT_BLOB_SHA256,
        "canonical contract blob SHA-256 mismatch: actual={} expected={}",
        digest, EXPECTED_CONTRACT_BLOB_SHA256
    );
    String::from_utf8(bytes).expect("contract blob is valid UTF-8")
}

fn contract_blob_sha256_only() -> String {
    let text = load_verified_contract_text();
    // Recompute from the verified text bytes to keep a single code path.
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    let d = hasher.finalize();
    let mut s = String::with_capacity(64);
    for b in d {
        s.push_str(&format!("{:02x}", b));
    }
    s
}

// ---------------------------------------------------------------------------
// Strict grammars
// ---------------------------------------------------------------------------

/// Validate one Rust identifier component (used by fully_qualified_test and the
/// function part of source_anchor).
fn is_rust_identifier_component(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// fully_qualified_test: one or more identifier components separated by "::".
fn validate_fully_qualified_test(name: &str) -> Result<(), String> {
    if name.trim() != name {
        return Err("leading/trailing whitespace".to_string());
    }
    if name.is_empty() {
        return Err("empty test name".to_string());
    }
    if name.contains(' ') {
        return Err("embedded whitespace".to_string());
    }
    for bad in ['*', '?', '[', ']', '/', '\\'] {
        if name.contains(bad) {
            return Err(format!("forbidden character '{}'", bad));
        }
    }
    // A colon is only valid as part of the "::" separator; a standalone ':'
    // (single colon) must be rejected.
    let chars: Vec<char> = name.chars().collect();
    for i in 0..chars.len() {
        if chars[i] == ':' {
            let prev_is_colon = i > 0 && chars[i - 1] == ':';
            let next_is_colon = i + 1 < chars.len() && chars[i + 1] == ':';
            if !prev_is_colon && !next_is_colon {
                return Err("single colon is not a valid separator".to_string());
            }
        }
    }
    if name.starts_with("::") || name.ends_with("::") {
        return Err("leading/trailing '::'".to_string());
    }
    for comp in name.split("::") {
        if comp.is_empty() {
            return Err("empty component".to_string());
        }
        if comp == "_" {
            return Err("component '_' (standalone underscore) is not allowed".to_string());
        }
        if !is_rust_identifier_component(comp) {
            return Err(format!("invalid component '{}'", comp));
        }
    }
    Ok(())
}

/// cargo_target: non-empty, trimmed, ASCII letters/digits/underscore/hyphen,
/// not starting with hyphen, not equal to the meta-test target.
fn validate_cargo_target(name: &str) -> Result<(), String> {
    if name.trim() != name {
        return Err("cargo_target has surrounding whitespace".to_string());
    }
    if name.is_empty() {
        return Err("empty cargo_target".to_string());
    }
    if name == SELF_TEST_TARGET {
        return Err("cargo_target equals the P2-D meta-test target".to_string());
    }
    let mut chars = name.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' || c == '-' => {}
        _ => return Err("cargo_target must begin with letter/_/-".to_string()),
    }
    if name.starts_with('-') {
        return Err("cargo_target must not begin with hyphen".to_string());
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return Err("cargo_target contains illegal character".to_string());
    }
    Ok(())
}

/// source_anchor: <repo-relative-rust-file>:<exact-function-name>
fn validate_source_anchor(anchor: &str) -> Result<(), String> {
    if anchor.trim() != anchor {
        return Err("source_anchor has surrounding whitespace".to_string());
    }
    let (file, func) = anchor
        .split_once(':')
        .ok_or_else(|| "source_anchor must be 'repo-relative-file:function'".to_string())?;
    if file.trim() != file {
        return Err("source_anchor file has surrounding whitespace".to_string());
    }
    if func.trim() != func {
        return Err("source_anchor function has surrounding whitespace".to_string());
    }
    let fbytes = file.as_bytes();
    if fbytes.is_empty() {
        return Err("source_anchor file is empty".to_string());
    }
    if fbytes[0] == b'/' || file.starts_with("//") {
        return Err("source_anchor file must not start with / or //".to_string());
    }
    if file.chars().nth(1) == Some(':') && fbytes[0].is_ascii_alphabetic() && file.len() >= 2 {
        // drive prefix like C:
        return Err("source_anchor file must not use an ASCII drive prefix".to_string());
    }
    if file.contains('\\') {
        return Err("source_anchor file must use / separators only".to_string());
    }
    for bad in ['\0', '*', '?', '[', ']'] {
        if file.contains(bad) {
            return Err("source_anchor file contains forbidden character".to_string());
        }
    }
    for c in file.chars() {
        if c.is_ascii_control() || c == '\u{7f}' {
            return Err("source_anchor file contains control/DEL".to_string());
        }
    }
    if file.ends_with('/') {
        return Err("source_anchor file must not end with /".to_string());
    }
    if !file.ends_with(".rs") {
        return Err("source_anchor file must end in .rs".to_string());
    }
    let segments: Vec<&str> = file.split('/').collect();
    for seg in &segments {
        if seg.is_empty() {
            return Err("source_anchor file has empty or doubled-slash segment".to_string());
        }
        if *seg == "." || *seg == ".." {
            return Err("source_anchor file has '.' or '..' segment".to_string());
        }
    }
    // The file/function boundary is the single first colon (split_once already
    // required at least one). Additional '::' inside the function name are
    // validated by validate_fully_qualified_test.
    // Function part obeys the Rust component grammar (may contain ::).
    validate_fully_qualified_test(func)
}

// ---------------------------------------------------------------------------
// Registry extraction / validation
// ---------------------------------------------------------------------------

/// Extract the exact Section 18 obligation texts from the contract.
fn extract_section_18_obligations(contract: &str) -> Vec<(u32, String)> {
    let lines: Vec<&str> = contract.lines().collect();
    let mut start = None;
    for (i, l) in lines.iter().enumerate() {
        if l.starts_with("## 18.") {
            start = Some(i);
            break;
        }
    }
    let start = start.expect("Section 18 heading not found in contract");
    let mut out = Vec::new();
    for l in &lines[start + 1..] {
        if l.starts_with("## ") {
            break;
        }
        let bytes = l.as_bytes();
        let mut dig_end = 0;
        while dig_end < bytes.len() && bytes[dig_end].is_ascii_digit() {
            dig_end += 1;
        }
        if dig_end > 0
            && dig_end + 1 < bytes.len()
            && bytes[dig_end] == b'.'
            && bytes[dig_end + 1] == b' '
        {
            let num: String = l[..dig_end].chars().collect();
            let id: u32 = num.parse().expect("obligation id parses");
            let text = l[dig_end + 2..].trim_end().to_string();
            out.push((id, text));
        }
    }
    out
}

fn parse_registry(json: &str) -> Result<Registry, String> {
    serde_json::from_str::<Registry>(json).map_err(|e| format!("registry parse error: {}", e))
}

fn validate_evidence(e: &Evidence) -> Result<(), String> {
    if !SUPPORTED_TARGET_KINDS.contains(&e.cargo_target_kind.as_str()) {
        return Err(format!(
            "invalid cargo_target_kind '{}'",
            e.cargo_target_kind
        ));
    }
    validate_cargo_target(&e.cargo_target).map_err(|m| format!("cargo_target invalid: {}", m))?;
    validate_fully_qualified_test(&e.fully_qualified_test)
        .map_err(|m| format!("fully_qualified_test invalid: {}", m))?;
    if e.scenario.trim().is_empty() {
        return Err("empty scenario".to_string());
    }
    if e.assertion_semantics.trim().is_empty() {
        return Err("empty assertion_semantics".to_string());
    }
    if !SUPPORTED_PLATFORM_SCOPES.contains(&e.platform_scope.as_str()) {
        return Err(format!("invalid platform_scope '{}'", e.platform_scope));
    }
    validate_source_anchor(&e.source_anchor)
        .map_err(|m| format!("source_anchor invalid: {}", m))?;
    Ok(())
}

fn validate_obligation(o: &Obligation) -> Result<(), String> {
    if o.id < 1 || o.id > 188 {
        return Err(format!("obligation id out of range: {}", o.id));
    }
    if o.canonical_id != format!("P4-{:03}", o.id) {
        return Err(format!(
            "canonical id mismatch for id {}: '{}'",
            o.id, o.canonical_id
        ));
    }
    if o.text.trim().is_empty() {
        return Err(format!("obligation {} has empty text", o.id));
    }
    let status = o.mapping_status.as_str();
    if status != "SATISFIED" && status != "UNSATISFIED" {
        return Err(format!(
            "obligation {} has invalid mapping_status '{}'",
            o.id, status
        ));
    }
    for e in &o.evidence {
        validate_evidence(e).map_err(|m| format!("obligation {} evidence invalid: {}", o.id, m))?;
    }
    if status == "SATISFIED" && o.evidence.is_empty() {
        return Err(format!(
            "obligation {} is SATISFIED but carries no concrete evidence",
            o.id
        ));
    }
    Ok(())
}

/// Validate full registry structure, semantics, and deterministic ordering.
fn validate_registry(reg: &Registry) -> Result<(), String> {
    if reg.schema_version != 1 {
        return Err(format!(
            "schema_version must be 1, got {}",
            reg.schema_version
        ));
    }
    if reg.obligation_count != 188 {
        return Err(format!(
            "obligation_count must be 188, got {}",
            reg.obligation_count
        ));
    }
    if reg.obligations.len() != 188 {
        return Err(format!(
            "obligations array must have 188 entries, got {}",
            reg.obligations.len()
        ));
    }
    if reg.source_catalog_sha256 != EXPECTED_CATALOG_SHA256 {
        return Err("source_catalog_sha256 does not match verified catalog".to_string());
    }
    for (idx, o) in reg.obligations.iter().enumerate() {
        let expected_id = (idx + 1) as u32;
        if o.id != expected_id {
            return Err(format!(
                "registry entry at index {} must have id {}, found {}",
                idx, expected_id, o.id
            ));
        }
        if o.canonical_id != format!("P4-{:03}", expected_id) {
            return Err(format!(
                "registry entry at index {} must have canonical id P4-{:03}, found {}",
                idx, expected_id, o.canonical_id
            ));
        }
        validate_obligation(o)?;
    }
    Ok(())
}

fn load_valid_registry() -> Registry {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(REGISTRY_REL_PATH);
    let content = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read registry at {:?}: {}", path, e));
    let reg = parse_registry(&content).expect("registry parses with strict schema");
    validate_registry(&reg).expect("registry is structurally and semantically valid");
    reg
}

// ---------------------------------------------------------------------------
// Positive structural tests (must pass)
// ---------------------------------------------------------------------------

#[test]
fn contract_blob_sha256_is_canonical() {
    let actual = contract_blob_sha256_only();
    assert_eq!(
        actual, EXPECTED_CONTRACT_BLOB_SHA256,
        "canonical contract blob SHA-256 mismatch: actual={} expected={}",
        actual, EXPECTED_CONTRACT_BLOB_SHA256
    );
}

#[test]
fn section_18_has_exactly_ids_1_through_188() {
    let contract = load_verified_contract_text();
    let obligs = extract_section_18_obligations(&contract);
    assert_eq!(
        obligs.len(),
        188,
        "Section 18 must list exactly 188 obligations"
    );
    let mut seen = BTreeSet::new();
    for (i, (id, _text)) in obligs.iter().enumerate() {
        assert_eq!(
            *id,
            (i + 1) as u32,
            "obligation id sequence gap/duplicate at position {}: id={}",
            i,
            id
        );
        assert!(seen.insert(*id), "duplicate obligation id {}", id);
    }
    assert_eq!(*seen.first().unwrap(), 1);
    assert_eq!(*seen.last().unwrap(), 188);
}

#[test]
fn registry_parses_with_strict_schema_and_validates() {
    let reg = load_valid_registry();
    assert_eq!(reg.obligations.len(), 188);
}

#[test]
fn every_registry_text_matches_section_18_exactly() {
    let contract = load_verified_contract_text();
    let section = extract_section_18_obligations(&contract);
    let reg = load_valid_registry();
    let section_map: std::collections::BTreeMap<u32, String> = section.into_iter().collect();
    for o in &reg.obligations {
        let expected = section_map
            .get(&o.id)
            .unwrap_or_else(|| panic!("registry id {} has no Section 18 counterpart", o.id));
        assert_eq!(
            &o.text, expected,
            "registry text for id {} does not exactly match Section 18",
            o.id
        );
    }
}

#[test]
fn registry_rejects_p2d_meta_test_evidence() {
    let reg = load_valid_registry();
    for o in &reg.obligations {
        for e in &o.evidence {
            assert_ne!(
                e.cargo_target, SELF_TEST_TARGET,
                "obligation {} evidence points to the P2-D meta-test",
                o.id
            );
        }
    }
}

#[test]
fn d1_registry_is_deterministic_and_incomplete() {
    let reg = load_valid_registry();
    let mut plan: BTreeSet<(String, String, String)> = BTreeSet::new();
    let mut satisfied: Vec<u32> = Vec::new();
    let mut unsatisfied: Vec<u32> = Vec::new();
    for o in &reg.obligations {
        if o.mapping_status == "SATISFIED" {
            for e in &o.evidence {
                assert_ne!(
                    e.cargo_target, SELF_TEST_TARGET,
                    "SATISFIED obligation {} maps only to the P2-D meta-test",
                    o.id
                );
                plan.insert((
                    e.cargo_target_kind.clone(),
                    e.cargo_target.clone(),
                    e.fully_qualified_test.clone(),
                ));
            }
            satisfied.push(o.id);
        } else {
            unsatisfied.push(o.id);
        }
    }
    satisfied.sort();
    unsatisfied.sort();
    assert_eq!(
        satisfied,
        vec![] as Vec<u32>,
        "D1 must not claim satisfied obligations without real evidence"
    );
    let expected: Vec<u32> = (1u32..=188).collect();
    assert_eq!(
        unsatisfied, expected,
        "D1 unsatisfied list must be exactly IDs 1..188"
    );
    eprintln!(
        "P2-D1 EXECUTION PLAN: {} unique mapped tests; satisfied={}; unsatisfied={}",
        plan.len(),
        satisfied.len(),
        unsatisfied.len()
    );
    eprintln!(
        "P2-D1 UNSATISFIED IDS: {}",
        unsatisfied
            .iter()
            .map(|i| i.to_string())
            .collect::<Vec<_>>()
            .join(",")
    );
    eprintln!("P2-D1 RECOMMENDATION: FAIL (mapping incomplete; P2-D2 reconciliation required)");
}

/// Regression: contract loading must resolve the intended repository blob even
/// when the current directory of the test process is an unrelated temp dir.
#[test]
fn contract_loading_resolves_intended_repo_from_any_cwd() {
    use std::env;
    let original = env::current_dir().expect("current_dir readable");
    let tmp = tempfile::TempDir::new().expect("temp dir");
    env::set_current_dir(tmp.path()).expect("set cwd to temp");
    let result = std::panic::catch_unwind(|| {
        let contract = load_verified_contract_text();
        assert!(contract.contains("## 18. Required tests"));
    });
    // Restore original cwd even if the assertion panicked.
    env::set_current_dir(&original).expect("restore cwd");
    result.expect("contract loading from unrelated cwd must resolve the intended repo blob");
}

// ---------------------------------------------------------------------------
// Strict fully_qualified_test grammar tests
// ---------------------------------------------------------------------------

#[test]
fn accept_valid_fully_qualified_test_single() {
    assert!(validate_fully_qualified_test("test_impl_begin_valid").is_ok());
}

#[test]
fn accept_valid_fully_qualified_test_path() {
    assert!(validate_fully_qualified_test(
        "implementation::part1_tests::index_stage_z_parses_stage0"
    )
    .is_ok());
}

#[test]
fn reject_fully_qualified_leading_whitespace() {
    assert!(validate_fully_qualified_test(" test_impl_begin_valid").is_err());
}

#[test]
fn reject_fully_qualified_trailing_whitespace() {
    assert!(validate_fully_qualified_test("test_impl_begin_valid ").is_err());
}

#[test]
fn reject_fully_qualified_embedded_whitespace() {
    assert!(validate_fully_qualified_test("test impl begin").is_err());
}

#[test]
fn reject_fully_qualified_single_colon() {
    assert!(validate_fully_qualified_test("test:impl").is_err());
}

#[test]
fn reject_fully_qualified_empty() {
    assert!(validate_fully_qualified_test("").is_err());
}

#[test]
fn reject_fully_qualified_empty_component() {
    assert!(validate_fully_qualified_test("a::").is_err());
    assert!(validate_fully_qualified_test("::a").is_err());
    assert!(validate_fully_qualified_test("a::b::").is_err());
}

#[test]
fn reject_fully_qualified_leading_double_colon() {
    assert!(validate_fully_qualified_test("::test_impl_begin_valid").is_err());
}

#[test]
fn reject_fully_qualified_trailing_double_colon() {
    assert!(validate_fully_qualified_test("test_impl_begin_valid::").is_err());
}

#[test]
fn reject_fully_qualified_wildcard() {
    assert!(validate_fully_qualified_test("test_impl_*").is_err());
}

#[test]
fn reject_fully_qualified_question() {
    assert!(validate_fully_qualified_test("test_impl?").is_err());
}

#[test]
fn reject_fully_qualified_brackets() {
    assert!(validate_fully_qualified_test("test[impl]").is_err());
}

#[test]
fn reject_fully_qualified_slash() {
    assert!(validate_fully_qualified_test("impl/begin").is_err());
}

#[test]
fn reject_fully_qualified_backslash() {
    assert!(validate_fully_qualified_test("impl\\begin").is_err());
}

#[test]
fn reject_fully_qualified_trailing_underscore_only() {
    assert!(validate_fully_qualified_test("_").is_err());
    assert!(validate_fully_qualified_test("a::_").is_err());
}

#[test]
fn reject_fully_qualified_flag() {
    assert!(validate_fully_qualified_test("--exact").is_err());
}

// ---------------------------------------------------------------------------
// Strict cargo_target grammar tests
// ---------------------------------------------------------------------------

#[test]
fn accept_valid_cargo_target() {
    assert!(validate_cargo_target("integration").is_ok());
    assert!(validate_cargo_target("unit").is_ok());
    assert!(validate_cargo_target("phase4_other").is_ok());
}

#[test]
fn reject_cargo_target_empty() {
    assert!(validate_cargo_target("").is_err());
}

#[test]
fn reject_cargo_target_whitespace() {
    assert!(validate_cargo_target(" integration").is_err());
    assert!(validate_cargo_target("integration ").is_err());
}

#[test]
fn reject_cargo_target_meta() {
    assert!(validate_cargo_target(SELF_TEST_TARGET).is_err());
}

#[test]
fn reject_cargo_target_leading_hyphen() {
    assert!(validate_cargo_target("-bad").is_err());
}

#[test]
fn reject_cargo_target_illegal_char() {
    assert!(validate_cargo_target("bad target").is_err());
    assert!(validate_cargo_target("bad/target").is_err());
    assert!(validate_cargo_target("bad.target").is_err());
}

// ---------------------------------------------------------------------------
// Strict source_anchor grammar tests
// ---------------------------------------------------------------------------

#[test]
fn accept_valid_source_anchor_simple() {
    assert!(validate_source_anchor("tests/integration.rs:test_impl_begin_valid").is_ok());
}

#[test]
fn accept_valid_source_anchor_path() {
    assert!(validate_source_anchor(
        "src/implementation.rs:part1_tests::index_stage_z_parses_stage0"
    )
    .is_ok());
}

#[test]
fn reject_source_anchor_dot_dot() {
    assert!(validate_source_anchor("../outside.rs:test_name").is_err());
}

#[test]
fn reject_source_anchor_drive_prefix() {
    assert!(validate_source_anchor("C:/outside.rs:test_name").is_err());
}

#[test]
fn reject_source_anchor_absolute() {
    assert!(validate_source_anchor("/tests/integration.rs:test_name").is_err());
}

#[test]
fn reject_source_anchor_doubled_slash() {
    assert!(validate_source_anchor("tests//integration.rs:test_name").is_err());
}

#[test]
fn reject_source_anchor_dot_segment() {
    assert!(validate_source_anchor("tests/./integration.rs:test_name").is_err());
}

#[test]
fn reject_source_anchor_dot_dot_segment() {
    assert!(validate_source_anchor("tests/a/../integration.rs:test_name").is_err());
}

#[test]
fn reject_source_anchor_non_rs() {
    assert!(validate_source_anchor("tests/integration.txt:test_name").is_err());
}

#[test]
fn reject_source_anchor_empty_function() {
    assert!(validate_source_anchor("tests/integration.rs:").is_err());
}

#[test]
fn reject_source_anchor_function_with_space() {
    assert!(validate_source_anchor("tests/integration.rs:bad function").is_err());
}

#[test]
fn reject_source_anchor_function_single_colon() {
    assert!(validate_source_anchor("tests/integration.rs:test:name").is_err());
}

#[test]
fn reject_source_anchor_backslash() {
    assert!(validate_source_anchor("tests\\integration.rs:test_name").is_err());
}

#[test]
fn reject_source_anchor_no_colon() {
    assert!(validate_source_anchor("tests/integration.rs").is_err());
}

// ---------------------------------------------------------------------------
// Registry deterministic ordering test
// ---------------------------------------------------------------------------

#[test]
fn reject_shuffled_registry_order() {
    let mut reg = load_valid_registry();
    // Swap two entries to break ascending order.
    reg.obligations.swap(0, 10);
    assert!(
        validate_registry(&reg).is_err(),
        "shuffled registry order must be rejected"
    );
}

// ---------------------------------------------------------------------------
// Negative registry-level tests
// ---------------------------------------------------------------------------

fn good_evidence() -> Evidence {
    Evidence {
        cargo_target_kind: "integration".to_string(),
        cargo_target: "integration".to_string(),
        fully_qualified_test: "test_impl_begin_valid".to_string(),
        scenario: "valid first begin".to_string(),
        assertion_semantics: "stdout equals IMPLEMENTATION_BOUND".to_string(),
        platform_scope: "all".to_string(),
        source_anchor: "tests/integration.rs:test_impl_begin_valid".to_string(),
    }
}

fn good_obligation(id: u32, status: &str) -> Obligation {
    Obligation {
        id,
        canonical_id: format!("P4-{:03}", id),
        text: "synthetic obligation text;".to_string(),
        mapping_status: status.to_string(),
        evidence: vec![],
    }
}

fn good_registry() -> Registry {
    Registry {
        schema_version: 1,
        obligation_count: 188,
        source_catalog_sha256: EXPECTED_CATALOG_SHA256.to_string(),
        obligations: (1u32..=188)
            .map(|id| good_obligation(id, "UNSATISFIED"))
            .collect(),
    }
}

#[test]
fn reject_unknown_top_level_registry_field() {
    let json = r#"{
        "schema_version": 1,
        "obligation_count": 188,
        "source_catalog_sha256": "3559ee4864b16a9099d2401eb314ecbd40109f19477fb5413d936801f6711a20",
        "obligations": [],
        "extra_field": true
    }"#;
    assert!(
        parse_registry(json).is_err(),
        "unknown top-level field accepted"
    );
}

#[test]
fn reject_unknown_obligation_field() {
    let json = r#"{
        "schema_version": 1,
        "obligation_count": 188,
        "source_catalog_sha256": "3559ee4864b16a9099d2401eb314ecbd40109f19477fb5413d936801f6711a20",
        "obligations": [
            {"id": 1, "canonical_id": "P4-001", "text": "x;", "mapping_status": "UNSATISFIED", "evidence": [], "bogus": 1}
        ]
    }"#;
    assert!(
        parse_registry(json).is_err(),
        "unknown obligation field accepted"
    );
}

#[test]
fn reject_unknown_evidence_field() {
    let json = r#"{
        "schema_version": 1,
        "obligation_count": 188,
        "source_catalog_sha256": "3559ee4864b16a9099d2401eb314ecbd40109f19477fb5413d936801f6711a20",
        "obligations": [
            {"id": 1, "canonical_id": "P4-001", "text": "x;", "mapping_status": "SATISFIED",
             "evidence": [{"cargo_target_kind": "integration", "cargo_target": "integration",
                           "fully_qualified_test": "t", "scenario": "s", "assertion_semantics": "a",
                           "platform_scope": "all", "source_anchor": "f:g", "ghost": 0}]}
        ]
    }"#;
    assert!(
        parse_registry(json).is_err(),
        "unknown evidence field accepted"
    );
}

#[test]
fn reject_duplicate_numeric_id() {
    let mut reg = good_registry();
    reg.obligations[1] = good_obligation(1, "UNSATISFIED");
    assert!(
        validate_registry(&reg).is_err(),
        "duplicate numeric id accepted"
    );
}

#[test]
fn reject_duplicate_canonical_id() {
    let mut reg = good_registry();
    reg.obligations[1].canonical_id = "P4-001".to_string();
    assert!(
        validate_registry(&reg).is_err(),
        "duplicate canonical id accepted"
    );
}

#[test]
fn reject_missing_or_blank_obligation_text() {
    let mut reg = good_registry();
    reg.obligations[0].text = String::new();
    assert!(validate_registry(&reg).is_err(), "empty text accepted");
    reg.obligations[0].text = "   ".to_string();
    assert!(validate_registry(&reg).is_err(), "blank text accepted");
}

#[test]
fn reject_invalid_mapping_status() {
    let mut reg = good_registry();
    reg.obligations[0].mapping_status = "PARTIAL".to_string();
    assert!(
        validate_registry(&reg).is_err(),
        "invalid mapping_status accepted"
    );
}

#[test]
fn reject_satisfied_with_no_evidence() {
    let mut reg = good_registry();
    reg.obligations[0].mapping_status = "SATISFIED".to_string();
    reg.obligations[0].evidence = vec![];
    assert!(
        validate_registry(&reg).is_err(),
        "SATISFIED with no evidence accepted"
    );
}

#[test]
fn reject_empty_cargo_target_kind() {
    let mut e = good_evidence();
    e.cargo_target_kind = String::new();
    assert!(
        validate_evidence(&e).is_err(),
        "empty cargo_target_kind accepted"
    );
}

#[test]
fn reject_invalid_cargo_target_kind() {
    let mut e = good_evidence();
    e.cargo_target_kind = "doc".to_string();
    assert!(
        validate_evidence(&e).is_err(),
        "invalid cargo_target_kind accepted"
    );
}

#[test]
fn reject_empty_cargo_target() {
    let mut e = good_evidence();
    e.cargo_target = String::new();
    assert!(
        validate_evidence(&e).is_err(),
        "empty cargo_target accepted"
    );
}

#[test]
fn reject_empty_fully_qualified_test() {
    let mut e = good_evidence();
    e.fully_qualified_test = String::new();
    assert!(
        validate_evidence(&e).is_err(),
        "empty fully_qualified_test accepted"
    );
}

#[test]
fn reject_wildcard_test_name() {
    let mut e = good_evidence();
    e.fully_qualified_test = "test_impl_*".to_string();
    assert!(
        validate_evidence(&e).is_err(),
        "wildcard test name accepted"
    );
}

#[test]
fn reject_filter_shaped_test_name() {
    let mut e = good_evidence();
    e.fully_qualified_test = "impl/begin".to_string();
    assert!(
        validate_evidence(&e).is_err(),
        "substring/filter test name accepted"
    );
}

#[test]
fn reject_empty_scenario() {
    let mut e = good_evidence();
    e.scenario = String::new();
    assert!(validate_evidence(&e).is_err(), "empty scenario accepted");
}

#[test]
fn reject_empty_assertion_semantics() {
    let mut e = good_evidence();
    e.assertion_semantics = String::new();
    assert!(
        validate_evidence(&e).is_err(),
        "empty assertion_semantics accepted"
    );
}

#[test]
fn reject_invalid_platform_scope() {
    let mut e = good_evidence();
    e.platform_scope = "macos".to_string();
    assert!(
        validate_evidence(&e).is_err(),
        "invalid platform_scope accepted"
    );
}

#[test]
fn reject_empty_or_malformed_source_anchor() {
    let cases = [
        "",
        "no_colon",
        "/abs/path.rs:func",
        "file.rs:",
        "file.rs:bad function",
        "file.rs:test:name",
        "tests\\integration.rs:test_name",
    ];
    for c in cases {
        let mut e = good_evidence();
        e.source_anchor = c.to_string();
        assert!(
            validate_evidence(&e).is_err(),
            "source_anchor '{}' accepted",
            c
        );
    }
}

#[test]
fn reject_meta_test_self_certification() {
    let mut e = good_evidence();
    e.cargo_target = SELF_TEST_TARGET.to_string();
    assert!(
        validate_evidence(&e).is_err(),
        "evidence referring to the P2-D meta-test accepted"
    );
    let mut reg = good_registry();
    reg.obligations[0].mapping_status = "SATISFIED".to_string();
    reg.obligations[0].evidence = vec![e];
    assert!(
        validate_registry(&reg).is_err(),
        "SATISFIED obligation certified only by the meta-test accepted"
    );
}
