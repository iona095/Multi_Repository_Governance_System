use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "mrgs")]
pub struct Cli {
    #[command(subcommand)]
    pub command: CliCommand,
}

#[derive(Subcommand, Debug)]
pub enum CliCommand {
    Plan(PlanSub),
    Phase(PhaseSub),
    Contract(ContractSub),
    Implementation(ImplementationSub),
    Audit(AuditSub),
    Repair(RepairSub),
}

#[derive(clap::Args, Debug)]
pub struct PlanSub {
    #[command(subcommand)]
    pub action: PlanAction,
}

#[derive(Subcommand, Debug)]
pub enum PlanAction {
    Accept {
        #[arg(long)]
        repo: String,
        #[arg(long)]
        plan: String,
    },
}

#[derive(clap::Args, Debug)]
pub struct PhaseSub {
    #[command(subcommand)]
    pub action: PhaseAction,
}

#[derive(Subcommand, Debug)]
pub enum PhaseAction {
    Select {
        #[arg(long)]
        repo: String,
        #[arg(long)]
        phase: String,
    },
}

#[derive(clap::Args, Debug)]
pub struct ContractSub {
    #[command(subcommand)]
    pub action: ContractAction,
}

#[derive(Subcommand, Debug)]
pub enum ContractAction {
    Draft {
        #[arg(long)]
        repo: String,
        #[arg(long)]
        contract: String,
    },
    Accept {
        #[arg(long)]
        repo: String,
        #[arg(long)]
        revision: u32,
        #[arg(long)]
        sha256: String,
        #[arg(long)]
        decision: String,
    },
    Revise {
        #[arg(long)]
        repo: String,
        #[arg(long)]
        contract: String,
        #[arg(long = "expected-revision")]
        expected_revision: u32,
        #[arg(long = "expected-sha256")]
        expected_sha256: String,
    },
}

#[derive(clap::Args, Debug)]
pub struct ImplementationSub {
    #[command(subcommand)]
    pub action: ImplementationAction,
}

#[derive(Subcommand, Debug)]
pub enum ImplementationAction {
    Begin {
        #[arg(long)]
        repo: String,
        #[arg(long)]
        revision: String,
        #[arg(long)]
        sha256: String,
    },
    Check {
        #[arg(long)]
        repo: String,
    },
}

#[derive(clap::Args, Debug)]
pub struct AuditSub {
    #[command(subcommand)]
    pub action: AuditAction,
}

#[derive(Subcommand, Debug)]
pub enum AuditAction {
    Begin {
        #[arg(long)]
        repo: String,
        #[arg(long)]
        auditor: String,
    },
    Record {
        #[arg(long)]
        repo: String,
        #[arg(long)]
        report: String,
    },
}

#[derive(clap::Args, Debug)]
pub struct RepairSub {
    #[command(subcommand)]
    pub action: RepairAction,
}

#[derive(Subcommand, Debug)]
pub enum RepairAction {
    Check {
        #[arg(long)]
        repo: String,
    },
}
