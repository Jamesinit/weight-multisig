import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";

import { MultisigWallet } from "../target/types/multisig_wallet";
import { PublicKey, SystemProgram, LAMPORTS_PER_SOL } from "@solana/web3.js";
import { expect } from "chai";
import { BN } from "bn.js";


describe("weighted-multisig-wallet", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.MultisigWallet as Program<MultisigWallet>;

  // 生成测试用户和账户
  const owner1 = anchor.web3.Keypair.generate(); // 权重 60
  const owner2 = anchor.web3.Keypair.generate(); // 权重 30
  const owner3 = anchor.web3.Keypair.generate(); // 权重 10
  const receiver = anchor.web3.Keypair.generate();
  const wallet = anchor.web3.Keypair.generate();
  let vault: PublicKey;
  
  it("Prepare accounts", async () => {
    // 给测试用户空投SOL
    for (const owner of [owner1, owner2, owner3]) {
      const signature = await provider.connection.requestAirdrop(
        owner.publicKey,
        2 * LAMPORTS_PER_SOL
      );
      await provider.connection.confirmTransaction(signature);
    }
  });

  it("Creates a weighted multisig wallet", async () => {
    try {
      // 查找vault PDA
      const [vaultPDA] = await PublicKey.findProgramAddress(
        [Buffer.from("vault"), wallet.publicKey.toBuffer()],
        program.programId
      );
      vault = vaultPDA;

      const owners = [
        { key: owner1.publicKey, weight: new BN(60) },
        { key: owner2.publicKey, weight: new BN(30)},
        { key: owner3.publicKey, weight: new BN(10) },
      ];

      await program.methods
        .createWallet(owners, new anchor.BN(70)) // 需要70%的权重
        .accounts({
          wallet: wallet.publicKey,
          payer: owner1.publicKey,
        })
        .signers([wallet, owner1])
        .rpc();

      const walletAccount = await program.account.wallet.fetch(wallet.publicKey);
      console.log("Wallet created with threshold weight:", walletAccount.thresholdWeight.toString());
      expect(walletAccount.owners.length).to.equal(3);
    } catch (error) {
      console.error("Error creating wallet:", error);
      throw error;
    }
  });

  it("Transfers SOL to vault", async () => {
    try {
      const transferIx = SystemProgram.transfer({
        fromPubkey: owner1.publicKey,
        toPubkey: vault,
        lamports: LAMPORTS_PER_SOL,
      });

      const tx = new anchor.web3.Transaction().add(transferIx);
      await provider.sendAndConfirm(tx, [owner1]);

      const balance = await provider.connection.getBalance(vault);
      expect(balance).to.equal(LAMPORTS_PER_SOL);
    } catch (error) {
      console.error("Error transferring SOL to vault:", error);
      throw error;
    }
  });

  it("Executes a transfer with sufficient weight", async () => {
    try {
      const preBalance = await provider.connection.getBalance(receiver.publicKey);
      
      // owner1(60) + owner2(30) = 90 > 70
      const signatures = [owner1.publicKey, owner2.publicKey];

      await program.methods
        .executeTransfer(
          new anchor.BN(LAMPORTS_PER_SOL / 2),
          signatures
        )
        .accounts({
          wallet: wallet.publicKey,
          receiver: receiver.publicKey,
          owner: owner1.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([owner1])
        .rpc();

      const postBalance = await provider.connection.getBalance(receiver.publicKey);
      expect(postBalance - preBalance).to.equal(LAMPORTS_PER_SOL / 2);
    } catch (error) {
      console.error("Error executing transfer:", error);
      throw error;
    }
  });

  it("Fails to execute with insufficient weight", async () => {
    try {
      // owner2(30) + owner3(10) = 40 < 70
      const signatures = [owner2.publicKey, owner3.publicKey];

      await program.methods
        .executeTransfer(
          new anchor.BN(LAMPORTS_PER_SOL / 4),
          signatures
        )
        .accounts({
          wallet: wallet.publicKey,
          receiver: receiver.publicKey,
          owner: owner2.publicKey,
        })
        .signers([owner2])
        .rpc();
      
      expect.fail("Should have failed");
    } catch (error) {
      expect(error.toString()).to.include("Insufficient signers weight");
    }
  });
});