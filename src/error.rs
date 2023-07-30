pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Cell error")]
    CellError(#[from] everscale_types::error::Error),
    #[error("Boc error")]
    BocError(#[from] everscale_types::boc::de::Error),
    #[error("IO error")]
    IoError(#[from] std::io::Error),

    #[error("Execution aborted")]
    ExecutionAborted,

    #[error("Type redefenition")]
    TypeRedefenition,
    #[error("Invalid number")]
    InvalidNumber,
    #[error("Invalid char")]
    InvalidChar,
    #[error("Invalid string")]
    InvalidString,
    #[error("Index out of range")]
    IndexOutOfRange,
    #[error("Undefined word")]
    UndefinedWord,
    #[error("Unexpected eof")]
    UnexpectedEof,

    #[error("Stack underflow")]
    StackUnderflow,
    #[error("Stack overflow")]
    StackOverflow,
    #[error("Invalid type")]
    InvalidType,
    #[error("Integer overflow")]
    IntegerOverflow,
    #[error("Tuple underflow")]
    TupleUnderflow,
    #[error("Tuple too large")]
    TupleTooLarge,
    #[error("Tuple size mismatch")]
    TupleSizeMismatch,

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
