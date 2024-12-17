use anchor_lang::prelude::*;

#[error_code]
pub enum MultisigError {
    // 所有者相关错误
    #[msg("Invalid owner")]
    InvalidOwner,
    #[msg("Owner not found")]
    OwnerNotFound,
    #[msg("Owner already exists")]
    OwnerAlreadyExists,
    #[msg("Maximum number of owners exceeded")]
    TooManyOwners,
    
    // 权重相关错误
    #[msg("Invalid weight threshold")]
    InvalidWeightThreshold,
    #[msg("Not enough weight to execute")]
    NotEnoughWeight,
    #[msg("Overflow in weight calculation")]
    WeightOverflow,
    
    // 交易状态错误
    #[msg("Transaction not found")]
    TransactionNotFound,
    #[msg("Transaction already executed")]
    AlreadyExecuted,
    #[msg("Transaction has expired")]
    TransactionExpired,
    #[msg("Transaction already cancelled")]
    AlreadyCancelled,
    #[msg("Invalid transaction state")]
    InvalidTransactionState,
    
    // 签名相关错误
    #[msg("Owner already signed")]
    AlreadySigned,
    #[msg("Invalid signature")]
    InvalidSignature,
    
    // 队列相关错误
    #[msg("No pending transactions")]
    NoPendingTransactions,
    #[msg("Pending transaction queue is full")]
    PendingQueueFull,
    #[msg("Invalid transaction index")]
    InvalidTransactionIndex,
    
    // 参数验证错误
    #[msg("Invalid name length")]
    InvalidNameLength,
    #[msg("Invalid expiry time")]
    InvalidExpiryTime,
    #[msg("Invalid instruction data")]
    InvalidInstructionData,
    
    // 权限错误
    #[msg("Unauthorized")]
    Unauthorized,
    #[msg("Only proposer can cancel")]
    NotProposer,
    
    // 账户状态错误
    #[msg("Account already initialized")]
    AlreadyInitialized,
    #[msg("Invalid account state")]
    InvalidAccountState,
    #[msg("Insufficient wallet balance")]
    InsufficientBalance,
    
    // 系统限制错误
    #[msg("Exceeded maximum accounts limit")]
    TooManyAccounts,
    #[msg("Exceeded maximum data size")]
    DataTooLarge,
    
    // 时间相关错误
    #[msg("Invalid timestamp")]
    InvalidTimestamp,
    #[msg("Operation time window expired")]
    TimeWindowExpired,
    InsufficientFunds
}