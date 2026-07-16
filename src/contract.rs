use serde::Deserialize;
use std::collections::HashSet;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Contract {
    pub schema_version: u32,
    pub contract_id: String,
    pub phase_id: String,
    pub title: String,
    pub objective: String,
    pub requirements: Vec<String>,
    pub allowed_paths: Vec<String>,
    pub forbidden_paths: Vec<String>,
    pub verification_commands: Vec<String>,
    pub handoff_fields: Vec<String>,
}

impl Contract {
    pub fn validate(&self) -> Result<(), crate::error::Error> {
        if self.schema_version != 1 {
            return Err(crate::error::Error::UnsupportedContractSchema(
                self.schema_version,
            ));
        }
        validate_not_blank(&self.contract_id, "contract_id")?;
        validate_not_blank(&self.phase_id, "phase_id")?;
        validate_not_blank(&self.title, "title")?;
        validate_not_blank(&self.objective, "objective")?;
        validate_list(&self.requirements, "requirements")?;
        validate_list(&self.allowed_paths, "allowed_paths")?;
        validate_list(&self.forbidden_paths, "forbidden_paths")?;
        validate_list(&self.verification_commands, "verification_commands")?;
        validate_list(&self.handoff_fields, "handoff_fields")?;
        Ok(())
    }
}

fn validate_not_blank(s: &str, field: &str) -> Result<(), crate::error::Error> {
    let trimmed = s.trim();
    if trimmed.is_empty() || s != trimmed {
        return Err(crate::error::Error::EmptyContractField(field.to_string()));
    }
    Ok(())
}

fn validate_list(list: &[String], name: &str) -> Result<(), crate::error::Error> {
    if list.is_empty() {
        return Err(crate::error::Error::EmptyContractList(name.to_string()));
    }
    let mut seen = HashSet::new();
    for entry in list {
        if entry.trim().is_empty() {
            return Err(crate::error::Error::EmptyContractListEntry(
                name.to_string(),
            ));
        }
        if !seen.insert(entry) {
            return Err(crate::error::Error::DuplicateContractListEntry(
                name.to_string(),
            ));
        }
    }
    Ok(())
}
