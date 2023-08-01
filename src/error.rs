pub use anyhow::Error;

#[derive(Debug, thiserror::Error)]
#[error("Execution aborted: {reason}")]
pub struct ExecutionAborted {
    pub reason: String,
}

#[derive(Debug, thiserror::Error)]
#[error("Unexpected eof")]
pub struct UnexpectedEof;
