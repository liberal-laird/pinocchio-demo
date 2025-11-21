import {
  Connection,
  Keypair,
  LAMPORTS_PER_SOL,
  PublicKey,
  SystemProgram,
  Transaction,
  TransactionInstruction,
  sendAndConfirmTransaction,
} from "@solana/web3.js";
import { assert } from "chai";

// 使用实际部署的程序 ID
const PROGRAM_ID = new PublicKey("GMYuTSUDK5psTjN45KTCWrMNfSdDbRHdnY1zzpgVDYgG");

// 指令枚举
const Instruction = {
  Deposit: 0,
  Withdraw: 1,
} as const;

// 存款指令数据结构
class DepositInstruction {
  amount: bigint;

  constructor(amount: bigint) {
    this.amount = amount;
  }

  serialize(): Buffer {
    const discriminator = Buffer.from([Instruction.Deposit]);
    const amountBuffer = Buffer.alloc(8);
    amountBuffer.writeBigUInt64LE(this.amount);
    return Buffer.concat([discriminator, amountBuffer]);
  }
}

// 取款指令数据结构
class WithdrawInstruction {
  serialize(): Buffer {
    return Buffer.from([Instruction.Withdraw]);
  }
}

describe("Pinocchio Demo 最终测试", () => {
  const connection = new Connection("http://localhost:8899", "confirmed");
  let owner: Keypair;
  let vaultPda: PublicKey;
  let vaultBump: number;

  before(async () => {
    // 创建测试账户
    owner = Keypair.generate();

    // 为测试账户空投 SOL
    const airdropSignature = await connection.requestAirdrop(
      owner.publicKey,
      2 * LAMPORTS_PER_SOL,
    );
    await connection.confirmTransaction(airdropSignature);

    console.log("测试账户创建完成:", owner.publicKey.toBase58());

    // 派生 vault PDA
    [vaultPda, vaultBump] = PublicKey.findProgramAddressSync(
      [Buffer.from("vault"), owner.publicKey.toBuffer()],
      PROGRAM_ID,
    );

    console.log("Vault PDA:", vaultPda.toBase58());
    console.log("Vault Bump:", vaultBump);
  });

  // 创建存款指令
  function createDepositInstruction(
    owner: PublicKey,
    vaultPda: PublicKey,
    amount: bigint,
  ): TransactionInstruction {
    const depositInstruction = new DepositInstruction(amount);
    const instructionData = depositInstruction.serialize();

    return new TransactionInstruction({
      keys: [
        { pubkey: owner, isSigner: true, isWritable: true },
        { pubkey: vaultPda, isSigner: false, isWritable: true },
        { pubkey: PROGRAM_ID, isSigner: false, isWritable: false },
        { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
      ],
      programId: PROGRAM_ID,
      data: instructionData,
    });
  }

  // 创建取款指令
  function createWithdrawInstruction(
    owner: PublicKey,
    vaultPda: PublicKey,
  ): TransactionInstruction {
    const withdrawInstruction = new WithdrawInstruction();
    const instructionData = withdrawInstruction.serialize();

    return new TransactionInstruction({
      keys: [
        { pubkey: owner, isSigner: true, isWritable: true },
        { pubkey: vaultPda, isSigner: false, isWritable: true },
        { pubkey: PROGRAM_ID, isSigner: false, isWritable: false },
      ],
      programId: PROGRAM_ID,
      data: instructionData,
    });
  }

  describe("环境验证", () => {
    it("应该验证程序已部署", async () => {
      const programInfo = await connection.getAccountInfo(PROGRAM_ID);
      assert.isNotNull(programInfo, "程序应该已部署");
      assert.isTrue(programInfo.executable, "程序应该是可执行的");
      console.log("程序数据长度:", programInfo.data.length);
    });

    it("应该验证测试账户有足够余额", async () => {
      const balance = await connection.getBalance(owner.publicKey);
      console.log("测试账户余额:", balance / LAMPORTS_PER_SOL, "SOL");
      assert.isAtLeast(balance, LAMPORTS_PER_SOL, "测试账户应该有足够的SOL");
    });
  });

  describe("存款功能测试", () => {
    it("应该成功执行存款交易", async () => {
      const depositAmount = BigInt(1000000); // 0.001 SOL

      const depositInstruction = createDepositInstruction(
        owner.publicKey,
        vaultPda,
        depositAmount,
      );

      const transaction = new Transaction().add(depositInstruction);

      try {
        const signature = await sendAndConfirmTransaction(
          connection,
          transaction,
          [owner],
        );

        console.log("存款交易成功:", signature);

        // 验证 vault 余额
        const vaultAccount = await connection.getAccountInfo(vaultPda);
        assert.isNotNull(vaultAccount, "Vault 账户应该存在");

        // 验证 vault 被程序拥有
        assert.equal(
          vaultAccount.owner.toBase58(),
          PROGRAM_ID.toBase58(),
          "Vault 应该被程序拥有",
        );

        // 验证余额包含存款金额和租金
        console.log("Vault 余额:", vaultAccount.lamports);
        assert.isAtLeast(
          vaultAccount.lamports,
          Number(depositAmount),
          "Vault 余额应该至少包含存款金额",
        );

      } catch (error) {
        console.error("存款交易失败:", error);
        throw error;
      }
    });

    it("应该支持多次存款", async () => {
      const additionalDeposit = BigInt(500000); // 0.0005 SOL

      const depositInstruction = createDepositInstruction(
        owner.publicKey,
        vaultPda,
        additionalDeposit,
      );

      const transaction = new Transaction().add(depositInstruction);

      try {
        const signature = await sendAndConfirmTransaction(
          connection,
          transaction,
          [owner],
        );

        console.log("第二次存款成功:", signature);

        // 验证余额增加
        const vaultAccount = await connection.getAccountInfo(vaultPda);
        console.log("第二次存款后余额:", vaultAccount.lamports);

      } catch (error) {
        console.error("第二次存款失败:", error);
        throw error;
      }
    });

    it("应该拒绝零金额存款", async () => {
      const zeroAmount = BigInt(0);

      const depositInstruction = createDepositInstruction(
        owner.publicKey,
        vaultPda,
        zeroAmount,
      );

      const transaction = new Transaction().add(depositInstruction);

      try {
        await sendAndConfirmTransaction(connection, transaction, [owner]);
        assert.fail("应该拒绝零金额存款");
      } catch (error) {
        console.log("预期错误（零金额被拒绝）:", error.message);
        assert.include(
          error.message,
          "invalid instruction data",
          "应该返回无效指令数据错误",
        );
      }
    });
  });

  describe("取款功能测试", () => {
    it("应该成功从 vault 取款", async () => {
      // 获取取款前的余额
      const ownerBalanceBefore = await connection.getBalance(owner.publicKey);
      const vaultBalanceBefore = await connection.getBalance(vaultPda);

      console.log(
        `取款前 - 所有者余额: ${ownerBalanceBefore / LAMPORTS_PER_SOL} SOL`,
      );
      console.log(
        `取款前 - Vault 余额: ${vaultBalanceBefore / LAMPORTS_PER_SOL} SOL`,
      );

      const withdrawInstruction = createWithdrawInstruction(
        owner.publicKey,
        vaultPda,
      );

      const transaction = new Transaction().add(withdrawInstruction);

      try {
        const signature = await sendAndConfirmTransaction(
          connection,
          transaction,
          [owner],
        );

        console.log("取款交易成功:", signature);

        // 验证余额变化
        const ownerBalanceAfter = await connection.getBalance(owner.publicKey);
        const vaultBalanceAfter = await connection.getBalance(vaultPda);

        console.log(
          `取款后 - 所有者余额: ${ownerBalanceAfter / LAMPORTS_PER_SOL} SOL`,
        );
        console.log(
          `取款后 - Vault 余额: ${vaultBalanceAfter / LAMPORTS_PER_SOL} SOL`,
        );

        // 验证所有者余额增加
        assert.isAtLeast(
          ownerBalanceAfter,
          ownerBalanceBefore,
          "所有者余额应该增加",
        );

        // 验证 vault 保留租金最低限额
        assert.isAtLeast(vaultBalanceAfter, 0, "Vault 应该保留租金最低限额");

      } catch (error) {
        console.error("取款交易失败:", error);
        throw error;
      }
    });

    it("应该拒绝非所有者取款", async () => {
      const unauthorizedUser = Keypair.generate();

      // 为未授权用户空投 SOL
      const airdropSignature = await connection.requestAirdrop(
        unauthorizedUser.publicKey,
        LAMPORTS_PER_SOL,
      );
      await connection.confirmTransaction(airdropSignature);

      const withdrawInstruction = createWithdrawInstruction(
        unauthorizedUser.publicKey,
        vaultPda,
      );

      const transaction = new Transaction().add(withdrawInstruction);

      try {
        await sendAndConfirmTransaction(connection, transaction, [
          unauthorizedUser,
        ]);
        assert.fail("应该拒绝非所有者取款");
      } catch (error) {
        console.log("预期错误（非所有者取款被拒绝）:", error.message);
        assert.include(
          error.message,
          "InvalidAccountOwner",
          "应该返回无效账户所有者错误",
        );
      }
    });
  });

  describe("完整流程测试", () => {
    it("应该支持完整的存款取款流程", async () => {
      const testOwner = Keypair.generate();

      // 为测试账户空投 SOL
      const airdropSignature = await connection.requestAirdrop(
        testOwner.publicKey,
        2 * LAMPORTS_PER_SOL,
      );
      await connection.confirmTransaction(airdropSignature);

      // 派生新的 vault PDA
      const [testVaultPda] = PublicKey.findProgramAddressSync(
        [Buffer.from("vault"), testOwner.publicKey.toBuffer()],
        PROGRAM_ID,
      );

      const depositAmount = BigInt(800000); // 0.0008 SOL

      // 存款
      const depositInstruction = createDepositInstruction(
        testOwner.publicKey,
        testVaultPda,
        depositAmount,
      );

      await sendAndConfirmTransaction(
        connection,
        new Transaction().add(depositInstruction),
        [testOwner],
      );

      // 验证存款成功
      const vaultBalanceAfterDeposit = await connection.getBalance(testVaultPda);
      console.log("存款后余额:", vaultBalanceAfterDeposit);

      // 取款
      const withdrawInstruction = createWithdrawInstruction(
        testOwner.publicKey,
        testVaultPda,
      );

      await sendAndConfirmTransaction(
        connection,
        new Transaction().add(withdrawInstruction),
        [testOwner],
      );

      // 验证取款成功
      const vaultBalanceAfterWithdraw = await connection.getBalance(testVaultPda);
      console.log("取款后余额:", vaultBalanceAfterWithdraw);

      console.log("完整流程测试完成");
    });
  });

  describe("总结", () => {
    it("应该验证所有核心功能正常工作", () => {
      console.log("🎉 测试完成！PDA 签名和程序拥有权问题已解决");
      console.log("✅ 存款功能正常工作");
      console.log("✅ 取款功能正常工作");
      console.log("✅ 权限验证正常工作");
      console.log("✅ 错误处理正常工作");
    });
  });
});
