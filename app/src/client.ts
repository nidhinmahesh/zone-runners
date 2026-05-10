import * as anchor from "@coral-xyz/anchor";
import { Program, AnchorProvider, BN } from "@coral-xyz/anchor";
import {
  Connection,
  PublicKey,
  Transaction,
  SystemProgram,
} from "@solana/web3.js";
import {
  ContributionPassportAccount,
  OperatorVaultAccount,
  SeasonAccount,
  ZoneClaimAccount,
  ZoneConfigAccount,
  GUAGE_PROGRAM_ID,
  ZONE_RUNNERS_PROGRAM_ID,
} from "./types";

const ZONE_CONFIG_SEED = Buffer.from("zone-config");
const SEASON_SEED = Buffer.from("season");
const ZONE_CLAIM_SEED = Buffer.from("zone-claim");
const OP_VAULT_SEED = Buffer.from("op-vault");
const PASSPORT_SEED = Buffer.from("passport");

function clubIdBuf(id: number): Buffer {
  const b = Buffer.alloc(8);
  b.writeBigUInt64LE(BigInt(id));
  return b;
}

function seasonIndexBuf(i: number): Buffer {
  const b = Buffer.alloc(4);
  b.writeUInt32BE(i);
  return b;
}

function h3IndexBuf(h3: bigint): Buffer {
  const b = Buffer.alloc(8);
  b.writeBigUInt64BE(h3);
  return b;
}

export class ZoneRunnersClient {
  readonly program: Program;
  readonly programId: PublicKey;

  constructor(provider: AnchorProvider, idl: anchor.Idl) {
    this.programId = new PublicKey(ZONE_RUNNERS_PROGRAM_ID);
    this.program = new Program(idl, this.programId, provider);
  }

  // ── PDA helpers ────────────────────────────────────────────────────────────

  zoneConfigPda(clubId: number): [PublicKey, number] {
    return PublicKey.findProgramAddressSync(
      [ZONE_CONFIG_SEED, clubIdBuf(clubId)],
      this.programId
    );
  }

  seasonPda(zoneConfig: PublicKey, seasonIndex: number): [PublicKey, number] {
    return PublicKey.findProgramAddressSync(
      [SEASON_SEED, zoneConfig.toBuffer(), seasonIndexBuf(seasonIndex)],
      this.programId
    );
  }

  zoneClaimPda(season: PublicKey, h3Index: bigint): [PublicKey, number] {
    return PublicKey.findProgramAddressSync(
      [ZONE_CLAIM_SEED, season.toBuffer(), h3IndexBuf(h3Index)],
      this.programId
    );
  }

  operatorVaultPda(season: PublicKey, operator: PublicKey): [PublicKey, number] {
    return PublicKey.findProgramAddressSync(
      [OP_VAULT_SEED, season.toBuffer(), operator.toBuffer()],
      this.programId
    );
  }

  passportPda(wallet: PublicKey): [PublicKey, number] {
    return PublicKey.findProgramAddressSync(
      [PASSPORT_SEED, wallet.toBuffer()],
      this.programId
    );
  }

  // ── Reads ──────────────────────────────────────────────────────────────────

  async getZoneConfig(clubId: number): Promise<ZoneConfigAccount | null> {
    const [pda] = this.zoneConfigPda(clubId);
    try {
      return (await this.program.account.zoneConfig.fetch(pda)) as ZoneConfigAccount;
    } catch {
      return null;
    }
  }

  async getSeason(seasonPda: PublicKey): Promise<SeasonAccount | null> {
    try {
      return (await this.program.account.season.fetch(seasonPda)) as SeasonAccount;
    } catch {
      return null;
    }
  }

  async getZoneClaims(seasonPda: PublicKey): Promise<ZoneClaimAccount[]> {
    const accounts = await this.program.account.zoneClaim.all([
      { memcmp: { offset: 8, bytes: seasonPda.toBase58() } },
    ]);
    return accounts.map((a) => a.account as ZoneClaimAccount);
  }

  async getPassport(wallet: PublicKey): Promise<ContributionPassportAccount | null> {
    const [pda] = this.passportPda(wallet);
    try {
      return (await this.program.account.contributionPassport.fetch(pda)) as ContributionPassportAccount;
    } catch {
      return null;
    }
  }

  async getOperatorVault(
    seasonPda: PublicKey,
    operator: PublicKey
  ): Promise<OperatorVaultAccount | null> {
    const [pda] = this.operatorVaultPda(seasonPda, operator);
    try {
      return (await this.program.account.operatorVault.fetch(pda)) as OperatorVaultAccount;
    } catch {
      return null;
    }
  }

  // ── Transaction builders ───────────────────────────────────────────────────

  async buildClaimZoneTx(
    operator: PublicKey,
    seasonPda: PublicKey,
    h3Index: bigint,
    facility: PublicKey,
    clubId: number,
    stakeLamports: bigint = BigInt(10_000_000)
  ): Promise<string> {
    const [zoneConfigPda] = this.zoneConfigPda(clubId);
    const [zoneClaimPda] = this.zoneClaimPda(seasonPda, h3Index);
    const [operatorVaultPda] = this.operatorVaultPda(seasonPda, operator);
    const [passportPda] = this.passportPda(operator);

    const ix = await this.program.methods
      .claimZone(new BN(h3Index.toString()), facility, new BN(stakeLamports.toString()))
      .accounts({
        operator,
        zoneConfig: zoneConfigPda,
        season: seasonPda,
        zoneClaim: zoneClaimPda,
        operatorVault: operatorVaultPda,
        passport: passportPda,
        systemProgram: SystemProgram.programId,
      })
      .instruction();

    return this._buildTx(operator, ix);
  }

  async buildVerifyCoverageTx(
    operator: PublicKey,
    seasonPda: PublicKey,
    h3Index: bigint,
    snapshotBuffer: PublicKey,
    clubId: number,
    minEntries: number = 3,
    minQualityFlags: bigint = BigInt(1)
  ): Promise<string> {
    const [zoneConfigPda] = this.zoneConfigPda(clubId);
    const [zoneClaimPda] = this.zoneClaimPda(seasonPda, h3Index);
    const [operatorVaultPda] = this.operatorVaultPda(seasonPda, operator);
    const [passportPda] = this.passportPda(operator);

    const ix = await this.program.methods
      .verifyZoneCoverage(new BN(h3Index.toString()), minEntries, new BN(minQualityFlags.toString()))
      .accounts({
        operator,
        zoneConfig: zoneConfigPda,
        season: seasonPda,
        zoneClaim: zoneClaimPda,
        operatorVault: operatorVaultPda,
        passport: passportPda,
        snapshotBuffer,
      })
      .instruction();

    return this._buildTx(operator, ix);
  }

  async buildChallengeZoneTx(
    challenger: PublicKey,
    seasonPda: PublicKey,
    h3Index: bigint,
    facility: PublicKey,
    snapshotBuffer: PublicKey,
    clubId: number,
    adminPubkey: PublicKey,
    minEntries: number = 3,
    minQualityFlags: bigint = BigInt(1)
  ): Promise<string> {
    const [zoneConfigPda] = this.zoneConfigPda(clubId);
    const [zoneClaimPda] = this.zoneClaimPda(seasonPda, h3Index);
    const [passportPda] = this.passportPda(challenger);

    // Fetch the current zone claim to get the operator (defender) pubkey
    const claim = await this.program.account.zoneClaim.fetch(zoneClaimPda) as ZoneClaimAccount;
    const defender = new PublicKey((claim as any).operator);

    const ix = await this.program.methods
      .challengeZone(new BN(h3Index.toString()), facility, minEntries, new BN(minQualityFlags.toString()))
      .accounts({
        challenger,
        zoneConfig: zoneConfigPda,
        season: seasonPda,
        zoneClaim: zoneClaimPda,
        operator: defender,
        admin: adminPubkey,
        challengerPassport: passportPda,
        snapshotBuffer,
        systemProgram: SystemProgram.programId,
      })
      .instruction();

    return this._buildTx(challenger, ix);
  }

  async buildWithdrawZoneStakeTx(
    operator: PublicKey,
    seasonPda: PublicKey,
    h3Index: bigint
  ): Promise<string> {
    const [zoneClaimPda] = this.zoneClaimPda(seasonPda, h3Index);

    const ix = await this.program.methods
      .withdrawZoneStake(new BN(h3Index.toString()))
      .accounts({
        operator,
        season: seasonPda,
        zoneClaim: zoneClaimPda,
      })
      .instruction();

    return this._buildTx(operator, ix);
  }

  private async _buildTx(feePayer: PublicKey, ix: anchor.web3.TransactionInstruction): Promise<string> {
    const tx = new Transaction().add(ix);
    tx.feePayer = feePayer;
    const { blockhash } = await this.program.provider.connection.getLatestBlockhash();
    tx.recentBlockhash = blockhash;
    return tx.serialize({ requireAllSignatures: false }).toString("base64");
  }
}
