import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { MultisigWallet} from "../target/types/multisig_wallet";
import { PublicKey, SystemProgram, LAMPORTS_PER_SOL } from "@solana/web3.js";
import { expect } from "chai";
import { BN } from "bn.js";
describe("multisig-wallet", () => {
    const provider = anchor.AnchorProvider.env();
    anchor.setProvider(provider);
  
    const program = anchor.workspace.MultisigWallet as Program<MultisigWallet>;
    
    // Generate test wallets
    const owner1 = anchor.web3.Keypair.generate();
    const owner2 = anchor.web3.Keypair.generate();
    const owner3 = anchor.web3.Keypair.generate();
    
    // Recipient of the SOL transfer
    const recipient = anchor.web3.Keypair.generate();
    const recipient1 = anchor.web3.Keypair.generate();
    const recipient2 = anchor.web3.Keypair.generate();
    const multiTx = anchor.web3.Keypair.generate();  // 用于多指令测试的交易账户
    
    // Test wallet and transaction accounts
    const wallet = anchor.web3.Keypair.generate();
    const transaction = anchor.web3.Keypair.generate();
    let walletPDA: PublicKey;
    let walletBump: number;
  
    before(async () => {
      // Airdrop SOL to owners for transaction fees
      await provider.connection.requestAirdrop(owner1.publicKey, 10 * LAMPORTS_PER_SOL);
      await provider.connection.requestAirdrop(owner2.publicKey, 10 * LAMPORTS_PER_SOL);
      await provider.connection.requestAirdrop(owner3.publicKey, 10 * LAMPORTS_PER_SOL);
      await provider.connection.requestAirdrop(recipient1.publicKey, LAMPORTS_PER_SOL);
      await provider.connection.requestAirdrop(recipient2.publicKey, LAMPORTS_PER_SOL);
      await new Promise(resolve => setTimeout(resolve, 1000)); // Wait for airdrop confirmation
      
      // Find the PDA that will be used as the wallet's vault
      const [_walletPDA, _walletBump] = await PublicKey.findProgramAddress(
        [Buffer.from("vault"), wallet.publicKey.toBuffer()],
        program.programId
      );
      walletPDA = _walletPDA;
      walletBump = _walletBump;
  
      // Fund the vault with some SOL for testing
      await provider.connection.requestAirdrop(walletPDA, 2 * LAMPORTS_PER_SOL);
      await new Promise(resolve => setTimeout(resolve, 1000)); // Wait for airdrop confirmation
    });
  
    it("Creates a multisig wallet", async () => {
      // Create owner configurations with different weights
      const owners = [
        { key: owner1.publicKey, weight: new BN(2) },
        { key: owner2.publicKey, weight: new BN(2) },
        { key: owner3.publicKey, weight: new BN(1) },
      ];
      
      const thresholdWeight = new BN(3); // Require at least weight of 3 to execute transactions
  
      await program.methods
        .createWallet(owners, thresholdWeight)
        .accountsPartial({
          wallet: wallet.publicKey,
          vault: walletPDA,
          payer: provider.wallet.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([wallet])
        .rpc();
  
      // Verify wallet state
      const walletAccount = await program.account.wallet.fetch(wallet.publicKey);
      expect(walletAccount.owners).to.have.length(3);
      expect(walletAccount.thresholdWeight.toString()).to.equal(thresholdWeight.toString());
    });
  
    it("Creates a transaction to transfer SOL", async () => {
      const transferAmount = 1 * LAMPORTS_PER_SOL;
      
      // Prepare the transfer instruction
      const transferIx = SystemProgram.transfer({
        fromPubkey: walletPDA,
        toPubkey: recipient.publicKey,
        lamports: transferAmount,
      });
  
      // Create the proposed instruction with correct types
      const proposedInstruction = {
        programId: transferIx.programId,
        accounts: transferIx.keys.map(key => ({
          pubkey: key.pubkey,
          isSigner: key.isSigner,
          isWritable: key.isWritable,
        })),
        data: transferIx.data,
      };
  
      await program.methods
        .createTransaction([proposedInstruction], 3, 100)
        .accountsPartial({
          wallet: wallet.publicKey,
          transaction: transaction.publicKey,
          owner: owner1.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([transaction, owner1])
        .rpc();
  
      // Verify transaction state
      const txAccount = await program.account.transaction.fetch(transaction.publicKey);
      expect(txAccount.executed).to.be.false;
      expect(txAccount.signers).to.have.length(1);
      expect(txAccount.signers[0].toString()).to.equal(owner1.publicKey.toString());
    });
  
    it("Approves the transaction with required weights", async () => {
      // Owner 2 approves
      await program.methods
        .approve()
        .accounts({
          wallet: wallet.publicKey,
          transaction: transaction.publicKey,
          owner: owner2.publicKey,
        })
        .signers([owner2])
        .rpc();
  
      // Verify updated signers
      const updatedTx = await program.account.transaction.fetch(transaction.publicKey);
      expect(updatedTx.signers).to.have.length(2);
    //   expect(updatedTx.signers).to.include.deep.memberOf([owner1.publicKey, owner2.publicKey]);
    });
  
    it("Executes the transaction", async () => {
      // Get recipient's initial balance
      const initialBalance = await provider.connection.getBalance(recipient.publicKey);
  
      // Execute the transaction
     const execute_ix = await program.methods
        .executeTransaction()
        .accountsPartial({
          wallet: wallet.publicKey,
          transaction: transaction.publicKey,
          owner: owner1.publicKey,
          vault: walletPDA,
          systemProgram: SystemProgram.programId,
        })
        .remainingAccounts([
          {
            pubkey: SystemProgram.programId,
            isWritable: false,
            isSigner: false,
          },
          {
            pubkey: walletPDA,
            isWritable: true,
            isSigner: false,
          },
          {
            pubkey: recipient.publicKey,
            isWritable: true,
            isSigner: false,
          },
        ])
        .signers([owner1])
        .rpc();
  
            await provider.connection.confirmTransaction(execute_ix,'confirmed');
      // Wait a bit for the transaction to be confirmed
      await new Promise(resolve => setTimeout(resolve, 1000));
  
      // Verify execution
      const txAccount = await program.account.transaction.fetch(transaction.publicKey);
      expect(txAccount.executed).to.be.true;
  
      // Verify recipient received the SOL
      const finalBalance = await provider.connection.getBalance(recipient.publicKey);
      expect(finalBalance).to.be.greaterThan(initialBalance);
    });
    it("Executes multiple instructions in a transaction", async () => {
        // crete two recipients
        const recipient1 = anchor.web3.Keypair.generate();
        const recipient2 = anchor.web3.Keypair.generate();
        
        // set transfer amounts
        const transferAmount1 = new BN(0.5 * LAMPORTS_PER_SOL);
        const transferAmount2 = new BN(0.3 * LAMPORTS_PER_SOL);
        
        // Create transfer instructions
        const transferIx1 = SystemProgram.transfer({
            fromPubkey: walletPDA,
            toPubkey: recipient1.publicKey,
            lamports: transferAmount1.toNumber(),
        });
    
        const transferIx2 = SystemProgram.transfer({
            fromPubkey: walletPDA,
            toPubkey: recipient2.publicKey,
            lamports: transferAmount2.toNumber(),
        });
    
        // Create proposed instructions
        const proposedInstructions = [
            {
                programId: transferIx1.programId,
                accounts: transferIx1.keys.map(key => ({
                    pubkey: key.pubkey,
                    isSigner: key.isSigner,
                    isWritable: key.isWritable,
                })),
                data: transferIx1.data,
            },
            {
                programId: transferIx2.programId,
                accounts: transferIx2.keys.map(key => ({
                    pubkey: key.pubkey,
                    isSigner: key.isSigner,
                    isWritable: key.isWritable,
                })),
                data: transferIx2.data,
            }
        ];
    
        // 创建多指令交易
        const multiTx = anchor.web3.Keypair.generate();
        await program.methods
            .createTransaction(proposedInstructions, 5, 100)
            .accountsPartial({
                wallet: wallet.publicKey,
                transaction: multiTx.publicKey,
                owner: owner1.publicKey,
                systemProgram: SystemProgram.programId,
            })
            .signers([multiTx, owner1])
            .rpc();
    
        // 获取两个接收者的初始余额
        const initialBalance1 = await provider.connection.getBalance(recipient1.publicKey);
        const initialBalance2 = await provider.connection.getBalance(recipient2.publicKey);
    
        // owner2 批准交易
        await program.methods
            .approve()
            .accounts({
                wallet: wallet.publicKey,
                transaction: multiTx.publicKey,
                owner: owner2.publicKey,
            })
            .signers([owner2])
            .rpc();
    
        // 执行多指令交易
        await program.methods
            .executeTransaction()
            .accountsPartial({
                wallet: wallet.publicKey,
                transaction: multiTx.publicKey,
                owner: owner1.publicKey,
                vault: walletPDA,
                systemProgram: SystemProgram.programId,
            })
            .remainingAccounts([
              // first transfer instruction's accounts
                {
                    pubkey: SystemProgram.programId,
                    isWritable: false,
                    isSigner: false,
                },
                {
                    pubkey: walletPDA,
                    isWritable: true,
                    isSigner: false,
                },
                {
                    pubkey: recipient1.publicKey,
                    isWritable: true,
                    isSigner: false,
                },
                //second transfer instruction's accounts
                {
                    pubkey: SystemProgram.programId,
                    isWritable: false,
                    isSigner: false,
                },
                {
                    pubkey: walletPDA,
                    isWritable: true,
                    isSigner: false,
                },
                {
                    pubkey: recipient2.publicKey,
                    isWritable: true,
                    isSigner: false,
                },
            ])
            .signers([owner1])
            .rpc();
    
            //wait for the transaction to be executed
        await new Promise(resolve => setTimeout(resolve, 1000));
    
        //verify transaction account is executed
        const txAccount = await program.account.transaction.fetch(multiTx.publicKey);
        expect(txAccount.executed).to.be.true;
    
        //verify that both recipients received SOL
        const finalBalance1 = await provider.connection.getBalance(recipient1.publicKey);
        const finalBalance2 = await provider.connection.getBalance(recipient2.publicKey);
        
        expect(finalBalance1).to.be.greaterThan(initialBalance1);
        expect(finalBalance2).to.be.greaterThan(initialBalance2);
        
        //verify that the transfer amounts are correct
        expect(finalBalance1 - initialBalance1).to.equal(transferAmount1.toNumber());
        expect(finalBalance2 - initialBalance2).to.equal(transferAmount2.toNumber());
    }); 
    it("Closes the executed transaction", async () => {
      await program.methods
        .closeTransaction()
        .accounts({
          wallet: wallet.publicKey,
          transaction: transaction.publicKey,
          recipient: owner1.publicKey,
          owner: owner1.publicKey,
        })
        .signers([owner1])
        .rpc();
  
      // Verify transaction account is closed
      const txAccount = await program.account.transaction.fetchNullable(transaction.publicKey);
      expect(txAccount).to.be.null;
    });
  });