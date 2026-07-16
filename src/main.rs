mod cli;
mod contract;
mod error;
mod path;
mod plan;
mod state;

use clap::Parser;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

fn main() {
    let cli = cli::Cli::parse();

    let result = match cli.command {
        cli::CliCommand::Plan(sub) => match sub.action {
            cli::PlanAction::Accept { repo, plan } => cmd_plan_accept(&repo, &plan),
        },
        cli::CliCommand::Phase(sub) => match sub.action {
            cli::PhaseAction::Select { repo, phase } => cmd_phase_select(&repo, &phase),
        },
        cli::CliCommand::Contract(sub) => match sub.action {
            cli::ContractAction::Draft { repo, contract } => cmd_contract_draft(&repo, &contract),
            cli::ContractAction::Accept {
                repo,
                revision,
                sha256,
                decision,
            } => cmd_contract_accept(&repo, revision, &sha256, &decision),
            cli::ContractAction::Revise {
                repo,
                contract,
                expected_revision,
                expected_sha256,
            } => cmd_contract_revise(&repo, &contract, expected_revision, &expected_sha256),
        },
    };

    match result {
        Ok(output) => {
            println!("{}", output);
            std::process::exit(0);
        }
        Err(e) => {
            eprintln!("error: {}", e);
            std::process::exit(1);
        }
    }
}

fn cmd_plan_accept(repo_arg: &str, plan_arg: &str) -> Result<String, error::Error> {
    let repo_path = Path::new(repo_arg);
    path::assert_existing_dir(repo_path)?;
    let repo = std::fs::canonicalize(repo_path)?;

    let plan_path = Path::new(plan_arg);
    path::assert_existing_file(plan_path)?;
    let plan = std::fs::canonicalize(plan_path)?;

    if !path::plan_is_inside_repo(&plan, &repo) {
        return Err(error::Error::PlanOutsideRepo(plan_arg.into()));
    }

    let plan_bytes = std::fs::read(&plan)?;

    let mut hasher = Sha256::new();
    hasher.update(&plan_bytes);
    let sha256 = format!("{:x}", hasher.finalize());

    let plan_str = String::from_utf8(plan_bytes)?;
    let parsed: plan::Plan = toml::from_str(&plan_str)?;
    parsed.validate()?;

    let gov_dir = match path::validate_gov_dir_exists(&repo) {
        Ok(gov_dir) => Some(gov_dir),
        Err(error::Error::GovDirNotExists(_)) => None,
        Err(error) => return Err(error),
    };
    if let Some(gov_dir) = gov_dir {
        let accepted_path = gov_dir.join("accepted-plan.json");
        let state_path = gov_dir.join("state.json");
        let accepted_exists = accepted_path.exists();
        let state_exists = state_path.exists();

        if accepted_exists != state_exists {
            return Err(error::Error::IncompleteGovernanceAuthority(gov_dir));
        }
        if accepted_exists {
            return validate_existing_authority(&gov_dir, &sha256);
        }
    }

    let gov_dir = path::validate_gov_dir(&repo)?;

    let relative_path = path::relative_plan_path(&plan, &repo);
    let plan_path_str = relative_path.to_string_lossy().replace('\\', "/");

    let accepted = state::AcceptedPlan {
        schema_version: 1,
        plan_id: parsed.plan_id.clone(),
        plan_path: plan_path_str,
        sha256: sha256.clone(),
        phase_count: parsed.phases.len(),
    };

    let gov_state = state::GovernanceState {
        schema_version: 1,
        accepted_plan_sha256: sha256.clone(),
        active_phase: None,
        closed_phases: vec![],
    };

    state::atomic_write_json(&gov_dir, "accepted-plan.json", &accepted)?;
    state::atomic_write_json(&gov_dir, "state.json", &gov_state)?;

    Ok(format!("{} {}", parsed.plan_id, sha256))
}

fn validate_existing_authority(
    gov_dir: &Path,
    submitted_sha256: &str,
) -> Result<String, error::Error> {
    let existing_accepted: state::AcceptedPlan =
        serde_json::from_slice(&std::fs::read(gov_dir.join("accepted-plan.json"))?)?;
    let existing_state: state::GovernanceState =
        serde_json::from_slice(&std::fs::read(gov_dir.join("state.json"))?)?;

    state::validate_accepted_plan_record(&existing_accepted)?;
    let repo = gov_dir
        .parent()
        .ok_or_else(|| error::Error::GovDirEscape(gov_dir.to_path_buf()))?;
    let recorded_plan_path = path::resolve_safe_plan_path(repo, &existing_accepted.plan_path)?;
    let recorded_plan_bytes = std::fs::read(&recorded_plan_path)?;
    let recorded_plan_str = String::from_utf8(recorded_plan_bytes.clone())?;
    let recorded_plan: plan::Plan = toml::from_str(&recorded_plan_str)?;
    recorded_plan.validate()?;

    let mut hasher = Sha256::new();
    hasher.update(&recorded_plan_bytes);
    let recorded_sha256 = format!("{:x}", hasher.finalize());

    state::validate_plan_consistency(&existing_accepted, &recorded_plan, &recorded_sha256)?;
    state::validate_state_record(&existing_state, &existing_accepted, &recorded_plan)?;

    if existing_accepted.sha256 != submitted_sha256 {
        return Err(error::Error::AcceptedPlanMismatch);
    }

    Ok(format!("{} {}", recorded_plan.plan_id, submitted_sha256))
}

fn cmd_phase_select(repo_arg: &str, phase_id: &str) -> Result<String, error::Error> {
    let repo_path = Path::new(repo_arg);
    path::assert_existing_dir(repo_path)?;
    let repo = std::fs::canonicalize(repo_path)?;

    let gov_dir = path::validate_gov_dir_exists(&repo)?;

    let accepted = state::read_accepted_plan(&repo)?;
    let mut gov_state = state::read_state(&repo)?;

    let plan_file = path::resolve_safe_plan_path(&repo, &accepted.plan_path)?;

    let plan_bytes = std::fs::read(&plan_file)?;
    let sha256 = {
        let mut hasher = Sha256::new();
        hasher.update(&plan_bytes);
        format!("{:x}", hasher.finalize())
    };

    let plan_str = String::from_utf8(plan_bytes)?;
    let parsed: plan::Plan = toml::from_str(&plan_str)?;

    state::validate_accepted_plan_record(&accepted)?;
    state::validate_state_record(&gov_state, &accepted, &parsed)?;
    state::validate_plan_consistency(&accepted, &parsed, &sha256)?;
    parsed.validate()?;

    let phase = parsed
        .phases
        .iter()
        .find(|p| p.id == phase_id)
        .ok_or_else(|| error::Error::UnknownPhase(phase_id.to_string()))?;

    if let Some(active) = &gov_state.active_phase {
        return Err(error::Error::ActivePhaseConflict(active.clone()));
    }

    for dep in &phase.depends_on {
        if !gov_state.closed_phases.contains(dep) {
            return Err(error::Error::BlockedDependency(
                phase_id.to_string(),
                dep.clone(),
            ));
        }
    }

    gov_state.active_phase = Some(phase_id.to_string());

    state::atomic_write_json(&gov_dir, "state.json", &gov_state)?;

    Ok(phase_id.to_string())
}

struct ContractAuthority {
    repo: PathBuf,
    gov_dir: PathBuf,
    accepted_plan_sha256: String,
    active_phase: String,
}

fn validate_contract_authority(repo_arg: &str) -> Result<ContractAuthority, error::Error> {
    let repo_path = Path::new(repo_arg);
    path::assert_existing_dir(repo_path)?;
    let repo = std::fs::canonicalize(repo_path)?;

    let gov_dir = path::validate_gov_dir_exists(&repo)?;

    let accepted = state::read_accepted_plan(&repo)?;
    let gov_state = state::read_state(&repo)?;

    let plan_file = path::resolve_safe_plan_path(&repo, &accepted.plan_path)?;
    let plan_bytes = std::fs::read(&plan_file)?;
    let plan_sha256 = {
        let mut hasher = Sha256::new();
        hasher.update(&plan_bytes);
        format!("{:x}", hasher.finalize())
    };
    let plan_str = String::from_utf8(plan_bytes)?;
    let parsed_plan: plan::Plan = toml::from_str(&plan_str)?;

    state::validate_accepted_plan_record(&accepted)?;
    state::validate_state_record(&gov_state, &accepted, &parsed_plan)?;
    state::validate_plan_consistency(&accepted, &parsed_plan, &plan_sha256)?;
    parsed_plan.validate()?;

    let active_phase = gov_state.active_phase.ok_or(error::Error::NoActivePhase)?;

    let ledger_path = gov_dir.join("accepted-contract.json");
    let draft_path = gov_dir.join("contract-draft.json");
    if ledger_path.exists() && !draft_path.exists() {
        return Err(error::Error::OrphanedAcceptedContract);
    }
    if draft_path.exists() {
        let draft: state::ContractDraft = serde_json::from_slice(&std::fs::read(&draft_path)?)?;
        state::validate_contract_draft_record(
            &draft,
            &accepted.sha256,
            &active_phase,
            &draft.contract_id,
        )?;
    }
    if ledger_path.exists() {
        let ledger: state::AcceptedContractLedger =
            serde_json::from_slice(&std::fs::read(&ledger_path)?)?;
        let draft = if draft_path.exists() {
            Some(state::read_contract_draft(&gov_dir)?)
        } else {
            None
        };
        state::validate_accepted_contract_ledger(
            &ledger,
            &accepted.sha256,
            &active_phase,
            draft.as_ref(),
        )?;
    }

    Ok(ContractAuthority {
        repo,
        gov_dir,
        accepted_plan_sha256: accepted.sha256,
        active_phase,
    })
}

fn cmd_contract_draft(repo_arg: &str, contract_arg: &str) -> Result<String, error::Error> {
    let auth = validate_contract_authority(repo_arg)?;
    let repo = &auth.repo;
    let gov_dir = &auth.gov_dir;

    let active_phase = &auth.active_phase;

    let contract_path = Path::new(contract_arg);
    path::assert_existing_file(contract_path)?;
    let contract_src = std::fs::canonicalize(contract_path)?;

    if !path::plan_is_inside_repo(&contract_src, repo) {
        return Err(error::Error::ContractSourceOutsideRepo);
    }
    if contract_src.starts_with(gov_dir) {
        return Err(error::Error::ContractSourceInsideMrgs);
    }

    let source_bytes = std::fs::read(&contract_src)?;
    let source_str = String::from_utf8(source_bytes.clone())?;

    let contract: contract::Contract = toml::from_str(&source_str)?;
    contract.validate()?;

    if contract.phase_id != *active_phase {
        return Err(error::Error::ContractPhaseMismatch(
            contract.phase_id,
            active_phase.clone(),
        ));
    }

    let source_sha256 = {
        let mut hasher = Sha256::new();
        hasher.update(&source_bytes);
        format!("{:x}", hasher.finalize())
    };

    let relative_path = path::relative_plan_path(&contract_src, repo);
    let source_path = relative_path
        .to_str()
        .ok_or_else(|| error::Error::UnsafePlanPath("<non-utf8-path>".to_string()))?
        .replace('\\', "/");
    path::validate_strict_normalized_path(&source_path)?;

    let draft_path = gov_dir.join("contract-draft.json");
    if draft_path.exists() {
        let existing_draft: state::ContractDraft =
            serde_json::from_slice(&std::fs::read(&draft_path)?)?;
        state::validate_contract_draft_record(
            &existing_draft,
            &auth.accepted_plan_sha256,
            active_phase,
            &contract.contract_id,
        )?;
        if state::is_draft_idempotent(
            &existing_draft.sha256,
            &existing_draft.content,
            &source_sha256,
            &source_bytes,
        ) {
            return Ok(format!(
                "{} {}",
                existing_draft.contract_id, existing_draft.sha256
            ));
        } else {
            return Err(error::Error::ContractDraftConflict);
        }
    }

    let draft = state::ContractDraft {
        schema_version: 1,
        accepted_plan_sha256: auth.accepted_plan_sha256.clone(),
        phase_id: active_phase.clone(),
        contract_id: contract.contract_id.clone(),
        revision: 1,
        preimage: None,
        source_path,
        sha256: source_sha256.clone(),
        content: source_str,
    };

    state::atomic_write_json(gov_dir, "contract-draft.json", &draft)?;

    Ok(format!("{} {}", contract.contract_id, source_sha256))
}

fn cmd_contract_accept(
    repo_arg: &str,
    revision_arg: u32,
    sha256_arg: &str,
    decision_arg: &str,
) -> Result<String, error::Error> {
    let auth = validate_contract_authority(repo_arg)?;
    let gov_dir = &auth.gov_dir;

    let draft_path = gov_dir.join("contract-draft.json");
    if !draft_path.exists() {
        return Err(error::Error::ContractNoDraft);
    }
    let draft = state::read_contract_draft(gov_dir)?;
    state::validate_contract_draft_record(
        &draft,
        &auth.accepted_plan_sha256,
        &auth.active_phase,
        &draft.contract_id,
    )?;

    if revision_arg < 1 {
        return Err(error::Error::DraftRevisionZero);
    }
    if revision_arg != draft.revision {
        return Err(error::Error::ContractAcceptRevisionMismatch {
            supplied: revision_arg,
            expected: draft.revision,
        });
    }
    if sha256_arg.len() != 64
        || !sha256_arg
            .chars()
            .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c))
    {
        return Err(error::Error::InvalidSha256);
    }
    if sha256_arg != draft.sha256 {
        return Err(error::Error::ContractAcceptShaMismatch);
    }
    if decision_arg != "ACCEPTED" {
        return Err(error::Error::ContractAcceptDecisionInvalid(
            decision_arg.to_string(),
        ));
    }

    let ledger_path = gov_dir.join("accepted-contract.json");
    if ledger_path.exists() {
        let ledger = state::read_accepted_contract_ledger(gov_dir)?;
        state::validate_accepted_contract_ledger(
            &ledger,
            &auth.accepted_plan_sha256,
            &auth.active_phase,
            Some(&draft),
        )?;
        if let Some(last) = ledger.revisions.last() {
            if last.revision == draft.revision {
                return Ok(format!(
                    "ACCEPTED {} {} {}",
                    draft.contract_id, draft.revision, draft.sha256
                ));
            }
        }
        let mut new_ledger = state::AcceptedContractLedger {
            schema_version: 1,
            accepted_plan_sha256: auth.accepted_plan_sha256.clone(),
            phase_id: auth.active_phase.clone(),
            contract_id: draft.contract_id.clone(),
            revisions: ledger.revisions,
        };
        new_ledger.revisions.push(state::AcceptedRevision {
            revision: draft.revision,
            source_path: draft.source_path.clone(),
            sha256: draft.sha256.clone(),
            content: draft.content.clone(),
        });
        state::validate_accepted_contract_ledger(
            &new_ledger,
            &auth.accepted_plan_sha256,
            &auth.active_phase,
            Some(&draft),
        )?;
        state::atomic_write_json(gov_dir, "accepted-contract.json", &new_ledger)?;
    } else {
        let ledger = state::AcceptedContractLedger {
            schema_version: 1,
            accepted_plan_sha256: auth.accepted_plan_sha256.clone(),
            phase_id: auth.active_phase.clone(),
            contract_id: draft.contract_id.clone(),
            revisions: vec![state::AcceptedRevision {
                revision: draft.revision,
                source_path: draft.source_path.clone(),
                sha256: draft.sha256.clone(),
                content: draft.content.clone(),
            }],
        };
        state::validate_accepted_contract_ledger(
            &ledger,
            &auth.accepted_plan_sha256,
            &auth.active_phase,
            Some(&draft),
        )?;
        state::atomic_write_json(gov_dir, "accepted-contract.json", &ledger)?;
    }

    Ok(format!(
        "ACCEPTED {} {} {}",
        draft.contract_id, draft.revision, draft.sha256
    ))
}

fn cmd_contract_revise(
    repo_arg: &str,
    contract_arg: &str,
    expected_revision: u32,
    expected_sha256: &str,
) -> Result<String, error::Error> {
    let auth = validate_contract_authority(repo_arg)?;
    let gov_dir = &auth.gov_dir;

    let draft_path = gov_dir.join("contract-draft.json");
    if !draft_path.exists() {
        return Err(error::Error::ContractNoDraft);
    }
    let current_draft = state::read_contract_draft(gov_dir)?;
    state::validate_contract_draft_record(
        &current_draft,
        &auth.accepted_plan_sha256,
        &auth.active_phase,
        &current_draft.contract_id,
    )?;

    if expected_revision < 1 {
        return Err(error::Error::DraftRevisionZero);
    }
    if expected_sha256.len() != 64
        || !expected_sha256
            .chars()
            .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c))
    {
        return Err(error::Error::InvalidSha256);
    }

    let new_path = Path::new(contract_arg);
    path::assert_existing_file(new_path)?;
    let new_src = std::fs::canonicalize(new_path)?;

    if !path::plan_is_inside_repo(&new_src, &auth.repo) {
        return Err(error::Error::ContractSourceOutsideRepo);
    }
    if new_src.starts_with(gov_dir) {
        return Err(error::Error::ContractSourceInsideMrgs);
    }

    let source_bytes = std::fs::read(&new_src)?;
    let source_str = String::from_utf8(source_bytes.clone())?;

    let contract: contract::Contract = toml::from_str(&source_str)?;
    contract.validate()?;

    let source_sha256 = {
        let mut hasher = Sha256::new();
        hasher.update(&source_bytes);
        format!("{:x}", hasher.finalize())
    };

    let relative_path = path::relative_plan_path(&new_src, &auth.repo);
    let source_path = relative_path
        .to_str()
        .ok_or_else(|| error::Error::UnsafePlanPath("<non-utf8-path>".to_string()))?
        .replace('\\', "/");
    path::validate_strict_normalized_path(&source_path)?;

    let replay_target = expected_revision.checked_add(1) == Some(current_draft.revision);
    if replay_target {
        let preimage = current_draft
            .preimage
            .as_ref()
            .ok_or(error::Error::ContractReviseReplayPreimageMissing)?;
        if preimage.revision != expected_revision {
            return Err(error::Error::ContractReviseReplayRevisionMismatch {
                expected: expected_revision,
                actual: preimage.revision,
            });
        }
        if preimage.sha256 != expected_sha256 {
            return Err(error::Error::ContractReviseReplayShaMismatch {
                expected: expected_sha256.to_string(),
                actual: preimage.sha256.clone(),
            });
        }
        if current_draft.sha256 != source_sha256 || current_draft.content.as_bytes() != source_bytes
        {
            return Err(error::Error::ContractReviseReplayContentMismatch);
        }
        if current_draft.source_path != source_path {
            return Err(error::Error::ContractReviseReplaySourcePathMismatch);
        }
        if current_draft.phase_id != contract.phase_id {
            return Err(error::Error::ContractReviseReplayPhaseMismatch);
        }
        if current_draft.contract_id != contract.contract_id {
            return Err(error::Error::ContractReviseReplayContractIdMismatch);
        }

        let ledger_path = gov_dir.join("accepted-contract.json");
        if !ledger_path.exists() {
            return Ok(format!(
                "DRAFT {} {} {}",
                current_draft.contract_id, current_draft.revision, current_draft.sha256
            ));
        }
        let ledger = state::read_accepted_contract_ledger(gov_dir)?;
        match ledger.revisions.last() {
            Some(last)
                if last.revision == current_draft.revision
                    && last.sha256 == current_draft.sha256
                    && last.source_path == current_draft.source_path
                    && last.content == current_draft.content =>
            {
                return Ok(format!(
                    "ACCEPTED {} {} {}",
                    current_draft.contract_id, current_draft.revision, current_draft.sha256
                ));
            }
            _ => {
                return Ok(format!(
                    "REVISION_DRAFT {} {} {}",
                    current_draft.contract_id, current_draft.revision, current_draft.sha256
                ));
            }
        }
    }

    if contract.phase_id != auth.active_phase {
        return Err(error::Error::ContractPhaseMismatch(
            contract.phase_id,
            auth.active_phase.clone(),
        ));
    }
    if contract.contract_id != current_draft.contract_id {
        return Err(error::Error::ContractReviseContractIdMismatch {
            supplied: contract.contract_id,
            expected: current_draft.contract_id.clone(),
        });
    }

    // ===== Normal CAS path (checked only when not a valid replay) =====
    if source_sha256 == current_draft.sha256 {
        return Err(error::Error::ContractReviseSameContent);
    }
    if expected_revision.checked_add(1).is_none() {
        return Err(error::Error::ContractReviseOverflow);
    }

    if expected_revision != current_draft.revision {
        return Err(error::Error::ContractReviseExpectedRevisionMismatch {
            supplied: expected_revision,
            current: current_draft.revision,
        });
    }
    if expected_sha256 != current_draft.sha256 {
        return Err(error::Error::ContractReviseExpectedShaMismatch);
    }

    let new_revision = expected_revision + 1;

    let new_draft = state::ContractDraft {
        schema_version: 1,
        accepted_plan_sha256: auth.accepted_plan_sha256.clone(),
        phase_id: auth.active_phase.clone(),
        contract_id: contract.contract_id.clone(),
        revision: new_revision,
        preimage: Some(state::ContractDraftPreimage {
            revision: expected_revision,
            sha256: expected_sha256.to_string(),
        }),
        source_path,
        sha256: source_sha256.clone(),
        content: source_str,
    };

    state::validate_contract_draft_record(
        &new_draft,
        &auth.accepted_plan_sha256,
        &auth.active_phase,
        &contract.contract_id,
    )?;

    state::atomic_write_json(gov_dir, "contract-draft.json", &new_draft)?;

    let ledger_path = gov_dir.join("accepted-contract.json");
    let prefix = if ledger_path.exists() {
        "REVISION_DRAFT"
    } else {
        "DRAFT"
    };

    Ok(format!(
        "{} {} {} {}",
        prefix, new_draft.contract_id, new_draft.revision, new_draft.sha256
    ))
}
