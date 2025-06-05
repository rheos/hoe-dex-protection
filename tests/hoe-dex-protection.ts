import * as anchor from "@coral-xyz/anchor";
import { Program, BN } from "@coral-xyz/anchor";
import { PublicKey, SystemProgram, SYSVAR_CLOCK_PUBKEY } from "@solana/web3.js";
import * as spl from "@solana/spl-token";
import { assert } from "chai";
import { HoeDexProtection } from "../target/types/hoe_dex_protection";

describe("hoe-dex-protection", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const program = anchor.workspace.HoeDexProtection as Program<HoeDexProtection>;
  const wallet = provider.wallet as anchor.Wallet;
  let poolState: anchor.web3.Keypair;
  let tokenMint: PublicKey;
  let adminTokenAccount: PublicKey;
  let poolTokenAccount: PublicKey;
  let buyerTokenAccount: PublicKey;

  beforeEach(async () => {
    tokenMint = await spl.createMint(
      provider.connection,
      wallet.payer,
      wallet.publicKey,
      null,
      9
    );

    adminTokenAccount = await spl.createAccount(
      provider.connection,
      wallet.payer,
      tokenMint,
      wallet.publicKey
    );
    poolTokenAccount = await spl.createAccount(
      provider.connection,
      wallet.payer,
      tokenMint,
      wallet.publicKey
    );
    buyerTokenAccount = await spl.createAccount(
      provider.connection,
      wallet.payer,
      tokenMint,
      wallet.publicKey
    );

    await spl.mintTo(
      provider.connection,
      wallet.payer,
      tokenMint,
      adminTokenAccount,
      wallet.payer,
      1_000_000_000
    );

    poolState = anchor.web3.Keypair.generate();
  });

  it("Initializes the pool protection", async () => {
    const snipeProtectionSeconds = new BN(60);
    const earlyTradeFeeBps = new BN(500);

    await program.methods
      .initializePoolProtection(snipeProtectionSeconds, earlyTradeFeeBps)
      .accounts({
        poolState: poolState.publicKey,
        admin: wallet.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .signers([poolState])
      .rpc();

    const state = await program.account.poolState.fetch(poolState.publicKey);
    assert.equal(state.admin.toString(), wallet.publicKey.toString());
    assert.equal(state.snipeProtectionSeconds.toNumber(), snipeProtectionSeconds.toNumber());
    assert.equal(state.earlyTradeFeeBps.toNumber(), earlyTradeFeeBps.toNumber());
    assert.equal(state.poolStartTime.toNumber(), 0);
    assert.equal(state.totalFeesCollected.toNumber(), 0);
  });

  it("Adds liquidity and starts the pool", async () => {
    const amount = new BN(100_000_000);

    await program.methods
      .initializePoolProtection(new BN(60), new BN(500))
      .accounts({
        poolState: poolState.publicKey,
        admin: wallet.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .signers([poolState])
      .rpc();

    await program.methods
      .addLiquidity(amount)
      .accounts({
        poolState: poolState.publicKey,
        admin: wallet.publicKey,
        adminTokenAccount,
        poolTokenAccount,
        tokenProgram: spl.TOKEN_PROGRAM_ID,
        clock: SYSVAR_CLOCK_PUBKEY,
      })
      .rpc();

    const state = await program.account.poolState.fetch(poolState.publicKey);
    const poolTokenAcc = await spl.getAccount(provider.connection, poolTokenAccount);
    assert.notEqual(state.poolStartTime.toNumber(), 0);
    assert.equal(poolTokenAcc.amount.toString(), amount.toString());
  });
});