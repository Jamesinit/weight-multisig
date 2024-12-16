use anchor_lang::prelude::*;
use crate::errors::MultisigError;
use crate::state::*;


#[derive(Accounts)]
#[instruction(owners: Vec<Pubkey>, threshold: u8)]
pub struct CreateMultisig<'info> {
    #[account(
        init,
        payer = payer,
        space = 8 + // discriminator
        4 + owners.len() * 32 + // Vec<Pubkey> owners
        1 + // threshold
        1 + // nonce
        4,  // transaction_count,
        seeds = [b"multisig", payer.key().as_ref()],
        bump
    )]
    pub multisig: Account<'info, Multisig>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(program_id: Pubkey, accounts: Vec<TransactionAccount>, data: Vec<u8>)]
pub struct ProposeTransaction<'info> {
    #[account(constraint = multisig.owners.contains(proposer.key) @ MultisigError::NotAnOwner)]
    pub multisig: Account<'info, Multisig>,
    #[account(
        init,
        payer = proposer,
        space = 8 + // discriminator
            32 + // program_id
            4 + accounts.len() * std::mem::size_of::<TransactionAccount>() + // accounts
            4 + data.len() + // data
            4 + 32 * multisig.owners.len() + // signers
            1 + // did_execute
            32 + // owner
            1,  // nonce
        seeds = [
            b"transaction",
            multisig.key().as_ref(),
            &multisig.transaction_count.to_le_bytes()
        ],
        bump
    )]
    pub transaction: Account<'info, Transaction>,
    #[account(mut)]
    pub proposer: Signer<'info>,
    pub system_program: Program<'info, System>,
}
#[derive(Accounts)]
pub struct Approve<'info> {
    #[account(constraint = multisig.owners.contains(owner.key) @ MultisigError::NotAnOwner)]
    pub multisig: Account<'info, Multisig>,
    #[account(
        mut,
        constraint = transaction.owner == multisig.key() @ MultisigError::InvalidOwner,
        constraint = !transaction.did_execute @ MultisigError::AlreadyExecuted
    )]
    pub transaction: Account<'info, Transaction>,
    pub owner: Signer<'info>,
}

#[derive(Accounts)]
pub struct ExecuteTransaction<'info> {
    #[account(mut)]
    pub multisig: Account<'info, Multisig>,
    #[account(
        mut,
        constraint = transaction.owner == multisig.key() @ MultisigError::InvalidOwner,
        constraint = !transaction.did_execute @ MultisigError::AlreadyExecuted,
        constraint = transaction.signers.len() >= multisig.threshold as usize 
            @ MultisigError::NotEnoughSigners
    )]
    pub transaction: Account<'info, Transaction>,
    /// CHECK: 仅接收 SOL
    #[account(mut)]
    pub to: AccountInfo<'info>,
    #[account(constraint = multisig.owners.contains(&owner.key()) @ MultisigError::NotAnOwner)]
    pub owner: Signer<'info>,
    pub system_program: Program<'info, System>,
}