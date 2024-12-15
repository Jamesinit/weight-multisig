use anchor_lang::prelude::*;
use anchor_lang::solana_program::{instruction::Instruction, program::invoke_signed};
use crate::state::*;
use crate::error::ErrorCode;
use crate::constants::*;

#[derive(Accounts)]
pub struct ChangeThreshold<'info> {
    #[account(mut)]
    pub wallet: Account<'info, Wallet>,
    pub proposer: Signer<'info>,
}

#[derive(Accounts)]
pub struct ChangeOwnerWeight<'info> {
    #[account(mut)]
    pub wallet: Account<'info, Wallet>,
    pub proposer: Signer<'info>,
}

#[derive(Accounts)]
pub struct ChangeOwnerWeights<'info> {
    #[account(mut)]
    pub wallet: Account<'info, Wallet>,
    pub proposer: Signer<'info>,
}

#[derive(Accounts)]
#[instruction(owners: Vec<OwnerConfig>)]
pub struct CreateWallet<'info> {
    #[account(
        init,
        payer = payer,
        space = 8 + // discriminator
            4 + (OwnerConfig::LEN * owners.len()) + // owners vec with length prefix
            8 + // threshold_weight
            1 + // nonce
            4   // owner_set_seqno
    )]
    pub wallet: Account<'info, Wallet>,

    #[account(
        seeds = [b"vault", wallet.key().as_ref()],
        bump,
    )]
    /// CHECK: This is a PDA that will hold SOL
    pub vault: UncheckedAccount<'info>,

    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(
    instructions: Vec<ProposedInstruction>,
    max_accounts_per_instruction: u8,
    max_data_size: u16
)]
pub struct CreateTransaction<'info> {
    pub wallet: Account<'info, Wallet>,

    #[account(
        init,
        payer = owner,
        space = 8 + // discriminator
            32 + // wallet pubkey
            32 + // creator
            1 + // executed
            4 + (32 * MAX_SIGNERS) + // signers vec with length prefix
            4 + // owner_set_seqno
            4 + (ProposedInstruction::size(max_accounts_per_instruction as usize, max_data_size as usize) * MAX_INSTRUCTIONS) // instructions vec with length prefix
    )]
    pub transaction: Account<'info, Transaction>,

    #[account(mut)]
    pub owner: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Approve<'info> {
    pub wallet: Account<'info, Wallet>,
    #[account(mut)]
    pub transaction: Account<'info, Transaction>,
    pub owner: Signer<'info>,
}

#[derive(Accounts)]
pub struct ExecuteTransaction<'info> {
    /// Multisig wallet account
    pub wallet: Account<'info, Wallet>,

    /// Transaction proposal account
    #[account(
        mut,
        constraint = transaction.wallet == wallet.key() @ ErrorCode::InvalidWallet,
        constraint = !transaction.executed @ ErrorCode::AlreadyExecuted,
        constraint = wallet.owner_set_seqno == transaction.owner_set_seqno @ ErrorCode::OwnerSetChanged,
        has_one = wallet @ ErrorCode::InvalidWallet
    )]
    pub transaction: Account<'info, Transaction>,

    /// Executor (must be an owner and have signed)
    #[account(
        constraint = wallet.owners.iter().any(|o| o.key == owner.key()) @ ErrorCode::NotOwner,
        constraint = transaction.signers.contains(&owner.key()) @ ErrorCode::NotSigned
    )]
    pub owner: Signer<'info>,

    /// Vault PDA account
    #[account(
        mut,  // Ensure vault is writable
        seeds = [b"vault", wallet.key().as_ref()],
        bump = wallet.nonce,
    )]
    /// CHECK: Vault PDA, will be used as a signer
    pub vault: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct CloseTransaction<'info> {
    pub wallet: Account<'info, Wallet>,

    #[account(
        mut,
        constraint = transaction.wallet == wallet.key() @ ErrorCode::InvalidWallet,
        constraint = transaction.executed @ ErrorCode::TransactionNotExecuted,
        close = recipient // This will close the account after instruction execution and transfer remaining rent to recipient
    )]
    pub transaction: Account<'info, Transaction>,

    #[account(mut)]
    pub recipient: SystemAccount<'info>,

    // Optional: only allow transaction creator to close the account
    #[account(constraint = owner.key() == transaction.creator @ ErrorCode::UnauthorizedClose)]
    pub owner: Signer<'info>,
}