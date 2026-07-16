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
