use anchor_lang::prelude::*;
use anchor_lang::solana_program::{instruction::Instruction, program::invoke_signed};
declare_id!("U8QgybKox2a31mTqKrpywzotFZ1nAqvk7erYTByDxui");
pub mod error;
use error::ErrorCode;


const MAX_SIGNERS: usize = 10;
const MAX_INSTRUCTIONS: usize = 5;

#[program]
pub mod multisig_wallet {
    use super::*;

    // 创建多签钱包
    pub fn create_wallet(
        ctx: Context<CreateWallet>,
        owners: Vec<OwnerConfig>,
        threshold_weight: u64,
    ) -> Result<()> {
        // 确保owner不重复
        assert_unique_owners(&owners)?;
        // 首先检查是否有owners
        require!(!owners.is_empty(), ErrorCode::NoOwners);
        // 然后检查owners的权重和唯一性
        assert_unique_owners(&owners)?;
        // 再检查阈值是否有效
        require!(threshold_weight > 0, ErrorCode::InvalidThreshold);
        let total_weight: u64 = owners.iter().map(|owner| owner.weight).sum();
        require!(
            threshold_weight <= total_weight,
            ErrorCode::ThresholdTooHigh
        );

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
        max_accounts_per_instruction: u8,
        max_data_size: u16,
    ) -> Result<()> {
        require!(
            instructions.len() <= MAX_INSTRUCTIONS,
            ErrorCode::TooManyInstructions
        );

        // 验证每条指令的账户数量和数据大小
        for instruction in instructions.iter() {
            require!(
                instruction.accounts.len() <= max_accounts_per_instruction as usize,
                ErrorCode::TooManyAccounts
            );
            require!(
                instruction.data.len() <= max_data_size as usize,
                ErrorCode::DataTooLarge
            );
        }
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
        transaction.creator = ctx.accounts.owner.key();

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
        let vault = &ctx.accounts.vault;

        // 计算签名权重
        let total_weight = calculate_total_weight(wallet, &transaction.signers)?;

        // 验证签名权重是否达到阈值
        require!(
            total_weight >= wallet.threshold_weight,
            ErrorCode::InsufficientSigners
        );

        // 准备vault PDA的签名种子
        let seeds = &[
            b"vault",
            wallet.to_account_info().key.as_ref(),
            &[wallet.nonce],
        ];
        let signer_seeds = &[&seeds[..]];

        // 添加日志来追踪账户信息
        msg!("Vault pubkey: {}", ctx.accounts.vault.key());
        msg!("Vault is_signer: {}", ctx.accounts.vault.is_signer);
        msg!("Vault is_writable: {}", ctx.accounts.vault.is_writable);
        // 执行每条指令
        for (i, instruction) in transaction.instructions.iter().enumerate() {
            msg!("Processing instruction {}", i);

            // 确保vault已经在指令的accounts中被正确设置为签名者
            let vault_index = instruction
                .accounts
                .iter()
                .position(|acc| acc.pubkey == vault.key())
                .ok_or(ErrorCode::AccountNotFound)?;

            // 转换账户元数据
            let accounts_metas: Vec<AccountMeta> = instruction
                .accounts
                .iter()
                .enumerate()
                .map(|(idx, acc)| {
                    let is_vault = idx == vault_index;
                    if is_vault {
                        // vault 需要特殊处理，因为它需要被标记为签名者
                        AccountMeta::new(acc.pubkey, true)
                    } else {
                        // 使用 to_account_meta 函数
                        acc.to_account_meta()
                    }
                })
                .collect();

            // 构建新指令
            let ix = Instruction {
                program_id: instruction.program_id,
                accounts: accounts_metas,
                data: instruction.data.clone(),
            };
            msg!("Instruction accounts:");
            for (j, acc) in ix.accounts.iter().enumerate() {
                msg!(
                    "Account {}: pubkey={}, is_signer={}, is_writable={}",
                    j,
                    acc.pubkey,
                    acc.is_signer,
                    acc.is_writable
                );
            }

            // 从remaining_accounts中收集账户信息
            let account_infos: Vec<AccountInfo> = ctx
                .remaining_accounts
                .iter()
                .map(|acc| acc.to_account_info())
                .collect();

            msg!("Invoking CPI with {} account infos", account_infos.len());

            // 执行CPI调用
            invoke_signed(&ix, &account_infos, signer_seeds)
                .map_err(|_| error!(ErrorCode::TransactionExecutionFailed))?;

            msg!("Instruction {} executed successfully", i);
        }

        transaction.executed = true;
        Ok(())
    }
    pub fn close_transaction(_ctx: Context<CloseTransaction>) -> Result<()> {
        // 账户的关闭和租金返
        msg!("Closing transaction account and returning rent to recipient");
        Ok(())
    }
    // 修改阈值权重
    pub fn change_threshold(ctx: Context<ChangeThreshold>, new_threshold: u64) -> Result<()> {
        let wallet = &mut ctx.accounts.wallet;
        let total_weight: u64 = wallet.owners.iter().map(|owner| owner.weight).sum();

        // 校验新阈值合理性
        require!(new_threshold > 0, ErrorCode::InvalidThreshold);
        require!(new_threshold <= total_weight, ErrorCode::ThresholdTooHigh);

        // 更新阈值并增加序列号
        wallet.threshold_weight = new_threshold;
        wallet.owner_set_seqno += 1;

        Ok(())
    }

    // 修改单个owner权重
    pub fn change_owner_weight(
        ctx: Context<ChangeOwnerWeight>,
        owner_key: Pubkey,
        new_weight: u64,
    ) -> Result<()> {
        let wallet = &mut ctx.accounts.wallet;

        // 校验新权重
        require!(new_weight > 0, ErrorCode::InvalidOwnerWeight);

        // 找到并修改owner权重
        if let Some(owner) = wallet.owners.iter_mut().find(|o| o.key == owner_key) {
            let old_weight = owner.weight;
            owner.weight = new_weight;

            // 计算新的总权重
            let total_weight: u64 = wallet.owners.iter().map(|o| o.weight).sum();

            // 确保阈值仍然有效
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

    // 修改整体权重方案
    pub fn change_owner_weights(
        ctx: Context<ChangeOwnerWeights>,
        new_weights: Vec<OwnerConfig>,
    ) -> Result<()> {
        let wallet = &mut ctx.accounts.wallet;

        // 验证新权重配置中包含所有现有owner
        require!(
            wallet.owners.len() == new_weights.len(),
            ErrorCode::InvalidOwnerCount
        );

        // 验证新权重配置中的owner都是有效的
        for new_config in new_weights.iter() {
            require!(
                wallet.owners.iter().any(|o| o.key == new_config.key),
                ErrorCode::OwnerNotFound
            );
            require!(new_config.weight > 0, ErrorCode::InvalidOwnerWeight);
        }

        // 计算新的总权重
        let new_total_weight: u64 = new_weights.iter().map(|o| o.weight).sum();
        require!(
            wallet.threshold_weight <= new_total_weight,
            ErrorCode::ThresholdTooHigh
        );

        // 更新权重
        wallet.owners = new_weights;
        wallet.owner_set_seqno += 1;

        Ok(())
    }
}

// 签名权重计算
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
    /// 多签钱包账户
    pub wallet: Account<'info, Wallet>,

    /// 交易提案账户
    #[account(
        mut,
        constraint = transaction.wallet == wallet.key() @ ErrorCode::InvalidWallet,
        constraint = !transaction.executed @ ErrorCode::AlreadyExecuted,
        constraint = wallet.owner_set_seqno == transaction.owner_set_seqno @ ErrorCode::OwnerSetChanged,
        has_one = wallet @ ErrorCode::InvalidWallet
    )]
    pub transaction: Account<'info, Transaction>,

    /// 执行者（必须是owner且已签名）
    #[account(
        constraint = wallet.owners.iter().any(|o| o.key == owner.key()) @ ErrorCode::NotOwner,
        constraint = transaction.signers.contains(&owner.key()) @ ErrorCode::NotSigned
    )]
    pub owner: Signer<'info>,

    /// Vault PDA账户
    #[account(
        mut,  // 确保vault是可写的
        seeds = [b"vault", wallet.key().as_ref()],
        bump = wallet.nonce,
    )]
    /// CHECK: Vault PDA, will be used as a signer
    pub vault: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
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
    pub creator: Pubkey,
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
impl OwnerConfig {
    const LEN: usize = 32 + // key
        8; // weight
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
// 首先为数据结构实现获取序列化大小的方法
impl TransactionAccount {
    const LEN: usize = 32 + // pubkey
        1 + // is_signer
        1; // is_writable
}

impl ProposedInstruction {
    fn size(accounts_len: usize, data_len: usize) -> usize {
        32 + // program_id
        4 + (TransactionAccount::LEN * accounts_len) + // accounts vec with length prefix
        4 + data_len // data vec with length prefix
    }
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
// 用于接收指令的结构
#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct IncomingInstruction {
    pub program_id: Pubkey,
    pub accounts: Vec<TransactionAccount>,
    pub data: Vec<u8>,
}

// 实现从solana指令到IncomingInstruction的转换
impl From<Instruction> for IncomingInstruction {
    fn from(ix: Instruction) -> Self {
        IncomingInstruction {
            program_id: ix.program_id,
            accounts: ix
                .accounts
                .into_iter()
                .map(|meta| TransactionAccount {
                    pubkey: meta.pubkey,
                    is_signer: meta.is_signer,
                    is_writable: meta.is_writable,
                })
                .collect(),
            data: ix.data,
        }
    }
}

// 与之前的ProposedInstruction保持一致
impl From<IncomingInstruction> for ProposedInstruction {
    fn from(incoming: IncomingInstruction) -> Self {
        ProposedInstruction {
            program_id: incoming.program_id,
            accounts: incoming.accounts,
            data: incoming.data,
        }
    }
}

// 添加关闭交易账户的指令上下文
#[derive(Accounts)]
pub struct CloseTransaction<'info> {
    pub wallet: Account<'info, Wallet>,

    #[account(
        mut,
        constraint = transaction.wallet == wallet.key() @ ErrorCode::InvalidWallet,
        constraint = transaction.executed @ ErrorCode::TransactionNotExecuted,
        close = recipient // 这会在指令执行后关闭账户并将剩余租金转给 recipient
    )]
    pub transaction: Account<'info, Transaction>,

    #[account(mut)]
    pub recipient: SystemAccount<'info>,

    // 可以选择只允许交易创建者关闭账户
    #[account(constraint = owner.key() == transaction.creator @ ErrorCode::UnauthorizedClose)]
    pub owner: Signer<'info>,
}
