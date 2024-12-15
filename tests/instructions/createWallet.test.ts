import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { MultisigWallet } from "../../target/types/multisig_wallet";
import { PublicKey, SystemProgram } from "@solana/web3.js";
import { BN } from "bn.js";
import { expect } from "chai";
import { TestContext, initializeContext } from "../helper";

describe("power-multisig: create-wallet", () => {
  let ctx: TestContext;

  beforeEach(async () => {
    ctx = await initializeContext();
  });

  it("successfully creates wallet with valid params", async () => {
    const owners = [
      { key: ctx.owners.owner1.publicKey, weight: new BN(60) },
      { key: ctx.owners.owner2.publicKey, weight: new BN(30) },
      { key: ctx.owners.owner3.publicKey, weight: new BN(10) },
    ];
    const threshold = new BN(70);

    await ctx.program.methods
      .createWallet(owners, threshold)
      .accounts({
        wallet: ctx.wallet.publicKey,

        payer: ctx.owners.owner1.publicKey,
      })
      .signers([ctx.wallet, ctx.owners.owner1])
      .rpc();

    const walletAccount = await ctx.program.account.wallet.fetch(
      ctx.wallet.publicKey
    );

    expect(walletAccount.owners).to.have.length(3);
    expect(walletAccount.thresholdWeight.toNumber()).to.equal(70);
    expect(walletAccount.ownerSetSeqno).to.equal(0);
    expect(walletAccount.owners[0].weight.toNumber()).to.equal(60);
    expect(walletAccount.owners[1].weight.toNumber()).to.equal(30);
    expect(walletAccount.owners[2].weight.toNumber()).to.equal(10);
  });

  it("fails with duplicate owners", async () => {
    const owners = [
      { key: ctx.owners.owner1.publicKey, weight: new BN(60) },
      { key: ctx.owners.owner1.publicKey, weight: new BN(40) },
    ];

    try {
      await ctx.program.methods
        .createWallet(owners, new BN(51))
        .accounts({
          wallet: ctx.wallet.publicKey,

          payer: ctx.owners.owner1.publicKey,
        })
        .signers([ctx.wallet, ctx.owners.owner1])
        .rpc();
      expect.fail("should have failed with duplicate owners");
    } catch (error) {
      expect(error.toString()).to.include("Owners must be unique");
    }
  });

  it("fails with no owners", async () => {
    try {
      await ctx.program.methods
        .createWallet([], new BN(1))
        .accounts({
          wallet: ctx.wallet.publicKey,

          payer: ctx.owners.owner1.publicKey,
        })
        .signers([ctx.wallet, ctx.owners.owner1])
        .rpc();
      expect.fail("should have failed with no owners");
    } catch (error) {
    //   console.log("Actual error:", error.toString());
      expect(error.toString()).to.include("Error Code: NoOwners");
    }
  });

  it("fails with zero weight owner", async () => {
    const owners = [
      { key: ctx.owners.owner1.publicKey, weight: new BN(0) },
      { key: ctx.owners.owner2.publicKey, weight: new BN(50) },
    ];

    try {
      await ctx.program.methods
        .createWallet(owners, new BN(51))
        .accounts({
          wallet: ctx.wallet.publicKey,

          payer: ctx.owners.owner1.publicKey,
        })
        .signers([ctx.wallet, ctx.owners.owner1])
        .rpc();
      expect.fail("should have failed with zero weight");
    } catch (error) {
    //   console.log("Actual error:", error.toString());
      expect(error.toString()).to.include("Error Code: InvalidOwnerWeight");
    }
  });

  it("fails with threshold higher than total weight", async () => {
    const owners = [
      { key: ctx.owners.owner1.publicKey, weight: new BN(30) },
      { key: ctx.owners.owner2.publicKey, weight: new BN(20) },
    ];
    const threshold = new BN(51);

    try {
      await ctx.program.methods
        .createWallet(owners, threshold)
        .accounts({
          wallet: ctx.wallet.publicKey,

          payer: ctx.owners.owner1.publicKey,
        })
        .signers([ctx.wallet, ctx.owners.owner1])
        .rpc();
      expect.fail("should have failed with threshold too high");
    } catch (error) {
      expect(error.toString()).to.include(
        "Threshold must be less than or equal to the total weight"
      );
    }
  });

  it("fails with zero threshold", async () => {
    const owners = [
      { key: ctx.owners.owner1.publicKey, weight: new BN(60) },
      { key: ctx.owners.owner2.publicKey, weight: new BN(40) },
    ];

    try {
      await ctx.program.methods
        .createWallet(owners, new BN(0))
        .accounts({
          wallet: ctx.wallet.publicKey,

          payer: ctx.owners.owner1.publicKey,
        })
        .signers([ctx.wallet, ctx.owners.owner1])
        .rpc();
      expect.fail("should have failed with zero threshold");
    } catch (error) {
      expect(error.toString()).to.include("Threshold must be greater than 0");
    }
  });

  it("creates wallet with minimum valid threshold", async () => {
    const owners = [
      { key: ctx.owners.owner1.publicKey, weight: new BN(60) },
      { key: ctx.owners.owner2.publicKey, weight: new BN(40) },
    ];
    const threshold = new BN(1);

    await ctx.program.methods
      .createWallet(owners, threshold)
      .accounts({
        wallet: ctx.wallet.publicKey,

        payer: ctx.owners.owner1.publicKey,
      })
      .signers([ctx.wallet, ctx.owners.owner1])
      .rpc();

    const walletAccount = await ctx.program.account.wallet.fetch(
      ctx.wallet.publicKey
    );
    expect(walletAccount.thresholdWeight.toNumber()).to.equal(1);
  });

  it("creates wallet with maximum valid threshold", async () => {
    const owners = [
      { key: ctx.owners.owner1.publicKey, weight: new BN(60) },
      { key: ctx.owners.owner2.publicKey, weight: new BN(40) },
    ];
    const threshold = new BN(100);

    await ctx.program.methods
      .createWallet(owners, threshold)
      .accounts({
        wallet: ctx.wallet.publicKey,

        payer: ctx.owners.owner1.publicKey,
      })
      .signers([ctx.wallet, ctx.owners.owner1])
      .rpc();

    const walletAccount = await ctx.program.account.wallet.fetch(
      ctx.wallet.publicKey
    );
    expect(walletAccount.thresholdWeight.toNumber()).to.equal(100);
  });
});
