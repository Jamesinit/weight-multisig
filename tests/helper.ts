import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { MultisigWallet } from "../target/types/multisig_wallet";
import { 
  PublicKey, 
  SystemProgram, 
  LAMPORTS_PER_SOL,
  TransactionInstruction,
  Transaction,
  sendAndConfirmTransaction
} from "@solana/web3.js";
import { BN } from "bn.js";

// 测试上下文类型
export type TestContext = {
  provider: anchor.AnchorProvider;
  program: Program<MultisigWallet>;
  wallet: anchor.web3.Keypair;
  vault: PublicKey;
  owners: {
    owner1: anchor.web3.Keypair;
    owner2: anchor.web3.Keypair;
    owner3: anchor.web3.Keypair;
  };
};

// 初始化测试上下文
export async function initializeContext(): Promise<TestContext> {
  const ctx: TestContext = {
    provider: anchor.AnchorProvider.env(),
    program: null as any,
    wallet: anchor.web3.Keypair.generate(),
    vault: null as any,
    owners: {
      owner1: anchor.web3.Keypair.generate(),
      owner2: anchor.web3.Keypair.generate(),
      owner3: anchor.web3.Keypair.generate(),
    },
  };

  anchor.setProvider(ctx.provider);
  ctx.program = anchor.workspace.MultisigWallet as Program<MultisigWallet>;

  // 空投SOL给owner1
  const signature = await ctx.provider.connection.requestAirdrop(
    ctx.owners.owner1.publicKey,
    10 * LAMPORTS_PER_SOL
  );
  await ctx.provider.connection.confirmTransaction(signature);

  // 给其他owner转账
  const tx = new Transaction();
  [ctx.owners.owner2, ctx.owners.owner3].forEach(owner => {
    tx.add(
      SystemProgram.transfer({
        fromPubkey: ctx.owners.owner1.publicKey,
        toPubkey: owner.publicKey,
        lamports: LAMPORTS_PER_SOL,
      })
    );
  });
  await sendAndConfirmTransaction(ctx.provider.connection, tx, [ctx.owners.owner1]);

  // 创建vault PDA
  const [vaultPDA] = await PublicKey.findProgramAddress(
    [Buffer.from("vault"), ctx.wallet.publicKey.toBuffer()],
    ctx.program.programId
  );
  ctx.vault = vaultPDA;

  return ctx;
}

// 创建钱包辅助函数
export async function createMultisigWallet(
  ctx: TestContext,
  owners: { key: PublicKey; weight: number }[] = [
    { key: ctx.owners.owner1.publicKey, weight: 60 },
    { key: ctx.owners.owner2.publicKey, weight: 30 },
    { key: ctx.owners.owner3.publicKey, weight: 10 },
  ],
  threshold: number = 70
) {
  await ctx.program.methods
    .createWallet(
      owners.map(o => ({ key: o.key, weight: new BN(o.weight) })),
      new BN(threshold)
    )
    .accounts({
      wallet: ctx.wallet.publicKey,
      payer: ctx.owners.owner1.publicKey,
    })
    .signers([ctx.wallet, ctx.owners.owner1])
    .rpc();

  // 给vault转SOL以便测试
  await ctx.provider.sendAndConfirm(
    new Transaction().add(
      SystemProgram.transfer({
        fromPubkey: ctx.owners.owner1.publicKey,
        toPubkey: ctx.vault,
        lamports: 2 * LAMPORTS_PER_SOL,
      })
    ),
    [ctx.owners.owner1]
  );
}

// 创建并执行提案的辅助函数
export async function createAndExecuteProposal(
  ctx: TestContext,
  instruction: TransactionInstruction,
  signers: anchor.web3.Keypair[] = [ctx.owners.owner1, ctx.owners.owner2]
) {
  const proposal = anchor.web3.Keypair.generate();
  
  // 转换为 ProposedInstruction 格式
  const proposedIx = {
    programId: instruction.programId,
    accounts: instruction.keys.map(key => ({
      pubkey: key.pubkey,
      isSigner: key.isSigner,
      isWritable: key.isWritable
    })),
    data: Buffer.from(instruction.data)
  };

  // 创建提案
  await ctx.program.methods
    .createTransaction([proposedIx])
    .accounts({
      wallet: ctx.wallet.publicKey,
      transaction: proposal.publicKey,
      owner: signers[0].publicKey,
    })
    .signers([proposal, signers[0]])
    .rpc();

  // 其他签名者审批
  for (const signer of signers.slice(1)) {
    await ctx.program.methods
      .approve()
      .accounts({
        wallet: ctx.wallet.publicKey,
        transaction: proposal.publicKey,
        owner: signer.publicKey,
      })
      .signers([signer])
      .rpc();
  }

  // 执行提案
  await ctx.program.methods
    .executeTransaction()
    .accounts({
      transaction: proposal.publicKey,
      owner: signers[0].publicKey,
    })
    .remainingAccounts([
      ...instruction.keys,
      {
        pubkey: instruction.programId,
        isWritable: false,
        isSigner: false,
      }
    ])
    .signers([signers[0]])
    .rpc();

  return proposal;
}

// 验证提案执行失败的辅助函数
export async function expectProposalToFail(
  ctx: TestContext,
  instruction: TransactionInstruction,
  expectedError: string,
  signer = ctx.owners.owner1
) {
  const proposal = anchor.web3.Keypair.generate();
  const proposedIx = {
    programId: instruction.programId,
    accounts: instruction.keys.map(key => ({
      pubkey: key.pubkey,
      isSigner: key.isSigner,
      isWritable: key.isWritable
    })),
    data: Buffer.from(instruction.data)
  };

  try {
    await ctx.program.methods
      .createTransaction([proposedIx])
      .accounts({
        wallet: ctx.wallet.publicKey,
        transaction: proposal.publicKey,
        owner: signer.publicKey,
      })
      .signers([proposal, signer])
      .rpc();
    
    await ctx.program.methods
      .executeTransaction()
      .accounts({
        transaction: proposal.publicKey,
        owner: signer.publicKey,
      })
      .remainingAccounts([
        ...instruction.keys,
        {
          pubkey: instruction.programId,
          isWritable: false,
          isSigner: false,
        }
      ])
      .signers([signer])
      .rpc();

    throw new Error("Transaction should have failed");
  } catch (error) {
    if (!error.toString().includes(expectedError)) {
      throw error;
    }
  }
}