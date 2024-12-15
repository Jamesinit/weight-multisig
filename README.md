# Weighted Multisig Wallet

这是一个基于 Solana 区块链的加权多重签名(multisig)钱包智能合约。该合约允许多个所有者基于权重共同管理资金,每个所有者都有不同的投票权重。

## 功能特点

- 基于权重的投票系统
- 可配置的权重阈值
- 灵活的所有者管理
- 交易提议和执行机制
- 安全的资金管理

## 主要组件

### 账户结构

1. `Wallet`: 多签钱包主账户
   - 存储所有者列表及其权重
   - 保存投票阈值
   - 追踪所有者集版本号(owner_set_seqno)

2. `Transaction`: 交易提议账户
   - 记录提议的指令
   - 追踪签名状态
   - 存储执行状态

3. `Vault`: PDA(程序派生地址)账户
   - 用于安全存储资金
   - 作为程序签名者

## 使用示例

### 1. 创建多签钱包

```typescript
// 创建所有者配置，设置不同权重
const owners = [
  { key: owner1.publicKey, weight: new BN(2) },
  { key: owner2.publicKey, weight: new BN(2) },
  { key: owner3.publicKey, weight: new BN(1) },
];

// 设置执行阈值为3
const thresholdWeight = new BN(3);

// 创建钱包
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
```

### 2. 创建转账交易

```typescript
// 准备转账指令
const transferAmount = 1 * LAMPORTS_PER_SOL;
const transferIx = SystemProgram.transfer({
  fromPubkey: walletPDA,
  toPubkey: recipient.publicKey,
  lamports: transferAmount,
});

// 创建交易提议
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
```

### 3. 批准交易

```typescript
await program.methods
  .approve()
  .accounts({
    wallet: wallet.publicKey,
    transaction: transaction.publicKey,
    owner: owner2.publicKey,
  })
  .signers([owner2])
  .rpc();
```

### 4. 执行交易

```typescript
await program.methods
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
```

### 5. 多指令交易示例

以下示例展示如何在一个交易中执行多个转账：

```typescript
// 创建两个转账指令
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

// 合并为一个交易提议
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

// 创建并执行多指令交易
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
```

### 6. 关闭已执行交易

```typescript
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
```

## 开发环境设置

1. 安装依赖:
```bash
npm install @coral-xyz/anchor @solana/web3.js
```

2. 构建项目:
```bash
anchor build
```

3. 运行测试:
```bash
anchor test
```

## 安全注意事项

1. 在修改所有者权重或阈值时要特别谨慎
2. 确保维护正确的所有者集版本号
3. 建议在主网部署前进行充分测试
4. 监控交易执行状态和错误处理
5. 确保所有交易都经过足够权重的签名

## 错误处理

合约定义了多种错误类型(ErrorCode):
- InvalidWallet: 无效的钱包地址
- NotOwner: 非所有者操作
- NotSigned: 未签名
- AlreadyExecuted: 交易已执行
- OwnerSetChanged: 所有者集已变更
- UnauthorizedClose: 未授权关闭交易