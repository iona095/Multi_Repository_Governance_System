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
//! D2 expectation: every obligation is mapped to concrete evidence for an
//! exact discovered Cargo test, and the governance recommendation is `PASS`
//! only when all 188 obligations are satisfied.

use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::process::Command;

const CONTRACT_BLOB_REF: &str = "HEAD:docs/contracts/phase-04-contract.md";

const EXPECTED_CONTRACT_BLOB_SHA256: &str =
    "542f38e78e0aa22dc8fec3692a608c80b0494b96678a43caa80fa7f78de22a04";

const EXPECTED_CONTRACT_BLOB_LEN: usize = 84306;

const EXPECTED_CATALOG_SHA256: &str =
    "3559ee4864b16a9099d2401eb314ecbd40109f19477fb5413d936801f6711a20";

const REGISTRY_REL_PATH: &str = "tests/phase4_obligations.json";

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

#[derive(Debug, Clone)]
struct DiscoverySets {
    unit: BTreeSet<String>,
    integration: BTreeSet<String>,
}

// ---------------------------------------------------------------------------
// Canonical contract blob helper
// ---------------------------------------------------------------------------

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
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    let digest = {
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

fn is_rust_identifier_component(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

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
    validate_fully_qualified_test(func)
}

// ---------------------------------------------------------------------------
// Lexical sanitizer — replace non-code with spaces, preserve line-feeds
// ---------------------------------------------------------------------------

fn sanitize_rust_non_code(source: &str) -> Result<String, String> {
    let bytes = source.as_bytes();
    let len = bytes.len();
    let mut out = vec![b' '; len];
    let mut i: usize = 0;

    while i < len {
        match bytes[i] {
            b'\n' => {
                out[i] = b'\n';
                i += 1;
            }
            b'/' if i + 1 < len && bytes[i + 1] == b'/' => {
                while i < len && bytes[i] != b'\n' {
                    i += 1;
                }
            }
            b'/' if i + 1 < len && bytes[i + 1] == b'*' => {
                i += 2;
                let mut depth: u32 = 1;
                while depth > 0 {
                    if i + 1 >= len {
                        return Err("unterminated block comment".to_string());
                    }
                    if bytes[i] == b'*' && bytes[i + 1] == b'/' {
                        i += 2;
                        depth -= 1;
                    } else if bytes[i] == b'/' && bytes[i + 1] == b'*' {
                        i += 2;
                        depth += 1;
                    } else if bytes[i] == b'\n' {
                        out[i] = b'\n';
                        i += 1;
                    } else {
                        i += 1;
                    }
                }
            }
            b'"' => {
                i += 1;
                while i < len && bytes[i] != b'"' {
                    if bytes[i] == b'\\' {
                        if i + 1 >= len {
                            return Err("unterminated ordinary string".to_string());
                        }
                        match bytes[i + 1] {
                            b'\n' => {
                                out[i + 1] = b'\n';
                                i += 2;
                            }
                            b'\r' if i + 2 < len && bytes[i + 2] == b'\n' => {
                                out[i + 2] = b'\n';
                                i += 3;
                            }
                            b'n' | b'r' | b't' | b'\\' | b'0' | b'\'' | b'"' => {
                                i += 2;
                            }
                            b'x' => {
                                if i + 3 >= len
                                    || !matches!(bytes[i + 2], b'0'..=b'7')
                                    || !bytes[i + 3].is_ascii_hexdigit()
                                {
                                    return Err("invalid ordinary string hex escape".to_string());
                                }
                                i += 4;
                            }
                            b'u' => {
                                if i + 2 >= len || bytes[i + 2] != b'{' {
                                    return Err(
                                        "invalid ordinary string unicode escape".to_string()
                                    );
                                }

                                let mut cursor = i + 3;
                                let mut digit_count = 0usize;
                                let mut scalar = 0u32;
                                while cursor < len && bytes[cursor] != b'}' {
                                    let digit = match bytes[cursor] {
                                        b'0'..=b'9' => u32::from(bytes[cursor] - b'0'),
                                        b'a'..=b'f' => u32::from(bytes[cursor] - b'a' + 10),
                                        b'A'..=b'F' => u32::from(bytes[cursor] - b'A' + 10),
                                        _ => {
                                            return Err("invalid ordinary string unicode escape"
                                                .to_string())
                                        }
                                    };
                                    digit_count += 1;
                                    if digit_count > 6 {
                                        return Err(
                                            "invalid ordinary string unicode escape".to_string()
                                        );
                                    }
                                    scalar = scalar * 16 + digit;
                                    cursor += 1;
                                    while cursor < len && bytes[cursor] == b'_' {
                                        cursor += 1;
                                    }
                                }
                                if digit_count == 0
                                    || cursor >= len
                                    || char::from_u32(scalar).is_none()
                                {
                                    return Err(
                                        "invalid ordinary string unicode escape".to_string()
                                    );
                                }
                                i = cursor + 1;
                            }
                            _ => return Err("invalid ordinary string escape".to_string()),
                        }
                    } else if bytes[i] == b'\n' {
                        out[i] = b'\n';
                        i += 1;
                    } else if bytes[i] == b'\r' {
                        if i + 1 < len && bytes[i + 1] == b'\n' {
                            out[i + 1] = b'\n';
                            i += 2;
                        } else {
                            return Err("invalid carriage return in ordinary string".to_string());
                        }
                    } else {
                        i += 1;
                    }
                }
                if i >= len {
                    return Err("unterminated ordinary string".to_string());
                }
                i += 1;
            }
            b'r' if i + 1 < len && (bytes[i + 1] == b'"' || bytes[i + 1] == b'#') => {
                if bytes[i + 1] == b'"' {
                    i += 2;
                    loop {
                        if i >= len {
                            return Err("unterminated raw string".to_string());
                        }
                        if bytes[i] == b'"' {
                            i += 1;
                            break;
                        }
                        if bytes[i] == b'\n' {
                            out[i] = b'\n';
                        } else if bytes[i] == b'\r' {
                            if i + 1 < len && bytes[i + 1] == b'\n' {
                                out[i + 1] = b'\n';
                                i += 2;
                                continue;
                            }
                            return Err("invalid carriage return in raw string".to_string());
                        }
                        i += 1;
                    }
                } else {
                    let mut hash_count: usize = 0;
                    let save = i;
                    i += 1;
                    while i < len && bytes[i] == b'#' {
                        hash_count += 1;
                        i += 1;
                    }
                    if hash_count > 255 {
                        return Err("raw string delimiter exceeds 255 hashes".to_string());
                    }
                    if i < len && bytes[i] == b'"' {
                        i += 1;
                        loop {
                            if i >= len {
                                return Err("unterminated raw string".to_string());
                            }
                            if bytes[i] == b'"' {
                                i += 1;
                                let mut seen: usize = 0;
                                while i < len && bytes[i] == b'#' && seen < hash_count {
                                    seen += 1;
                                    i += 1;
                                }
                                if seen == hash_count {
                                    break;
                                }
                            } else {
                                if bytes[i] == b'\n' {
                                    out[i] = b'\n';
                                } else if bytes[i] == b'\r' {
                                    if i + 1 < len && bytes[i + 1] == b'\n' {
                                        out[i + 1] = b'\n';
                                        i += 2;
                                        continue;
                                    }
                                    return Err("invalid carriage return in raw string".to_string());
                                }
                                i += 1;
                            }
                        }
                    } else {
                        out[save] = bytes[save];
                        i = save + 1;
                    }
                }
            }
            b'b' if i + 1 < len && bytes[i + 1] == b'"' => {
                i += 2;
                while i < len && bytes[i] != b'"' {
                    if bytes[i] == b'\\' {
                        if i + 1 >= len {
                            return Err("unterminated byte string".to_string());
                        }
                        match bytes[i + 1] {
                            b'\n' => {
                                out[i + 1] = b'\n';
                                i += 2;
                            }
                            b'\r' if i + 2 < len && bytes[i + 2] == b'\n' => {
                                out[i + 2] = b'\n';
                                i += 3;
                            }
                            b'n' | b'r' | b't' | b'\\' | b'0' | b'\'' | b'"' => {
                                i += 2;
                            }
                            b'x' => {
                                if i + 3 >= len
                                    || !bytes[i + 2].is_ascii_hexdigit()
                                    || !bytes[i + 3].is_ascii_hexdigit()
                                {
                                    return Err("invalid byte string hex escape".to_string());
                                }
                                i += 4;
                            }
                            _ => return Err("invalid byte string escape".to_string()),
                        }
                    } else if bytes[i] == b'\n' {
                        out[i] = b'\n';
                        i += 1;
                    } else if bytes[i] == b'\r' {
                        if i + 1 < len && bytes[i + 1] == b'\n' {
                            out[i + 1] = b'\n';
                            i += 2;
                        } else {
                            return Err("invalid carriage return in byte string".to_string());
                        }
                    } else if bytes[i] >= 0x80 {
                        return Err("non-ASCII content in byte string".to_string());
                    } else {
                        i += 1;
                    }
                }
                if i >= len {
                    return Err("unterminated byte string".to_string());
                }
                i += 1;
            }
            b'b' if i + 2 < len && bytes[i + 1] == b'r' => {
                if bytes[i + 2] == b'"' {
                    i += 3;
                    loop {
                        if i >= len {
                            return Err("unterminated raw byte string".to_string());
                        }
                        if bytes[i] == b'"' {
                            i += 1;
                            break;
                        }
                        if bytes[i] == b'\n' {
                            out[i] = b'\n';
                        } else if bytes[i] == b'\r' {
                            if i + 1 < len && bytes[i + 1] == b'\n' {
                                out[i + 1] = b'\n';
                                i += 2;
                                continue;
                            }
                            return Err("invalid carriage return in raw byte string".to_string());
                        } else if bytes[i] >= 0x80 {
                            return Err("non-ASCII content in raw byte string".to_string());
                        }
                        i += 1;
                    }
                } else if bytes[i + 2] == b'#' {
                    let mut hash_count: usize = 0;
                    i += 2;
                    while i < len && bytes[i] == b'#' {
                        hash_count += 1;
                        i += 1;
                    }
                    if hash_count > 255 {
                        return Err("raw byte string delimiter exceeds 255 hashes".to_string());
                    }
                    if i < len && bytes[i] == b'"' {
                        i += 1;
                        loop {
                            if i >= len {
                                return Err("unterminated raw byte string".to_string());
                            }
                            if bytes[i] == b'"' {
                                i += 1;
                                let mut seen: usize = 0;
                                while i < len && bytes[i] == b'#' && seen < hash_count {
                                    seen += 1;
                                    i += 1;
                                }
                                if seen == hash_count {
                                    break;
                                }
                            } else {
                                if bytes[i] == b'\n' {
                                    out[i] = b'\n';
                                } else if bytes[i] == b'\r' {
                                    if i + 1 < len && bytes[i + 1] == b'\n' {
                                        out[i + 1] = b'\n';
                                        i += 2;
                                        continue;
                                    }
                                    return Err(
                                        "invalid carriage return in raw byte string".to_string()
                                    );
                                } else if bytes[i] >= 0x80 {
                                    return Err("non-ASCII content in raw byte string".to_string());
                                }
                                i += 1;
                            }
                        }
                    } else {
                        out[i] = bytes[i];
                        i += 1;
                    }
                } else {
                    out[i] = bytes[i];
                    i += 1;
                }
            }
            b'b' if i + 2 < len && bytes[i + 1] == b'\'' => {
                i += 2;
                if i >= len {
                    return Err("unterminated byte character literal".to_string());
                }
                if bytes[i] == b'\\' {
                    if i + 1 >= len {
                        return Err("unterminated byte character literal".to_string());
                    }
                    if bytes[i + 1] == b'x' {
                        if i + 3 >= len
                            || !bytes[i + 2].is_ascii_hexdigit()
                            || !bytes[i + 3].is_ascii_hexdigit()
                        {
                            return Err("invalid byte character hex escape".to_string());
                        }
                        i += 4;
                    } else if matches!(
                        bytes[i + 1],
                        b'n' | b'r' | b't' | b'\\' | b'0' | b'\'' | b'"'
                    ) {
                        i += 2;
                    } else {
                        return Err("invalid byte character escape".to_string());
                    }
                } else if bytes[i].is_ascii() && !matches!(bytes[i], b'\n' | b'\r' | b'\t' | b'\'')
                {
                    i += 1;
                } else {
                    return Err("invalid byte character literal".to_string());
                }
                if i >= len || bytes[i] != b'\'' {
                    return Err("unterminated byte character literal".to_string());
                }
                i += 1;
            }
            b'\'' => {
                if i + 1 >= len {
                    return Err("unterminated character literal".to_string());
                }
                let identifier_end = if bytes[i + 1].is_ascii_alphabetic() || bytes[i + 1] == b'_' {
                    Some(i + 2)
                } else if bytes[i + 1] >= 0x80 {
                    source[i + 1..]
                        .chars()
                        .next()
                        .filter(|character| *character != '\n')
                        .map(|character| i + 1 + character.len_utf8())
                } else {
                    None
                };
                if identifier_end.is_some_and(|end| end >= len || bytes[end] != b'\'') {
                    out[i] = b'\'';
                    i += 1;
                } else {
                    i += 1;
                    if bytes[i] == b'\\' {
                        if i + 1 >= len {
                            return Err("unterminated character literal".to_string());
                        }
                        match bytes[i + 1] {
                            b'x' => {
                                if i + 3 >= len
                                    || !bytes[i + 2].is_ascii_hexdigit()
                                    || !bytes[i + 3].is_ascii_hexdigit()
                                {
                                    return Err("invalid character hex escape".to_string());
                                }
                                let value = u32::from_str_radix(&source[i + 2..i + 4], 16)
                                    .map_err(|_| "invalid character hex escape".to_string())?;
                                if value > 0x7f {
                                    return Err(
                                        "ordinary character hex escape exceeds 0x7F".to_string()
                                    );
                                }
                                i += 4;
                            }
                            b'u' => {
                                if i + 2 >= len || bytes[i + 2] != b'{' {
                                    return Err("invalid character unicode escape".to_string());
                                }

                                let mut cursor = i + 3;
                                let mut digit_count = 0usize;
                                let mut scalar = 0u32;
                                while cursor < len && bytes[cursor] != b'}' {
                                    let digit = match bytes[cursor] {
                                        b'0'..=b'9' => u32::from(bytes[cursor] - b'0'),
                                        b'a'..=b'f' => u32::from(bytes[cursor] - b'a' + 10),
                                        b'A'..=b'F' => u32::from(bytes[cursor] - b'A' + 10),
                                        _ => {
                                            return Err(
                                                "invalid character unicode escape".to_string()
                                            )
                                        }
                                    };
                                    digit_count += 1;
                                    if digit_count > 6 {
                                        return Err("invalid character unicode escape".to_string());
                                    }
                                    scalar = scalar * 16 + digit;
                                    cursor += 1;
                                    while cursor < len && bytes[cursor] == b'_' {
                                        cursor += 1;
                                    }
                                }
                                if digit_count == 0 || cursor >= len {
                                    return Err("invalid character unicode escape".to_string());
                                }
                                if char::from_u32(scalar).is_none() {
                                    return Err("invalid character unicode scalar".to_string());
                                }
                                i = cursor + 1;
                            }
                            b'n' | b'r' | b't' | b'\\' | b'0' | b'\'' | b'"' => i += 2,
                            _ => return Err("invalid character escape".to_string()),
                        }
                    } else {
                        if matches!(bytes[i], b'\n' | b'\r' | b'\t' | b'\'') {
                            return Err("invalid character literal".to_string());
                        }
                        let scalar_len = source[i..]
                            .chars()
                            .next()
                            .expect("source slice is nonempty")
                            .len_utf8();
                        i += scalar_len;
                    }
                    if i >= len || bytes[i] != b'\'' {
                        return Err("unterminated character literal".to_string());
                    }
                    i += 1;
                }
            }
            _ => {
                out[i] = bytes[i];
                i += 1;
            }
        }
    }

    String::from_utf8(out).map_err(|e| format!("internal: sanitizer output not valid UTF-8: {}", e))
}

// ---------------------------------------------------------------------------
// Exact test-function recognition (operates on sanitized source)
// ---------------------------------------------------------------------------

fn verify_test_function_in_source(source: &str, function: &str) -> Result<(), String> {
    let sanitized = sanitize_rust_non_code(source)?;
    let final_name = function.rsplit("::").next().unwrap_or(function);
    let bytes = sanitized.as_bytes();
    let is_ws = |byte: u8| matches!(byte, b' ' | b'\t' | b'\r' | b'\n');
    let skip_ws = |mut index: usize| {
        while index < bytes.len() && is_ws(bytes[index]) {
            index += 1;
        }
        index
    };
    let word_at = |index: usize, word: &[u8]| {
        bytes.get(index..index + word.len()) == Some(word)
            && (index == 0
                || !(bytes[index - 1].is_ascii_alphanumeric() || bytes[index - 1] == b'_'))
    };
    let mut found_declarations = Vec::new();

    for start in 0..bytes.len() {
        let mut index = start;
        if word_at(index, b"pub") {
            index = skip_ws(index + 3);
            if index == start + 3 {
                continue;
            }
            if word_at(index, b"async") {
                index = skip_ws(index + 5);
                if index == start + 3 + 5 {
                    continue;
                }
            }
        } else if word_at(index, b"async") {
            index = skip_ws(index + 5);
            if index == start + 5 {
                continue;
            }
        }
        if !word_at(index, b"fn") {
            continue;
        }
        let fn_index = index;
        let after_fn = index + 2;
        index = skip_ws(after_fn);
        if index == after_fn || index + final_name.len() > bytes.len() {
            continue;
        }
        if bytes.get(index..index + final_name.len()) != Some(final_name.as_bytes()) {
            continue;
        }
        index = skip_ws(index + final_name.len());
        if index < bytes.len() && bytes[index] == b'(' {
            found_declarations.push((fn_index, start));
        }
    }

    found_declarations.sort_unstable();
    found_declarations.dedup_by_key(|(fn_index, _)| *fn_index);

    if found_declarations.len() > 1 {
        return Err(format!(
            "function '{}' appears {} times in source (ambiguous)",
            final_name,
            found_declarations.len()
        ));
    }

    if found_declarations.is_empty() {
        return Err(format!("function '{}' not found in source", final_name));
    }

    let mut has_test_attr = false;
    let mut end = found_declarations[0].1;
    loop {
        while end > 0 && is_ws(bytes[end - 1]) {
            end -= 1;
        }
        if end == 0 || bytes[end - 1] != b']' {
            break;
        }
        let mut cursor = end;
        let mut depth = 0usize;
        let mut open = None;
        while cursor > 0 {
            cursor -= 1;
            match bytes[cursor] {
                b']' => depth += 1,
                b'[' => {
                    depth -= 1;
                    if depth == 0 {
                        open = Some(cursor);
                        break;
                    }
                }
                _ => {}
            }
        }
        let Some(open) = open else {
            break;
        };
        let mut hash = open;
        while hash > 0 && is_ws(bytes[hash - 1]) {
            hash -= 1;
        }
        if hash == 0 || bytes[hash - 1] != b'#' {
            break;
        }
        let start = hash - 1;
        let normalized: Vec<u8> = bytes[start..end]
            .iter()
            .copied()
            .filter(|byte| !is_ws(*byte))
            .collect();
        if normalized == b"#[test]" {
            has_test_attr = true;
        }
        end = start;
    }

    if !has_test_attr {
        return Err(format!(
            "function '{}' exists but lacks an exact #[test] attribute",
            final_name
        ));
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Cargo test discovery
// ---------------------------------------------------------------------------

fn parse_discovery_stdout(stdout: &[u8]) -> Result<BTreeSet<String>, String> {
    let text = std::str::from_utf8(stdout)
        .map_err(|e| format!("discovery stdout not valid UTF-8: {}", e))?;
    let mut set = BTreeSet::new();
    for line in text.lines() {
        // Check for lines that match "name: test" pattern
        if let Some(pos) = line.rfind(": test") {
            let after = &line[pos + 6..];
            // If there's anything after ": test", it must be rejected
            if !after.is_empty() {
                // Check if the trailing content is only whitespace (malformed)
                if after.chars().all(|c| c.is_ascii_whitespace()) {
                    return Err(format!(
                        "line has trailing whitespace after ': test': '{}'",
                        line
                    ));
                }
                // Otherwise it's not a test line, skip
                continue;
            }
            // Line ends exactly with ": test"
            let raw_name = &line[..pos];
            let trimmed = raw_name.trim();
            if trimmed.is_empty() {
                return Err("empty test name on discovery line".to_string());
            }
            if trimmed != raw_name {
                return Err(format!(
                    "discovered test name has leading/trailing whitespace: '{}'",
                    raw_name
                ));
            }
            if !set.insert(trimmed.to_string()) {
                return Err(format!("duplicate discovered test name: '{}'", trimmed));
            }
        }
    }
    Ok(set)
}

fn discover_cargo_tests(args: &[&str]) -> BTreeSet<String> {
    let manifest = env!("CARGO_MANIFEST_DIR");
    let out = Command::new("cargo")
        .current_dir(manifest)
        .args(args)
        .output()
        .unwrap_or_else(|e| panic!("cannot run cargo discovery: {}", e));
    assert!(
        out.status.success(),
        "cargo discovery must succeed; args={:?} exit={:?} stderr={}",
        args,
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );
    parse_discovery_stdout(&out.stdout).expect("discovery stdout parsing failed")
}

fn discover_unit_tests() -> BTreeSet<String> {
    discover_cargo_tests(&["test", "--bin", "mrgs", "--", "--list"])
}

fn discover_integration_tests() -> BTreeSet<String> {
    discover_cargo_tests(&["test", "--test", "integration", "--", "--list"])
}

fn build_discovery_sets() -> DiscoverySets {
    DiscoverySets {
        unit: discover_unit_tests(),
        integration: discover_integration_tests(),
    }
}

// ---------------------------------------------------------------------------
// Registry extraction / validation
// ---------------------------------------------------------------------------

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

fn evidence_key(e: &Evidence) -> (String, String, String, String, String, String) {
    (
        e.cargo_target_kind.clone(),
        e.cargo_target.clone(),
        e.fully_qualified_test.clone(),
        e.platform_scope.clone(),
        e.source_anchor.clone(),
        e.scenario.clone(),
    )
}

fn source_anchor_parts(anchor: &str) -> Result<(&str, &str), String> {
    validate_source_anchor(anchor)?;
    anchor
        .split_once(':')
        .ok_or_else(|| "source_anchor must contain a file and function".to_string())
}

fn validate_evidence_source(e: &Evidence, discovery: &DiscoverySets) -> Result<(), String> {
    let (source_file, source_function) = source_anchor_parts(&e.source_anchor)?;
    let root = std::fs::canonicalize(env!("CARGO_MANIFEST_DIR"))
        .map_err(|e| format!("cannot canonicalize repository root: {}", e))?;
    let candidate = root.join(source_file);
    let canonical = std::fs::canonicalize(&candidate)
        .map_err(|e| format!("source anchor file cannot be resolved: {}", e))?;
    if !canonical.starts_with(&root) {
        return Err("source anchor resolves outside repository".to_string());
    }
    if e.cargo_target_kind == "integration" {
        if e.cargo_target != "integration" || !source_file.starts_with("tests/") {
            return Err("integration evidence target/source mismatch".to_string());
        }
    } else if e.cargo_target != "mrgs" || !source_file.starts_with("src/") {
        return Err("unit evidence target/source mismatch".to_string());
    }
    let source = std::fs::read_to_string(&canonical)
        .map_err(|e| format!("cannot read source anchor file: {}", e))?;
    let test_function = e
        .fully_qualified_test
        .rsplit("::")
        .next()
        .expect("validated fully qualified test has a function component");
    if source_function
        .rsplit("::")
        .next()
        .expect("validated source anchor has a function component")
        != test_function
    {
        return Err("fully_qualified_test and source_anchor function mismatch".to_string());
    }

    verify_test_function_in_source(&source, source_function)?;

    let discovered = if e.cargo_target_kind == "integration" {
        &discovery.integration
    } else {
        &discovery.unit
    };

    if !discovered.contains(&e.fully_qualified_test) {
        return Err(format!(
            "fully_qualified_test '{}' not found in Cargo discovery set for target '{}' (kind='{}')",
            e.fully_qualified_test, e.cargo_target, e.cargo_target_kind
        ));
    }

    Ok(())
}

fn validate_d2_completion(reg: &Registry) -> Result<(), String> {
    let mut satisfied = Vec::new();
    let mut unsatisfied = Vec::new();
    for obligation in &reg.obligations {
        if obligation.mapping_status == "SATISFIED" {
            satisfied.push(obligation.id);
        } else {
            unsatisfied.push(obligation.id);
        }
    }
    let expected: Vec<u32> = (1..=188).collect();
    if satisfied != expected || !unsatisfied.is_empty() {
        return Err(format!(
            "D2 completion requires satisfied IDs 1..188 and no unsatisfied IDs; satisfied={:?} unsatisfied={:?}",
            satisfied, unsatisfied
        ));
    }
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

    // Build discovery sets only if there are evidence records
    let has_evidence = reg.obligations.iter().any(|o| !o.evidence.is_empty());
    let discovery = if has_evidence {
        Some(build_discovery_sets())
    } else {
        None
    };

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
        let mut previous = None;
        let mut seen = BTreeSet::new();
        if let Some(ref discs) = discovery {
            for evidence in &o.evidence {
                validate_evidence_source(evidence, discs).map_err(|message| {
                    format!("obligation {} evidence source invalid: {}", o.id, message)
                })?;
                let key = evidence_key(evidence);
                if !seen.insert(key.clone()) {
                    return Err(format!("obligation {} contains duplicate evidence", o.id));
                }
                if previous.as_ref().is_some_and(|previous| previous > &key) {
                    return Err(format!("obligation {} evidence is not deterministic", o.id));
                }
                previous = Some(key);
            }
        }
    }
    Ok(())
}

fn validate_registry_against_contract(reg: &Registry) -> Result<(), String> {
    validate_registry(reg)?;
    let section: std::collections::BTreeMap<u32, String> =
        extract_section_18_obligations(&load_verified_contract_text())
            .into_iter()
            .collect();
    for obligation in &reg.obligations {
        let expected = section
            .get(&obligation.id)
            .ok_or_else(|| format!("missing Section 18 obligation {}", obligation.id))?;
        if &obligation.text != expected {
            return Err(format!(
                "obligation {} text differs from Section 18",
                obligation.id
            ));
        }
    }
    Ok(())
}

fn load_valid_registry() -> Registry {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(REGISTRY_REL_PATH);
    let content = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read registry at {:?}: {}", path, e));
    let reg = parse_registry(&content).expect("registry parses with strict schema");
    validate_registry_against_contract(&reg)
        .expect("registry is structurally and semantically valid");
    reg
}

// ---------------------------------------------------------------------------
// Positive structural tests
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
fn zero_evidence_registry_validation_is_discovery_free() {
    let reg = load_valid_registry();
    assert_eq!(reg.obligation_count, 188);
    assert_eq!(
        reg.obligations
            .iter()
            .filter(|o| o.mapping_status == "SATISFIED")
            .count(),
        0
    );
    assert_eq!(
        reg.obligations
            .iter()
            .filter(|o| o.mapping_status == "UNSATISFIED")
            .count(),
        188
    );
    assert_eq!(
        reg.obligations
            .iter()
            .map(|o| o.evidence.len())
            .sum::<usize>(),
        0
    );
    validate_registry(&reg).expect("exact D1 registry must validate without evidence discovery");
}

#[test]
fn d2_registry_is_deterministic_and_complete() {
    let reg = load_valid_registry();
    validate_d2_completion(&reg).expect("D2 registry must be complete");
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
    assert_eq!(satisfied, (1u32..=188).collect::<Vec<_>>());
    assert!(
        unsatisfied.is_empty(),
        "D2 must have no unsatisfied obligations"
    );
    eprintln!(
        "P2-D2 EXECUTION PLAN: {} unique mapped tests; satisfied={}; unsatisfied={}",
        plan.len(),
        satisfied.len(),
        unsatisfied.len()
    );
    eprintln!("P2-D2 RECOMMENDATION: PASS");
}

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
        fully_qualified_test: "test_implementation_begin_idempotent".to_string(),
        scenario: "valid first begin".to_string(),
        assertion_semantics: "stdout equals IMPLEMENTATION_BOUND".to_string(),
        platform_scope: "all".to_string(),
        source_anchor: "tests/integration.rs:test_implementation_begin_idempotent".to_string(),
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
fn reject_missing_obligation() {
    let mut reg = good_registry();
    reg.obligations.pop();
    assert!(
        validate_registry(&reg).is_err(),
        "registry with a missing obligation accepted"
    );
}

#[test]
fn reject_modified_obligation_text_against_contract() {
    let mut reg = load_valid_registry();
    reg.obligations[0].text.push('x');
    assert!(
        validate_registry_against_contract(&reg).is_err(),
        "modified Section 18 text accepted"
    );
}

#[test]
fn reject_invalid_source_evidence() {
    let discovery = build_discovery_sets();
    let mut evidence = good_evidence();
    evidence.source_anchor = "tests/missing.rs:test_implementation_begin_idempotent".into();
    assert!(validate_evidence_source(&evidence, &discovery).is_err());

    evidence.source_anchor = "tests/integration.rs:missing_test".into();
    assert!(validate_evidence_source(&evidence, &discovery).is_err());

    evidence.source_anchor = "tests/integration.rs:test_implementation_check_after_begin".into();
    assert!(validate_evidence_source(&evidence, &discovery).is_err());
}

#[test]
fn reject_target_source_mismatch() {
    let discovery = build_discovery_sets();
    let mut evidence = good_evidence();
    evidence.cargo_target = "other-integration-target".into();
    assert!(validate_evidence_source(&evidence, &discovery).is_err());
}

#[test]
fn reject_duplicate_evidence() {
    let mut reg = good_registry();
    let evidence = good_evidence();
    reg.obligations[0].evidence = vec![evidence, good_evidence()];
    assert!(
        validate_registry(&reg).is_err(),
        "duplicate evidence accepted"
    );
}

#[test]
fn reject_one_remaining_unsatisfied_obligation() {
    let reg = good_registry();
    assert!(
        validate_d2_completion(&reg).is_err(),
        "D2 completion accepted an unsatisfied registry"
    );
}

#[test]
fn reject_nondeterministic_evidence_order() {
    let mut reg = good_registry();
    let mut first = good_evidence();
    first.scenario = "z scenario".into();
    let mut second = good_evidence();
    second.scenario = "a scenario".into();
    reg.obligations[0].evidence = vec![first, second];
    assert!(
        validate_registry(&reg).is_err(),
        "nondeterministic evidence order accepted"
    );
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

// ---------------------------------------------------------------------------
// Lexical sanitizer tests
// ---------------------------------------------------------------------------

#[test]
fn sanitize_basic_code_preserved() {
    let src = "fn hello() { }\n";
    let result = sanitize_rust_non_code(src).unwrap();
    assert_eq!(result, "fn hello() { }\n");
}

#[test]
fn sanitize_line_comment_replaced() {
    let src = "fn hello() { // comment\n}\n";
    let result = sanitize_rust_non_code(src).unwrap();
    assert_eq!(result, "fn hello() {           \n}\n");
}

#[test]
fn sanitize_block_comment_replaced() {
    let src = "fn hello() /* comment */ {}\n";
    let result = sanitize_rust_non_code(src).unwrap();
    assert_eq!(result, "fn hello()               {}\n");
}

#[test]
fn sanitize_nested_block_comment_replaced() {
    let src = "fn /* outer /* inner */ end */ x()\n";
    let result = sanitize_rust_non_code(src).unwrap();
    assert_eq!(result, "fn                             x()\n");
}

#[test]
fn sanitize_ordinary_string_replaced() {
    let src = "fn main() { let s = \"hello world\"; }\n";
    let result = sanitize_rust_non_code(src).unwrap();
    assert_eq!(result, "fn main() { let s =              ; }\n");
}

#[test]
fn sanitize_string_with_escapes_replaced() {
    let src = "fn main() { let s = \"hel\\\"lo\"; }\n";
    let result = sanitize_rust_non_code(src).unwrap();
    assert_eq!(result, "fn main() { let s =          ; }\n");
}

#[test]
fn sanitize_raw_string_replaced() {
    let src = "fn main() { let s = r\"raw text\"; }\n";
    let result = sanitize_rust_non_code(src).unwrap();
    assert_eq!(result, "fn main() { let s =            ; }\n");
}

#[test]
fn raw_string_unicode_still_accepted() {
    let src = "let s = r\"é\";\n";
    let result = sanitize_rust_non_code(src).unwrap();
    assert_eq!(result, "let s =      ;\n");
}

#[test]
fn sanitize_raw_string_with_hashes_replaced() {
    let src = "fn main() { let s = r#\"raw \"text\"#; }\n";
    let result = sanitize_rust_non_code(src).unwrap();
    assert_eq!(result, "fn main() { let s =               ; }\n");
}

#[test]
fn sanitize_raw_byte_string_replaced() {
    let src = "fn main() { let s = br\"raw bytes\"; }\n";
    let result = sanitize_rust_non_code(src).unwrap();
    assert_eq!(result, "fn main() { let s =              ; }\n");
}

#[test]
fn sanitize_character_literal_replaced() {
    let src = "fn main() { let c = 'x'; }\n";
    let result = sanitize_rust_non_code(src).unwrap();
    assert_eq!(result, "fn main() { let c =    ; }\n");
}

#[test]
fn sanitize_byte_string_replaced() {
    let src = "fn main() { let b = b\"bytes\"; }\n";
    let result = sanitize_rust_non_code(src).unwrap();
    assert_eq!(result, "fn main() { let b =         ; }\n");
}

#[test]
fn byte_string_non_ascii_rejected() {
    let src = "let b = b\"é\";\n";
    assert!(sanitize_rust_non_code(src).is_err());
}

#[test]
fn byte_string_unescaped_cr_rejected() {
    let src = "let b = b\"abc\rdef\";\n";
    assert!(sanitize_rust_non_code(src).is_err());
}

#[test]
fn byte_string_literal_lf_accepted() {
    let src = "let value = b\"first
second\";\n";
    let result = sanitize_rust_non_code(src).unwrap();
    assert_eq!(result.len(), src.len());
}

#[test]
fn byte_string_literal_lf_exact_positions() {
    let src = "let value = b\"first
second\";\n";
    let result = sanitize_rust_non_code(src).unwrap();
    let input_lfs: Vec<usize> = src
        .bytes()
        .enumerate()
        .filter_map(|(index, byte)| (byte == b'\n').then_some(index))
        .collect();
    let output_lfs: Vec<usize> = result
        .bytes()
        .enumerate()
        .filter_map(|(index, byte)| (byte == b'\n').then_some(index))
        .collect();

    assert_eq!(result.len(), src.len());
    assert_eq!(output_lfs, input_lfs);
}

#[test]
fn byte_string_crlf_accepted_as_logical_lf() {
    let src = "let value = b\"first\r\nsecond\";\n";
    let result = sanitize_rust_non_code(src).unwrap();
    assert_eq!(result.len(), src.len());
}

#[test]
fn byte_string_crlf_exact_positions() {
    let src = "let value = b\"first\r\nsecond\";\n";
    let result = sanitize_rust_non_code(src).unwrap();
    let input_lfs: Vec<usize> = src
        .bytes()
        .enumerate()
        .filter_map(|(index, byte)| (byte == b'\n').then_some(index))
        .collect();
    let output_lfs: Vec<usize> = result
        .bytes()
        .enumerate()
        .filter_map(|(index, byte)| (byte == b'\n').then_some(index))
        .collect();
    let cr_index = src.bytes().position(|byte| byte == b'\r').unwrap();

    assert_eq!(result.len(), src.len());
    assert_eq!(output_lfs, input_lfs);
    assert_eq!(result.as_bytes()[cr_index], b' ');
}

#[test]
fn byte_string_isolated_cr_rejected() {
    let src = "let value = b\"first\rsecond\";\n";
    assert!(sanitize_rust_non_code(src).is_err());
}

#[test]
fn byte_string_backslash_crlf_continuation_accepted() {
    let src = "let value = b\"first\\\r\nsecond\";\n";
    let result = sanitize_rust_non_code(src).unwrap();
    assert_eq!(result.len(), src.len());
}

#[test]
fn byte_string_backslash_crlf_exact_positions() {
    let src = "let value = b\"first\\\r\nsecond\";\n";
    let result = sanitize_rust_non_code(src).unwrap();
    let input_lfs: Vec<usize> = src
        .bytes()
        .enumerate()
        .filter_map(|(index, byte)| (byte == b'\n').then_some(index))
        .collect();
    let output_lfs: Vec<usize> = result
        .bytes()
        .enumerate()
        .filter_map(|(index, byte)| (byte == b'\n').then_some(index))
        .collect();
    let backslash_index = src.bytes().position(|byte| byte == b'\\').unwrap();
    let cr_index = src.bytes().position(|byte| byte == b'\r').unwrap();

    assert_eq!(result.len(), src.len());
    assert_eq!(output_lfs, input_lfs);
    assert_eq!(result.as_bytes()[backslash_index], b' ');
    assert_eq!(result.as_bytes()[cr_index], b' ');
}

#[test]
fn byte_string_ascii_content_accepted() {
    let src = "let b = b\"abc\";\n";
    let result = sanitize_rust_non_code(src).unwrap();
    assert_eq!(result, format!("let b = {};\n", " ".repeat(6)));
}

#[test]
fn sanitize_byte_character_replaced() {
    let src = "fn main() { let b = b'x'; }\n";
    let result = sanitize_rust_non_code(src).unwrap();
    assert_eq!(result, "fn main() { let b =     ; }\n");
}

#[test]
fn sanitize_preserves_non_ascii_outside_literals() {
    let src = "fn café() { }\n";
    let result = sanitize_rust_non_code(src).unwrap();
    assert_eq!(result, "fn café() { }\n");
}

#[test]
fn sanitize_rejects_unterminated_block_comment() {
    let src = "fn hello() /* unterminated\n";
    assert!(sanitize_rust_non_code(src).is_err());
}

#[test]
fn sanitize_rejects_unterminated_ordinary_string() {
    let src = "fn hello() { let s = \"unterminated\n";
    assert!(sanitize_rust_non_code(src).is_err());
}

#[test]
fn sanitize_rejects_unterminated_raw_string() {
    let src = "fn hello() { let s = r\"unterminated\n";
    assert!(sanitize_rust_non_code(src).is_err());
}

// ---------------------------------------------------------------------------
// Lifetime and label tests
// ---------------------------------------------------------------------------

#[test]
fn unicode_lifetime_generic_preserved() {
    let src = "fn f<'α>() {}\n";
    let result = sanitize_rust_non_code(src).unwrap();
    assert_eq!(result, src);
}

#[test]
fn unicode_lifetime_reference_preserved() {
    let src = "fn f<'α>(x: &'α str) -> &'α str { x }\n";
    let result = sanitize_rust_non_code(src).unwrap();
    assert_eq!(result, src);
}

#[test]
fn unicode_label_loop_preserved() {
    let src = "'β: loop { break 'β; }\n";
    let result = sanitize_rust_non_code(src).unwrap();
    assert_eq!(result, src);
}

#[test]
fn unicode_label_break_preserved() {
    let src = "break 'β;\n";
    let result = sanitize_rust_non_code(src).unwrap();
    assert_eq!(result, src);
}

#[test]
fn unicode_character_literal_still_sanitized() {
    let src = "let c = 'é';\n";
    let result = sanitize_rust_non_code(src).unwrap();
    assert_eq!(result, "let c =     ;\n");
}

#[test]
fn unicode_lifetime_adjacent_to_character_literal() {
    let src = "fn f<'α>() { let c = 'é'; }\n";
    let result = sanitize_rust_non_code(src).unwrap();
    assert_eq!(result, "fn f<'α>() { let c =     ; }\n");
}

#[test]
fn unicode_apostrophe_scan_does_not_cross_lf() {
    let src = "fn f<'α>()\n{ let c = 'é'; }\n";
    let result = sanitize_rust_non_code(src).unwrap();
    assert_eq!(result, "fn f<'α>()\n{ let c =     ; }\n");
    let input_lfs: Vec<usize> = src
        .bytes()
        .enumerate()
        .filter_map(|(index, byte)| (byte == b'\n').then_some(index))
        .collect();
    let output_lfs: Vec<usize> = result
        .bytes()
        .enumerate()
        .filter_map(|(index, byte)| (byte == b'\n').then_some(index))
        .collect();
    assert_eq!(output_lfs, input_lfs);
}

#[test]
fn lifetime_ref_static_preserved() {
    let src = "let x: &'static str = val;\n";
    let result = sanitize_rust_non_code(src).unwrap();
    assert_eq!(result, "let x: &'static str = val;\n");
}

#[test]
fn lifetime_ref_a_preserved() {
    let src = "let x: &'a T = &val;\n";
    let result = sanitize_rust_non_code(src).unwrap();
    assert_eq!(result, "let x: &'a T = &val;\n");
}

#[test]
fn lifetime_generic_preserved() {
    let src = "fn f<'a>() {}\n";
    let result = sanitize_rust_non_code(src).unwrap();
    assert_eq!(result, "fn f<'a>() {}\n");
}

#[test]
fn label_loop_preserved() {
    let src = "'label: loop { break; }\n";
    let result = sanitize_rust_non_code(src).unwrap();
    assert_eq!(result, "'label: loop { break; }\n");
}

#[test]
fn label_break_preserved() {
    let src = "break 'label;\n";
    let result = sanitize_rust_non_code(src).unwrap();
    assert_eq!(result, "break 'label;\n");
}

#[test]
fn label_continue_preserved() {
    let src = "continue 'label;\n";
    let result = sanitize_rust_non_code(src).unwrap();
    assert_eq!(result, "continue 'label;\n");
}

#[test]
fn lifetime_and_unrelated_char_literal_same_line() {
    let src = "fn f<'a>(c: char) { let x = 'y'; }\n";
    let result = sanitize_rust_non_code(src).unwrap();
    assert_eq!(result, "fn f<'a>(c: char) { let x =    ; }\n");
}

#[test]
fn lifetime_and_unrelated_char_literal_next_line() {
    let src = "fn f<'a>()\n{ let x = 'y'; }\n";
    let result = sanitize_rust_non_code(src).unwrap();
    assert_eq!(result, "fn f<'a>()\n{ let x =    ; }\n");
}

#[test]
fn lifetime_and_label_code_preserve_lf_positions() {
    let src = "fn f() -> &'static str { \"x\" }\nfn f<'a>(x: &'a str) -> &'a str { x }\nfn f<'a>() {}\n'label: loop { break 'label; }\n";
    let result = sanitize_rust_non_code(src).unwrap();
    let input_lfs: Vec<usize> = src
        .bytes()
        .enumerate()
        .filter_map(|(index, byte)| (byte == b'\n').then_some(index))
        .collect();
    let output_lfs: Vec<usize> = result
        .bytes()
        .enumerate()
        .filter_map(|(index, byte)| (byte == b'\n').then_some(index))
        .collect();

    assert_eq!(result, "fn f() -> &'static str {     }\nfn f<'a>(x: &'a str) -> &'a str { x }\nfn f<'a>() {}\n'label: loop { break 'label; }\n");
    assert_eq!(result.len(), src.len());
    assert_eq!(output_lfs, input_lfs);
}

// ---------------------------------------------------------------------------
// Character literal tests
// ---------------------------------------------------------------------------

#[test]
fn character_literal_fixtures_preserve_length_and_lfs() {
    let fixtures = [
        ("let c = 'x';\n", 3),
        ("let c = 'é';\n", 4),
        ("let c = '\\n';\n", 4),
        ("let c = '\\'';\n", 4),
        ("let c = '\\\\';\n", 4),
        ("let c = '\\x41';\n", 6),
        ("let c = '\\u{41}';\n", 8),
    ];
    for (src, literal_width) in fixtures {
        let result = sanitize_rust_non_code(src).unwrap();
        let input_lfs: Vec<usize> = src
            .bytes()
            .enumerate()
            .filter_map(|(index, byte)| (byte == b'\n').then_some(index))
            .collect();
        let output_lfs: Vec<usize> = result
            .bytes()
            .enumerate()
            .filter_map(|(index, byte)| (byte == b'\n').then_some(index))
            .collect();

        assert_eq!(result, format!("let c = {};\n", " ".repeat(literal_width)));
        assert_eq!(result.len(), src.len());
        assert_eq!(output_lfs, input_lfs);
    }
}

#[test]
fn character_direct_tab_rejected() {
    let src = "let c = '\t';\n";
    assert!(sanitize_rust_non_code(src).is_err());
}

#[test]
fn character_direct_cr_rejected() {
    let src = "let c = '\r';\n";
    assert!(sanitize_rust_non_code(src).is_err());
}

#[test]
fn character_direct_lf_rejected() {
    let src = "let c = '
';\n";
    assert!(sanitize_rust_non_code(src).is_err());
}

#[test]
fn character_escaped_tab_still_accepted() {
    let src = "let c = '\\t';\n";
    let result = sanitize_rust_non_code(src).unwrap();
    assert_eq!(result.len(), src.len());
}

#[test]
fn character_escaped_cr_still_accepted() {
    let src = "let c = '\\r';\n";
    let result = sanitize_rust_non_code(src).unwrap();
    assert_eq!(result.len(), src.len());
}

#[test]
fn character_hex_escape_valid() {
    let src = "let c = '\\x41';\n";
    let result = sanitize_rust_non_code(src).unwrap();
    assert_eq!(result, "let c =       ;\n");
}

#[test]
fn character_unknown_escape_rejected() {
    assert!(sanitize_rust_non_code("let c = '\\q';\n").is_err());
}

#[test]
fn character_hex_7f_accepted() {
    let src = "let c = '\\x7F';\n";
    let result = sanitize_rust_non_code(src).unwrap();
    assert_eq!(result, "let c =       ;\n");
}

#[test]
fn character_hex_80_rejected() {
    assert!(sanitize_rust_non_code("let c = '\\x80';\n").is_err());
}

#[test]
fn character_hex_ff_rejected() {
    assert!(sanitize_rust_non_code("let c = '\\xFF';\n").is_err());
}

#[test]
fn character_hex_escape_short_rejected() {
    assert!(sanitize_rust_non_code("let c = '\\x4';\n").is_err());
}

#[test]
fn character_hex_escape_nonhex_rejected() {
    assert!(sanitize_rust_non_code("let c = '\\xZZ';\n").is_err());
}

#[test]
fn character_unicode_escape_valid() {
    let src = "let c = '\\u{1F600}';\n";
    let result = sanitize_rust_non_code(src).unwrap();
    assert_eq!(result, "let c =            ;\n");
}

#[test]
fn character_unicode_escape_with_underscores_accepted() {
    for source in ["let c = '\\u{1F_600}';\n", "let c = '\\u{1__F__6_0_0_}';\n"] {
        let result = sanitize_rust_non_code(source).unwrap();
        assert_eq!(result.len(), source.len());
        assert!(result.ends_with(";\n"));
    }
}

#[test]
fn character_unicode_escape_invalid_underscore_forms_rejected() {
    for source in [
        "let c = '\\u{_1F600}';\n",
        "let c = '\\u{___}';\n",
        "let c = '\\u{1_G}';\n",
        "let c = '\\u{1_2_3_4_5_6_7}';\n",
        "let c = '\\u{D8_00}';\n",
        "let c = '\\u{11_0000}';\n",
    ] {
        assert!(
            sanitize_rust_non_code(source).is_err(),
            "invalid underscored character Unicode escape accepted: {source:?}"
        );
    }
}

#[test]
fn character_unicode_scalar_max_accepted() {
    let src = "let c = '\\u{10FFFF}';\n";
    let result = sanitize_rust_non_code(src).unwrap();
    assert_eq!(result, format!("let c = {};\n", " ".repeat(12)));
}

#[test]
fn character_unicode_surrogate_rejected() {
    assert!(sanitize_rust_non_code("let c = '\\u{D800}';\n").is_err());
}

#[test]
fn character_unicode_out_of_range_rejected() {
    assert!(sanitize_rust_non_code("let c = '\\u{110000}';\n").is_err());
}

#[test]
fn character_unicode_escape_empty_rejected() {
    assert!(sanitize_rust_non_code("let c = '\\u{}';\n").is_err());
}

#[test]
fn character_unicode_escape_nonhex_rejected() {
    assert!(sanitize_rust_non_code("let c = '\\u{ZZ}';\n").is_err());
}

#[test]
fn character_unicode_escape_too_long_rejected() {
    assert!(sanitize_rust_non_code("let c = '\\u{1234567}';\n").is_err());
}

#[test]
fn character_unicode_escape_surrogate_rejected() {
    assert!(sanitize_rust_non_code("let c = '\\u{D800}';\n").is_err());
}

#[test]
fn character_unicode_escape_out_of_range_rejected() {
    assert!(sanitize_rust_non_code("let c = '\\u{110000}';\n").is_err());
}

#[test]
fn char_literal_simple_preserved_length() {
    let src = "let c = 'x';\n";
    let result = sanitize_rust_non_code(src).unwrap();
    assert_eq!(result, "let c =    ;\n");
}

#[test]
fn char_literal_unicode_preserved_length() {
    let src = "let c = '\u{e9}';\n";
    let result = sanitize_rust_non_code(src).unwrap();
    assert_eq!(result.len(), src.len());
    assert!(result.contains("let c ="));
}

#[test]
fn char_literal_newline_escape() {
    let src = "let c = '\\n';\n";
    let result = sanitize_rust_non_code(src).unwrap();
    assert_eq!(result, "let c =     ;\n");
}

#[test]
fn char_literal_escaped_quote() {
    let src = "let c = '\\'';\n";
    let result = sanitize_rust_non_code(src).unwrap();
    assert_eq!(result, "let c =     ;\n");
}

#[test]
fn char_literal_escaped_backslash() {
    let src = "let c = '\\\\';\n";
    let result = sanitize_rust_non_code(src).unwrap();
    assert_eq!(result, "let c =     ;\n");
}

#[test]
fn char_literal_hex_escape() {
    let src = "let c = '\\x41';\n";
    let result = sanitize_rust_non_code(src).unwrap();
    // ' \x41 ' = 6 chars → 6 spaces
    assert_eq!(result.len(), src.len());
    assert!(result.starts_with("let c ="));
    assert!(result.ends_with(";\n"));
}

#[test]
fn char_literal_unicode_escape() {
    let src = "let c = '\\u{41}';\n";
    let result = sanitize_rust_non_code(src).unwrap();
    assert_eq!(result.len(), src.len());
    assert!(result.starts_with("let c ="));
    assert!(result.ends_with(";\n"));
}

#[test]
fn char_literal_unicode_multi_digit() {
    let src = "let c = '\\u{1F600}';\n";
    let result = sanitize_rust_non_code(src).unwrap();
    assert_eq!(result.len(), src.len());
    assert!(result.starts_with("let c ="));
    assert!(result.ends_with(";\n"));
}

#[test]
fn char_literal_malformed_unterminated() {
    assert!(sanitize_rust_non_code("let c = '\n").is_err());
}

#[test]
fn char_literal_malformed_backslash_eof() {
    assert!(sanitize_rust_non_code("let c = '\\").is_err());
}

#[test]
fn char_literal_malformed_incomplete_hex() {
    assert!(sanitize_rust_non_code("let c = '\\x4\n").is_err());
}

#[test]
fn char_literal_malformed_empty_unicode() {
    assert!(sanitize_rust_non_code("let c = '\\u{}\n").is_err());
}

#[test]
fn char_literal_malformed_bad_unicode_hex() {
    assert!(sanitize_rust_non_code("let c = '\\u{ZZ}'\n").is_err());
}

#[test]
fn complete_malformed_character_and_byte_escapes_rejected() {
    let fixtures = [
        "let c = '",
        "let c = '\\x'",
        "let c = '\\x4'",
        "let c = '\\xZZ'",
        "let c = '\\u{}'",
        "let c = '\\u{110000}'",
        "let c = '\\u{1234567}'",
        "let b = b'",
        "let b = b'\\x'",
        "let b = b'\\x4'",
        "let b = b'\\xZZ'",
    ];
    for fixture in fixtures {
        assert!(
            sanitize_rust_non_code(fixture).is_err(),
            "malformed literal accepted: {fixture:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// Byte character tests
// ---------------------------------------------------------------------------

#[test]
fn byte_character_literal_fixtures_preserve_length_and_lfs() {
    let fixtures = [
        ("let b = b'x';\n", 4),
        ("let b = b'\\n';\n", 5),
        ("let b = b'\\'';\n", 5),
        ("let b = b'\\\\';\n", 5),
        ("let b = b'\\x41';\n", 7),
    ];
    for (src, literal_width) in fixtures {
        let result = sanitize_rust_non_code(src).unwrap();
        let input_lfs: Vec<usize> = src
            .bytes()
            .enumerate()
            .filter_map(|(index, byte)| (byte == b'\n').then_some(index))
            .collect();
        let output_lfs: Vec<usize> = result
            .bytes()
            .enumerate()
            .filter_map(|(index, byte)| (byte == b'\n').then_some(index))
            .collect();

        assert_eq!(result, format!("let b = {};\n", " ".repeat(literal_width)));
        assert_eq!(result.len(), src.len());
        assert_eq!(output_lfs, input_lfs);
    }
}

#[test]
fn byte_character_hex_escape_valid() {
    let src = "let b = b'\\x41';\n";
    let result = sanitize_rust_non_code(src).unwrap();
    assert_eq!(result, "let b =        ;\n");
}

#[test]
fn byte_character_unknown_escape_rejected() {
    assert!(sanitize_rust_non_code("let b = b'\\q';\n").is_err());
}

#[test]
fn byte_character_hex_00_accepted() {
    let src = "let b = b'\\x00';\n";
    let result = sanitize_rust_non_code(src).unwrap();
    assert_eq!(result, format!("let b = {};\n", " ".repeat(7)));
}

#[test]
fn byte_character_hex_ff_accepted() {
    let src = "let b = b'\\xFF';\n";
    let result = sanitize_rust_non_code(src).unwrap();
    assert_eq!(result, format!("let b = {};\n", " ".repeat(7)));
}

#[test]
fn byte_character_unicode_escape_rejected() {
    assert!(sanitize_rust_non_code("let b = b'\\u{41}';\n").is_err());
}

#[test]
fn byte_character_direct_tab_rejected() {
    let src = "let b = b'\t';\n";
    assert!(sanitize_rust_non_code(src).is_err());
}

#[test]
fn byte_character_direct_cr_rejected() {
    let src = "let b = b'\r';\n";
    assert!(sanitize_rust_non_code(src).is_err());
}

#[test]
fn byte_character_hex_escape_short_rejected() {
    assert!(sanitize_rust_non_code("let b = b'\\x4';\n").is_err());
}

#[test]
fn byte_character_hex_escape_nonhex_rejected() {
    assert!(sanitize_rust_non_code("let b = b'\\xZZ';\n").is_err());
}

#[test]
fn byte_character_non_ascii_rejected() {
    assert!(sanitize_rust_non_code("let b = b'é';\n").is_err());
}

#[test]
fn byte_char_simple() {
    let src = "let b = b'x';\n";
    let result = sanitize_rust_non_code(src).unwrap();
    assert_eq!(result, "let b =     ;\n");
}

#[test]
fn byte_char_newline_escape() {
    let src = "let b = b'\\n';\n";
    let result = sanitize_rust_non_code(src).unwrap();
    assert_eq!(result, "let b =      ;\n");
}

#[test]
fn byte_char_escaped_quote() {
    let src = "let b = b'\\'';\n";
    let result = sanitize_rust_non_code(src).unwrap();
    assert_eq!(result, "let b =      ;\n");
}

#[test]
fn byte_character_unescaped_apostrophe_rejected() {
    assert!(sanitize_rust_non_code("let b = b''';\n").is_err());
}

#[test]
fn byte_char_escaped_backslash() {
    let src = "let b = b'\\\\';\n";
    let result = sanitize_rust_non_code(src).unwrap();
    assert_eq!(result, "let b =      ;\n");
}

#[test]
fn byte_char_hex_escape() {
    let src = "let b = b'\\x41';\n";
    let result = sanitize_rust_non_code(src).unwrap();
    assert_eq!(result.len(), src.len());
    assert!(result.starts_with("let b ="));
    assert!(result.ends_with(";\n"));
}

#[test]
fn byte_char_malformed_unterminated() {
    assert!(sanitize_rust_non_code("let b = b'\n").is_err());
}

// ---------------------------------------------------------------------------
// String continuation LF preservation tests
// ---------------------------------------------------------------------------

#[test]
fn ordinary_string_continuation_preserves_exact_lf_positions() {
    let src = "let s = \"abc\\\nxyz\";\n";
    let result = sanitize_rust_non_code(src).unwrap();
    let input_lfs: Vec<usize> = src
        .bytes()
        .enumerate()
        .filter_map(|(index, byte)| (byte == b'\n').then_some(index))
        .collect();
    let output_lfs: Vec<usize> = result
        .bytes()
        .enumerate()
        .filter_map(|(index, byte)| (byte == b'\n').then_some(index))
        .collect();

    assert_eq!(result.len(), src.len());
    assert_eq!(output_lfs, input_lfs);
    assert_eq!(result, "let s =      \n    ;\n");
}

#[test]
fn byte_string_continuation_preserves_exact_lf_positions() {
    let src = "let b = b\"abc\\\nxyz\";\n";
    let result = sanitize_rust_non_code(src).unwrap();
    let input_lfs: Vec<usize> = src
        .bytes()
        .enumerate()
        .filter_map(|(index, byte)| (byte == b'\n').then_some(index))
        .collect();
    let output_lfs: Vec<usize> = result
        .bytes()
        .enumerate()
        .filter_map(|(index, byte)| (byte == b'\n').then_some(index))
        .collect();

    assert_eq!(result.len(), src.len());
    assert_eq!(output_lfs, input_lfs);
    assert_eq!(result, "let b =       \n    ;\n");
}

#[test]
fn byte_string_continuation_exact_lf_positions_preserved() {
    let src = "let b = b\"abc\\\nxyz\";\n";
    let result = sanitize_rust_non_code(src).unwrap();
    let input_lfs: Vec<usize> = src
        .bytes()
        .enumerate()
        .filter_map(|(index, byte)| (byte == b'\n').then_some(index))
        .collect();
    let output_lfs: Vec<usize> = result
        .bytes()
        .enumerate()
        .filter_map(|(index, byte)| (byte == b'\n').then_some(index))
        .collect();

    assert_eq!(result.len(), src.len());
    assert_eq!(output_lfs, input_lfs);
    assert_eq!(result, "let b =       \n    ;\n");
}

#[test]
fn ordinary_string_continuation_lf_preserved() {
    let src = "let s = \"abc\\\nxyz\";\n";
    let result = sanitize_rust_non_code(src).unwrap();
    assert!(
        result.contains('\n'),
        "LF must be preserved in continuation string"
    );
    assert_eq!(result.len(), src.len());
}

#[test]
fn byte_string_continuation_lf_preserved() {
    let src = "let b = b\"abc\\\nxyz\";\n";
    let result = sanitize_rust_non_code(src).unwrap();
    assert!(
        result.contains('\n'),
        "LF must be preserved in byte continuation string"
    );
    assert_eq!(result.len(), src.len());
}

#[test]
fn byte_string_trailing_backslash_eof_rejected() {
    assert!(sanitize_rust_non_code("let b = b\"abc\\").is_err());
}

#[test]
fn byte_string_invalid_escapes_rejected() {
    for source in [
        "let b = b\"\\q\";\n",
        "let b = b\"\\u{41}\";\n",
        "let b = b\"\\xZ1\";\n",
        "let b = b\"\\x4\";\n",
    ] {
        assert!(
            sanitize_rust_non_code(source).is_err(),
            "invalid byte-string escape accepted: {source:?}"
        );
    }
}

#[test]
fn sanitize_rejects_unterminated_byte_string() {
    assert!(sanitize_rust_non_code("let b = b\"unterminated\n").is_err());
}

#[test]
fn string_trailing_backslash_eof_rejected() {
    assert!(sanitize_rust_non_code("let s = \"abc\\").is_err());
}

#[test]
fn string_escaped_quote_handled() {
    let src = "let s = \"hello\\\"world\";\n";
    let result = sanitize_rust_non_code(src).unwrap();
    assert_eq!(result.len(), src.len());
}

#[test]
fn string_comment_markers_inside_ignored() {
    let src = "let s = \"// not a comment\";\n";
    let result = sanitize_rust_non_code(src).unwrap();
    // The entire string content should be spaces, not a comment
    assert_eq!(result.len(), src.len());
}

#[test]
fn ordinary_string_unicode_still_accepted() {
    let src = "let s = \"é\";\n";
    let result = sanitize_rust_non_code(src).unwrap();
    assert_eq!(result, "let s =     ;\n");
}

#[test]
fn ordinary_string_invalid_escapes_rejected() {
    for source in [
        "let s = \"\\q\";\n",
        "let s = \"\\x4\";\n",
        "let s = \"\\xZZ\";\n",
        "let s = \"\\x80\";\n",
        "let s = \"\\u{}\";\n",
        "let s = \"\\u{ZZ}\";\n",
        "let s = \"\\u{D800}\";\n",
        "let s = \"\\u{110000}\";\n",
        "let s = \"\\u{1234567}\";\n",
    ] {
        assert!(
            sanitize_rust_non_code(source).is_err(),
            "invalid ordinary-string escape accepted: {source:?}"
        );
    }
}

#[test]
fn ordinary_string_valid_escapes_accepted() {
    for source in [
        "let s = \"\\n\\r\\t\\\\\\0\\'\\\"\";\n",
        "let s = \"\\x00\\x7f\";\n",
        "let s = \"\\u{41}\\u{1F_600}\";\n",
    ] {
        let result = sanitize_rust_non_code(source).unwrap();
        assert_eq!(result.len(), source.len());
        assert!(result.ends_with(";\n"));
    }
}

#[test]
fn ordinary_string_isolated_cr_rejected() {
    assert!(sanitize_rust_non_code("let s = \"first\rsecond\";\n").is_err());
    assert!(sanitize_rust_non_code("let s = \"first\\\rsecond\";\n").is_err());
}

#[test]
fn ordinary_string_crlf_is_logical_lf() {
    for source in [
        "let s = \"first\r\nsecond\";\r\n",
        "let s = \"first\\\r\nsecond\";\r\n",
    ] {
        let result = sanitize_rust_non_code(source).unwrap();
        let input_lfs: Vec<usize> = source
            .bytes()
            .enumerate()
            .filter_map(|(index, byte)| (byte == b'\n').then_some(index))
            .collect();
        let output_lfs: Vec<usize> = result
            .bytes()
            .enumerate()
            .filter_map(|(index, byte)| (byte == b'\n').then_some(index))
            .collect();

        assert_eq!(result.len(), source.len());
        assert_eq!(output_lfs, input_lfs);
    }
}

#[test]
fn raw_string_isolated_cr_rejected() {
    assert!(sanitize_rust_non_code("let s = r\"first\rsecond\";\n").is_err());
    assert!(sanitize_rust_non_code("let s = r#\"first\rsecond\"#;\n").is_err());
}

#[test]
fn raw_string_crlf_is_logical_lf() {
    for source in [
        "let s = r\"first\r\nsecond\";\r\n",
        "let s = r#\"first\r\nsecond\"#;\r\n",
    ] {
        let result = sanitize_rust_non_code(source).unwrap();
        let input_lfs: Vec<usize> = source
            .bytes()
            .enumerate()
            .filter_map(|(index, byte)| (byte == b'\n').then_some(index))
            .collect();
        let output_lfs: Vec<usize> = result
            .bytes()
            .enumerate()
            .filter_map(|(index, byte)| (byte == b'\n').then_some(index))
            .collect();

        assert_eq!(result.len(), source.len());
        assert_eq!(output_lfs, input_lfs);
    }
}

#[test]
fn comment_string_markers_inside_ignored() {
    let src = "let x = 1; // has \"quotes\" inside\n";
    let result = sanitize_rust_non_code(src).unwrap();
    // The comment should be spaces, quotes inside comment don't start strings
    assert_eq!(result.len(), src.len());
    assert!(result.contains("let x = 1;"));
}

// ---------------------------------------------------------------------------
// 255-hash raw string tests
// ---------------------------------------------------------------------------

#[test]
fn raw_string_255_hashes() {
    let hashes = "#".repeat(255);
    let src = format!("let s = r{h}\"hello\"{h};\n", h = hashes);
    let result = sanitize_rust_non_code(&src).unwrap();
    assert_eq!(result.len(), src.len());
    // The content should be spaces except for newlines
    assert!(result.ends_with(";\n"));
}

#[test]
fn raw_byte_string_255_hashes() {
    let hashes = "#".repeat(255);
    let src = format!("let b = br{h}\"hello\"{h};\n", h = hashes);
    let result = sanitize_rust_non_code(&src).unwrap();
    assert_eq!(result.len(), src.len());
    assert!(result.ends_with(";\n"));
}

#[test]
fn raw_byte_string_non_ascii_rejected() {
    let src = "let b = br\"é\";\n";
    assert!(sanitize_rust_non_code(src).is_err());
}

#[test]
fn raw_byte_string_cr_rejected() {
    let src = "let b = br\"abc\rdef\";\n";
    assert!(sanitize_rust_non_code(src).is_err());
}

#[test]
fn raw_byte_string_crlf_accepted_as_logical_lf() {
    let src = "let value = br\"first\r\nsecond\";\n";
    let result = sanitize_rust_non_code(src).unwrap();
    assert_eq!(result.len(), src.len());
}

#[test]
fn raw_byte_string_crlf_exact_positions() {
    let src = "let value = br\"first\r\nsecond\";\n";
    let result = sanitize_rust_non_code(src).unwrap();
    let input_lfs: Vec<usize> = src
        .bytes()
        .enumerate()
        .filter_map(|(index, byte)| (byte == b'\n').then_some(index))
        .collect();
    let output_lfs: Vec<usize> = result
        .bytes()
        .enumerate()
        .filter_map(|(index, byte)| (byte == b'\n').then_some(index))
        .collect();
    let cr_index = src.bytes().position(|byte| byte == b'\r').unwrap();

    assert_eq!(result.len(), src.len());
    assert_eq!(output_lfs, input_lfs);
    assert_eq!(result.as_bytes()[cr_index], b' ');
}

#[test]
fn raw_byte_string_isolated_cr_rejected() {
    let src = "let value = br\"first\rsecond\";\n";
    assert!(sanitize_rust_non_code(src).is_err());
}

#[test]
fn raw_byte_string_hash_crlf_accepted() {
    let src = "let value = br#\"first\r\nsecond\"#;\n";
    let result = sanitize_rust_non_code(src).unwrap();
    assert_eq!(result.len(), src.len());
}

#[test]
fn raw_byte_string_255_hash_crlf_accepted() {
    let hashes = "#".repeat(255);
    let src = format!("let value = br{h}\"first\r\nsecond\"{h};\n", h = hashes);
    let result = sanitize_rust_non_code(&src).unwrap();
    let input_lfs: Vec<usize> = src
        .bytes()
        .enumerate()
        .filter_map(|(index, byte)| (byte == b'\n').then_some(index))
        .collect();
    let output_lfs: Vec<usize> = result
        .bytes()
        .enumerate()
        .filter_map(|(index, byte)| (byte == b'\n').then_some(index))
        .collect();

    assert_eq!(result.len(), src.len());
    assert_eq!(output_lfs, input_lfs);
}

#[test]
fn raw_byte_string_lf_preserved() {
    let src = "let b = br\"abc\ndef\";\n";
    let result = sanitize_rust_non_code(src).unwrap();
    let input_lfs: Vec<usize> = src
        .bytes()
        .enumerate()
        .filter_map(|(index, byte)| (byte == b'\n').then_some(index))
        .collect();
    let output_lfs: Vec<usize> = result
        .bytes()
        .enumerate()
        .filter_map(|(index, byte)| (byte == b'\n').then_some(index))
        .collect();

    assert_eq!(output_lfs, input_lfs);
    assert_eq!(result, "let b =       \n    ;\n");
}

#[test]
fn raw_byte_string_ascii_content_accepted() {
    let src = "let b = br\"abc\";\n";
    let result = sanitize_rust_non_code(src).unwrap();
    assert_eq!(result, format!("let b = {};\n", " ".repeat(7)));
}

#[test]
fn raw_byte_string_255_hashes_still_accepted() {
    let hashes = "#".repeat(255);
    let src = format!("let b = br{h}\"hello\"{h};\n", h = hashes);
    let result = sanitize_rust_non_code(&src).unwrap();
    assert_eq!(result.len(), src.len());
    assert!(result.ends_with(";\n"));
}

#[test]
fn raw_hashes_above_255_rejected() {
    let hashes = "#".repeat(256);
    let sources = [
        format!("let s = r{h}\"hello\"{h};\n", h = hashes),
        format!("let b = br{h}\"hello\"{h};\n", h = hashes),
    ];
    for src in sources {
        assert!(sanitize_rust_non_code(&src).is_err());
    }
}

#[test]
fn raw_string_mismatched_hashes_rejected() {
    // r##"..."# should fail (2 opening hashes, 1 closing hash)
    assert!(sanitize_rust_non_code("let s = r##\"hello\"#;\n").is_err());
}

#[test]
fn raw_string_unterminated_rejected() {
    assert!(sanitize_rust_non_code("let s = r\"unterminated\n").is_err());
}

#[test]
fn raw_byte_string_unterminated_rejected() {
    assert!(sanitize_rust_non_code("let b = br\"unterminated\n").is_err());
}

// ---------------------------------------------------------------------------
// Attribute whitespace variant tests
// ---------------------------------------------------------------------------

#[test]
fn attribute_exact_test_with_spaces() {
    let src = "# [test]\nfn my_test() {}\n";
    assert!(verify_test_function_in_source(src, "my_test").is_ok());
}

#[test]
fn attribute_exact_test_with_inner_spaces() {
    let src = "# [ test ]\nfn my_test() {}\n";
    assert!(verify_test_function_in_source(src, "my_test").is_ok());
}

#[test]
fn attribute_multiline_whitespace_test() {
    // Valid Rust: # [ test ] with spaces (newline between # and [ is not valid Rust)
    let src = "# [ test ]\nfn my_test() {}\n";
    assert!(verify_test_function_in_source(src, "my_test").is_ok());
}

#[test]
fn attribute_cfg_plus_test_coexist() {
    let src = "#[cfg(test)]\n#[test]\nfn my_test() {}\n";
    assert!(verify_test_function_in_source(src, "my_test").is_ok());
}

#[test]
fn attribute_ignore_plus_test_coexist() {
    let src = "#[ignore]\n#[test]\nfn my_test() {}\n";
    assert!(verify_test_function_in_source(src, "my_test").is_ok());
}

#[test]
fn attribute_test_case_rejected() {
    let src = "#[test_case]\nfn my_test() {}\n";
    assert!(verify_test_function_in_source(src, "my_test").is_err());
}

#[test]
fn attribute_test_paren_rejected() {
    let src = "#[test()]\nfn my_test() {}\n";
    assert!(verify_test_function_in_source(src, "my_test").is_err());
}

#[test]
fn attribute_test_equals_rejected() {
    let src = "#[test = \"x\"]\nfn my_test() {}\n";
    assert!(verify_test_function_in_source(src, "my_test").is_err());
}

#[test]
fn attribute_test_extra_rejected() {
    let src = "#[test_extra]\nfn my_test() {}\n";
    assert!(verify_test_function_in_source(src, "my_test").is_err());
}

#[test]
fn attribute_cfg_test_without_test_attr_rejected() {
    let src = "#[cfg(test)]\nfn my_helper() {}\n";
    assert!(verify_test_function_in_source(src, "my_helper").is_err());
}

#[test]
fn raw_string_fake_attribute_before_helper_rejected() {
    let src = "let s = r\"#[test]\";\nfn helper() {}\n";
    assert!(verify_test_function_in_source(src, "helper").is_err());
}

#[test]
fn raw_byte_string_fake_attribute_before_helper_rejected() {
    let src = "let b = br\"#[test]\";\nfn helper() {}\n";
    assert!(verify_test_function_in_source(src, "helper").is_err());
}

#[test]
fn byte_string_fake_attribute_and_declaration_rejected() {
    let attribute_src = "let b = b\"#[test]\";\nfn helper() {}\n";
    assert!(verify_test_function_in_source(attribute_src, "helper").is_err());

    let declaration_src = "let b = b\"fn my_test() {}\";\n#[test]\nfn real_test() {}\n";
    assert!(verify_test_function_in_source(declaration_src, "my_test").is_err());
}

#[test]
fn block_comment_fake_attribute_rejected() {
    let src = "/* #[test] */\nfn helper() {}\n";
    assert!(verify_test_function_in_source(src, "helper").is_err());
}

#[test]
fn string_fake_attribute_rejected() {
    let src = "let s = \"#[test]\";\nfn helper() {}\n";
    assert!(verify_test_function_in_source(src, "helper").is_err());
}

#[test]
fn comment_separated_fake_attribute_rejected() {
    let src = "// #[test]\nfn helper() {}\n";
    assert!(verify_test_function_in_source(src, "helper").is_err());
}

// ---------------------------------------------------------------------------
// Discovery trailing whitespace / CRLF tests
// ---------------------------------------------------------------------------

#[test]
fn discovery_trailing_space_after_test_rejected() {
    assert!(parse_discovery_stdout(b"name: test \n").is_err());
}

#[test]
fn discovery_trailing_tab_after_test_rejected() {
    assert!(parse_discovery_stdout(b"name: test\t\n").is_err());
}

#[test]
fn discovery_trailing_mixed_whitespace_rejected() {
    assert!(parse_discovery_stdout(b"name: test  \t \n").is_err());
}

#[test]
fn discovery_crlf_lines_handled() {
    // CRLF line endings should be handled by stripping \r
    let set = parse_discovery_stdout(b"name: test\r\n").unwrap();
    assert!(set.contains("name"));
}

#[test]
fn discovery_bare_trailing_carriage_return_rejected() {
    assert!(parse_discovery_stdout(b"name: test\r").is_err());
}

#[test]
fn discovery_extra_carriage_return_before_crlf_rejected() {
    assert!(parse_discovery_stdout(b"name: test\r\r\n").is_err());
}

// ---------------------------------------------------------------------------
// Source recognition tests (use sanitizer)
// ---------------------------------------------------------------------------

#[test]
fn declaration_tab_whitespace_accepted() {
    let src = "#[test]\nfn\tmy_test\t() {}\n";
    assert!(verify_test_function_in_source(src, "my_test").is_ok());
}

#[test]
fn declaration_newline_whitespace_accepted() {
    let src = "#[test]\nfn\nmy_test() {}\n";
    assert!(verify_test_function_in_source(src, "my_test").is_ok());
}

#[test]
fn declaration_pub_async_multiline_whitespace_accepted() {
    let src = "#[test]\npub\nasync\nfn\nmy_test\n() {}\n";
    assert!(verify_test_function_in_source(src, "my_test").is_ok());
}

#[test]
fn declaration_all_qualifier_forms_accepted() {
    let fixtures = [
        "#[test]\nfn my_test() {}\n",
        "#[test]\npub fn my_test() {}\n",
        "#[test]\nasync fn my_test() {}\n",
        "#[test]\npub async fn my_test() {}\n",
    ];
    for source in fixtures {
        assert!(verify_test_function_in_source(source, "my_test").is_ok());
    }
}

#[test]
fn attribute_hash_bracket_multiline_whitespace_accepted() {
    let src = "#\n[\n    test\n]\nfn my_test() {}\n";
    assert!(verify_test_function_in_source(src, "my_test").is_ok());
}

#[test]
fn attribute_cfg_ignore_test_with_pub_async_accepted() {
    let src = "#[cfg(windows)]\n#[ignore]\n#[test]\npub async fn my_test() {}\n";
    assert!(verify_test_function_in_source(src, "my_test").is_ok());
}

#[test]
fn accept_ordinary_test_attribute() {
    let src = "#[test]\nfn my_test() {}\n";
    assert!(verify_test_function_in_source(src, "my_test").is_ok());
}

#[test]
fn accept_cfg_plus_exact_test() {
    let src = "#[cfg(test)]\n#[test]\nfn my_test() {}\n";
    assert!(verify_test_function_in_source(src, "my_test").is_ok());
}

#[test]
fn accept_fully_qualified_unit_test_by_final_component() {
    let src = "#[test]\nfn index_stage_z_parses_stage0() {}\n";
    let sanitized = sanitize_rust_non_code(src).unwrap();
    eprintln!("Sanitized: {:?}", sanitized);
    let result = verify_test_function_in_source(src, "part1_tests::index_stage_z_parses_stage0");
    eprintln!("Result: {:?}", result);
    assert!(result.is_ok());
}

#[test]
fn reject_helper_without_test_attribute() {
    let src = "fn helper() {}\n";
    assert!(verify_test_function_in_source(src, "helper").is_err());
}

#[test]
fn reject_missing_function() {
    let src = "#[test]\nfn other_test() {}\n";
    assert!(verify_test_function_in_source(src, "missing").is_err());
}

#[test]
fn reject_prefix_only_function() {
    let src = "#[test]\nfn test_name_extra() {}\n";
    assert!(verify_test_function_in_source(src, "test_name").is_err());
}

#[test]
fn reject_duplicate_function() {
    let src = "#[test]\nfn dup_test() {}\n#[test]\nfn dup_test() {}\n";
    assert!(verify_test_function_in_source(src, "dup_test").is_err());
}

#[test]
fn reject_test_case_attribute() {
    let src = "#[test_case(1)]\nfn my_test() {}\n";
    assert!(verify_test_function_in_source(src, "my_test").is_err());
}

#[test]
fn reject_test_paren_attribute() {
    let src = "#[test()]\nfn my_test() {}\n";
    assert!(verify_test_function_in_source(src, "my_test").is_err());
}

#[test]
fn reject_test_equals_attribute() {
    let src = "#[test = \"x\"]\nfn my_test() {}\n";
    assert!(verify_test_function_in_source(src, "my_test").is_err());
}

#[test]
fn reject_function_inside_line_comment() {
    let src = "// fn my_test() {}\n#[test]\nfn real_test() {}\n";
    assert!(verify_test_function_in_source(src, "my_test").is_err());
}

#[test]
fn reject_function_inside_nested_block_comment() {
    let src = "/* nested /* fn my_test() {} */ code */\n#[test]\nfn real_test() {}\n";
    assert!(verify_test_function_in_source(src, "my_test").is_err());
}

#[test]
fn reject_function_inside_ordinary_multiline_string() {
    let src = "let s = \"fn my_test() {}\";\n#[test]\nfn real_test() {}\n";
    assert!(verify_test_function_in_source(src, "my_test").is_err());
}

#[test]
fn reject_function_inside_raw_string() {
    let src = "let s = r\"fn my_test() {}\";\n#[test]\nfn real_test() {}\n";
    assert!(verify_test_function_in_source(src, "my_test").is_err());
}

#[test]
fn reject_function_inside_raw_byte_string() {
    let src = "let s = br\"fn my_test() {}\";\n#[test]\nfn real_test() {}\n";
    assert!(verify_test_function_in_source(src, "my_test").is_err());
}

#[test]
fn reject_fake_test_attribute_inside_block_comment() {
    let src = "/* #[test] */ fn my_test() {}\n";
    assert!(verify_test_function_in_source(src, "my_test").is_err());
}

#[test]
fn reject_fake_test_attribute_inside_raw_string() {
    let src = "let s = r\"#[test]\"; fn my_test() {}\n";
    assert!(verify_test_function_in_source(src, "my_test").is_err());
}

#[test]
fn reject_source_function_mismatch() {
    let discovery = build_discovery_sets();
    let mut evidence = good_evidence();
    evidence.source_anchor = "tests/integration.rs:test_implementation_check_after_begin".into();
    assert!(validate_evidence_source(&evidence, &discovery).is_err());
}

// ---------------------------------------------------------------------------
// Discovery parser tests
// ---------------------------------------------------------------------------

#[test]
fn parse_exact_name_test_accepted() {
    let set = parse_discovery_stdout(b"test_impl_begin: test\n").unwrap();
    assert!(set.contains("test_impl_begin"));
}

#[test]
fn parse_leading_whitespace_in_name_rejected() {
    assert!(parse_discovery_stdout(b" test_impl_begin: test\n").is_err());
}

#[test]
fn parse_whitespace_before_suffix_rejected() {
    assert!(parse_discovery_stdout(b"test_impl_begin : test\n").is_err());
}

#[test]
fn parse_empty_name_rejected() {
    assert!(parse_discovery_stdout(b": test\n").is_err());
}

#[test]
fn parse_non_test_line_ignored() {
    let set = parse_discovery_stdout(b"\n42 tests, 0 benchmarks\n").unwrap();
    assert!(set.is_empty());
}

#[test]
fn parse_fully_qualified_unit_test_name() {
    let set =
        parse_discovery_stdout(b"implementation::part1_tests::index_stage_z_parses_stage0: test\n")
            .unwrap();
    assert!(set.contains("implementation::part1_tests::index_stage_z_parses_stage0"));
}

#[test]
fn parse_duplicate_name_rejected() {
    assert!(parse_discovery_stdout(b"name: test\nname: test\n").is_err());
}

#[test]
fn parse_invalid_utf8_rejected() {
    assert!(parse_discovery_stdout(&[0xFF, 0xFE]).is_err());
}

// ---------------------------------------------------------------------------
// Target reconciliation tests
// ---------------------------------------------------------------------------

#[test]
fn accept_real_integration_test_under_integration() {
    let discovery = build_discovery_sets();
    let evidence = Evidence {
        cargo_target_kind: "integration".to_string(),
        cargo_target: "integration".to_string(),
        fully_qualified_test: "test_impl_begin_records_exact_sparse_state_commands".to_string(),
        scenario: "begin recording sparse commands".to_string(),
        assertion_semantics: "proves exact sparse state commands".to_string(),
        platform_scope: "all".to_string(),
        source_anchor: "tests/integration.rs:test_impl_begin_records_exact_sparse_state_commands"
            .to_string(),
    };
    let result = validate_evidence_source(&evidence, &discovery);
    eprintln!("Result: {:?}", result);
    assert!(result.is_ok());
}

#[test]
fn reject_integration_test_as_unit() {
    let discovery = build_discovery_sets();
    let evidence = Evidence {
        cargo_target_kind: "unit".to_string(),
        cargo_target: "mrgs".to_string(),
        fully_qualified_test: "test_implementation_begin_idempotent".to_string(),
        scenario: "wrong target test".to_string(),
        assertion_semantics: "rejected".to_string(),
        platform_scope: "all".to_string(),
        source_anchor: "src/implementation.rs:test_implementation_begin_idempotent".to_string(),
    };
    assert!(validate_evidence_source(&evidence, &discovery).is_err());
}

#[test]
fn reject_unit_test_as_integration() {
    let discovery = build_discovery_sets();
    let evidence = Evidence {
        cargo_target_kind: "integration".to_string(),
        cargo_target: "integration".to_string(),
        fully_qualified_test: "implementation::part1_tests::index_stage_z_parses_stage0"
            .to_string(),
        scenario: "wrong target test".to_string(),
        assertion_semantics: "rejected".to_string(),
        platform_scope: "all".to_string(),
        source_anchor:
            "tests/integration.rs:implementation::part1_tests::index_stage_z_parses_stage0"
                .to_string(),
    };
    assert!(validate_evidence_source(&evidence, &discovery).is_err());
}

#[test]
fn reject_real_source_nonexistent_discovered_name() {
    let discovery = build_discovery_sets();
    let mut evidence = good_evidence();
    evidence.fully_qualified_test = "test_implementation_begin_valid_nonexistent".to_string();
    evidence.source_anchor =
        "tests/integration.rs:test_implementation_begin_valid_nonexistent".to_string();
    assert!(validate_evidence_source(&evidence, &discovery).is_err());
}

#[test]
fn accept_real_integration_test_check_records() {
    let discovery = build_discovery_sets();
    let evidence = Evidence {
        cargo_target_kind: "integration".to_string(),
        cargo_target: "integration".to_string(),
        fully_qualified_test: "test_impl_check_records_exact_sparse_state_commands".to_string(),
        scenario: "check recording sparse commands".to_string(),
        assertion_semantics: "proves exact sparse state commands".to_string(),
        platform_scope: "all".to_string(),
        source_anchor: "tests/integration.rs:test_impl_check_records_exact_sparse_state_commands"
            .to_string(),
    };
    let result = validate_evidence_source(&evidence, &discovery);
    eprintln!("Result: {:?}", result);
    assert!(result.is_ok());
}

#[test]
fn accept_real_unit_test_index_stage_z_parses() {
    let discovery = build_discovery_sets();
    let evidence = Evidence {
        cargo_target_kind: "unit".to_string(),
        cargo_target: "mrgs".to_string(),
        fully_qualified_test: "implementation::part1_tests::index_stage_z_parses_stage0"
            .to_string(),
        scenario: "index stage parsing".to_string(),
        assertion_semantics: "stage 0 record parsed correctly".to_string(),
        platform_scope: "all".to_string(),
        source_anchor: "src/implementation.rs:part1_tests::index_stage_z_parses_stage0".to_string(),
    };
    let result = validate_evidence_source(&evidence, &discovery);
    eprintln!("Result: {:?}", result);
    assert!(result.is_ok());
}

#[test]
fn reject_p2d_meta_test_target() {
    assert!(validate_cargo_target(SELF_TEST_TARGET).is_err());
}

#[test]
fn reject_p2d_meta_target_in_source_validation() {
    let evidence = Evidence {
        cargo_target_kind: "unit".to_string(),
        cargo_target: SELF_TEST_TARGET.to_string(),
        fully_qualified_test: "zero_evidence_registry_validation_is_discovery_free".to_string(),
        scenario: "meta target rejection".to_string(),
        assertion_semantics: "must reject the framework target".to_string(),
        platform_scope: "all".to_string(),
        source_anchor:
            "tests/phase4_obligations.rs:zero_evidence_registry_validation_is_discovery_free"
                .to_string(),
    };
    assert!(validate_evidence(&evidence).is_err());
    let discovery = DiscoverySets {
        unit: BTreeSet::new(),
        integration: BTreeSet::new(),
    };
    assert!(validate_evidence_source(&evidence, &discovery).is_err());
}
