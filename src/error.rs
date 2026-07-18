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

    #[error("invalid argument")]
    InvalidArgument,
    #[error("governance authority invalid")]
    GovernanceAuthorityInvalid,
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

    #[error("contract draft revision must be at least 1, got 0")]
    DraftRevisionZero,

    #[error("contract draft field mismatch: {0}")]
    DraftFieldMismatch(String),

    #[error("contract draft content hash mismatch")]
    DraftContentHashMismatch,

    #[error("contract command requires a valid draft")]
    ContractNoDraft,

    #[error("contract accept revision {supplied} does not match draft revision {expected}")]
    ContractAcceptRevisionMismatch { supplied: u32, expected: u32 },

    #[error("contract accept SHA does not match draft SHA")]
    ContractAcceptShaMismatch,

    #[error("contract accept decision must be exactly ACCEPTED, got '{0}'")]
    ContractAcceptDecisionInvalid(String),

    #[error("accepted contract plan SHA mismatch")]
    AcceptedContractPlanShaMismatch,

    #[error("accepted contract phase mismatch: expected '{expected}', got '{actual}'")]
    AcceptedContractPhaseMismatch { expected: String, actual: String },

    #[error("accepted contract revision zero")]
    AcceptedContractRevisionZero,

    #[error("duplicate accepted revision: {0}")]
    AcceptedContractDuplicateRevision(u32),

    #[error("non-increasing accepted revision: {0} follows {1}")]
    AcceptedContractNonIncreasingRevision(u32, u32),

    #[error("accepted contract source path under .mrgs")]
    AcceptedContractSourceUnderMrgs,

    #[error("accepted contract content parse error")]
    AcceptedContractContentParse,

    #[error("accepted contract content phase mismatch")]
    AcceptedContractContentPhaseMismatch,

    #[error("accepted contract content ID mismatch")]
    AcceptedContractContentIdMismatch,

    #[error("accepted contract content hash mismatch")]
    AcceptedContractContentHashMismatch,

    #[error("accepted contract final revision {revision} exceeds draft revision {draft_revision}")]
    AcceptedContractFinalRevisionExceedsDraft { revision: u32, draft_revision: u32 },

    #[error("accepted contract final revision equals draft but content differs")]
    AcceptedContractEqualRevisionContentMismatch,

    #[error("accepted contract revisions list is empty")]
    AcceptedContractEmptyRevisions,

    #[error("accepted contract ID '{accepted}' does not match draft contract ID '{draft}'")]
    AcceptedContractDraftContractIdMismatch { accepted: String, draft: String },

    #[error("contract revise expected revision {supplied} does not match current {current}")]
    ContractReviseExpectedRevisionMismatch { supplied: u32, current: u32 },

    #[error("contract revise expected SHA does not match current draft SHA")]
    ContractReviseExpectedShaMismatch,

    #[error("contract revise would produce same content as current draft")]
    ContractReviseSameContent,

    #[error("contract revision overflow")]
    ContractReviseOverflow,

    #[error("contract revise contract ID mismatch: supplied '{supplied}', expected '{expected}'")]
    ContractReviseContractIdMismatch { supplied: String, expected: String },

    #[error("contract revise replay preimage SHA mismatch: expected '{expected}', preimage has '{actual}'")]
    ContractReviseReplayShaMismatch { expected: String, actual: String },

    #[error("contract revise replay preimage revision mismatch: expected {expected}, preimage has {actual}")]
    ContractReviseReplayRevisionMismatch { expected: u32, actual: u32 },

    #[error("contract revise replay content mismatch")]
    ContractReviseReplayContentMismatch,

    #[error("contract revise replay source path mismatch")]
    ContractReviseReplaySourcePathMismatch,

    #[error("contract revise replay preimage is missing")]
    ContractReviseReplayPreimageMissing,

    #[error("contract revise replay phase mismatch")]
    ContractReviseReplayPhaseMismatch,

    #[error("contract revise replay contract ID mismatch")]
    ContractReviseReplayContractIdMismatch,

    #[error("orphaned accepted-contract.json without contract-draft.json")]
    OrphanedAcceptedContract,
    #[error("contract draft revision 1 must not have a preimage")]
    DraftPreimageUnexpected,
    #[error("contract draft revision {draft_revision} requires a preimage")]
    DraftPreimageRequired { draft_revision: u32 },
    #[error("contract draft preimage revision must be positive")]
    DraftPreimageRevisionZero,
    #[error("contract draft preimage revision {preimage_revision} does not equal draft revision {draft_revision} - 1 = {expected}")]
    DraftPreimageRevisionMismatch {
        preimage_revision: u32,
        draft_revision: u32,
        expected: u32,
    },
    #[error("contract draft preimage sha256 is invalid")]
    DraftPreimageShaInvalid,

    // Phase 4 errors
    #[error("GIT_COMMAND_FAILED")]
    GitCommandFailed(String),
    #[error("GIT_ROOT_MISMATCH")]
    GitRootMismatch,
    #[error("GIT_DETACHED_HEAD")]
    GitDetachedHead,
    #[error("GIT_HEAD_INVALID")]
    GitHeadInvalid,
    #[error("GIT_DIRTY")]
    GitDirty,
    #[error("GIT_OPERATION_IN_PROGRESS")]
    GitOperationInProgress,
    #[error("GIT_SUBMODULE_UNSUPPORTED")]
    GitSubmoduleUnsupported,
    #[error("CONTRACT_NOT_ACCEPTED")]
    ContractNotAccepted,
    #[error("REQUESTED_REVISION_STALE")]
    RequestedRevisionStale,
    #[error("REQUESTED_SHA_STALE")]
    RequestedShaStale,
    #[error("CONTRACT_PATH_RULE_INVALID")]
    ContractPathRuleInvalid,
    #[error("IMPLEMENTATION_AUTHORITY_MISSING")]
    ImplementationAuthorityMissing,
    #[error("IMPLEMENTATION_AUTHORITY_INVALID")]
    ImplementationAuthorityInvalid,
    #[error("IMPLEMENTATION_AUTHORITY_CONFLICT")]
    ImplementationAuthorityConflict,
    #[error("IMPLEMENTATION_AUTHORITY_STALE")]
    ImplementationAuthorityStale,
    #[error("BASELINE_BRANCH_CHANGED")]
    BaselineBranchChanged,
    #[error("BASELINE_COMMIT_MISSING")]
    BaselineCommitMissing,
    #[error("BASELINE_HISTORY_CHANGED")]
    BaselineHistoryChanged,
    #[error("GIT_INVENTORY_INVALID")]
    GitInventoryInvalid,
    #[error("GIT_CONFLICT")]
    GitConflict,
    #[error("CHANGE_PATH_INVALID")]
    ChangePathInvalid,
    #[error("CHANGE_FORBIDDEN")]
    ChangeForbidden,
    #[error("CHANGE_NOT_ALLOWED")]
    ChangeNotAllowed,
    #[error("FILESYSTEM_BOUNDARY_UNSAFE")]
    FilesystemBoundaryUnsafe,
    #[error("REPOSITORY_INVALID")]
    RepositoryInvalid,
    #[error("PERSISTENCE_FAILED")]
    PersistenceFailed,
}

impl Error {
    pub fn phase4_category(&self) -> &'static str {
        match self {
            Error::InvalidArgument => "INVALID_ARGUMENT",
            Error::GitCommandFailed(_) => "GIT_COMMAND_FAILED",
            Error::GitRootMismatch => "GIT_ROOT_MISMATCH",
            Error::GitDetachedHead => "GIT_DETACHED_HEAD",
            Error::GitHeadInvalid => "GIT_HEAD_INVALID",
            Error::GitDirty => "GIT_DIRTY",
            Error::GitOperationInProgress => "GIT_OPERATION_IN_PROGRESS",
            Error::GitSubmoduleUnsupported => "GIT_SUBMODULE_UNSUPPORTED",
            Error::ContractNotAccepted => "CONTRACT_NOT_ACCEPTED",
            Error::RequestedRevisionStale => "REQUESTED_REVISION_STALE",
            Error::RequestedShaStale => "REQUESTED_SHA_STALE",
            Error::ContractPathRuleInvalid => "CONTRACT_PATH_RULE_INVALID",
            Error::ImplementationAuthorityMissing => "IMPLEMENTATION_AUTHORITY_MISSING",
            Error::ImplementationAuthorityInvalid => "IMPLEMENTATION_AUTHORITY_INVALID",
            Error::ImplementationAuthorityConflict => "IMPLEMENTATION_AUTHORITY_CONFLICT",
            Error::ImplementationAuthorityStale => "IMPLEMENTATION_AUTHORITY_STALE",
            Error::BaselineBranchChanged => "BASELINE_BRANCH_CHANGED",
            Error::BaselineCommitMissing => "BASELINE_COMMIT_MISSING",
            Error::BaselineHistoryChanged => "BASELINE_HISTORY_CHANGED",
            Error::GitInventoryInvalid => "GIT_INVENTORY_INVALID",
            Error::GitConflict => "GIT_CONFLICT",
            Error::ChangePathInvalid => "CHANGE_PATH_INVALID",
            Error::ChangeForbidden => "CHANGE_FORBIDDEN",
            Error::ChangeNotAllowed => "CHANGE_NOT_ALLOWED",
            Error::FilesystemBoundaryUnsafe => "FILESYSTEM_BOUNDARY_UNSAFE",
            Error::RepositoryInvalid => "REPOSITORY_INVALID",
            Error::PersistenceFailed => "PERSISTENCE_FAILED",
            // In the Phase 4 adapter context, an unhandled I/O error is an
            // authority read failure, never a generic persistence failure
            // (BLOCKER 9). Explicit publication I/O already maps to
            // PERSISTENCE_FAILED via Error::PersistenceFailed.
            Error::Io(_) => "GOVERNANCE_AUTHORITY_INVALID",
            Error::GovDirNotExists(_) => "GOVERNANCE_AUTHORITY_INVALID",
            Error::PlanIdMismatch(_) => "GOVERNANCE_AUTHORITY_INVALID",
            Error::PlanDrift { .. } => "GOVERNANCE_AUTHORITY_INVALID",
            Error::AcceptedPlanMismatch => "GOVERNANCE_AUTHORITY_INVALID",
            Error::NoAcceptedPlan(_) => "GOVERNANCE_AUTHORITY_INVALID",
            Error::NoState(_) => "GOVERNANCE_AUTHORITY_INVALID",
            Error::TomlParse(_) => "GOVERNANCE_AUTHORITY_INVALID",
            Error::JsonParse(_) => "GOVERNANCE_AUTHORITY_INVALID",
            Error::Utf8(_) => "GOVERNANCE_AUTHORITY_INVALID",
            Error::PlanOutsideRepo(_) => "GOVERNANCE_AUTHORITY_INVALID",
            Error::PlanNotFound(_) => "GOVERNANCE_AUTHORITY_INVALID",
            Error::NotRegularFile(_) => "GOVERNANCE_AUTHORITY_INVALID",
            Error::NotDirectory(_) => "GOVERNANCE_AUTHORITY_INVALID",
            Error::UnsupportedSchema(_) => "GOVERNANCE_AUTHORITY_INVALID",
            Error::EmptyPlanId => "GOVERNANCE_AUTHORITY_INVALID",
            Error::ZeroPhases => "GOVERNANCE_AUTHORITY_INVALID",
            Error::EmptyPhaseId => "GOVERNANCE_AUTHORITY_INVALID",
            Error::EmptyPhaseTitle => "GOVERNANCE_AUTHORITY_INVALID",
            Error::DuplicatePhaseId(_) => "GOVERNANCE_AUTHORITY_INVALID",
            Error::UnknownDependency(_, _) => "GOVERNANCE_AUTHORITY_INVALID",
            Error::SelfDependency(_) => "GOVERNANCE_AUTHORITY_INVALID",
            Error::DependencyCycle => "GOVERNANCE_AUTHORITY_INVALID",
            Error::StateShaMismatch => "GOVERNANCE_AUTHORITY_INVALID",
            Error::AcceptedSchemaVersion(_) => "GOVERNANCE_AUTHORITY_INVALID",
            Error::StateSchemaVersion(_) => "GOVERNANCE_AUTHORITY_INVALID",
            Error::InvalidSha256 => "GOVERNANCE_AUTHORITY_INVALID",
            Error::UnknownPhase(_) => "GOVERNANCE_AUTHORITY_INVALID",
            Error::ActivePhaseConflict(_) => "GOVERNANCE_AUTHORITY_INVALID",
            Error::BlockedDependency(_, _) => "GOVERNANCE_AUTHORITY_INVALID",
            Error::EmptyPlanPath => "GOVERNANCE_AUTHORITY_INVALID",
            Error::UnsafePlanPath(_) => "GOVERNANCE_AUTHORITY_INVALID",
            Error::PlanPathOutsideRepo => "GOVERNANCE_AUTHORITY_INVALID",
            Error::GovDirNotDirectory(_) => "GOVERNANCE_AUTHORITY_INVALID",
            Error::GovDirEscape(_) => "GOVERNANCE_AUTHORITY_INVALID",
            Error::IncompleteGovernanceAuthority(_) => "GOVERNANCE_AUTHORITY_INVALID",
            Error::ActivePhaseDependencyUnmet(_, _) => "GOVERNANCE_AUTHORITY_INVALID",
            Error::UnauthorizedFilename(_) => "GOVERNANCE_AUTHORITY_INVALID",
            Error::UnsupportedContractSchema(_) => "GOVERNANCE_AUTHORITY_INVALID",
            Error::EmptyContractField(_) => "GOVERNANCE_AUTHORITY_INVALID",
            Error::EmptyContractList(_) => "GOVERNANCE_AUTHORITY_INVALID",
            Error::EmptyContractListEntry(_) => "GOVERNANCE_AUTHORITY_INVALID",
            Error::DuplicateContractListEntry(_) => "GOVERNANCE_AUTHORITY_INVALID",
            Error::NoActivePhase => "GOVERNANCE_AUTHORITY_INVALID",
            Error::ContractPhaseMismatch(_, _) => "GOVERNANCE_AUTHORITY_INVALID",
            Error::ContractSourceInsideMrgs => "GOVERNANCE_AUTHORITY_INVALID",
            Error::ContractSourceOutsideRepo => "GOVERNANCE_AUTHORITY_INVALID",
            Error::ContractDraftConflict => "GOVERNANCE_AUTHORITY_INVALID",
            Error::UnsupportedDraftSchema(_) => "GOVERNANCE_AUTHORITY_INVALID",
            Error::DraftRevisionZero => "GOVERNANCE_AUTHORITY_INVALID",
            Error::DraftFieldMismatch(_) => "GOVERNANCE_AUTHORITY_INVALID",
            Error::DraftContentHashMismatch => "GOVERNANCE_AUTHORITY_INVALID",
            Error::ContractNoDraft => "GOVERNANCE_AUTHORITY_INVALID",
            Error::ContractAcceptRevisionMismatch { .. } => "GOVERNANCE_AUTHORITY_INVALID",
            Error::ContractAcceptShaMismatch => "GOVERNANCE_AUTHORITY_INVALID",
            Error::ContractAcceptDecisionInvalid(_) => "GOVERNANCE_AUTHORITY_INVALID",
            Error::AcceptedContractPlanShaMismatch => "GOVERNANCE_AUTHORITY_INVALID",
            Error::AcceptedContractPhaseMismatch { .. } => "GOVERNANCE_AUTHORITY_INVALID",
            Error::AcceptedContractRevisionZero => "GOVERNANCE_AUTHORITY_INVALID",
            Error::AcceptedContractDuplicateRevision(_) => "GOVERNANCE_AUTHORITY_INVALID",
            Error::AcceptedContractNonIncreasingRevision(_, _) => "GOVERNANCE_AUTHORITY_INVALID",
            Error::AcceptedContractSourceUnderMrgs => "GOVERNANCE_AUTHORITY_INVALID",
            Error::AcceptedContractContentParse => "GOVERNANCE_AUTHORITY_INVALID",
            Error::AcceptedContractContentPhaseMismatch => "GOVERNANCE_AUTHORITY_INVALID",
            Error::AcceptedContractContentIdMismatch => "GOVERNANCE_AUTHORITY_INVALID",
            Error::AcceptedContractContentHashMismatch => "GOVERNANCE_AUTHORITY_INVALID",
            Error::AcceptedContractFinalRevisionExceedsDraft { .. } => {
                "GOVERNANCE_AUTHORITY_INVALID"
            }
            Error::AcceptedContractEqualRevisionContentMismatch => "GOVERNANCE_AUTHORITY_INVALID",
            Error::AcceptedContractEmptyRevisions => "GOVERNANCE_AUTHORITY_INVALID",
            Error::AcceptedContractDraftContractIdMismatch { .. } => "GOVERNANCE_AUTHORITY_INVALID",
            Error::ContractReviseExpectedRevisionMismatch { .. } => "GOVERNANCE_AUTHORITY_INVALID",
            Error::ContractReviseExpectedShaMismatch => "GOVERNANCE_AUTHORITY_INVALID",
            Error::ContractReviseSameContent => "GOVERNANCE_AUTHORITY_INVALID",
            Error::ContractReviseOverflow => "GOVERNANCE_AUTHORITY_INVALID",
            Error::ContractReviseContractIdMismatch { .. } => "GOVERNANCE_AUTHORITY_INVALID",
            Error::ContractReviseReplayShaMismatch { .. } => "GOVERNANCE_AUTHORITY_INVALID",
            Error::ContractReviseReplayRevisionMismatch { .. } => "GOVERNANCE_AUTHORITY_INVALID",
            Error::ContractReviseReplayContentMismatch => "GOVERNANCE_AUTHORITY_INVALID",
            Error::ContractReviseReplaySourcePathMismatch => "GOVERNANCE_AUTHORITY_INVALID",
            Error::ContractReviseReplayPreimageMissing => "GOVERNANCE_AUTHORITY_INVALID",
            Error::ContractReviseReplayPhaseMismatch => "GOVERNANCE_AUTHORITY_INVALID",
            Error::ContractReviseReplayContractIdMismatch => "GOVERNANCE_AUTHORITY_INVALID",
            Error::OrphanedAcceptedContract => "GOVERNANCE_AUTHORITY_INVALID",
            Error::DraftPreimageUnexpected => "GOVERNANCE_AUTHORITY_INVALID",
            Error::DraftPreimageRequired { .. } => "GOVERNANCE_AUTHORITY_INVALID",
            Error::DraftPreimageRevisionZero => "GOVERNANCE_AUTHORITY_INVALID",
            Error::DraftPreimageRevisionMismatch { .. } => "GOVERNANCE_AUTHORITY_INVALID",
            Error::DraftPreimageShaInvalid => "GOVERNANCE_AUTHORITY_INVALID",
            Error::GovernanceAuthorityInvalid => "GOVERNANCE_AUTHORITY_INVALID",
            Error::PhaseCountMismatch { .. } => "GOVERNANCE_AUTHORITY_INVALID",
            Error::UnknownClosedPhase(_) => "GOVERNANCE_AUTHORITY_INVALID",
            Error::DuplicateClosedPhase(_) => "GOVERNANCE_AUTHORITY_INVALID",
            Error::UnknownActivePhase(_) => "GOVERNANCE_AUTHORITY_INVALID",
            Error::ActivePhaseAlsoClosed(_) => "GOVERNANCE_AUTHORITY_INVALID",
            Error::InconsistentClosedDep(_, _) => "GOVERNANCE_AUTHORITY_INVALID",
        }
    }
}
