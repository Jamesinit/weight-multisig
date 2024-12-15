import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { MultisigWallet } from "../../target/types/multisig_wallet";
import { 
  PublicKey, 
  SystemProgram, 
  LAMPORTS_PER_SOL,
  Transaction,
} from "@solana/web3.js";
import { expect } from "chai";
import { initializeContext, createMultisigWallet, TestContext } from "../helper";
import { describe } from "mocha";


describe("execute_transaction", () => {
    let ctx: TestContext;
    
    beforeEach(async () => {
      console.log("Initializing test context...");
      ctx = await initializeContext();
      console.log("Test context initialized");
      
      console.log("Creating multisig wallet...");
      await createMultisigWallet(ctx);
      console.log("Multisig wallet created");
      
      // 打印初始状态
      console.log("Initial setup complete:");
      console.log("- Wallet public key:", ctx.wallet.publicKey.toBase58());
      console.log("- Vault public key:", ctx.vault.toBase58());
      console.log("- Owner1 public key:", ctx.owners.owner1.publicKey.toBase58());
      console.log("- Owner2 public key:", ctx.owners.owner2.publicKey.toBase58());
      console.log("- Owner3 public key:", ctx.owners.owner3.publicKey.toBase58());
    });
  
    it("should successfully execute a transfer transaction", async () => {
      // 创建一个接收地址
      const receiver = anchor.web3.Keypair.generate();
      console.log("Created receiver account:", receiver.publicKey.toBase58());
  
      // 记录初始余额
      const initialVaultBalance = await ctx.provider.connection.getBalance(ctx.vault);
      const initialReceiverBalance = await ctx.provider.connection.getBalance(receiver.publicKey);
      console.log("Initial balances:");
      console.log("- Vault balance:", initialVaultBalance);
      console.log("- Receiver balance:", initialReceiverBalance);
  
      // 创建转账提案
      const proposal = anchor.web3.Keypair.generate();
      console.log("Created proposal account:", proposal.publicKey.toBase58());
  
      // 构造转账指令
      const transferAmount = 0.1 * LAMPORTS_PER_SOL;
      const transferIx = SystemProgram.transfer({
        fromPubkey: ctx.vault,
        toPubkey: receiver.publicKey,
        lamports: transferAmount,
      });
  
      // 转换为提案格式
      const proposedIx = {
        programId: transferIx.programId,
        accounts: transferIx.keys.map(key => ({
          pubkey: key.pubkey,
          isSigner: key.pubkey.equals(ctx.vault),  // 如果是vault则设置为签名者
          isWritable: key.isWritable
        })),
        data: Buffer.from(transferIx.data)
      };
  
      console.log("Creating transaction proposal...");
      console.log("Proposed instruction details:", {
        programId: proposedIx.programId.toBase58(),
        accounts: proposedIx.accounts.map(acc => ({
          pubkey: acc.pubkey.toBase58(),
          isSigner: acc.isSigner,
          isWritable: acc.isWritable
        }))
      });
  
      try {
        await ctx.program.methods
          .createTransaction([proposedIx])
          .accounts({
            wallet: ctx.wallet.publicKey,
            transaction: proposal.publicKey,
            owner: ctx.owners.owner1.publicKey,
          })
          .signers([proposal, ctx.owners.owner1])
          .rpc();
        console.log("Transaction proposal created successfully");
      } catch (error) {
        console.error("Error creating transaction:", error);
        throw error;
      }
  
      // Owner2 签名
      console.log("Owner2 approving transaction...");
      try {
        await ctx.program.methods
          .approve()
          .accounts({
            wallet: ctx.wallet.publicKey,
            transaction: proposal.publicKey,
            owner: ctx.owners.owner2.publicKey,
          })
          .signers([ctx.owners.owner2])
          .rpc();
        console.log("Owner2 approved successfully");
      } catch (error) {
        console.error("Error in owner2 approval:", error);
        throw error;
      }
  
      // 执行提案
      console.log("Executing transaction...");
      try {
        // 获取vault的bump
        const [vaultPDA, bump] = await PublicKey.findProgramAddress(
          [Buffer.from("vault"), ctx.wallet.publicKey.toBuffer()],
          ctx.program.programId
        );
        console.log("Vault PDA details:", {
          address: vaultPDA.toBase58(),
          bump
        });

        // 获取所有必要的账户信息
const allAccounts = [
    {
      pubkey: ctx.vault,
      isWritable: true,
      isSigner: true,
    },
    {
      pubkey: receiver.publicKey,
      isWritable: true,
      isSigner: false,
    },
    {
      pubkey: SystemProgram.programId,
      isWritable: false,
      isSigner: false,
    }
  ];
  
  await ctx.program.methods
    .executeTransaction()
    .accounts({
      transaction: proposal.publicKey,
      owner: ctx.owners.owner1.publicKey,
    })
    .remainingAccounts(allAccounts)
    .signers([ctx.owners.owner1])
    .rpc({
      skipPreflight: true,  // 跳过预检
      commitment: 'confirmed'  // 使用确认级别
    });
        console.log("Transaction executed successfully");
      } catch (error) {
        // 打印更详细的错误信息
        console.error("Error executing transaction:");
        if (error.logs) {
          console.error("Transaction logs:", error.logs);
        } else {
          console.error(error);
        }
        throw error;
      }
  
      // 验证结果
      const finalVaultBalance = await ctx.provider.connection.getBalance(ctx.vault);
      const finalReceiverBalance = await ctx.provider.connection.getBalance(receiver.publicKey);
      
      console.log("Final balances:");
      console.log("- Vault balance:", finalVaultBalance);
      console.log("- Receiver balance:", finalReceiverBalance);
      console.log("Balance changes:");
      console.log("- Vault change:", finalVaultBalance - initialVaultBalance);
      console.log("- Receiver change:", finalReceiverBalance - initialReceiverBalance);
  
      // 余额断言
      expect(finalVaultBalance).to.be.below(initialVaultBalance);
      expect(finalReceiverBalance).to.equal(initialReceiverBalance + transferAmount);
  
      // 验证提案状态
      const transactionAccount = await ctx.program.account.transaction.fetch(proposal.publicKey);
      console.log("Final transaction state:", {
        executed: transactionAccount.executed,
        signerCount: transactionAccount.signers.length
      });
      
      expect(transactionAccount.executed).to.be.true;
      expect(transactionAccount.signers).to.have.lengthOf(2);
    });
  });