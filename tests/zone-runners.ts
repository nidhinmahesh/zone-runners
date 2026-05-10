import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { ZoneRunners } from "../target/types/zone_runners";
import {
  Keypair,
  PublicKey,
  SystemProgram,
  LAMPORTS_PER_SOL,
} from "@solana/web3.js";
import { assert } from "chai";

const ORACLE_PROGRAM_ID = new PublicKey(
  "4Ch9vYQJyXtyZ7Swr9EMU9xaCtpZDckv4E1thjX7FZjW"
);

const ZONE_CONFIG_SEED = Buffer.from("zone-config");
const SEASON_SEED = Buffer.from("season");
const ZONE_CLAIM_SEED = Buffer.from("zone-claim");
const OP_VAULT_SEED = Buffer.from("op-vault");
const PASSPORT_SEED = Buffer.from("passport");

const MIN_ZONE_STAKE = 10_000_000; // 0.01 SOL in lamports

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
  let challenger: Keypair;
  const clubId = 6;

  let zoneConfigPda: PublicKey;
  let seasonPda: PublicKey;
  let zoneClaimPda: PublicKey;
  let operatorVaultPda: PublicKey;
  let operatorPassportPda: PublicKey;
  let challengerPassportPda: PublicKey;

  const H3_INDEX = BigInt("617700169958293503");

  before(async () => {
    admin = Keypair.generate();
    operator = Keypair.generate();
    challenger = Keypair.generate();

    for (const kp of [admin, operator, challenger]) {
      const sig = await connection.requestAirdrop(kp.publicKey, 2 * LAMPORTS_PER_SOL);
      await connection.confirmTransaction(sig);
    }

    [zoneConfigPda] = PublicKey.findProgramAddressSync(
      [ZONE_CONFIG_SEED, clubIdBuffer(clubId)],
      program.programId
    );

    [operatorPassportPda] = PublicKey.findProgramAddressSync(
      [PASSPORT_SEED, operator.publicKey.toBuffer()],
      program.programId
    );

    [challengerPassportPda] = PublicKey.findProgramAddressSync(
      [PASSPORT_SEED, challenger.publicKey.toBuffer()],
      program.programId
    );
  });

  it("initializes zone config", async () => {
    await program.methods
      .initializeZoneConfig(new anchor.BN(clubId), ORACLE_PROGRAM_ID)
      .accounts({
        admin: admin.publicKey,
        zoneConfig: zoneConfigPda,
        systemProgram: SystemProgram.programId,
      })
      .signers([admin])
      .rpc();

    const cfg = await program.account.zoneConfig.fetch(zoneConfigPda);
    assert.equal(cfg.admin.toBase58(), admin.publicKey.toBase58());
    assert.equal(cfg.oracleProgramId.toBase58(), ORACLE_PROGRAM_ID.toBase58());
    assert.equal(cfg.seasonCount, 0);
  });

  it("creates a season", async () => {
    const now = Math.floor(Date.now() / 1000);

    [seasonPda] = PublicKey.findProgramAddressSync(
      [SEASON_SEED, zoneConfigPda.toBuffer(), seasonIndexBuffer(0)],
      program.programId
    );

    await program.methods
      .createSeason("helium", 7, new anchor.BN(now - 60), new anchor.BN(now + 30 * 86400))
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
    assert.equal(season.isSettled, false);
    assert.equal(season.zonesVerified, 0);
  });

  it("funds the season pool with SOL", async () => {
    const fundAmount = new anchor.BN(0.5 * LAMPORTS_PER_SOL);

    await program.methods
      .fundSeasonPool(fundAmount)
      .accounts({
        funder: admin.publicKey,
        season: seasonPda,
        systemProgram: SystemProgram.programId,
      })
      .signers([admin])
      .rpc();

    const season = await program.account.season.fetch(seasonPda);
    assert.equal(season.bountyPool.toNumber(), fundAmount.toNumber());
  });

  it("operator claims a zone with SOL stake", async () => {
    const facilityKey = Keypair.generate().publicKey;

    [zoneClaimPda] = PublicKey.findProgramAddressSync(
      [ZONE_CLAIM_SEED, seasonPda.toBuffer(), h3IndexBuffer(H3_INDEX)],
      program.programId
    );

    [operatorVaultPda] = PublicKey.findProgramAddressSync(
      [OP_VAULT_SEED, seasonPda.toBuffer(), operator.publicKey.toBuffer()],
      program.programId
    );

    await program.methods
      .claimZone(
        new anchor.BN(H3_INDEX.toString()),
        facilityKey,
        new anchor.BN(MIN_ZONE_STAKE)
      )
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
    assert.equal(claim.stakeLamports.toNumber(), MIN_ZONE_STAKE);
    assert.equal(claim.coverageScore, 0);
    assert.equal(claim.challengeCount, 0);

    const passport = await program.account.contributionPassport.fetch(operatorPassportPda);
    assert.equal(passport.zonesClaimedTotal, 1);
  });

  it("rejects a claim below minimum stake", async () => {
    const facilityKey = Keypair.generate().publicKey;
    const otherH3 = BigInt("617700169958293504");

    const [otherClaimPda] = PublicKey.findProgramAddressSync(
      [ZONE_CLAIM_SEED, seasonPda.toBuffer(), h3IndexBuffer(otherH3)],
      program.programId
    );

    try {
      await program.methods
        .claimZone(new anchor.BN(otherH3.toString()), facilityKey, new anchor.BN(1000))
        .accounts({
          operator: operator.publicKey,
          zoneConfig: zoneConfigPda,
          season: seasonPda,
          zoneClaim: otherClaimPda,
          operatorVault: operatorVaultPda,
          passport: operatorPassportPda,
          systemProgram: SystemProgram.programId,
        })
        .signers([operator])
        .rpc();
      assert.fail("Should have thrown StakeBelowMinimum");
    } catch (e: any) {
      assert.include(e.toString(), "StakeBelowMinimum");
    }
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

    const passport = await program.account.contributionPassport.fetch(operatorPassportPda);
    assert.ok(passport.lastUpdated.toNumber() > 0);
  });

  // verify_zone_coverage and challenge_zone both require a live DePIN oracle
  // SnapshotBuffer on devnet. Run these manually after deployment.
  it("verifies zone coverage (skipped on localnet — requires live SnapshotBuffer)", async () => {
    console.log("  → skipped: deploy to devnet and run with live DePIN oracle data");
  });

  it("challenge_zone (skipped on localnet — requires verified zone with live data)", async () => {
    console.log("  → skipped: zone must be verified before it can be challenged");
  });
});
