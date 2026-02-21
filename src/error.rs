use thiserror::Error;

#[derive(Error, Debug)]
pub enum MyosotisError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Format error: {0}")]
    Serde(#[from] serde_json::Error),

    #[error("Invariant violation: {0}")]
    Invariant(String),

    #[error("Node not found: {0}")]
    NodeNotFound(u64),

    #[error("Commit not found: {0}")]
    CommitNotFound(u64),

    #[error("Invalid input: {0}")]
    InvalidInput(String),
    #[error("Invalid hash encountered")]
    InvalidHash,

    #[error("Parent hash mismatch at commit {0}")]
    ParentHashMismatch(u64),

    #[error("Corrupt commit chain: {0}")]
    CorruptCommitChain(String),

    #[error("Invalid checkpoint")]
    InvalidCheckpoint,

    #[error("Checkpoint state hash mismatch")]
    CheckpointHashMismatch,

    #[error("Checkpoint commit mismatch")]
    CheckpointCommitMismatch,

    #[error("Node is deleted: {0}")]
    NodeDeleted(u64),

    #[error("Field not found: {0}")]
    FieldNotFound(String),

    #[error("Delete on already deleted node: {0}")]
    DeleteOnDeletedNode(u64),

    #[error("Delete on non-existent node: {0}")]
    DeleteNonexistentNode(u64),

    #[error("Invalid compaction target")]
    InvalidCompactionTarget,

    #[error("Compaction integrity mismatch")]
    CompactionIntegrityMismatch,

    #[error("Invalid file magic")]
    InvalidFileMagic,

    #[error("Unsupported format version: {0}")]
    UnsupportedFormatVersion(u32),

    #[error("Missing format version")]
    MissingFormatVersion,

    #[error("Corrupt commit hash")]
    CorruptCommitHash,

    #[error("Corrupt parent hash")]
    CorruptParentHash,

    #[error("Corrupt checkpoint hash")]
    CorruptCheckpointHash,

    #[error("Corrupt genesis hash")]
    CorruptGenesisHash,

    #[error("Malformed file structure")]
    MalformedFileStructure,
}
