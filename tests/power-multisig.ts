import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";

import { MultisigWallet } from "../target/types/multisig_wallet";
import { BN } from "bn.js";

import { 
  PublicKey, 
  SystemProgram, 
  LAMPORTS_PER_SOL,
  TransactionInstruction,
  sendAndConfirmTransaction,
  Transaction,
} from "@solana/web3.js";
import { expect } from "chai";

describe("flexible-multisig-wallet", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.MultisigWallet as Program<MultisigWallet>;

  const owner1 = anchor.web3.Keypair.generate(); // 权重 60
  const owner2 = anchor.web3.Keypair.generate(); // 权重 30
  const owner3 = anchor.web3.Keypair.generate(); // 权重 10
  const receiver = anchor.web3.Keypair.generate();
  const wallet = anchor.web3.Keypair.generate();
  const proposalAccount = anchor.web3.Keypair.generate();
  let vault: PublicKey;
  
  before(async () => {
    // 先给测试账户空投SOL
    const connection = provider.connection;
    await connection.requestAirdrop(owner1.publicKey, 10 * LAMPORTS_PER_SOL);
    
    // 等待确认
    let balance = 0;
    while (balance < 10 * LAMPORTS_PER_SOL) {
      await new Promise(resolve => setTimeout(resolve, 1000));
      balance = await connection.getBalance(owner1.publicKey);
    }
    
    console.log("Owner1 balance:", balance / LAMPORTS_PER_SOL, "SOL");

    // 给其他owner转一些SOL用于支付交易费用
    const transferTx = new Transaction();
    for (const owner of [owner2, owner3]) {
      transferTx.add(
        SystemProgram.transfer({
          fromPubkey: owner1.publicKey,
          toPubkey: owner.publicKey,
          lamports: LAMPORTS_PER_SOL,
        })
      );
    }
    
    await sendAndConfirmTransaction(connection, transferTx, [owner1]);
    console.log("Transferred SOL to other owners");
  });

  it("Creates a multisig wallet", async () => {
    try {
      // 找到vault PDA
      const [vaultPDA] = await PublicKey.findProgramAddress(
        [Buffer.from("vault"), wallet.publicKey.toBuffer()],
        program.programId
      );
      vault = vaultPDA;
      console.log("Vault PDA:", vault.toString());

      // 创建多签钱包
      const owners = [
        { key: owner1.publicKey, weight: new BN(60) },
        { key: owner2.publicKey, weight: new BN(30) },
        { key: owner3.publicKey, weight: new BN(10) },
      ];

      await program.methods
        .createWallet(owners, new anchor.BN(70))
        .accounts({
          wallet: wallet.publicKey,
          payer: owner1.publicKey,
        })
        .signers([wallet, owner1])
        .rpc();

      // 验证钱包创建
      const walletAccount = await program.account.wallet.fetch(wallet.publicKey);
      console.log("Wallet created with owners:", walletAccount.owners.map(o => o.key.toString()));
      expect(walletAccount.owners.length).to.equal(3);
    } catch (error) {
      console.error("Error creating wallet:", error);
      throw error;
    }
  });

  it("Transfers SOL to vault", async () => {
    try {
      const transferTx = new Transaction().add(
        SystemProgram.transfer({
          fromPubkey: owner1.publicKey,
          toPubkey: vault,
          lamports: LAMPORTS_PER_SOL,
        })
      );

      await sendAndConfirmTransaction(
        provider.connection,
        transferTx,
        [owner1]
      );

      const balance = await provider.connection.getBalance(vault);
      console.log("Vault balance:", balance / LAMPORTS_PER_SOL, "SOL");
      expect(balance).to.be.above(0);
    } catch (error) {
      console.error("Error transferring SOL to vault:", error);
      throw error;
    }
  });

  it("Creates a SOL transfer transaction", async () => {
    try {
      // 创建SOL转账指令
      const transferAmount = LAMPORTS_PER_SOL / 2;
      const instruction = SystemProgram.transfer({
        fromPubkey: vault,
        toPubkey: receiver.publicKey,
        lamports: transferAmount,
      });

      // 创建交易提案
      await program.methods
        .createTransaction(
          instruction.programId,
          instruction.keys.map(key => ({
            pubkey: key.pubkey,
            isSigner: key.isSigner,
            isWritable: key.isWritable,
          })),
          Buffer.from(instruction.data)
        )
        .accounts({
          wallet: wallet.publicKey,
          transaction: proposalAccount.publicKey,
          owner: owner1.publicKey,
        })
        .signers([proposalAccount, owner1])
        .rpc();

      // 验证交易创建
      const txAccount = await program.account.transaction.fetch(proposalAccount.publicKey);
      console.log("Transaction created with first signer:", txAccount.signers[0].toString());
      expect(txAccount.executed).to.be.false;
      expect(txAccount.signers.length).to.equal(1);
    } catch (error) {
      console.error("Error creating transaction:", error);
      throw error;
    }
  });

  it("Signs the transaction", async () => {
    try {
      await program.methods
        .signTransaction()
        .accounts({
          wallet: wallet.publicKey,
          transaction: proposalAccount.publicKey,
          owner: owner2.publicKey,
        })
        .signers([owner2])
        .rpc();

      const txAccount = await program.account.transaction.fetch(proposalAccount.publicKey);
      console.log("Transaction signed by:", txAccount.signers.map(s => s.toString()));
      expect(txAccount.signers.length).to.equal(2);
    } catch (error) {
      console.error("Error signing transaction:", error);
      throw error;
    }
  });

  it("Executes the SOL transfer", async () => {
    try {
      const preBalance = await provider.connection.getBalance(receiver.publicKey);

      await program.methods
        .executeTransaction()
        .accounts({
          wallet: wallet.publicKey,
          transaction: proposalAccount.publicKey,
          owner: owner1.publicKey,
        })
        .remainingAccounts([
          { pubkey: vault, isWritable: true, isSigner: false },
          { pubkey: receiver.publicKey, isWritable: true, isSigner: false },
          { pubkey: SystemProgram.programId, isWritable: false, isSigner: false },
        ])
        .signers([owner1])
        .rpc();

      const postBalance = await provider.connection.getBalance(receiver.publicKey);
      console.log("Transfer successful, receiver balance change:", 
        (postBalance - preBalance) / LAMPORTS_PER_SOL, "SOL");
      expect(postBalance).to.be.above(preBalance);

      const txAccount = await program.account.transaction.fetch(proposalAccount.publicKey);
      expect(txAccount.executed).to.be.true;
    } catch (error) {
      console.error("Error executing transaction:", error);
      throw error;
    }
  });
});