use anchor_lang::prelude::*;
use anchor_lang::solana_program::{system_instruction, program::invoke_signed};
declare_id!("U8QgybKox2a31mTqKrpywzotFZ1nAqvk7erYTByDxui");

#[program]
pub mod multisig_wallet {
    use super::*;

    // 创建多签钱包
    pub fn create_wallet(
        ctx: Context<CreateWallet>,
        owners: Vec<Pubkey>,
        threshold: u64,
    ) -> Result<()> {
        let wallet = &mut ctx.accounts.wallet;
        wallet.owners = owners;
        wallet.threshold = threshold;
        wallet.nonce = ctx.bumps.vault;
        Ok(())
    }

    // 执行转账
    pub fn execute_transfer(
        ctx: Context<ExecuteTransfer>,
        amount: u64,
    ) -> Result<()> {
        let wallet = &ctx.accounts.wallet;
        
        // 验证签名者是否是owner
        let signer = ctx.accounts.owner.key();
        require!(
            wallet.owners.contains(&signer),
            ErrorCode::NotOwner
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
}

#[derive(Accounts)]
pub struct CreateWallet<'info> {
    #[account(
        init,
        payer = payer,
        space = 8 + 32 * 10 + 8 + 1 // 预留10个owner + threshold + nonce
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

#[account]
pub struct Wallet {
    pub owners: Vec<Pubkey>,
    pub threshold: u64,
    pub nonce: u8,
}

#[error_code]
pub enum ErrorCode {
    #[msg("Not an owner of the wallet")]
    NotOwner,
}