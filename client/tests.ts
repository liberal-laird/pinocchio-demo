// client/tests.ts
import { describe, it, before } from "node:test";
import assert from "node:assert";
import {
  airdropFactory,
  createSolanaRpc,
  createSolanaRpcSubscriptions,
  generateKeyPairSigner,
  lamports,
  sendAndConfirmTransactionFactory,
  pipe,
  createTransactionMessage,
  setTransactionMessageFeePayer,
  setTransactionMessageLifetimeUsingBlockhash,
  appendTransactionMessageInstruction,
  signTransactionMessageWithSigners,
  getSignatureFromTransaction,
  getProgramDerivedAddress,
  getAddressEncoder,
  getUtf8Encoder,
} from "@solana/kit";
import { SYSTEM_PROGRAM_ADDRESS } from "@solana-program/system";

// 使用实际部署的程序地址
const PROGRAM_ID = "GMYuTSUDK5psTjN45KTCWrMNfSdDbRHdnY1zzpgVDYgG";

const LAMPORTS_PER_SOL = BigInt(1_000_000_000);

describe("Vault Program", () => {
  let rpc: any;
  let rpcSubscriptions: any;
  let signer: any;
  let vaultRent: BigInt;
  let vaultPDA: any;

  const ACCOUNT_DISCRIMINATOR_SIZE = 8; // same as Anchor/Rust
  const U64_SIZE = 8; // u64 is 8 bytes
  const VAULT_SIZE = ACCOUNT_DISCRIMINATOR_SIZE + U64_SIZE; // 16
  const DEPOSIT_AMOUNT = BigInt(100000000);

  before(async () => {
    // Establish connection to Solana cluster
    // / 建立与 Solana 集群的连接
    const httpProvider = "http://127.0.0.1:8899";
    const wssProvider = "ws://127.0.0.1:8900";
    rpc = createSolanaRpc(httpProvider);
    rpcSubscriptions = createSolanaRpcSubscriptions(wssProvider);

    // Generate signers
    // / 生成签名者
    signer = await generateKeyPairSigner();
    const signerAddress = await signer.address;

    // Airdrop SOL to signer
    // / 将 SOL 空投到签名者
    const airdrop = airdropFactory({ rpc, rpcSubscriptions });
    await airdrop({
      commitment: "confirmed",
      lamports: lamports(LAMPORTS_PER_SOL),
      recipientAddress: signerAddress,
    });

    console.log(`Airdropped SOL to Signer: ${signerAddress}`);

    // get vault rent
    // / 获取 vault 租金
    vaultRent = await rpc.getMinimumBalanceForRentExemption(VAULT_SIZE).send();

    // Get vault PDA
    // / 获取 vault PDA
    const seedTag = getUtf8Encoder().encode("vault");
    const seedSigner = getAddressEncoder().encode(await signer.address);
    const pdaResult = await getProgramDerivedAddress({
      programAddress: PROGRAM_ID,
      seeds: [seedTag, seedSigner],
    });

    vaultPDA = pdaResult.address;
    console.log(`Vault PDA: ${vaultPDA}`);
  });

  it("can deposit to vault", async () => {
    // 创建存款指令数据
    const depositData = new Uint8Array(9);
    depositData[0] = 0; // 存款指令标识符
    const amountBytes = new Uint8Array(
      new BigUint64Array([DEPOSIT_AMOUNT]).buffer,
    );
    depositData.set(amountBytes, 1);

    const depositInstruction = {
      accounts: [
        {
          address: await signer.address,
          role: "writableSigner" as const,
        },
        {
          address: vaultPDA,
          role: "writable" as const,
        },
        {
          address: PROGRAM_ID,
          role: "readonly" as const,
        },
        {
          address: SYSTEM_PROGRAM_ADDRESS,
          role: "readonly" as const,
        },
      ],
      data: depositData,
      programAddress: PROGRAM_ID,
    };

    const { value: latestBlockhash } = await rpc.getLatestBlockhash().send();
    const tx = await pipe(
      createTransactionMessage({ version: 0 }),
      (tx) => setTransactionMessageFeePayer(signer.address, tx),
      (tx) => setTransactionMessageLifetimeUsingBlockhash(latestBlockhash, tx),
      (tx) => appendTransactionMessageInstruction(depositInstruction, tx),
    );

    // Sign and send transaction
    // / 签署并发送交易
    const signedTransaction = await signTransactionMessageWithSigners(tx);
    const sendAndConfirmTransaction = sendAndConfirmTransactionFactory({
      rpc,
      rpcSubscriptions,
    });

    await sendAndConfirmTransaction(signedTransaction, {
      commitment: "confirmed",
    });

    const signature = getSignatureFromTransaction(signedTransaction);
    console.log("Transaction signature:", signature);

    const { value } = await rpc.getBalance(vaultPDA).send();
    assert.equal(DEPOSIT_AMOUNT, Number(value) - Number(vaultRent));
  });

  it("can withdraw from vault", async () => {
    // 创建取款指令数据
    const withdrawData = new Uint8Array([1]); // 取款指令标识符

    const withdrawInstruction = {
      accounts: [
        {
          address: await signer.address,
          role: "writableSigner" as const,
        },
        {
          address: vaultPDA,
          role: "writable" as const,
        },
        {
          address: PROGRAM_ID,
          role: "readonly" as const,
        },
      ],
      data: withdrawData,
      programAddress: PROGRAM_ID,
    };

    const { value: latestBlockhash } = await rpc.getLatestBlockhash().send();
    const tx = await pipe(
      createTransactionMessage({ version: 0 }),
      (tx) => setTransactionMessageFeePayer(signer.address, tx),
      (tx) => setTransactionMessageLifetimeUsingBlockhash(latestBlockhash, tx),
      (tx) => appendTransactionMessageInstruction(withdrawInstruction, tx),
    );

    const signedTransaction = await signTransactionMessageWithSigners(tx);
    const sendAndConfirmTransaction = sendAndConfirmTransactionFactory({
      rpc,
      rpcSubscriptions,
    });

    await sendAndConfirmTransaction(signedTransaction, {
      commitment: "confirmed",
    });

    const signature = getSignatureFromTransaction(signedTransaction);
    console.log("Transaction signature:", signature);

    const { value } = await rpc.getBalance(vaultPDA).send();
    assert.equal(Number(vaultRent), value);
  });

  it("doesn't allow other users to withdraw from the vault", async () => {
    // signer that DOES NOT own the vault
    // / 不拥有 vault 的签名者
    const otherSigner = await generateKeyPairSigner();

    // 创建取款指令数据
    const withdrawData = new Uint8Array([1]); // 取款指令标识符

    const withdrawInstruction = {
      accounts: [
        {
          address: await otherSigner.address,
          role: "writableSigner" as const,
        },
        {
          address: vaultPDA,
          role: "writable" as const,
        },
        {
          address: PROGRAM_ID,
          role: "readonly" as const,
        },
      ],
      data: withdrawData,
      programAddress: PROGRAM_ID,
    };

    const { value: latestBlockhash } = await rpc.getLatestBlockhash().send();
    const tx = await pipe(
      createTransactionMessage({ version: 0 }),
      (tx) => setTransactionMessageFeePayer(otherSigner.address, tx),
      (tx) => setTransactionMessageLifetimeUsingBlockhash(latestBlockhash, tx),
      (tx) => appendTransactionMessageInstruction(withdrawInstruction, tx),
    );

    const signedTransaction = await signTransactionMessageWithSigners(tx);
    const sendAndConfirmTransaction = sendAndConfirmTransactionFactory({
      rpc,
      rpcSubscriptions,
    });

    await assert.rejects(
      sendAndConfirmTransaction(signedTransaction, {
        commitment: "confirmed",
      }),
      {
        message: "Transaction simulation failed",
      },
    );
  });
});
