use anchor_lang::prelude::*;
use anchor_lang::solana_program::{
    instruction::Instruction,
    program::invoke_signed,
};
use anchor_lang::context::CpiContext;
pub mod errors;
pub mod state;
pub mod instructions;
pub mod constants;

use constants::*;
use state::*;
use instructions::*;

use errors::MultisigError;
declare_id!("U8QgybKox2a31mTqKrpywzotFZ1nAqvk7erYTByDxui");

#[program]
pub mod multisig_wallet {
    use super::*;

    pub fn create_multisig(ctx: Context<CreateMultisig>, args: CreateMultisigArgs) -> Result<()> {
        let wallet = &mut ctx.accounts.wallet;
        
        // 验证输入参数
        require!(args.owners.len() <= 255, MultisigError::TooManyOwners);
        require!(args.min_weight_required > 0, MultisigError::InvalidWeightThreshold);
        
        // 计算总权重并验证所有者
        let mut total_weight = 0u64;
        let mut unique_owners = std::collections::HashSet::new();
        
        for owner_info in args.owners.iter() {
            // 检查重复所有者
            require!(
                unique_owners.insert(owner_info.owner),
                MultisigError::InvalidOwner
            );
            
            // 检查权重 > 0
            require!(owner_info.weight > 0, MultisigError::InvalidWeightThreshold);
            
            // 累加总权重
            total_weight = total_weight
                .checked_add(owner_info.weight)
                .ok_or(MultisigError::WeightOverflow)?;
        }
        
        // 验证最小权重阈值
        require!(
            args.min_weight_required <= total_weight,
            MultisigError::InvalidWeightThreshold
        );
        
        // 初始化钱包
        wallet.base = ctx.accounts.base.key();
        wallet.bump = ctx.bumps.wallet;
        wallet.name = args.name;
        wallet.min_weight_required = args.min_weight_required;
        wallet.total_weight = total_weight;
        wallet.owner_set_seqno = 0;
        wallet.num_owners = args.owners.len() as u8;
        wallet.owners = args.owners;
        wallet.transaction_count = 0;
        wallet.pending_count = 0;
        wallet.pending_transactions = Vec::new();
        
        Ok(())
    }

    pub fn create_transaction(
        ctx: Context<CreateTransaction>,
        args: CreateTransactionArgs
    ) -> Result<()> {
        let wallet = &mut ctx.accounts.wallet;
        let transaction = &mut ctx.accounts.transaction;
        let clock = Clock::get()?;
        let proposer_key = ctx.accounts.proposer.key();
        
        // 验证过期时间
        if let Some(expires_at) = args.expires_at {
            require!(expires_at > clock.unix_timestamp, MultisigError::TransactionExpired);
        }
        
        // 验证并获取提案者权重
        let proposer_info = wallet.validate_owner(&proposer_key)?;
        
        // 初始化交易
        transaction.wallet = wallet.key();
        transaction.transaction_index = wallet.transaction_count;
        transaction.bump = ctx.bumps.transaction;
        transaction.proposer = proposer_key;
        transaction.status = TransactionStatus::Pending;
        transaction.current_weight = proposer_info.weight;
        transaction.approvals = vec![proposer_key];
        transaction.created_at = clock.unix_timestamp;
        transaction.expires_at = args.expires_at;
        transaction.executed_at = None;
        transaction.destination = args.destination;  // 确保这里正确设置
        transaction.amount = args.amount;           // 确保这里正确设置
        
        let count_ = wallet.transaction_count;
        // 添加到待执行队列
        wallet.add_pending_transaction(
            count_,
            transaction.key(),
            clock.unix_timestamp,
            proposer_key,
        )?;
        
        // 更新交易计数
        wallet.transaction_count += 1;
        
        Ok(())
    }

    pub fn sign_transaction(ctx: Context<SignTransaction>) -> Result<()> {
        let wallet = &ctx.accounts.wallet;
        let transaction = &mut ctx.accounts.transaction;
        let owner_key = ctx.accounts.owner.key();
        let clock = Clock::get()?;
        
        // 检查过期时间
        require!(!transaction.is_expired(clock.unix_timestamp), 
                MultisigError::TransactionExpired);
        
        // 验证并获取签名者权重
        let owner_info = wallet.validate_owner(&owner_key)?;
        
        // 添加签名
        transaction.add_signature(&owner_key, owner_info.weight)?;
        
        Ok(())
    }
    pub fn execute_transaction(
        ctx: Context<ExecuteTransaction>, 
        transaction_index: u64
    ) -> Result<()> {
        let clock = Clock::get()?;
        
        // 验证交易未过期
        require!(
            !ctx.accounts.transaction.is_expired(clock.unix_timestamp),
            MultisigError::TransactionExpired
        );
    
        // 从多签钱包转账到目标账户
        let amount = ctx.accounts.transaction.amount;
        
        // 验证多签钱包的余额是否足够
        require!(
            **ctx.accounts.wallet.to_account_info().lamports.borrow() >= amount,
            MultisigError::InsufficientFunds
        );
    
        // 执行转账
        **ctx.accounts.wallet.to_account_info().try_borrow_mut_lamports()? -= amount;
        **ctx.accounts.destination.to_account_info().try_borrow_mut_lamports()? += amount;
    
        // 更新交易状态
        ctx.accounts.transaction.status = TransactionStatus::Executed;
        ctx.accounts.transaction.executed_at = Some(clock.unix_timestamp);
    
        // 从待处理列表移除
        ctx.accounts.wallet.remove_pending_transaction(transaction_index)?;
        
        Ok(())
    }


    pub fn cancel_transaction(ctx: Context<CancelTransaction>) -> Result<()> {
        let wallet = &mut ctx.accounts.wallet;
        let transaction = &mut ctx.accounts.transaction;
        
        // 更新交易状态
        transaction.status = TransactionStatus::Cancelled;
        
        // 从待执行列表移除
        wallet.remove_pending_transaction(transaction.transaction_index)?;
        
        Ok(())
    }

    pub fn get_pending_transactions(
        ctx: Context<GetPendingTransactions>,
        start_index: u64,     // 改为 u64
        limit: u8
    ) -> Result<Vec<PendingTransactionInfo>> {
        let wallet = &ctx.accounts.wallet;
        
        // 获取待执行交易列表
        let pending = wallet.pending_transactions
            .iter()
            .skip(start_index as usize)  // 转换为 usize 用于切片
            .take(limit as usize)        // 明确转换为 usize
            .cloned()
            .collect::<Vec<_>>();
        
        require!(!pending.is_empty(), MultisigError::NoPendingTransactions);
        
        Ok(pending)
    }

}
