use anchor_lang::prelude::*;
#[error_code]
pub enum MultisigError {
    #[msg("The threshold must be greater than 0 and less than or equal to the number of owners")]
    InvalidThreshold,
    #[msg("The owners list cannot be empty")]
    NoOwners,
    #[msg("Not an owner of the multisig")]
    NotAnOwner,
    #[msg("Transaction has already been executed")]
    AlreadyExecuted,
    #[msg("Cannot approve a transaction twice")]
    AlreadySigned,
    #[msg("Not enough signers to execute the transaction")]
    NotEnoughSigners,
    #[msg("Invalid transaction owner")]
    InvalidTransactionOwner,
    #[msg("Invalid program id")]
    InvalidProgramId,
    #[msg("Invalid account provided")]
    InvalidAccount,
    #[msg("Invalid owner")]
    InvalidOwner,
}
