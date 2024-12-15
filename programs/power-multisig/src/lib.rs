use anchor_lang::prelude::*;
use anchor_lang::solana_program::{system_instruction, program::invoke_signed};
declare_id!("U8QgybKox2a31mTqKrpywzotFZ1nAqvk7erYTByDxui");

#[program]
pub mod multisig_wallet {
    use super::*;

    // 创建多签钱包
    pub fn create_wallet(
        ctx: Context<CreateWallet>,
        owners: Vec<OwnerConfig>,
        threshold_weight: u64,
    ) -> Result<()> {
        require!(threshold_weight > 0, ErrorCode::InvalidThreshold);
        
        // 计算所有者的总权重
        let total_weight: u64 = owners.iter()
            .map(|owner| owner.weight)
            .sum();
            
        require!(threshold_weight <= total_weight, ErrorCode::ThresholdTooHigh);
        require!(!owners.is_empty(), ErrorCode::NoOwners);

        let wallet = &mut ctx.accounts.wallet;
        wallet.owners = owners;
        wallet.threshold_weight = threshold_weight;
        wallet.nonce = ctx.bumps.vault;
        
        Ok(())
    }

    // 执行转账
    pub fn execute_transfer(
        ctx: Context<ExecuteTransfer>,
        amount: u64,
        signatures: Vec<Pubkey>,
    ) -> Result<()> {
        let wallet = &ctx.accounts.wallet;
        
        // 计算签名的总权重
        let mut total_signed_weight = 0u64;
        for signer_key in signatures.iter() {
            if let Some(owner) = wallet.owners.iter().find(|o| o.key == *signer_key) {
                total_signed_weight += owner.weight;
            }
        }

        // 验证签名权重是否达到阈值
        require!(
            total_signed_weight >= wallet.threshold_weight,
            ErrorCode::InsufficientSigners
        );

        // 验证当前交易签名者是否在签名列表中
        require!(
            signatures.contains(&ctx.accounts.owner.key()),
            ErrorCode::InvalidSigner
        );

        // 使用PDA签名执行转账
        let seeds = &[
            b"vault",
            wallet.to_account_info().key.as_ref(),
            &[wallet.nonce],
        ];
        let signer_seeds = &[&seeds[..]];

        invoke_signed(
            &system_instruction::transfer(
                &ctx.accounts.vault.key(),
                &ctx.accounts.receiver.key(),
                amount,
            ),
            &[
                ctx.accounts.vault.to_account_info(),
                ctx.accounts.receiver.to_account_info(),
                ctx.accounts.system_program.to_account_info(),
            ],
            signer_seeds
        )?;

        Ok(())
    }

    // 更新所有者权重
    pub fn update_owner_weights(
        ctx: Context<UpdateOwners>,
        new_owners: Vec<OwnerConfig>,
        new_threshold_weight: u64,
    ) -> Result<()> {
        require!(new_threshold_weight > 0, ErrorCode::InvalidThreshold);
        
        let total_weight: u64 = new_owners.iter()
            .map(|owner| owner.weight)
            .sum();
            
        require!(new_threshold_weight <= total_weight, ErrorCode::ThresholdTooHigh);
        require!(!new_owners.is_empty(), ErrorCode::NoOwners);

        let wallet = &mut ctx.accounts.wallet;
        wallet.owners = new_owners;
        wallet.threshold_weight = new_threshold_weight;
        
        Ok(())
    }
}

#[derive(Accounts)]
pub struct CreateWallet<'info> {
    #[account(
        init,
        payer = payer,
        space = 8 + // discriminator
            (32 + 8) * 10 + // owner pubkey + weight (预留10个owner)
            8 + // threshold_weight
            1 // nonce
    )]
    pub wallet: Account<'info, Wallet>,
    
    #[account(
        seeds = [b"vault", wallet.key().as_ref()],
        bump,
    )]
    /// CHECK: This is a PDA that will hold SOL
    pub vault: AccountInfo<'info>,
    
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ExecuteTransfer<'info> {
    pub wallet: Account<'info, Wallet>,
    
    #[account(
        mut,
        seeds = [b"vault", wallet.key().as_ref()],
        bump = wallet.nonce,
    )]
    /// CHECK: This is a PDA that holds SOL
    pub vault: AccountInfo<'info>,
    
    /// CHECK: This is the receiver of the SOL
    #[account(mut)]
    pub receiver: AccountInfo<'info>,
    
    pub owner: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct UpdateOwners<'info> {
    #[account(mut)]
    pub wallet: Account<'info, Wallet>,
    pub owner: Signer<'info>,
}

#[account]
pub struct Wallet {
    pub owners: Vec<OwnerConfig>,
    pub threshold_weight: u64,
    pub nonce: u8,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct OwnerConfig {
    pub key: Pubkey,
    pub weight: u64,
}

#[error_code]
pub enum ErrorCode {
    #[msg("Threshold must be greater than 0")]
    InvalidThreshold,
    #[msg("Threshold must be less than or equal to the total weight")]
    ThresholdTooHigh,
    #[msg("No owners provided")]
    NoOwners,
    #[msg("Insufficient signers weight")]
    InsufficientSigners,
    #[msg("Invalid signer")]
    InvalidSigner,
}