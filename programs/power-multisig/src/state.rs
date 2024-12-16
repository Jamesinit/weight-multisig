use anchor_lang::prelude::*;

#[account]
pub struct  Member{
    key: Pubkey,
    weight: u8,
}
#[account]
pub struct Multisig {
    pub owners: Vec<Member>,
    pub threshold: u64,
    pub nonce: u8,
    pub transaction_count: u32,
}

#[account]
pub struct Transaction {
    pub to: Pubkey,        // 收款地址
    pub amount: u64,       // 金额
    pub signers: Vec<Pubkey>,
    pub did_execute: bool,
    pub owner: Pubkey,
    pub nonce: u8,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct TransactionAccount {
    pub pubkey: Pubkey,
    pub is_signer: bool,
    pub is_writable: bool,
}

impl TransactionAccount {
    pub fn to_account_meta(&self) -> AccountMeta {
        AccountMeta {
            pubkey: self.pubkey,
            is_signer: self.is_signer,
            is_writable: self.is_writable,
        }
    }
}