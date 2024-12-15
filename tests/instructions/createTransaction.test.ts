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

describe("power-multisig: create-transaction", () => {
  let ctx: TestContext;

  beforeEach(async () => {
    ctx = await initializeContext();
    await createMultisigWallet(ctx);
  });

  it("successfully creates a single transfer transaction", async () => {
    const proposal = anchor.web3.Keypair.generate();
    const transferAmount = new BN(1_000_000);
    
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

    await ctx.program.methods
      .createTransaction([proposedIx])
      .accounts({
        wallet: ctx.wallet.publicKey,
        transaction: proposal.publicKey,
        owner: ctx.owners.owner1.publicKey,
   
      })
      .signers([proposal, ctx.owners.owner1])
      .rpc();

    const txAccount = await ctx.program.account.transaction.fetch(proposal.publicKey);
    expect(txAccount.wallet.equals(ctx.wallet.publicKey)).to.be.true;
    expect(txAccount.executed).to.be.false;
    expect(txAccount.ownerSetSeqno).to.equal(0);
    expect(txAccount.instructions).to.have.length(1);
    expect(txAccount.signers).to.have.length(1);
    expect(txAccount.signers[0].equals(ctx.owners.owner1.publicKey)).to.be.true;
  });

  it("successfully creates a multi-instruction transaction", async () => {
    const proposal = anchor.web3.Keypair.generate();
    const amount1 = new BN(1_000_000);
    const amount2 = new BN(500_000);
    
    const instruction1 = SystemProgram.transfer({
      fromPubkey: ctx.vault,
      toPubkey: ctx.owners.owner2.publicKey,
      lamports: amount1.toNumber(),
    });

    const instruction2 = SystemProgram.transfer({
      fromPubkey: ctx.vault,
      toPubkey: ctx.owners.owner3.publicKey,
      lamports: amount2.toNumber(),
    });

    const proposedInstructions = [instruction1, instruction2].map(ix => ({
      programId: ix.programId,
      accounts: ix.keys.map(key => ({
        pubkey: key.pubkey,
        isSigner: key.isSigner,
        isWritable: key.isWritable,
      })),
      data: Buffer.from(ix.data),
    }));

    await ctx.program.methods
      .createTransaction(proposedInstructions)
      .accounts({
        wallet: ctx.wallet.publicKey,
        transaction: proposal.publicKey,
        owner: ctx.owners.owner1.publicKey,
   
      })
      .signers([proposal, ctx.owners.owner1])
      .rpc();

    const txAccount = await ctx.program.account.transaction.fetch(proposal.publicKey);
    expect(txAccount.instructions).to.have.length(2);
    expect(txAccount.signers).to.have.length(1);
  });

  it("fails when non-owner tries to create transaction", async () => {
    const proposal = anchor.web3.Keypair.generate();
    const nonOwner = anchor.web3.Keypair.generate();
    const transferAmount = new BN(1_000_000);
    
    // 给非所有者转一些SOL，用于支付交易费用
    await ctx.provider.connection.requestAirdrop(nonOwner.publicKey, LAMPORTS_PER_SOL);
    await new Promise(resolve => setTimeout(resolve, 1000)); // 等待确认
    
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

    try {
      await ctx.program.methods
        .createTransaction([proposedIx])
        .accounts({
          wallet: ctx.wallet.publicKey,
          transaction: proposal.publicKey,
          owner: nonOwner.publicKey,
     
        })
        .signers([proposal, nonOwner])
        .rpc();
      expect.fail("should have failed with non-owner");
    } catch (error) {
      console.log("Actual error:", error.toString());
      expect(error.toString()).to.include("Not an owner");
    }
  });

  it("correctly sets initial transaction state", async () => {
    const proposal = anchor.web3.Keypair.generate();
    const transferAmount = new BN(1_000_000);
    
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

    await ctx.program.methods
      .createTransaction([proposedIx])
      .accounts({
        wallet: ctx.wallet.publicKey,
        transaction: proposal.publicKey,
        owner: ctx.owners.owner1.publicKey,
   
      })
      .signers([proposal, ctx.owners.owner1])
      .rpc();

    const txAccount = await ctx.program.account.transaction.fetch(proposal.publicKey);
    expect(txAccount.executed).to.be.false;
    expect(txAccount.ownerSetSeqno).to.equal(0);
    expect(txAccount.signers).to.deep.equal([ctx.owners.owner1.publicKey]);
    expect(txAccount.wallet.equals(ctx.wallet.publicKey)).to.be.true;
  });
});