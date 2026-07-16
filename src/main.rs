mod cli;
mod error;
mod path;
mod plan;
mod state;

use clap::Parser;
use sha2::{Digest, Sha256};
use std::path::Path;

fn main() {
    let cli = cli::Cli::parse();

    let result = match cli.command {
        cli::CliCommand::Plan(sub) => match sub.action {
            cli::PlanAction::Accept { repo, plan } => cmd_plan_accept(&repo, &plan),
        },
        cli::CliCommand::Phase(sub) => match sub.action {
            cli::PhaseAction::Select { repo, phase } => cmd_phase_select(&repo, &phase),
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
