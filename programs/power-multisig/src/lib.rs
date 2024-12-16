use anchor_lang::prelude::*;
use anchor_lang::solana_program::{
    instruction::Instruction,
    program::invoke_signed,
    system_instruction,
};
use anchor_lang::system_program;
pub mod errors;
pub mod state;
pub mod  instructions;
use errors::MultisigError;

use instructions::*;
use state::*;
declare_id!("U8QgybKox2a31mTqKrpywzotFZ1nAqvk7erYTByDxui");



#[program]
pub mod multisig_wallet {
    use super::*;

    pub fn create_multisig(
        ctx: Context<CreateMultisig>,
        owners: Vec<Member>,
        threshold: u64,
    ) -> Result<()> {
        require!(threshold > 0, MultisigError::InvalidThreshold);
        require!(threshold as usize <= owners.len(), MultisigError::InvalidThreshold);
        require!(!owners.is_empty(), MultisigError::NoOwners);
        
        Ok(())
    }

    pub fn propose_transaction(
        ctx: Context<ProposeTransaction>,
    ) -> Result<()> {
        let transaction = &mut ctx.accounts.transaction;
        Ok(())
    }

    pub fn approve(ctx: Context<Approve>) -> Result<()> {
        Ok(())
    }
    pub fn execute_transaction(ctx: Context<ExecuteTransaction>) -> Result<()> {
        Ok(())
    }

}