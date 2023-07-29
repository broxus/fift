pub type FiftResult<T> = Result<T, FiftError>;

#[derive(Debug, thiserror::Error)]
pub enum FiftError {
    #[error("Cell error")]
    CellError(#[from] everscale_types::error::Error),
    #[error("IO error")]
    IoError(#[from] std::io::Error),

    #[error("Execution aborted")]
    ExecutionAborted,

    #[error("Type redefenition")]
    TypeRedefenition,
    #[error("Invalid number")]
    InvalidNumber,
    #[error("Undefined word")]
    UndefinedWord,

    #[error("Stack underflow")]
    StackUnderflow,
    #[error("Stack overflow")]
    StackOverflow,
    #[error("Invalid type")]
    InvalidType,
    #[error("Integer overflow")]
    IntegerOverflow,

    #[error("Expected interpreter mode")]
    ExpectedInterpreterMode,
    #[error("Expected compilation mode")]
    ExpectedCompilationMode,
    #[error("Expected internal interpreter mode")]
    ExpectedInternalInterpreterMode,
    #[error("Expected non-internal interpreter mode")]
    ExpectedNonInternalInterpreterMode,

    #[error("Expected empty slice")]
    ExpectedEmptySlice,
    #[error("Expected integer in the specified range")]
    ExpectedIntegerInRange,
}
