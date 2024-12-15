use anchor_lang::prelude::*;
use crate::error::ErrorCode;
use anchor_lang::solana_program::{instruction::Instruction, program::invoke_signed};

#[account]
pub struct Wallet {
    pub owners: Vec<OwnerConfig>,
    pub threshold_weight: u64,
    pub nonce: u8,
    pub owner_set_seqno: u32,
}

impl Wallet {
    pub fn is_owner(&self, key: &Pubkey) -> bool {
        self.owners.iter().any(|o| o.key == *key)
    }
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

impl Transaction {
    pub fn initialize(
        &mut self,
        instructions: Vec<ProposedInstruction>,
        wallet: Pubkey,
        creator: Pubkey,
        owner_set_seqno: u32,
    ) {
        self.instructions = instructions;
        self.wallet = wallet;
        self.executed = false;
        self.signers = vec![creator];
        self.owner_set_seqno = owner_set_seqno;
        self.creator = creator;
    }
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct OwnerConfig {
    pub key: Pubkey,
    pub weight: u64,
}

impl OwnerConfig {
    pub const LEN: usize = 32 + // key
        8;  // weight
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct ProposedInstruction {
    pub program_id: Pubkey,
    pub accounts: Vec<TransactionAccount>,
    pub data: Vec<u8>,
}

impl ProposedInstruction {
    pub fn size(accounts_len: usize, data_len: usize) -> usize {
        32 + // program_id
        4 + (TransactionAccount::LEN * accounts_len) + // accounts vec with length prefix
        4 + data_len // data vec with length prefix
    }
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct TransactionAccount {
    pub pubkey: Pubkey,
    pub is_signer: bool,
    pub is_writable: bool,
}

impl TransactionAccount {
    const LEN: usize = 32 + // pubkey
        1 + // is_signer
        1;  // is_writable

    pub fn to_account_meta(&self) -> AccountMeta {
        match self.is_writable {
            true => AccountMeta::new(self.pubkey, self.is_signer),
            false => AccountMeta::new_readonly(self.pubkey, self.is_signer),
        }
    }
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct IncomingInstruction {
    pub program_id: Pubkey,
    pub accounts: Vec<TransactionAccount>,
    pub data: Vec<u8>,
}

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

impl From<IncomingInstruction> for ProposedInstruction {
    fn from(incoming: IncomingInstruction) -> Self {
        ProposedInstruction {
            program_id: incoming.program_id,
            accounts: incoming.accounts,
            data: incoming.data,
        }
    }
}