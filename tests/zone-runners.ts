import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { ZoneRunners } from "../target/types/zone_runners";
import {
  Keypair,
  PublicKey,
  SystemProgram,
  LAMPORTS_PER_SOL,
} from "@solana/web3.js";
import {
  createMint,
  getOrCreateAssociatedTokenAccount,
  mintTo,
  TOKEN_PROGRAM_ID,
} from "@solana/spl-token";
import { assert } from "chai";

const GUAGE_PROGRAM_ID = new PublicKey(
  "4Ch9vYQJyXtyZ7Swr9EMU9xaCtpZDckv4E1thjX7FZjW"
);

// Seeds — must match Rust constants
const ZONE_CONFIG_SEED = Buffer.from("zone-config");
const SEASON_SEED = Buffer.from("season");
const ZONE_CLAIM_SEED = Buffer.from("zone-claim");
const OP_VAULT_SEED = Buffer.from("op-vault");
const DELEGATION_SEED = Buffer.from("delegation");
const PASSPORT_SEED = Buffer.from("passport");
const SEASON_TOKEN_VAULT_SEED = Buffer.from("season-token-vault");
const OP_TOKEN_VAULT_SEED = Buffer.from("op-token-vault");

function clubIdBuffer(id: number): Buffer {
  const buf = Buffer.alloc(8);
  buf.writeBigUInt64LE(BigInt(id));
  return buf;
}

function h3IndexBuffer(index: bigint): Buffer {
  const buf = Buffer.alloc(8);
  buf.writeBigUInt64BE(index);
  return buf;
}

function seasonIndexBuffer(index: number): Buffer {
  const buf = Buffer.alloc(4);
  buf.writeUInt32BE(index);
  return buf;
}

describe("zone-runners", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.ZoneRunners as Program<ZoneRunners>;
  const connection = provider.connection;

  let admin: Keypair;
  let operator: Keypair;
  let delegator: Keypair;
  let zoneMint: PublicKey;
  let clubId = 6; // Zone Runners club_id

  let zoneConfigPda: PublicKey;
  let seasonPda: PublicKey;
  let zoneClaimPda: PublicKey;
  let operatorVaultPda: PublicKey;
  let delegationPda: PublicKey;
  let operatorPassportPda: PublicKey;
  let delegatorPassportPda: PublicKey;
  let seasonVaultPda: PublicKey;
  let operatorTokenVaultPda: PublicKey;

  const H3_INDEX = BigInt("617700169958293503"); // sample H3 res-7 cell

  before(async () => {
    admin = Keypair.generate();
    operator = Keypair.generate();
    delegator = Keypair.generate();

    // Airdrop
    for (const kp of [admin, operator, delegator]) {
      const sig = await connection.requestAirdrop(
        kp.publicKey,
        2 * LAMPORTS_PER_SOL
      );
      await connection.confirmTransaction(sig);
    }

    // Create $ZONE mint
    zoneMint = await createMint(connection, admin, admin.publicKey, null, 6);

    // Derive PDAs
    [zoneConfigPda] = PublicKey.findProgramAddressSync(
      [ZONE_CONFIG_SEED, clubIdBuffer(clubId)],
      program.programId
    );

    [operatorPassportPda] = PublicKey.findProgramAddressSync(
      [PASSPORT_SEED, operator.publicKey.toBuffer()],
      program.programId
    );

    [delegatorPassportPda] = PublicKey.findProgramAddressSync(
      [PASSPORT_SEED, delegator.publicKey.toBuffer()],
      program.programId
    );
  });

  it("initializes zone config", async () => {
    await program.methods
      .initializeZoneConfig(new anchor.BN(clubId), GUAGE_PROGRAM_ID)
      .accounts({
        admin: admin.publicKey,
        zoneConfig: zoneConfigPda,
        zoneTokenMint: zoneMint,
        systemProgram: SystemProgram.programId,
      })
      .signers([admin])
      .rpc();

    const cfg = await program.account.zoneConfig.fetch(zoneConfigPda);
    assert.equal(cfg.admin.toBase58(), admin.publicKey.toBase58());
    assert.equal(cfg.guageProgramId.toBase58(), GUAGE_PROGRAM_ID.toBase58());
    assert.equal(cfg.seasonCount, 0);
  });

  it("creates a season", async () => {
    const now = Math.floor(Date.now() / 1000);
    const start = now - 60; // started 1 min ago
    const end = now + 30 * 24 * 3600; // ends in 30 days

    [seasonPda] = PublicKey.findProgramAddressSync(
      [SEASON_SEED, zoneConfigPda.toBuffer(), seasonIndexBuffer(0)],
      program.programId
    );

    await program.methods
      .createSeason("helium", 7, new anchor.BN(start), new anchor.BN(end))
      .accounts({
        admin: admin.publicKey,
        zoneConfig: zoneConfigPda,
        season: seasonPda,
        systemProgram: SystemProgram.programId,
      })
      .signers([admin])
      .rpc();

    const season = await program.account.season.fetch(seasonPda);
    assert.equal(season.networkName, "helium");
    assert.equal(season.h3Resolution, 7);
    assert.equal(season.isSettled, false);
    assert.equal(season.zonesVerified, 0);

    const cfg = await program.account.zoneConfig.fetch(zoneConfigPda);
    assert.equal(cfg.seasonCount, 1);
  });

  it("funds the season pool", async () => {
    const adminTokenAccount = await getOrCreateAssociatedTokenAccount(
      connection,
      admin,
      zoneMint,
      admin.publicKey
    );

    await mintTo(
      connection,
      admin,
      zoneMint,
      adminTokenAccount.address,
      admin,
      1_000_000 * 1_000_000 // 1M ZONE
    );

    [seasonVaultPda] = PublicKey.findProgramAddressSync(
      [SEASON_TOKEN_VAULT_SEED, seasonPda.toBuffer()],
      program.programId
    );

    await program.methods
      .fundSeasonPool(new anchor.BN(100_000 * 1_000_000)) // 100k ZONE
      .accounts({
        funder: admin.publicKey,
        season: seasonPda,
        zoneTokenMint: zoneMint,
        funderTokenAccount: adminTokenAccount.address,
        seasonTokenVault: seasonVaultPda,
        tokenProgram: TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
        rent: anchor.web3.SYSVAR_RENT_PUBKEY,
      })
      .signers([admin])
      .rpc();

    const season = await program.account.season.fetch(seasonPda);
    assert.equal(season.rewardPool.toNumber(), 100_000 * 1_000_000);
  });

  it("operator claims a zone", async () => {
    const facilityKey = Keypair.generate().publicKey; // mock facility pubkey

    [zoneClaimPda] = PublicKey.findProgramAddressSync(
      [ZONE_CLAIM_SEED, seasonPda.toBuffer(), h3IndexBuffer(H3_INDEX)],
      program.programId
    );

    [operatorVaultPda] = PublicKey.findProgramAddressSync(
      [OP_VAULT_SEED, seasonPda.toBuffer(), operator.publicKey.toBuffer()],
      program.programId
    );

    await program.methods
      .claimZone(new anchor.BN(H3_INDEX.toString()), facilityKey)
      .accounts({
        operator: operator.publicKey,
        zoneConfig: zoneConfigPda,
        season: seasonPda,
        zoneClaim: zoneClaimPda,
        operatorVault: operatorVaultPda,
        passport: operatorPassportPda,
        systemProgram: SystemProgram.programId,
      })
      .signers([operator])
      .rpc();

    const claim = await program.account.zoneClaim.fetch(zoneClaimPda);
    assert.equal(claim.h3Index.toString(), H3_INDEX.toString());
    assert.equal(claim.operator.toBase58(), operator.publicKey.toBase58());
    assert.equal(claim.isVerified, false);

    const passport = await program.account.contributionPassport.fetch(
      operatorPassportPda
    );
    assert.equal(passport.zonesClaimedTotal, 1);
  });

  it("delegates stake to operator", async () => {
    const delegatorTokenAccount = await getOrCreateAssociatedTokenAccount(
      connection,
      delegator,
      zoneMint,
      delegator.publicKey
    );

    await mintTo(
      connection,
      admin,
      zoneMint,
      delegatorTokenAccount.address,
      admin,
      50_000 * 1_000_000
    );

    [operatorTokenVaultPda] = PublicKey.findProgramAddressSync(
      [OP_TOKEN_VAULT_SEED, seasonPda.toBuffer(), operator.publicKey.toBuffer()],
      program.programId
    );

    [delegationPda] = PublicKey.findProgramAddressSync(
      [
        DELEGATION_SEED,
        seasonPda.toBuffer(),
        operator.publicKey.toBuffer(),
        delegator.publicKey.toBuffer(),
      ],
      program.programId
    );

    await program.methods
      .delegateStake(new anchor.BN(10_000 * 1_000_000))
      .accounts({
        delegator: delegator.publicKey,
        operator: operator.publicKey,
        season: seasonPda,
        delegationStake: delegationPda,
        operatorVault: operatorVaultPda,
        passport: delegatorPassportPda,
        zoneTokenMint: zoneMint,
        delegatorTokenAccount: delegatorTokenAccount.address,
        operatorTokenVault: operatorTokenVaultPda,
        tokenProgram: TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
        rent: anchor.web3.SYSVAR_RENT_PUBKEY,
      })
      .signers([delegator])
      .rpc();

    const stake = await program.account.delegationStake.fetch(delegationPda);
    assert.equal(stake.amount.toNumber(), 10_000 * 1_000_000);
    assert.equal(stake.isActive, true);

    const passport = await program.account.contributionPassport.fetch(
      delegatorPassportPda
    );
    assert.equal(passport.totalDelegatedEver.toNumber(), 10_000 * 1_000_000);
    assert.equal(passport.currentTier, 1); // Runner (>= 5000 ZONE)
  });

  it("updates passport tier", async () => {
    await program.methods
      .updatePassport()
      .accounts({
        payer: admin.publicKey,
        authority: operator.publicKey,
        passport: operatorPassportPda,
        systemProgram: SystemProgram.programId,
      })
      .signers([admin])
      .rpc();

    const passport = await program.account.contributionPassport.fetch(
      operatorPassportPda
    );
    assert.ok(passport.lastUpdated.toNumber() > 0);
  });

  // verify_zone_coverage requires a live guage-commons SnapshotBuffer on devnet
  // so it is tested manually. The instruction logic is unit-tested in the program.
  it("verifies zone coverage (skipped on localnet — requires live SnapshotBuffer)", async () => {
    // To test on devnet:
    // 1. Deploy guage-commons to devnet (already at 4Ch9vY...)
    // 2. Register a Facility + publish a Snapshot
    // 3. Call verify_zone_coverage with the SnapshotBuffer pubkey
    console.log("  → skipped: run on devnet with live guage-commons data");
  });
});
