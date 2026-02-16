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
}
