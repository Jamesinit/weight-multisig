use anchor_lang::prelude::*;
use crate::errors::MultisigError;
use crate::constants::*;
// ============= Owner Info =============
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct OwnerInfo {
    pub owner: Pubkey,
    pub weight: u64,
}

// ============= Transaction Status =============
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, PartialEq)]
pub enum TransactionStatus {
    Pending,    // 等待足够签名
    Executed,   // 已执行
    Cancelled,  // 已取消
    Expired,    // 已过期
}

// ============= Transaction Account Meta =============
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct TransactionAccountMeta {
    pub pubkey: Pubkey,
    pub is_signer: bool,
    pub is_writable: bool,
}

impl From<TransactionAccountMeta> for AccountMeta {
    fn from(meta: TransactionAccountMeta) -> Self {
        Self {
            pubkey: meta.pubkey,
            is_signer: meta.is_signer,
            is_writable: meta.is_writable,
        }
    }
}

impl From<AccountMeta> for TransactionAccountMeta {
    fn from(account_meta: AccountMeta) -> Self {
        Self {
            pubkey: account_meta.pubkey,
            is_signer: account_meta.is_signer,
            is_writable: account_meta.is_writable,
        }
    }
}

// ============= Transaction Instruction =============
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct TransactionInstruction {
    pub program_id: Pubkey,
    pub accounts: Vec<TransactionAccountMeta>,
    pub data: Vec<u8>,
}

// ============= Pending Transaction Info =============
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct PendingTransactionInfo {
    pub index: u64,
    pub pubkey: Pubkey,
    pub created_at: i64,
    pub proposer: Pubkey,
}

// ============= Multisig Wallet Account =============
#[account]
pub struct MultisigWallet {
    // 基本配置
    pub base: Pubkey,                    // 用于派生 PDA 的基础地址
    pub bump: u8,                        // PDA 的 bump seed
    pub name: String,                    // 多签钱包名称
    pub min_weight_required: u64,        // 执行所需的最小权重
    pub total_weight: u64,               // 所有者权重总和
    pub owner_set_seqno: u32,           // 所有者集合的序列号
    
    // 所有者管理
    pub owners: Vec<OwnerInfo>,          // 所有者列表
    pub num_owners: u8,                  // 所有者数量
    
    // 交易管理
    pub transaction_count: u64,          // 总交易数量
    pub pending_count: u64,              // 待处理交易数量
    pub pending_transactions: Vec<PendingTransactionInfo>, // 待处理交易列表
}

impl MultisigWallet {
    // 查找所有者信息
    pub fn find_owner(&self, owner: &Pubkey) -> Option<&OwnerInfo> {
        self.owners.iter().find(|info| &info.owner == owner)
    }
    
    // 验证所有者并返回所有者信息
    pub fn validate_owner(&self, owner: &Pubkey) -> Result<&OwnerInfo> {
        self.find_owner(owner).ok_or(MultisigError::OwnerNotFound.into())
    }
    
    // 添加待处理交易
    pub fn add_pending_transaction(
        &mut self,
        index: u64,
        pubkey: Pubkey,
        created_at: i64,
        proposer: Pubkey,
    ) -> Result<()>{
        require!(
            self.pending_transactions.len() < MAX_PENDING_TXS,
            MultisigError::PendingQueueFull
        );
        self.pending_transactions.push(PendingTransactionInfo {
            index,
            pubkey,
            created_at,
            proposer,
        });
        self.pending_count += 1;
        Ok(())
    }
    
    // 移除待处理交易
    pub fn remove_pending_transaction(&mut self, index: u64) -> Result<()> {
        if let Some(pos) = self.pending_transactions
            .iter()
            .position(|x| x.index == index) {
            self.pending_transactions.remove(pos);
            self.pending_count = self.pending_count.checked_sub(1).unwrap_or(0);
            Ok(())
        } else {
            Err(MultisigError::TransactionNotFound.into())
        }
    }
}

// ============= Transaction Account =============
#[account]
pub struct Transaction {
    pub wallet: Pubkey,                // 多签钱包地址
    pub transaction_index: u64,        // 交易索引
    pub bump: u8,                      // PDA bump
    
    pub proposer: Pubkey,              // 提案人
    pub destination: Pubkey,           // 接收方地址
    pub amount: u64,                   // 转账金额
    pub status: TransactionStatus,     // 交易状态
    pub current_weight: u64,           // 当前权重
    pub approvals: Vec<Pubkey>,        // 已批准的签名者
    
    pub created_at: i64,               // 创建时间
    pub expires_at: Option<i64>,       // 过期时间（可选）
    pub executed_at: Option<i64>,      // 执行时间（可选）
}
impl Transaction {
    // 检查交易是否可执行
    pub fn is_executable(&self, min_weight_required: u64, current_time: i64) -> bool {
        matches!(self.status, TransactionStatus::Pending) && 
        !self.is_expired(current_time) && 
        self.current_weight >= min_weight_required
    }
    
    // 检查交易是否过期
    pub fn is_expired(&self, current_time: i64) -> bool {
        self.expires_at.map_or(false, |expires| current_time > expires)
    }
    
    // 检查是否已签名
    pub fn has_signed(&self, owner: &Pubkey) -> bool {
        self.approvals.contains(owner)
    }
    
    // 添加签名
    pub fn add_signature(&mut self, owner: &Pubkey, weight: u64) -> Result<()> {
        require!(!self.has_signed(owner), MultisigError::AlreadySigned);
        require!(
            matches!(self.status, TransactionStatus::Pending),
            MultisigError::InvalidTransactionState
        );
        
        self.approvals.push(*owner);
        self.current_weight = self.current_weight
            .checked_add(weight)
            .ok_or(MultisigError::WeightOverflow)?;
            
        Ok(())
    }
    
    // 更新交易状态
    pub fn update_status(&mut self, current_time: i64) {
        if self.is_expired(current_time) {
            self.status = TransactionStatus::Expired;
        }
    }
}