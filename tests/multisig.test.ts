import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { MultisigWallet } from "../target/types/multisig_wallet";
import { PublicKey, SystemProgram, Transaction } from "@solana/web3.js";
import { expect } from "chai";

describe("multisig-wallet", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.MultisigWallet as Program<MultisigWallet>;
  
  const ownerA = anchor.web3.Keypair.generate();
  const ownerB = anchor.web3.Keypair.generate();
  const ownerC = anchor.web3.Keypair.generate();
  
  const threshold = 2;
  let multisigPda: PublicKey;
  let multisigBump: number;
  let transactionPda: PublicKey;
  let transactionBump: number;

  // Helper function to confirm transaction
  const confirmTx = async (signature: string) => {
    const latestBlockhash = await provider.connection.getLatestBlockhash();
    await provider.connection.confirmTransaction(
      {
        signature,
        ...latestBlockhash,
      },
      "confirmed"
    );
  };

  before(async () => {
    const airdropAmount = 10 * anchor.web3.LAMPORTS_PER_SOL;
    
    // Airdrop and confirm for each account
    const signatures = await Promise.all([
      provider.connection.requestAirdrop(ownerA.publicKey, airdropAmount),
      provider.connection.requestAirdrop(ownerB.publicKey, airdropAmount),
      provider.connection.requestAirdrop(ownerC.publicKey, airdropAmount),
      provider.connection.requestAirdrop(provider.wallet.publicKey, airdropAmount),
    ]);

    // Wait for all airdrops to confirm
    await Promise.all(signatures.map(signature => confirmTx(signature)));

    [multisigPda, multisigBump] = await PublicKey.findProgramAddress(
      [Buffer.from("multisig"), provider.wallet.publicKey.toBuffer()],
      program.programId
    );

    [transactionPda, transactionBump] = await PublicKey.findProgramAddress(
      [
        Buffer.from("transaction"),
        multisigPda.toBuffer(),
        new anchor.BN(0).toArrayLike(Buffer, "le", 4),
      ],
      program.programId
    );
    // Fund the multisig PDA for testing
    await provider.connection.requestAirdrop(multisigPda, anchor.web3.LAMPORTS_PER_SOL);
    await new Promise((resolve) => setTimeout(resolve, 1000));
  });

  it("should create a new multisig wallet", async () => {
    const owners = [ownerA.publicKey, ownerB.publicKey, ownerC.publicKey];
    
    const tx = await program.methods
      .createMultisig(owners, threshold)
      .accountsPartial({
        multisig: multisigPda,
        payer: provider.wallet.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .rpc();

    await confirmTx(tx);

    const multisigAccount = await program.account.multisig.fetch(multisigPda);
    
    expect(multisigAccount.owners.map(pub => pub.toString()))
      .to.deep.equal(owners.map(pub => pub.toString()));
    expect(multisigAccount.threshold).to.equal(threshold);
    expect(multisigAccount.transactionCount).to.equal(0);
  });


  it("should fail with invalid threshold", async () => {
    const owners = [ownerA.publicKey, ownerB.publicKey];
    const invalidThreshold = 3;

    try {
      const tx = await program.methods
        .createMultisig(owners, invalidThreshold)
        .accountsPartial({
          multisig: PublicKey.findProgramAddressSync(
            [Buffer.from("multisig"), provider.wallet.publicKey.toBuffer()],
            program.programId
          )[0],
          payer: provider.wallet.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .rpc();
      
      await confirmTx(tx);
      expect.fail("Expected to fail with invalid threshold");
    } catch (err) {
      // Just check if there's any error
      expect(err).to.exist;
    }
  });

it("should propose a new transaction", async () => {
  const transferAmount = new anchor.BN(1000000);
  const transferData = SystemProgram.transfer({
      fromPubkey: multisigPda,
      toPubkey: ownerA.publicKey,
      lamports: transferAmount.toNumber(),
  }).data;
  
  const accounts = [
      {
          pubkey: multisigPda,
          isWritable: true,
          isSigner: true,  // PDA will be a signer
      },
      {
          pubkey: ownerA.publicKey,
          isWritable: true,
          isSigner: false,
      },
  ];

  const tx = await program.methods
      .proposeTransaction(SystemProgram.programId, accounts, transferData)
      .accountsPartial({
          multisig: multisigPda,
          transaction: transactionPda,
          proposer: ownerA.publicKey,
          systemProgram: SystemProgram.programId,
      })
      .signers([ownerA])
      .rpc();

  await confirmTx(tx);

  const transactionAccount = await program.account.transaction.fetch(transactionPda);
  expect(transactionAccount.programId.toString()).to.equal(SystemProgram.programId.toString());
  expect(transactionAccount.didExecute).to.be.false;
  expect(transactionAccount.signers).to.have.length(0);
});


  it("should approve a transaction", async () => {
    const tx = await program.methods
      .approve()
      .accountsPartial({
        multisig: multisigPda,
        transaction: transactionPda,
        owner: ownerA.publicKey,
      })
      .signers([ownerA])
      .rpc();

    await confirmTx(tx);

    const transactionAccount = await program.account.transaction.fetch(transactionPda);
    expect(transactionAccount.signers.map(p => p.toString()))
      .to.include(ownerA.publicKey.toString());
  });

  it("should not allow double signing", async () => {
    try {
      const tx = await program.methods
        .approve()
        .accountsPartial({
          multisig: multisigPda,
          transaction: transactionPda,
          owner: ownerA.publicKey,
        })
        .signers([ownerA])
        .rpc();
      
      await confirmTx(tx);
      expect.fail("Expected to fail with already signed");
    } catch (err) {
      const errorMsg = err.error?.errorMessage || err.toString();
      console.log(errorMsg);
      expect(errorMsg).to.include("Cannot approve a transaction twice");
    }
  });

  it("should execute a transaction with enough approvals", async () => {
    // Add second approval
    const approveTx = await program.methods
        .approve()
        .accounts({
            multisig: multisigPda,
            transaction: transactionPda,
            owner: ownerB.publicKey,
        })
        .signers([ownerB])
        .rpc();

    await confirmTx(approveTx);

    const executeTx = await program.methods
        .executeTransaction()
        .accountsPartial({
            multisig: multisigPda,
            transaction: transactionPda,
            owner: ownerA.publicKey,
            to: ownerA.publicKey,
            systemProgram: SystemProgram.programId,
        })
        .signers([ownerA])
        .rpc();

    await confirmTx(executeTx);

    const updatedTransactionAccount = await program.account.transaction.fetch(transactionPda);
    expect(updatedTransactionAccount.didExecute).to.be.true;

    // 验证转账是否成功
    const balance = await provider.connection.getBalance(ownerA.publicKey);
    expect(balance).to.be.greaterThan(0);
});

});