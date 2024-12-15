import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { MultisigWallet } from "../../target/types/multisig_wallet";
import { PublicKey, SystemProgram, LAMPORTS_PER_SOL } from "@solana/web3.js";
import { BN } from "bn.js";
import { expect } from "chai";
import {
  TestContext,
  initializeContext,
  createMultisigWallet,
} from "../helper";

describe("power-multisig: approve", () => {
  let ctx: TestContext;
  let proposalKey: PublicKey;

  beforeEach(async () => {
    // 初始化测试环境
    ctx = await initializeContext();
    await createMultisigWallet(ctx);

    // 创建一个标准的转账提案用于测试
    const proposal = anchor.web3.Keypair.generate();
    proposalKey = proposal.publicKey;

    const transferAmount = new BN(LAMPORTS_PER_SOL);
    const instruction = SystemProgram.transfer({
      fromPubkey: ctx.vault,
      toPubkey: ctx.owners.owner2.publicKey,
      lamports: transferAmount.toNumber(),
    });

    const proposedIx = {
      programId: instruction.programId,
      accounts: instruction.keys.map(key => ({
        pubkey: key.pubkey,
        isSigner: key.isSigner,
        isWritable: key.isWritable,
      })),
      data: Buffer.from(instruction.data),
    };

    // 使用 owner1 创建提案
    await ctx.program.methods
      .createTransaction([proposedIx])
      .accounts({
        wallet: ctx.wallet.publicKey,
        transaction: proposalKey,
        owner: ctx.owners.owner1.publicKey,
      })
      .signers([proposal, ctx.owners.owner1])
      .rpc();
  });

  it("successfully approves transaction by another owner", async () => {
    // owner2 批准交易
    await ctx.program.methods
      .approve()
      .accounts({
        wallet: ctx.wallet.publicKey,
        transaction: proposalKey,
        owner: ctx.owners.owner2.publicKey,
      })
      .signers([ctx.owners.owner2])
      .rpc();

    // 验证交易状态
    const txAccount = await ctx.program.account.transaction.fetch(proposalKey);
    expect(txAccount.signers).to.have.length(2);
    expect(txAccount.signers[0].equals(ctx.owners.owner1.publicKey)).to.be.true;
    expect(txAccount.signers[1].equals(ctx.owners.owner2.publicKey)).to.be.true;
  });

  it("fails when non-owner tries to approve", async () => {
    const nonOwner = anchor.web3.Keypair.generate();
    
    // 给非所有者一些SOL支付交易费用
    await ctx.provider.connection.requestAirdrop(nonOwner.publicKey, LAMPORTS_PER_SOL);
    await new Promise(resolve => setTimeout(resolve, 1000)); // 等待确认

    try {
      await ctx.program.methods
        .approve()
        .accounts({
          wallet: ctx.wallet.publicKey,
          transaction: proposalKey,
          owner: nonOwner.publicKey,
        })
        .signers([nonOwner])
        .rpc();
      expect.fail("should have failed with non-owner");
    } catch (error) {
    //   console.log("Actual error:", error.toString());
      expect(error.toString()).to.include("Not an owner");
    }
  });

  it("fails when owner tries to approve twice", async () => {
    // owner1 第一次批准（已经在创建时自动添加）
    try {
      // owner1 尝试再次批准
      await ctx.program.methods
        .approve()
        .accounts({
          wallet: ctx.wallet.publicKey,
          transaction: proposalKey,
          owner: ctx.owners.owner1.publicKey,
        })
        .signers([ctx.owners.owner1])
        .rpc();
      expect.fail("should have failed with already signed");
    } catch (error) {
    //   console.log("Actual error:", error.toString());
      expect(error.toString()).to.include("Already signed");
    }
  });

  it("fails to approve an executed transaction", async () => {
    // 首先让足够的所有者签名并执行交易
    await ctx.program.methods
      .approve()
      .accounts({
        wallet: ctx.wallet.publicKey,
        transaction: proposalKey,
        owner: ctx.owners.owner2.publicKey,
      })
      .signers([ctx.owners.owner2])
      .rpc();

    // 执行交易
    await ctx.program.methods
      .executeTransaction()
      .accounts({
        transaction: proposalKey,
        owner: ctx.owners.owner1.publicKey,
        
      })
      .remainingAccounts([
        {
          pubkey: ctx.vault,
          isWritable: true,
          isSigner: false,
        },
        {
          pubkey: ctx.owners.owner2.publicKey,
          isWritable: true,
          isSigner: false,
        },
        {
          pubkey: SystemProgram.programId,
          isWritable: false,
          isSigner: false,
        }
      ])
      .signers([ctx.owners.owner1])
      .rpc();

    // owner3 尝试批准已执行的交易
    try {
      await ctx.program.methods
        .approve()
        .accounts({
          wallet: ctx.wallet.publicKey,
          transaction: proposalKey,
          owner: ctx.owners.owner3.publicKey,
        })
        .signers([ctx.owners.owner3])
        .rpc();
      expect.fail("should have failed with already executed");
    } catch (error) {
    //   console.log("Actual error:", error.toString());
      expect(error.toString()).to.include("Transaction already executed");
    }
  });

  it("correctly maintains signer order", async () => {
    // owner2 和 owner3 按顺序批准
    await ctx.program.methods
      .approve()
      .accounts({
        wallet: ctx.wallet.publicKey,
        transaction: proposalKey,
        owner: ctx.owners.owner2.publicKey,
      })
      .signers([ctx.owners.owner2])
      .rpc();

    await ctx.program.methods
      .approve()
      .accounts({
        wallet: ctx.wallet.publicKey,
        transaction: proposalKey,
        owner: ctx.owners.owner3.publicKey,
      })
      .signers([ctx.owners.owner3])
      .rpc();

    // 验证签名者列表顺序
    const txAccount = await ctx.program.account.transaction.fetch(proposalKey);
    expect(txAccount.signers).to.have.length(3);
    expect(txAccount.signers[0].equals(ctx.owners.owner1.publicKey)).to.be.true;
    expect(txAccount.signers[1].equals(ctx.owners.owner2.publicKey)).to.be.true;
    expect(txAccount.signers[2].equals(ctx.owners.owner3.publicKey)).to.be.true;
  });
});