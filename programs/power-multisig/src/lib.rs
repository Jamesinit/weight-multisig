use anchor_lang::prelude::*;
use anchor_lang::solana_program::{instruction::Instruction, program::invoke_signed};
declare_id!("U8QgybKox2a31mTqKrpywzotFZ1nAqvk7erYTByDxui");

pub mod constants;
pub mod error;
pub mod instructions;
pub mod state;

use constants::*;
use error::ErrorCode;
use instructions::*;
use state::*;

#[program]
pub mod multisig_wallet {
    use super::*;

    pub fn create_wallet(
        ctx: Context<CreateWallet>,
        owners: Vec<OwnerConfig>,
        threshold_weight: u64,
    ) -> Result<()> {
        // Validate owners configuration
        validate_owners(&owners, threshold_weight)?;

        let wallet = &mut ctx.accounts.wallet;
        wallet.owners = owners;
        wallet.threshold_weight = threshold_weight;
        wallet.nonce = ctx.bumps.vault;
        wallet.owner_set_seqno = 0;

        Ok(())
    }

    pub fn create_transaction(
        ctx: Context<CreateTransaction>,
        instructions: Vec<ProposedInstruction>,
        max_accounts_per_instruction: u8,
        max_data_size: u16,
    ) -> Result<()> {
        // Validate transaction instructions
        validate_instructions(&instructions, max_accounts_per_instruction, max_data_size)?;

        let wallet = &ctx.accounts.wallet;
        let owner = &ctx.accounts.owner;
        require!(wallet.is_owner(&owner.key()), ErrorCode::NotOwner);

        let transaction = &mut ctx.accounts.transaction;
        transaction.initialize(
            instructions,
            wallet.key(),
            owner.key(),
            wallet.owner_set_seqno,
        );

        Ok(())
    }

    pub fn approve(ctx: Context<Approve>) -> Result<()> {
        let wallet = &ctx.accounts.wallet;
        let transaction = &mut ctx.accounts.transaction;
        let signer = &ctx.accounts.owner;

        validate_approval(wallet, transaction, signer)?;

        transaction.signers.push(signer.key());
        Ok(())
    }

    pub fn execute_transaction(ctx: Context<ExecuteTransaction>) -> Result<()> {
        let wallet = &ctx.accounts.wallet;
        let transaction = &mut ctx.accounts.transaction;
        let vault = &ctx.accounts.vault;

        validate_execution(wallet, transaction)?;

        // Prepare PDA signer seeds
        let seeds = &[
            VAULT_SEED,
            wallet.to_account_info().key.as_ref(),
            &[wallet.nonce],
        ];
        let signer_seeds = &[&seeds[..]];

        // Execute each instruction in the transaction
        for (i, instruction) in transaction.instructions.iter().enumerate() {
            msg!("Processing instruction {}", i);

            // Find vault's position in accounts list
            let vault_index = instruction
                .accounts
                .iter()
                .position(|acc| acc.pubkey == vault.key())
                .ok_or(ErrorCode::AccountNotFound)?;

            // Prepare account metas with vault as signer
            let accounts_metas: Vec<AccountMeta> = instruction
                .accounts
                .iter()
                .enumerate()
                .map(|(idx, acc)| {
                    if idx == vault_index {
                        AccountMeta::new(acc.pubkey, true)
                    } else {
                        acc.to_account_meta()
                    }
                })
                .collect();

            let ix = Instruction {
                program_id: instruction.program_id,
                accounts: accounts_metas,
                data: instruction.data.clone(),
            };

            // Execute CPI call
            invoke_signed(&ix, ctx.remaining_accounts, signer_seeds)
                .map_err(|_| error!(ErrorCode::TransactionExecutionFailed))?;

            msg!("Instruction {} executed successfully", i);
        }

        transaction.executed = true;
        Ok(())
    }

    pub fn close_transaction(_ctx: Context<CloseTransaction>) -> Result<()> {
        // Close account and return rent
        msg!("Closing transaction account and returning rent to recipient");
        Ok(())
    }

    // Modify threshold weight for the wallet
    pub fn change_threshold(ctx: Context<ChangeThreshold>, new_threshold: u64) -> Result<()> {
        let wallet = &mut ctx.accounts.wallet;
        let total_weight: u64 = wallet.owners.iter().map(|owner| owner.weight).sum();

        // Validate new threshold
        require!(new_threshold > 0, ErrorCode::InvalidThreshold);
        require!(new_threshold <= total_weight, ErrorCode::ThresholdTooHigh);

        // Update threshold and increment sequence number
        wallet.threshold_weight = new_threshold;
        wallet.owner_set_seqno += 1;

        Ok(())
    }

    // Modify weight for a single owner
    pub fn change_owner_weight(
        ctx: Context<ChangeOwnerWeight>,
        owner_key: Pubkey,
        new_weight: u64,
    ) -> Result<()> {
        let wallet = &mut ctx.accounts.wallet;

        // Validate new weight
        require!(new_weight > 0, ErrorCode::InvalidOwnerWeight);

        // Find and update owner weight
        if let Some(owner) = wallet.owners.iter_mut().find(|o| o.key == owner_key) {
            owner.weight = new_weight;

            // Calculate new total weight
            let total_weight: u64 = wallet.owners.iter().map(|o| o.weight).sum();

            // Ensure threshold remains valid
            require!(
                wallet.threshold_weight <= total_weight,
                ErrorCode::ThresholdTooHigh
            );

            wallet.owner_set_seqno += 1;
        } else {
            return err!(ErrorCode::OwnerNotFound);
        }

        Ok(())
    }

    // Update entire weight configuration
    pub fn change_owner_weights(
        ctx: Context<ChangeOwnerWeights>,
        new_weights: Vec<OwnerConfig>,
    ) -> Result<()> {
        let wallet = &mut ctx.accounts.wallet;

        // Verify all existing owners are included
        require!(
            wallet.owners.len() == new_weights.len(),
            ErrorCode::InvalidOwnerCount
        );

        // Validate new weight configuration
        for new_config in new_weights.iter() {
            require!(
                wallet.owners.iter().any(|o| o.key == new_config.key),
                ErrorCode::OwnerNotFound
            );
            require!(new_config.weight > 0, ErrorCode::InvalidOwnerWeight);
        }

        // Calculate new total weight
        let new_total_weight: u64 = new_weights.iter().map(|o| o.weight).sum();
        require!(
            wallet.threshold_weight <= new_total_weight,
            ErrorCode::ThresholdTooHigh
        );

        // Update weights and increment sequence
        wallet.owners = new_weights;
        wallet.owner_set_seqno += 1;

        Ok(())
    }
}

// Calculate total signing weight
fn calculate_total_weight(wallet: &Account<Wallet>, signers: &[Pubkey]) -> Result<u64> {
    let mut total_weight = 0u64;

    for signer in signers.iter() {
        if let Some(owner) = wallet.owners.iter().find(|o| o.key == *signer) {
            total_weight = total_weight
                .checked_add(owner.weight)
                .ok_or(ErrorCode::ArithmeticOverflow)?;
        }
    }

    Ok(total_weight)
}

// Helper validation functions
fn validate_owners(owners: &[OwnerConfig], threshold_weight: u64) -> Result<()> {
    require!(!owners.is_empty(), ErrorCode::NoOwners);
    assert_unique_owners(owners)?;
    require!(threshold_weight > 0, ErrorCode::InvalidThreshold);

    let total_weight: u64 = owners.iter().map(|owner| owner.weight).sum();
    require!(
        threshold_weight <= total_weight,
        ErrorCode::ThresholdTooHigh
    );

    Ok(())
}

fn validate_instructions(
    instructions: &[ProposedInstruction],
    max_accounts_per_instruction: u8,
    max_data_size: u16,
) -> Result<()> {
    require!(
        instructions.len() <= MAX_INSTRUCTIONS,
        ErrorCode::TooManyInstructions
    );

    for instruction in instructions {
        require!(
            instruction.accounts.len() <= max_accounts_per_instruction as usize,
            ErrorCode::TooManyAccounts
        );
        require!(
            instruction.data.len() <= max_data_size as usize,
            ErrorCode::DataTooLarge
        );
    }

    Ok(())
}

fn validate_approval(
    wallet: &Account<Wallet>,
    transaction: &Account<Transaction>,
    signer: &Signer,
) -> Result<()> {
    require!(wallet.is_owner(&signer.key()), ErrorCode::NotOwner);
    require!(!transaction.executed, ErrorCode::AlreadyExecuted);
    require!(
        wallet.owner_set_seqno == transaction.owner_set_seqno,
        ErrorCode::OwnerSetChanged
    );
    require!(
        !transaction.signers.contains(&signer.key()),
        ErrorCode::AlreadySigned
    );

    Ok(())
}

fn validate_execution(wallet: &Account<Wallet>, transaction: &Account<Transaction>) -> Result<()> {
    let total_weight = calculate_total_weight(wallet, &transaction.signers)?;
    require!(
        total_weight >= wallet.threshold_weight,
        ErrorCode::InsufficientSigners
    );
    Ok(())
}

fn assert_unique_owners(owners: &[OwnerConfig]) -> Result<()> {
    for (i, owner) in owners.iter().enumerate() {
        // Check for non-zero weight
        require!(owner.weight > 0, ErrorCode::InvalidOwnerWeight);

        // Check for duplicates
        require!(
            !owners.iter().skip(i + 1).any(|item| item.key == owner.key),
            ErrorCode::DuplicateOwner
        );
    }
    Ok(())
}