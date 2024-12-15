use anchor_lang::prelude::*;
#[error_code]
pub enum ErrorCode {

    #[msg("Too many instruction")]
    TooManyInstructions,
    #[msg("Too many accounts in instruction")]
    TooManyAccounts,
    #[msg("Invalid wallet")]
    InvalidWallet,
    #[msg("Threshold must be greater than 0")]
    InvalidThreshold,
    #[msg("Threshold must be less than or equal to the total weight")]
    ThresholdTooHigh,
    #[msg("No owners provided")]
    NoOwners,
    #[msg("Not an owner")]
    NotOwner,
    #[msg("Transaction already executed")]
    AlreadyExecuted,
    #[msg("Already signed")]
    AlreadySigned,
    #[msg("Insufficient signers weight")]
    InsufficientSigners,
    #[msg("Owners must be unique")]
    DuplicateOwner,
    #[msg("Owner set has changed since transaction creation")]
    OwnerSetChanged,
    #[msg("Owner weight must be greater than 0")]
    InvalidOwnerWeight,
    #[msg("Owner not found")]
    OwnerNotFound,
    #[msg("Invalid number of owners")]
    InvalidOwnerCount,
    #[msg("Transaction execution failed")]
    TransactionExecutionFailed,
    #[msg("Arithmetic overflow")]
    ArithmeticOverflow,
    #[msg("Owner has not signed this transaction")]
    NotSigned,
    #[msg("Required account not found")]
    AccountNotFound,
    #[msg("Transaction not executed yet")]
    TransactionNotExecuted,
    #[msg("Only transaction creator can close it")]
    UnauthorizedClose,
    #[msg("Instruction data too large")]
    DataTooLarge,
}