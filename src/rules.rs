use std::collections::HashSet;

#[derive(Debug, Clone)]
pub struct PathRuleSet {
    pub allowed: Vec<String>,
    pub forbidden: Vec<String>,
}

impl PathRuleSet {
    pub fn from_contract(
        contract: &crate::contract::Contract,
    ) -> Result<Self, crate::error::Error> {
        let mut seen_allowed = HashSet::new();
        let mut validated_allowed = Vec::new();
        for rule in &contract.allowed_paths {
            validate_rule(rule)?;
            if is_reserved_scope(rule) {
                return Err(crate::error::Error::ContractPathRuleInvalid);
            }
            if !seen_allowed.insert(rule.clone()) {
                return Err(crate::error::Error::ContractPathRuleInvalid);
            }
            validated_allowed.push(rule.clone());
        }

        let mut seen_forbidden = HashSet::new();
        let mut validated_forbidden = Vec::new();
        for rule in &contract.forbidden_paths {
            validate_rule(rule)?;
            if !seen_forbidden.insert(rule.clone()) {
                return Err(crate::error::Error::ContractPathRuleInvalid);
            }
            validated_forbidden.push(rule.clone());
        }

        Ok(PathRuleSet {
            allowed: validated_allowed,
            forbidden: validated_forbidden,
        })
    }

    pub fn evaluate(&self, path: &str) -> Result<(), crate::error::Error> {
        let forbidden = self
            .forbidden
            .iter()
            .any(|rule| matches_forbidden(path, rule));
        if forbidden {
            return Err(crate::error::Error::ChangeForbidden);
        }
        let allowed = self.allowed.iter().any(|rule| matches_allowed(path, rule));
        if !allowed {
            return Err(crate::error::Error::ChangeNotAllowed);
        }
        Ok(())
    }
}

pub fn matches_allowed(path: &str, rule: &str) -> bool {
    if let Some(prefix) = rule.strip_suffix('/') {
        path.starts_with(prefix)
            && (path.len() == prefix.len() || path.as_bytes().get(prefix.len()) == Some(&b'/'))
    } else {
        path == rule
    }
}

fn ascii_lowercase(s: &str) -> Vec<u8> {
    s.bytes()
        .map(|b| if b.is_ascii_uppercase() { b + 32 } else { b })
        .collect()
}

pub fn matches_forbidden(path: &str, rule: &str) -> bool {
    if matches_allowed(path, rule) {
        return true;
    }
    if let Some(prefix) = rule.strip_suffix('/') {
        let lower_path = ascii_lowercase(path);
        let lower_prefix = ascii_lowercase(prefix);
        lower_path.starts_with(&lower_prefix)
            && (lower_path.len() == lower_prefix.len()
                || lower_path.get(lower_prefix.len()) == Some(&b'/'))
    } else {
        path.len() == rule.len()
            && path.bytes().zip(rule.bytes()).all(|(a, b)| {
                a == b || (a.is_ascii_alphabetic() && b.is_ascii_alphabetic() && a | 32 == b | 32)
            })
    }
}

pub fn validate_rule(rule: &str) -> Result<(), crate::error::Error> {
    if rule.is_empty() {
        return Err(crate::error::Error::ContractPathRuleInvalid);
    }
    if rule.starts_with('/') || rule.starts_with("//") {
        return Err(crate::error::Error::ContractPathRuleInvalid);
    }
    if let Some(c) = rule.chars().next() {
        if c.is_ascii_alphabetic() && rule.len() > 1 && rule.as_bytes()[1] == b':' {
            return Err(crate::error::Error::ContractPathRuleInvalid);
        }
    }
    if rule.contains('\\') {
        return Err(crate::error::Error::ContractPathRuleInvalid);
    }
    let normalized = rule.strip_suffix('/').unwrap_or(rule);
    if normalized.is_empty() {
        return Err(crate::error::Error::ContractPathRuleInvalid);
    }
    for seg in normalized.split('/') {
        if seg.is_empty() || seg == "." || seg == ".." {
            return Err(crate::error::Error::ContractPathRuleInvalid);
        }
    }
    if rule
        .chars()
        .any(|c| c as u32 == 0 || (c as u32 > 0 && (c as u32) < 32) || c as u32 == 127)
    {
        return Err(crate::error::Error::ContractPathRuleInvalid);
    }
    if rule.contains('*') || rule.contains('?') || rule.contains('[') || rule.contains(']') {
        return Err(crate::error::Error::ContractPathRuleInvalid);
    }
    if rule.starts_with("./") || rule.contains("//") {
        return Err(crate::error::Error::ContractPathRuleInvalid);
    }
    Ok(())
}

pub fn is_reserved_scope(rule: &str) -> bool {
    let first_seg = rule.split('/').next().unwrap_or("");
    first_seg.eq_ignore_ascii_case(".git") || first_seg.eq_ignore_ascii_case(".mrgs")
}

pub fn is_reserved_path(path: &str) -> bool {
    let first_seg = path.split('/').next().unwrap_or("");
    first_seg.eq_ignore_ascii_case(".git") || first_seg.eq_ignore_ascii_case(".mrgs")
}

pub fn validate_changed_path(path: &str) -> Result<(), crate::error::Error> {
    if path.is_empty() {
        return Err(crate::error::Error::ChangePathInvalid);
    }
    if path.starts_with('/') || path.starts_with("//") {
        return Err(crate::error::Error::ChangePathInvalid);
    }
    if let Some(c) = path.chars().next() {
        if c.is_ascii_alphabetic() && path.len() > 1 && path.as_bytes()[1] == b':' {
            return Err(crate::error::Error::ChangePathInvalid);
        }
    }
    if path.contains('\\') {
        return Err(crate::error::Error::ChangePathInvalid);
    }
    for seg in path.split('/') {
        if seg == "." || seg == ".." || seg.is_empty() {
            return Err(crate::error::Error::ChangePathInvalid);
        }
    }
    if path
        .chars()
        .any(|c| c as u32 == 0 || (c as u32 > 0 && (c as u32) < 32) || c as u32 == 127)
    {
        return Err(crate::error::Error::ChangePathInvalid);
    }
    if path.contains("//") {
        return Err(crate::error::Error::ChangePathInvalid);
    }
    if path.ends_with('/') {
        return Err(crate::error::Error::ChangePathInvalid);
    }
    if is_reserved_path(path) {
        return Err(crate::error::Error::ChangeForbidden);
    }
    Ok(())
}
