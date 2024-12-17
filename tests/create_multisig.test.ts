import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { MultisigWallet } from "../target/types/multisig_wallet";
import { Keypair, LAMPORTS_PER_SOL, PublicKey, SystemProgram } from "@solana/web3.js";
import { assert, expect } from "chai";
import { Transaction } from '@solana/web3.js';

describe('multisig_wallet', () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.MultisigWallet as Program<MultisigWallet>;
  
  // 测试账户
  const payer = anchor.web3.Keypair.generate();
  const owner1 = anchor.web3.Keypair.generate();
  const owner2 = anchor.web3.Keypair.generate();
  const owner3 = anchor.web3.Keypair.generate();
  const destination = anchor.web3.Keypair.generate();
  
  // PDA 种子常量
  const MULTISIG_SEED = Buffer.from("multisig");
  const TRANSACTION_SEED = Buffer.from("transaction");
  
  // 存储测试过程中的重要变量
  let walletPda: PublicKey;
  let walletBump: number;
  let transactionPda: PublicKey;
  let transactionBump: number;
  
  // 辅助函数：等待交易确认并打印结果
  async function confirmTx(signature: string, label: string) {
    console.log(`Confirming ${label} transaction:`, signature);
    const result = await provider.connection.confirmTransaction(signature);
    console.log(`${label} transaction confirmed:`, result.value);
    return result;
  }

  // 辅助函数：打印账户余额
  async function logBalance(pubkey: PublicKey, label: string) {
    const balance = await provider.connection.getBalance(pubkey);
    console.log(`${label} balance:`, balance / LAMPORTS_PER_SOL, "SOL");
    return balance;
  }
  
  before(async () => {
    console.log("Setting up test accounts...");
    console.log("Payer pubkey:", payer.publicKey.toString());
    
    // 给每个账户空投 SOL
    const accounts = [
      { keypair: payer, amount: 100 * LAMPORTS_PER_SOL, label: "Payer" },
      { keypair: owner1, amount: 10 * LAMPORTS_PER_SOL, label: "Owner 1" },
      { keypair: owner2, amount: 10 * LAMPORTS_PER_SOL, label: "Owner 2" },
      { keypair: owner3, amount: 10 * LAMPORTS_PER_SOL, label: "Owner 3" }
    ];

    for (const account of accounts) {
      const signature = await provider.connection.requestAirdrop(
        account.keypair.publicKey,
        account.amount
      );
      await confirmTx(signature, `${account.label} airdrop`);
      await logBalance(account.keypair.publicKey, account.label);
    }
    
    // 查找钱包 PDA
    [walletPda, walletBump] = await PublicKey.findProgramAddress(
      [MULTISIG_SEED, payer.publicKey.toBuffer()],
      program.programId
    );
    console.log("Wallet PDA:", walletPda.toString());
    console.log("Wallet bump:", walletBump);
  });

  it("Create Multisig Wallet", async () => {
    console.log("\nCreating multisig wallet...");
    
    // 创建钱包参数
    const createArgs = {
      name: "Test Wallet",
      minWeightRequired: new anchor.BN(2),
      owners: [
        { owner: owner1.publicKey, weight: new anchor.BN(1) },
        { owner: owner2.publicKey, weight: new anchor.BN(1) },
        { owner: owner3.publicKey, weight: new anchor.BN(1) }
      ]
    };

    console.log("Create wallet arguments:", {
      name: createArgs.name,
      minWeightRequired: createArgs.minWeightRequired.toString(),
      owners: createArgs.owners.map(o => ({
        owner: o.owner.toString(),
        weight: o.weight.toString()
      }))
    });

    // 创建钱包
    const tx = await program.methods
      .createMultisig(createArgs)
      .accountsPartial({
        wallet: walletPda,
        base: payer.publicKey,
        payer: payer.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .signers([payer])
      .rpc();
      
    await confirmTx(tx, "Create wallet");

    // 验证钱包状态
    console.log("\nVerifying wallet state...");
    const walletAccount = await program.account.multisigWallet.fetch(walletPda);
    console.log("Wallet account state:", {
      base: walletAccount.base.toString(),
      name: walletAccount.name,
      minWeightRequired: walletAccount.minWeightRequired.toString(),
      totalWeight: walletAccount.totalWeight.toString(),
      ownerSetSeqno: walletAccount.ownerSetSeqno,
      numOwners: walletAccount.numOwners,
      pendingCount: walletAccount.pendingCount.toString()
    });

    expect(walletAccount.base.toString()).to.equal(payer.publicKey.toString());
    expect(walletAccount.name).to.equal(createArgs.name);
    expect(walletAccount.minWeightRequired.toString()).to.equal(createArgs.minWeightRequired.toString());
    expect(walletAccount.totalWeight.toString()).to.equal("3");
    expect(walletAccount.ownerSetSeqno).to.equal(0);
    expect(walletAccount.numOwners).to.equal(3);
  });

  it("Create Transaction", async () => {
    console.log("\nCreating transaction...");
    
    // 先给钱包转账
    const transferAmount = 2 * LAMPORTS_PER_SOL;
    console.log("Transferring", transferAmount / LAMPORTS_PER_SOL, "SOL to wallet");
    
    const transferIx = SystemProgram.transfer({
      fromPubkey: payer.publicKey,
      toPubkey: walletPda,
      lamports: transferAmount
    });
    
    const transferTx = new Transaction().add(transferIx);
    const transferSig = await provider.sendAndConfirm(transferTx, [payer]);
    await confirmTx(transferSig, "Transfer to wallet");
    
    await logBalance(walletPda, "Wallet");

    // 查找交易 PDA
    [transactionPda, transactionBump] = await PublicKey.findProgramAddress(
      [
        TRANSACTION_SEED,
        walletPda.toBuffer(),
        Buffer.from(new anchor.BN(0).toArray("le", 8))
      ],
      program.programId
    );
    console.log("Transaction PDA:", transactionPda.toString());

    const amount = new anchor.BN(0.1 * LAMPORTS_PER_SOL);
    const now = Math.floor(Date.now() / 1000);
    const expiresAt = now + 3600; // 1小时后过期

    console.log("Creating transaction with params:", {
      destination: destination.publicKey.toString(),
      amount: amount.toString(),
      expiresAt: new Date(expiresAt * 1000).toISOString()
    });

    // 创建交易
    const tx = await program.methods
      .createTransaction({
        destination: destination.publicKey,
        amount: amount,
        expiresAt: new anchor.BN(expiresAt),
      })
      .accountsPartial({
        wallet: walletPda,
        transaction: transactionPda,
        proposer: owner1.publicKey,
        payer: payer.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .signers([payer, owner1])
      .rpc();

    await confirmTx(tx, "Create transaction");

    // 验证交易状态
    console.log("\nVerifying transaction state...");
    const transactionAccount = await program.account.transaction.fetch(transactionPda);
    console.log("Transaction account state:", {
      wallet: transactionAccount.wallet.toString(),
      transactionIndex: transactionAccount.transactionIndex.toString(),
      proposer: transactionAccount.proposer.toString(),
      destination: transactionAccount.destination.toString(),
      amount: transactionAccount.amount.toString(),
      status: transactionAccount.status,
      currentWeight: transactionAccount.currentWeight.toString(),
      approvals: transactionAccount.approvals.map(a => a.toString())
    });

    // 验证钱包的待处理交易状态
    console.log("\nVerifying wallet pending transactions...");
    const walletAccount = await program.account.multisigWallet.fetch(walletPda);
    console.log("Wallet pending transactions:", {
      count: walletAccount.pendingCount.toString(),
      transactions: walletAccount.pendingTransactions.map(tx => ({
        index: tx.index.toString(),
        pubkey: tx.pubkey.toString(),
        proposer: tx.proposer.toString()
      }))
    });
  });

  it("Sign Transaction", async () => {
    console.log("\nSigning transaction with owner2...");
    
    const tx = await program.methods
      .signTransaction()
      .accountsPartial({
        wallet: walletPda,
        transaction: transactionPda,
        owner: owner2.publicKey,
      })
      .signers([owner2])
      .rpc();

    await confirmTx(tx, "Sign transaction");

    // 验证交易状态
    console.log("\nVerifying transaction state after signing...");
    const transactionAccount = await program.account.transaction.fetch(transactionPda);
    console.log("Transaction account state:", {
      currentWeight: transactionAccount.currentWeight.toString(),
      approvals: transactionAccount.approvals.map(a => a.toString())
    });
  });

  it("Execute Transaction", async () => {
    console.log("\nExecuting transaction...");
    
    // 记录执行前的余额
    await logBalance(destination.publicKey, "Destination (before)");
    await logBalance(walletPda, "Wallet (before)");

    const tx = await program.methods
      .executeTransaction(new anchor.BN(0))
      .accountsPartial({
        wallet: walletPda,
        transaction: transactionPda,
        destination: destination.publicKey,
        executor: owner1.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .signers([owner1])
      .rpc();

    await confirmTx(tx, "Execute transaction");

    // 记录执行后的余额
    await logBalance(destination.publicKey, "Destination (after)");
    await logBalance(walletPda, "Wallet (after)");

    // 验证交易状态
    console.log("\nVerifying transaction state after execution...");
    const transactionAccount = await program.account.transaction.fetch(transactionPda);
    console.log("Transaction account state:", {
      status: transactionAccount.status,
      executedAt: transactionAccount.executedAt?.toString()
    });

    // 验证钱包状态
    console.log("\nVerifying wallet state after execution...");
    const walletAccount = await program.account.multisigWallet.fetch(walletPda);
    console.log("Wallet pending transactions:", {
      count: walletAccount.pendingCount.toString(),
      transactions: walletAccount.pendingTransactions.map(tx => ({
        index: tx.index.toString(),
        pubkey: tx.pubkey.toString()
      }))
    });
  });

  it("Create Another Transaction for Testing", async () => {
    console.log("\nCreating another transaction for testing...");
    const walletAccount = await program.account.multisigWallet.fetch(walletPda);
    const txIndex = walletAccount.transactionCount;
    console.log("Current transaction count:", txIndex.toString()); 
    [transactionPda] = await PublicKey.findProgramAddress(
      [
        TRANSACTION_SEED,
        walletPda.toBuffer(),
        Buffer.from(txIndex.toArray("le", 8))
      ],
      program.programId
    );
    
    const amount = new anchor.BN(0.1 * LAMPORTS_PER_SOL);
    const now = Math.floor(Date.now() / 1000);
    const expiresAt = now + 3600;

    const tx = await program.methods
      .createTransaction({
        destination: destination.publicKey,
        amount: amount,
        expiresAt: new anchor.BN(expiresAt),
      })
      .accountsPartial({
        wallet: walletPda,
        transaction: transactionPda,
        proposer: owner1.publicKey,
        payer: payer.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .signers([payer, owner1])
      .rpc();

    await confirmTx(tx, "Create another transaction");
  });

  it("Get Pending Transactions", async () => {
    console.log("\nGetting pending transactions...");
    
    const pendingTxs = await program.methods
      .getPendingTransactions(new anchor.BN(0), 10)
      .accountsPartial({
        wallet: walletPda,
        systemProgram: SystemProgram.programId,
      })
      .view();

    console.log("Pending transactions:", pendingTxs.map(tx => ({
      index: tx.index.toString(),
      pubkey: tx.pubkey.toString(),
      proposer: tx.proposer.toString()
    })));
  });

  it("Cancel Transaction", async () => {
    console.log("\nCancelling transaction...");
    
    const tx = await program.methods
      .cancelTransaction()
      .accountsPartial({
        wallet: walletPda,
        transaction: transactionPda,
        proposer: owner1.publicKey,
      })
      .signers([owner1])
      .rpc();

    await confirmTx(tx, "Cancel transaction");

    // 验证交易状态
    console.log("\nVerifying transaction state after cancellation...");
    const transactionAccount = await program.account.transaction.fetch(transactionPda);
    console.log("Transaction account state:", {
      status: transactionAccount.status,
    });

    // 验证钱包状态
    console.log("\nVerifying wallet state after cancellation...");
    const walletAccount = await program.account.multisigWallet.fetch(walletPda);
    console.log("Wallet pending transactions:", {
      count: walletAccount.pendingCount.toString(),
      transactions: walletAccount.pendingTransactions.map(tx => ({
        index: tx.index.toString(),
        pubkey: tx.pubkey.toString()
      }))
    });
  });
});