const anchor = require("@coral-xyz/anchor");
const { SystemProgram, SYSVAR_CLOCK_PUBKEY } = anchor.web3;
const spl = require("@solana/spl-token");
const assert = require("assert");

describe("hoe-dex-protection", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const program = anchor.workspace.HoeDexProtection;
  const wallet = provider.wallet;
  let poolState, tokenMint, adminTokenAccount, poolTokenAccount, buyerTokenAccount;

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
    const snipeProtectionSeconds = 60;
    const earlyTradeFeeBps = 500;

    await program.methods
      .initializePoolProtection(
        new anchor.BN(snipeProtectionSeconds),
        new anchor.BN(earlyTradeFeeBps)
      )
      .accounts({
        poolState: poolState.publicKey,
        admin: wallet.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .signers([poolState])
      .rpc();

    const state = await program.account.poolState.fetch(poolState.publicKey);
    assert.equal(state.admin.toString(), wallet.publicKey.toString());
    assert.equal(state.snipeProtectionSeconds.toNumber(), snipeProtectionSeconds);
    assert.equal(state.earlyTradeFeeBps.toNumber(), earlyTradeFeeBps);
    assert.equal(state.poolStartTime.toNumber(), 0);
    assert.equal(state.totalFeesCollected.toNumber(), 0);
  });

  it("Adds liquidity and starts the pool", async () => {
    const amount = 100_000_000;

    await program.methods
      .initializePoolProtection(new anchor.BN(60), new anchor.BN(500))
      .accounts({
        poolState: poolState.publicKey,
        admin: wallet.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .signers([poolState])
      .rpc();

    await program.methods
      .addLiquidity(new anchor.BN(amount))
      .accounts({
        poolState: poolState.publicKey,
        admin: wallet.publicKey,
        adminTokenAccount: adminTokenAccount,
        poolTokenAccount: poolTokenAccount,
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