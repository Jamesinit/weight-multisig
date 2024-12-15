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

describe("multisig-wallet", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.MultisigWallet as Program<MultisigWallet>;

  // 生成测试账户
  const owner1 = anchor.web3.Keypair.generate(); // 权重 60
  const owner2 = anchor.web3.Keypair.generate(); // 权重 30
  const owner3 = anchor.web3.Keypair.generate(); // 权重 10
  const receiver1 = anchor.web3.Keypair.generate();
  const receiver2 = anchor.web3.Keypair.generate();
  const wallet = anchor.web3.Keypair.generate();
  const proposal = anchor.web3.Keypair.generate();
  let vault: PublicKey;
  
  before(async () => {
    try {
      // 给owner1空投足够的SOL
      const signature = await provider.connection.requestAirdrop(
        owner1.publicKey,
        10 * LAMPORTS_PER_SOL
      );
      await provider.connection.confirmTransaction(signature);
      console.log("Airdropped SOL to owner1");

      // 给其他owner转账一些SOL
      const tx = new Transaction();
      [owner2, owner3].forEach(owner => {
        tx.add(
          SystemProgram.transfer({
            fromPubkey: owner1.publicKey,
            toPubkey: owner.publicKey,
            lamports: LAMPORTS_PER_SOL,
          })
        );
      });
      await sendAndConfirmTransaction(provider.connection, tx, [owner1]);
      console.log("Transferred SOL to other owners");
    } catch (error) {
      console.error("Error in setup:", error);
      throw error;
    }
  });

  it("Creates a multisig wallet", async () => {
    try {
      // 查找vault PDA
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

  it("Funds the vault", async () => {
    try {
      // 转2 SOL到vault
      await provider.sendAndConfirm(
        new Transaction().add(
          SystemProgram.transfer({
            fromPubkey: owner1.publicKey,
            toPubkey: vault,
            lamports: 2 * LAMPORTS_PER_SOL,
          })
        ),
        [owner1]
      );

      const balance = await provider.connection.getBalance(vault);
      console.log("Vault balance:", balance / LAMPORTS_PER_SOL, "SOL");
      expect(balance).to.equal(2 * LAMPORTS_PER_SOL);
    } catch (error) {
      console.error("Error funding vault:", error);
      throw error;
    }
  });

  it("Creates a multi-instruction transaction proposal", async () => {
    try {
      // 创建两个转账指令
      const transferAmount1 = LAMPORTS_PER_SOL / 2;
      const transferAmount2 = LAMPORTS_PER_SOL / 4;

      const instruction1 = SystemProgram.transfer({
        fromPubkey: vault,
        toPubkey: receiver1.publicKey,
        lamports: transferAmount1,
      });

      const instruction2 = SystemProgram.transfer({
        fromPubkey: vault,
        toPubkey: receiver2.publicKey,
        lamports: transferAmount2,
      });

      // 将指令转换为提案格式
      const proposedInstructions = [instruction1, instruction2].map(ix => ({
        programId: ix.programId,
        accounts: ix.keys.map(key => ({
          pubkey: key.pubkey,
          isSigner: key.isSigner,
          isWritable: key.isWritable,
        })),
        data: Buffer.from(ix.data),
      }));

      // 创建交易提案
      await program.methods
        .createTransaction(proposedInstructions)
        .accounts({
          wallet: wallet.publicKey,
          transaction: proposal.publicKey,
          owner: owner1.publicKey,
        })
        .signers([proposal, owner1])
        .rpc();

      // 验证提案创建
      const proposalAccount = await program.account.transaction.fetch(proposal.publicKey);
      console.log("Transaction proposal created with instructions:", proposalAccount.instructions.length);
      expect(proposalAccount.instructions.length).to.equal(2);
      expect(proposalAccount.executed).to.be.false;
      expect(proposalAccount.signers.length).to.equal(1);
    } catch (error) {
      console.error("Error creating transaction proposal:", error);
      throw error;
    }
  });

  it("Approves the transaction proposal", async () => {
    try {
      // owner2 签名交易
      await program.methods
        .approve()
        .accounts({
          wallet: wallet.publicKey,
          transaction: proposal.publicKey,
          owner: owner2.publicKey,
        })
        .signers([owner2])
        .rpc();

      const proposalAccount = await program.account.transaction.fetch(proposal.publicKey);
      console.log("Transaction signed by:", proposalAccount.signers.map(s => s.toString()));
      expect(proposalAccount.signers.length).to.equal(2);
    } catch (error) {
      console.error("Error approving transaction:", error);
      throw error;
    }
  });

  it("Executes the multi-instruction transaction", async () => {
    try {
      const preBalance1 = await provider.connection.getBalance(receiver1.publicKey);
      const preBalance2 = await provider.connection.getBalance(receiver2.publicKey);

      // 执行交易
      await program.methods
        .executeTransaction()
        .accounts({
          wallet: wallet.publicKey,
          transaction: proposal.publicKey,
          owner: owner1.publicKey,
        })
        .remainingAccounts([
          // 第一个转账指令的账户
          { pubkey: vault, isWritable: true, isSigner: false },
          { pubkey: receiver1.publicKey, isWritable: true, isSigner: false },
          { pubkey: SystemProgram.programId, isWritable: false, isSigner: false },
          // 第二个转账指令的账户
          { pubkey: vault, isWritable: true, isSigner: false },
          { pubkey: receiver2.publicKey, isWritable: true, isSigner: false },
          { pubkey: SystemProgram.programId, isWritable: false, isSigner: false },
        ])
        .signers([owner1])
        .rpc();

      // 验证转账结果
      const postBalance1 = await provider.connection.getBalance(receiver1.publicKey);
      const postBalance2 = await provider.connection.getBalance(receiver2.publicKey);
      
      console.log("Transfer results:");
      console.log("Receiver1 balance change:", (postBalance1 - preBalance1) / LAMPORTS_PER_SOL, "SOL");
      console.log("Receiver2 balance change:", (postBalance2 - preBalance2) / LAMPORTS_PER_SOL, "SOL");

      expect(postBalance1 - preBalance1).to.equal(LAMPORTS_PER_SOL / 2);
      expect(postBalance2 - preBalance2).to.equal(LAMPORTS_PER_SOL / 4);

      // 验证交易状态
      const proposalAccount = await program.account.transaction.fetch(proposal.publicKey);
      expect(proposalAccount.executed).to.be.true;
    } catch (error) {
      console.error("Error executing transaction:", error);
      throw error;
    }
  });
  it("Fails to execute the same transaction again", async () => {
    try {
      await program.methods
        .executeTransaction()
        .accounts({
          wallet: wallet.publicKey,
          transaction: proposal.publicKey,
          owner: owner1.publicKey,
        })
        .remainingAccounts([
          // 第一个转账指令的账户
          { pubkey: vault, isWritable: true, isSigner: false },
          { pubkey: receiver1.publicKey, isWritable: true, isSigner: false },
          { pubkey: SystemProgram.programId, isWritable: false, isSigner: false },
          // 第二个转账指令的账户
          { pubkey: vault, isWritable: true, isSigner: false },
          { pubkey: receiver2.publicKey, isWritable: true, isSigner: false },
          { pubkey: SystemProgram.programId, isWritable: false, isSigner: false },
        ])
        .signers([owner1])
        .rpc();
      
      expect.fail("Should have failed");
    } catch (error) {
      const errorMsg = error.toString();
      console.log("Expected error:", errorMsg);
      // 检查是否包含 Anchor 错误信息
      expect(errorMsg).to.include("Error Code: AlreadyExecuted");
      expect(errorMsg).to.include("Transaction already executed");
    }
  });
});