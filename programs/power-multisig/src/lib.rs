use anchor_lang::prelude::*;
use anchor_lang::solana_program::{instruction::Instruction, program::invoke_signed};
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
        
        let total_weight: u64 = owners.iter()
            .map(|owner| owner.weight)
            .sum();
            
        require!(threshold_weight <= total_weight, ErrorCode::ThresholdTooHigh);
        require!(!owners.is_empty(), ErrorCode::NoOwners);

        let wallet = &mut ctx.accounts.wallet;
        wallet.owners = owners;
        wallet.threshold_weight = threshold_weight;
        wallet.nonce = ctx.bumps.vault;
        wallet.transaction_count = 0;
        
        Ok(())
    }

    // 创建交易提案
    pub fn create_transaction(
        ctx: Context<CreateTransaction>,
        program_id: Pubkey,
        accounts: Vec<TransactionAccount>,
        data: Vec<u8>,
    ) -> Result<()> {
        let wallet = &ctx.accounts.wallet;
        let transaction = &mut ctx.accounts.transaction;
        let owner = &ctx.accounts.owner;

        // 验证创建者是否是owner
        require!(
            wallet.owners.iter().any(|o| o.key == owner.key()),
            ErrorCode::NotOwner
        );

        // 初始化交易数据
        transaction.program_id = program_id;
        transaction.accounts = accounts;
        transaction.data = data;
        transaction.wallet = wallet.key();
        transaction.executed = false;
        transaction.signers = vec![owner.key()];
        
        Ok(())
    }

    // 为交易提案签名
    pub fn sign_transaction(
        ctx: Context<SignTransaction>
    ) -> Result<()> {
        let wallet = &ctx.accounts.wallet;
        let transaction = &mut ctx.accounts.transaction;
        let signer = &ctx.accounts.owner;

        // 验证签名者是否是owner
        require!(
            wallet.owners.iter().any(|o| o.key == signer.key()),
            ErrorCode::NotOwner
        );

        // 验证交易未执行
        require!(!transaction.executed, ErrorCode::AlreadyExecuted);

        // 验证未重复签名
        require!(
            !transaction.signers.contains(&signer.key()),
            ErrorCode::AlreadySigned
        );

        transaction.signers.push(signer.key());

        Ok(())
    }

    // 执行交易
    pub fn execute_transaction(
        ctx: Context<ExecuteTransaction>
    ) -> Result<()> {
        let wallet = &ctx.accounts.wallet;
        let transaction = &mut ctx.accounts.transaction;
        
        // 验证交易未执行
        require!(!transaction.executed, ErrorCode::AlreadyExecuted);

        // 计算签名权重
        let mut total_weight = 0u64;
        for signer in transaction.signers.iter() {
            if let Some(owner) = wallet.owners.iter().find(|o| o.key == *signer) {
                total_weight += owner.weight;
            }
        }

        // 验证权重是否达到阈值
        require!(
            total_weight >= wallet.threshold_weight,
            ErrorCode::InsufficientSigners
        );

        // 构建指令
        let ix = Instruction {
            program_id: transaction.program_id,
            accounts: transaction.accounts.iter().map(|acc| acc.to_account_meta()).collect(),
            data: transaction.data.clone(),
        };

        // 如果需要PDA签名，添加seeds
        let vault_seeds = &[
            b"vault",
            wallet.to_account_info().key.as_ref(),
            &[wallet.nonce],
        ];
        let signer_seeds = &[&vault_seeds[..]];

        // 执行指令
        invoke_signed(
            &ix,
            ctx.remaining_accounts,
            signer_seeds,
        )?;

        transaction.executed = true;
        
        Ok(())
    }
}

#[derive(Accounts)]
pub struct CreateWallet<'info> {
    #[account(
        init,
        payer = payer,
        space = 8 + // discriminator
            (32 + 8) * 10 + // owner pubkey + weight (10 owners)
            8 + // threshold_weight
            1 + // nonce
            8   // transaction_count
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
pub struct CreateTransaction<'info> {
    pub wallet: Account<'info, Wallet>,
    
    #[account(
        init,
        payer = owner,
        space = 8 + // discriminator
            32 + // program_id
            4 + (32 + 1 + 1) * 10 + // accounts vec (10 accounts max)
            4 + 1024 + // data vec (1KB max)
            32 + // wallet
            1 + // executed
            4 + 32 * 10 // signers vec (10 signers max)
    )]
    pub transaction: Account<'info, Transaction>,
    
    #[account(mut)]
    pub owner: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct SignTransaction<'info> {
    pub wallet: Account<'info, Wallet>,
    #[account(mut)]
    pub transaction: Account<'info, Transaction>,
    pub owner: Signer<'info>,
}

#[derive(Accounts)]
pub struct ExecuteTransaction<'info> {
    pub wallet: Account<'info, Wallet>,
    #[account(mut)]
    pub transaction: Account<'info, Transaction>,
    pub owner: Signer<'info>,
}

#[account]
pub struct Wallet {
    pub owners: Vec<OwnerConfig>,
    pub threshold_weight: u64,
    pub nonce: u8,
    pub transaction_count: u64,
}

#[account]
pub struct Transaction {
    pub program_id: Pubkey,
    pub accounts: Vec<TransactionAccount>,
    pub data: Vec<u8>,
    pub wallet: Pubkey,
    pub executed: bool,
    pub signers: Vec<Pubkey>,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct OwnerConfig {
    pub key: Pubkey,
    pub weight: u64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct TransactionAccount {
    pub pubkey: Pubkey,
    pub is_signer: bool,
    pub is_writable: bool,
}

impl TransactionAccount {
    fn to_account_meta(&self) -> anchor_lang::solana_program::instruction::AccountMeta {
        anchor_lang::solana_program::instruction::AccountMeta {
            pubkey: self.pubkey,
            is_signer: self.is_signer,
            is_writable: self.is_writable,
        }
    }
}

#[error_code]
pub enum ErrorCode {
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
}