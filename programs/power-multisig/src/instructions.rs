use anchor_lang::prelude::*;
use crate::errors::MultisigError;
use crate::state::*;
use crate::constants::*;

// ============= Create Multisig =============
#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct CreateMultisigArgs {
    pub name: String,
    pub min_weight_required: u64,
    pub owners: Vec<OwnerInfo>,
}

#[derive(Accounts)]
#[instruction(args: CreateMultisigArgs)]
pub struct CreateMultisig<'info> {
    #[account(
        init,
        payer = payer,
        space = 8  // discriminator
            + 32   // base
            + 1    // bump
            + 4 + args.name.len()  // name (String)
            + 8    // min_weight_required
            + 8    // total_weight
            + 4    // owner_set_seqno
            + 4 + (32 + 8) * args.owners.len()  // Vec<OwnerInfo>
            + 1    // num_owners
            + 8    // transaction_count
            + 8    // pending_count
            + 4 + (8 + 32 + 8 + 32) * 32, // pending_transactions (Vec<PendingTransactionInfo>，预留32个空间)
        seeds = [MULTISIG_SEED, base.key().as_ref()],
        bump
    )]
    pub wallet: Account<'info, MultisigWallet>,
    
    pub base: Signer<'info>,
    
    #[account(mut)]
    pub payer: Signer<'info>,
    
    pub system_program: Program<'info, System>,
}

// ============= Create Transaction =============
#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct CreateTransactionArgs {
    pub destination: Pubkey,         // 接收方地址
    pub amount: u64,                 // 转账金额
    pub expires_at: Option<i64>,     // 过期时间（可选）
}

#[derive(Accounts)]
#[instruction(args: CreateTransactionArgs)]
pub struct CreateTransaction<'info> {
    #[account(
        mut,
        seeds = [MULTISIG_SEED, wallet.base.as_ref()],
        bump = wallet.bump,
    )]
    pub wallet: Account<'info, MultisigWallet>,
    
    #[account(
        init,
        payer = payer,
        space = 8  // discriminator
            + 32   // wallet
            + 8    // transaction_index
            + 1    // bump
            + 32   // proposer
            + (32 + 8 + 1) * MAX_ACCOUNTS  // instruction accounts
            + 4 + MAX_DATA_SIZE            // instruction data
            + 32   // program_id
            + 1    // status (enum)
            + 8    // current_weight
            + 4 + (32 * MAX_SIGNERS)      // approvals
            + 8    // created_at
            + 9    // expires_at (Option<i64>)
            + 9,   // executed_at (Option<i64>)
        seeds = [
            TRANSACTION_SEED,
            wallet.key().as_ref(),
            wallet.transaction_count.to_le_bytes().as_ref()
        ],
        bump
    )]
    pub transaction: Account<'info, Transaction>,
    
    #[account(mut)]
    pub proposer: Signer<'info>,
    
    #[account(mut)]
    pub payer: Signer<'info>,
    
    pub system_program: Program<'info, System>,
}

// ============= Sign Transaction =============
#[derive(Accounts)]
pub struct SignTransaction<'info> {
    #[account(
        mut,
        seeds = [MULTISIG_SEED, wallet.base.as_ref()],
        bump = wallet.bump
    )]
    pub wallet: Account<'info, MultisigWallet>,
    
    #[account(
        mut,
        seeds = [
            TRANSACTION_SEED,
            wallet.key().as_ref(),
            transaction.transaction_index.to_le_bytes().as_ref()
        ],
        bump = transaction.bump,
        constraint = transaction.wallet == wallet.key(),
        constraint = matches!(transaction.status, TransactionStatus::Pending) 
            @ MultisigError::InvalidTransactionState,
    )]
    pub transaction: Account<'info, Transaction>,
    
    pub owner: Signer<'info>,
}

// ============= Execute Transaction =============
#[derive(Accounts)]
#[instruction(transaction_index: u64)]
pub struct ExecuteTransaction<'info> {
    #[account(
        mut,
        seeds = [MULTISIG_SEED, wallet.base.as_ref()],
        bump = wallet.bump
    )]
    pub wallet: Account<'info, MultisigWallet>,
    
    #[account(
        mut,
        seeds = [
            TRANSACTION_SEED,
            wallet.key().as_ref(),
            transaction.transaction_index.to_le_bytes().as_ref()
        ],
        bump = transaction.bump,
        constraint = transaction.wallet == wallet.key(),
        constraint = transaction.transaction_index == transaction_index,
        constraint = matches!(transaction.status, TransactionStatus::Pending),
        constraint = transaction.current_weight >= wallet.min_weight_required
    )]
    pub transaction: Account<'info, Transaction>,
    
    /// CHECK: Transfer destination
    #[account(mut)]
    pub destination: AccountInfo<'info>,
    
    pub executor: Signer<'info>,
    
    pub system_program: Program<'info, System>,
}
// ============= Get Pending Transactions =============
#[derive(Accounts)]
pub struct GetPendingTransactions<'info> {
    #[account(
        seeds = [MULTISIG_SEED, wallet.base.as_ref()],
        bump = wallet.bump
    )]
    pub wallet: Account<'info, MultisigWallet>,
    
    pub system_program: Program<'info, System>,
}
// ============= Cancel Transaction =============
#[derive(Accounts)]
pub struct CancelTransaction<'info> {
    #[account(
        mut,
        seeds = [MULTISIG_SEED, wallet.base.as_ref()],
        bump = wallet.bump
    )]
    pub wallet: Account<'info, MultisigWallet>,
    
    #[account(
        mut,
        seeds = [
            TRANSACTION_SEED,
            wallet.key().as_ref(),
            transaction.transaction_index.to_le_bytes().as_ref()
        ],
        bump = transaction.bump,
        constraint = transaction.wallet == wallet.key(),
        constraint = matches!(transaction.status, TransactionStatus::Pending) 
            @ MultisigError::InvalidTransactionState,
    )]
    pub transaction: Account<'info, Transaction>,
    
    #[account(constraint = transaction.proposer == proposer.key())]
    pub proposer: Signer<'info>,
}
