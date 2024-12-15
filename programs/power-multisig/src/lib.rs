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
        let total_weight: u64 = owners.iter().map(|owner| owner.weight).sum();
        require!(threshold_weight <= total_weight, ErrorCode::ThresholdTooHigh);
        require!(!owners.is_empty(), ErrorCode::NoOwners);
        
        // 确保owner不重复
        assert_unique_owners(&owners)?;

        let wallet = &mut ctx.accounts.wallet;
        wallet.owners = owners;
        wallet.threshold_weight = threshold_weight;
        wallet.nonce = ctx.bumps.vault;
        wallet.owner_set_seqno = 0;
        
        Ok(())
    }

    // 创建交易提案，支持多个指令
    pub fn create_transaction(
        ctx: Context<CreateTransaction>,
        instructions: Vec<ProposedInstruction>,
    ) -> Result<()> {
        let wallet = &ctx.accounts.wallet;
        let owner = &ctx.accounts.owner;

        // 验证提案者是否是owner
        require!(
            wallet.owners.iter().any(|o| o.key == owner.key()),
            ErrorCode::NotOwner
        );

        let transaction = &mut ctx.accounts.transaction;
        transaction.instructions = instructions;
        transaction.wallet = wallet.key();
        transaction.executed = false;
        transaction.signers = vec![owner.key()];
        transaction.owner_set_seqno = wallet.owner_set_seqno;
        
        Ok(())
    }

    // 为交易提案签名
    pub fn approve(ctx: Context<Approve>) -> Result<()> {
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

        // 验证owner set没有变化
        require!(
            wallet.owner_set_seqno == transaction.owner_set_seqno,
            ErrorCode::OwnerSetChanged
        );

        // 验证未重复签名
        require!(
            !transaction.signers.contains(&signer.key()),
            ErrorCode::AlreadySigned
        );

        transaction.signers.push(signer.key());
        
        Ok(())
    }

    // 执行交易提案
    pub fn execute_transaction(ctx: Context<ExecuteTransaction>) -> Result<()> {
        let wallet = &ctx.accounts.wallet;
        let transaction = &mut ctx.accounts.transaction;
        
        // 验证交易未执行
        require!(!transaction.executed, ErrorCode::AlreadyExecuted);

        // 验证owner set没有变化
        require!(
            wallet.owner_set_seqno == transaction.owner_set_seqno,
            ErrorCode::OwnerSetChanged
        );

        // 计算签名权重
        let mut total_weight = 0u64;
        for signer in transaction.signers.iter() {
            if let Some(owner) = wallet.owners.iter().find(|o| o.key == *signer) {
                total_weight += owner.weight;
            }
        }

        // 验证签名权重是否达到阈值
        require!(
            total_weight >= wallet.threshold_weight,
            ErrorCode::InsufficientSigners
        );

        // 准备PDA签名
        let vault_seeds = &[
            b"vault",
            wallet.to_account_info().key.as_ref(),
            &[wallet.nonce],
        ];
        let signer_seeds = &[&vault_seeds[..]];

        // 执行所有指令
        for instruction in transaction.instructions.iter() {
            // 根据是否需要PDA签名修改账户元数据
            let mut ix = Instruction {
                program_id: instruction.program_id,
                accounts: instruction.accounts.iter().map(|acc| {
                    let mut meta = acc.to_account_meta();
                    if meta.pubkey == ctx.accounts.vault.key() {
                        meta.is_signer = true;
                    }
                    meta
                }).collect(),
                data: instruction.data.clone(),
            };

            // 执行指令
            invoke_signed(
                &ix,
                ctx.remaining_accounts,
                signer_seeds,
            )?;
        }

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
pub struct CreateTransaction<'info> {
    pub wallet: Account<'info, Wallet>,
    
    #[account(
        init,
        payer = owner,
        space = 8 + // discriminator
            4 + (32 + 4 + (32 + 1 + 1) * 10 + 4 + 1024) * 5 + // instructions vec (5 instructions)
            32 + // wallet
            1 + // executed
            4 + 32 * 10 + // signers vec (10 signers)
            4   // owner_set_seqno
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
    pub wallet: Account<'info, Wallet>,
    #[account(mut)]
    pub transaction: Account<'info, Transaction>,
    pub owner: Signer<'info>,
    
    #[account(
        seeds = [b"vault", wallet.key().as_ref()],
        bump = wallet.nonce,
    )]
    /// CHECK: This is a PDA that holds SOL
    pub vault: UncheckedAccount<'info>,
}

#[account]
pub struct Wallet {
    pub owners: Vec<OwnerConfig>,
    pub threshold_weight: u64,
    pub nonce: u8,
    pub owner_set_seqno: u32,
}

#[account]
pub struct Transaction {
    pub wallet: Pubkey,
    pub instructions: Vec<ProposedInstruction>,
    pub executed: bool,
    pub signers: Vec<Pubkey>,
    pub owner_set_seqno: u32,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct OwnerConfig {
    pub key: Pubkey,
    pub weight: u64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct ProposedInstruction {
    pub program_id: Pubkey,
    pub accounts: Vec<TransactionAccount>,
    pub data: Vec<u8>,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct TransactionAccount {
    pub pubkey: Pubkey,
    pub is_signer: bool,
    pub is_writable: bool,
}

impl TransactionAccount {
    fn to_account_meta(&self) -> AccountMeta {
        match self.is_writable {
            true => AccountMeta::new(self.pubkey, self.is_signer),
            false => AccountMeta::new_readonly(self.pubkey, self.is_signer),
        }
    }
}

fn assert_unique_owners(owners: &[OwnerConfig]) -> Result<()> {
    for (i, owner) in owners.iter().enumerate() {
        // 检查权重不能为零
        require!(owner.weight > 0, ErrorCode::InvalidOwnerWeight);
        
        // 原有的重复检查
        require!(
            !owners.iter().skip(i + 1).any(|item| item.key == owner.key),
            ErrorCode::DuplicateOwner
        );
    }
    Ok(())
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
    #[msg("Owners must be unique")]
    DuplicateOwner,
    #[msg("Owner set has changed since transaction creation")]
    OwnerSetChanged,
    #[msg("Owner weight must be greater than 0")]
    InvalidOwnerWeight,
}