import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";

import { MultisigWallet } from "../target/types/multisig_wallet";
import { PublicKey, SystemProgram, LAMPORTS_PER_SOL } from "@solana/web3.js";
import { expect } from "chai";

describe("multisig-wallet", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.MultisigWallet as Program<MultisigWallet>;

  // 生成测试用户和账户
  const owner1 = anchor.web3.Keypair.generate();
  const owner2 = anchor.web3.Keypair.generate();
  const receiver = anchor.web3.Keypair.generate();
  const wallet = anchor.web3.Keypair.generate();
  let vault: PublicKey;
  
  it("Prepare accounts", async () => {
    // 给owner1空投SOL
    const signature = await provider.connection.requestAirdrop(
      owner1.publicKey,
      2 * LAMPORTS_PER_SOL
    );
    await provider.connection.confirmTransaction(signature,'confirmed');
    
    const balance = await provider.connection.getBalance(owner1.publicKey);
    console.log("Owner1 balance:", balance / LAMPORTS_PER_SOL, "SOL");
    expect(balance).to.be.above(0);
  });

  it("Creates a multisig wallet", async () => {
    try {
      // 查找vault PDA
      const [vaultPDA] = await PublicKey.findProgramAddress(
        [
          Buffer.from("vault"),
          wallet.publicKey.toBuffer(),
        ],
        program.programId
      );
      vault = vaultPDA;
      console.log("Vault PDA:", vault.toString());

      await program.methods
        .createWallet(
          [owner1.publicKey, owner2.publicKey],
          new anchor.BN(2)
        )
        .accounts({
          wallet: wallet.publicKey,
          payer: owner1.publicKey,
        })
        .signers([wallet, owner1])
        .rpc();

      const walletAccount = await program.account.wallet.fetch(wallet.publicKey);
      console.log("Wallet created with owners:", walletAccount.owners.map(o => o.toString()));
      expect(walletAccount.owners.length).to.equal(2);
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
        lamports: LAMPORTS_PER_SOL / 2,
      });

      const tx = new anchor.web3.Transaction().add(transferIx);
      await provider.sendAndConfirm(tx, [owner1]);

      const balance = await provider.connection.getBalance(vault);
      console.log("Vault balance:", balance / LAMPORTS_PER_SOL, "SOL");
      expect(balance).to.be.above(0);
    } catch (error) {
      console.error("Error transferring SOL to vault:", error);
      throw error;
    }
  });

  it("Executes a transfer", async () => {
    try {
      const preBalance = await provider.connection.getBalance(receiver.publicKey);
      
      await program.methods
        .executeTransfer(new anchor.BN(LAMPORTS_PER_SOL / 4))
        .accounts({
          wallet: wallet.publicKey,
          receiver: receiver.publicKey,
          owner: owner1.publicKey,
        })
        .signers([owner1])
        .rpc();

      const postBalance = await provider.connection.getBalance(receiver.publicKey);
      console.log("Transfer amount:", (postBalance - preBalance) / LAMPORTS_PER_SOL, "SOL");
      expect(postBalance).to.be.above(preBalance);
    } catch (error) {
      console.error("Error executing transfer:", error);
      throw error;
    }
  });
});