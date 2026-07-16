use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("TOML parse error: {0}")]
    TomlParse(#[from] toml::de::Error),

    #[error("JSON parse error: {0}")]
    JsonParse(#[from] serde_json::Error),

    #[error("UTF-8 error: {0}")]
    Utf8(#[from] std::string::FromUtf8Error),

    #[error("plan path not inside repository: {0}")]
    PlanOutsideRepo(PathBuf),

    #[error("plan not found: {0}")]
    PlanNotFound(PathBuf),

    #[error("not a regular file: {0}")]
    NotRegularFile(PathBuf),

    #[error("not a directory: {0}")]
    NotDirectory(PathBuf),

    #[error("unsupported schema version: {0}")]
    UnsupportedSchema(u32),

    #[error("empty plan ID")]
    EmptyPlanId,

    #[error("plan has zero phases")]
    ZeroPhases,

    #[error("empty phase ID")]
    EmptyPhaseId,

    #[error("empty phase title")]
    EmptyPhaseTitle,

    #[error("duplicate phase ID: {0}")]
    DuplicatePhaseId(String),

    #[error("unknown dependency '{0}' in phase '{1}'")]
    UnknownDependency(String, String),

    #[error("self-dependency in phase '{0}'")]
    SelfDependency(String),

    #[error("dependency cycle detected")]
    DependencyCycle,

    #[error("no accepted plan found in {0}")]
    NoAcceptedPlan(PathBuf),

    #[error("no governance state found in {0}")]
    NoState(PathBuf),

    #[error("plan drift detected: expected {expected}, actual {actual}")]
    PlanDrift { expected: String, actual: String },

    #[error("unknown phase: {0}")]
    UnknownPhase(String),

    #[error("phase '{0}' is already active")]
    ActivePhaseConflict(String),

    #[error("phase '{0}' dependency '{1}' not closed")]
    BlockedDependency(String, String),

    #[error("cannot accept different plan when authority exists")]
    AcceptedPlanMismatch,

    #[error("empty plan path in persisted record")]
    EmptyPlanPath,

    #[error("unsafe plan path: {0}")]
    UnsafePlanPath(String),

    #[error("persisted plan path resolves outside repository")]
    PlanPathOutsideRepo,

    #[error("governance directory is not a directory: {0}")]
    GovDirNotDirectory(PathBuf),

    #[error("governance directory escapes repository: {0}")]
    GovDirEscape(PathBuf),

    #[error("accepted record schema version: expected 1, got {0}")]
    AcceptedSchemaVersion(u32),

    #[error("state record schema version: expected 1, got {0}")]
    StateSchemaVersion(u32),

    #[error("invalid SHA-256 hex string")]
    InvalidSha256,

    #[error("state SHA does not match accepted plan SHA")]
    StateShaMismatch,

    #[error("accepted plan ID mismatch: {0}")]
    PlanIdMismatch(String),

    #[error("accepted phase count mismatch: expected {expected}, actual {actual}")]
    PhaseCountMismatch { expected: usize, actual: usize },

    #[error("unknown phase '{0}' in closed_phases")]
    UnknownClosedPhase(String),

    #[error("duplicate phase '{0}' in closed_phases")]
    DuplicateClosedPhase(String),

    #[error("unknown active phase: {0}")]
    UnknownActivePhase(String),

    #[error("active phase '{0}' is also in closed_phases")]
    ActivePhaseAlsoClosed(String),

    #[error("inconsistent closed dependency: phase '{0}' has unclosed dependency '{1}'")]
    InconsistentClosedDep(String, String),

    #[error("governance directory does not exist: {0}")]
    GovDirNotExists(PathBuf),

    #[error("incomplete governance authority in {0}")]
    IncompleteGovernanceAuthority(PathBuf),

    #[error("active phase '{0}' has unmet dependency '{1}'")]
    ActivePhaseDependencyUnmet(String, String),

    #[error("unauthorized governance filename: {0}")]
    UnauthorizedFilename(String),

    #[error("unsupported contract schema version: {0}")]
    UnsupportedContractSchema(u32),

    #[error("empty or whitespace-only field '{0}' in contract")]
    EmptyContractField(String),

    #[error("'{0}' list is empty in contract")]
    EmptyContractList(String),

    #[error("empty or whitespace-only entry in contract '{0}' list")]
    EmptyContractListEntry(String),

    #[error("duplicate entry in contract '{0}' list")]
    DuplicateContractListEntry(String),

    #[error("no active phase selected")]
    NoActivePhase,

    #[error("contract phase ID '{0}' does not match active phase '{1}'")]
    ContractPhaseMismatch(String, String),

    #[error("contract source file is inside .mrgs directory")]
    ContractSourceInsideMrgs,

    #[error("contract source file is outside repository")]
    ContractSourceOutsideRepo,

    #[error("contract draft already exists with different content")]
    ContractDraftConflict,

    #[error("unsupported contract draft schema version: {0}")]
    UnsupportedDraftSchema(u32),

    #[error("contract draft revision must be 1, got {0}")]
    DraftRevisionNotOne(u32),

    #[error("contract draft field mismatch: {0}")]
    DraftFieldMismatch(String),

    #[error("contract draft content hash mismatch")]
    DraftContentHashMismatch,
}
