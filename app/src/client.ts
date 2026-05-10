import * as anchor from "@coral-xyz/anchor";
import { Program, AnchorProvider, BN } from "@coral-xyz/anchor";
import {
  Connection,
  Keypair,
  PublicKey,
  Transaction,
  SystemProgram,
} from "@solana/web3.js";
import {
  getAssociatedTokenAddress,
  TOKEN_PROGRAM_ID,
  ASSOCIATED_TOKEN_PROGRAM_ID,
} from "@solana/spl-token";
import {
  ContributionPassportAccount,
  DelegationStakeAccount,
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
const DELEGATION_SEED = Buffer.from("delegation");
const PASSPORT_SEED = Buffer.from("passport");
const SEASON_TOKEN_VAULT_SEED = Buffer.from("season-token-vault");
const OP_TOKEN_VAULT_SEED = Buffer.from("op-token-vault");

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

  delegationPda(
    season: PublicKey,
    operator: PublicKey,
    delegator: PublicKey
  ): [PublicKey, number] {
    return PublicKey.findProgramAddressSync(
      [DELEGATION_SEED, season.toBuffer(), operator.toBuffer(), delegator.toBuffer()],
      this.programId
    );
  }

  passportPda(wallet: PublicKey): [PublicKey, number] {
    return PublicKey.findProgramAddressSync(
      [PASSPORT_SEED, wallet.toBuffer()],
      this.programId
    );
  }

  seasonTokenVaultPda(season: PublicKey): [PublicKey, number] {
    return PublicKey.findProgramAddressSync(
      [SEASON_TOKEN_VAULT_SEED, season.toBuffer()],
      this.programId
    );
  }

  operatorTokenVaultPda(season: PublicKey, operator: PublicKey): [PublicKey, number] {
    return PublicKey.findProgramAddressSync(
      [OP_TOKEN_VAULT_SEED, season.toBuffer(), operator.toBuffer()],
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
      {
        memcmp: {
          offset: 8, // skip discriminator
          bytes: seasonPda.toBase58(),
        },
      },
    ]);
    return accounts.map((a) => a.account as ZoneClaimAccount);
  }

  async getDelegationStakes(
    seasonPda: PublicKey,
    operator?: PublicKey
  ): Promise<DelegationStakeAccount[]> {
    const filters: anchor.web3.GetProgramAccountsFilter[] = [
      { memcmp: { offset: 8, bytes: seasonPda.toBase58() } },
    ];
    if (operator) {
      filters.push({ memcmp: { offset: 8 + 32, bytes: operator.toBase58() } });
    }
    const accounts = await this.program.account.delegationStake.all(filters);
    return accounts.map((a) => a.account as DelegationStakeAccount);
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

  // ── Transaction builders (return base64-encoded unsigned transactions) ─────

  async buildClaimZoneTx(
    operator: PublicKey,
    seasonPda: PublicKey,
    h3Index: bigint,
    facility: PublicKey,
    clubId: number
  ): Promise<string> {
    const [zoneConfigPda] = this.zoneConfigPda(clubId);
    const season = await this.getSeason(seasonPda);
    if (!season) throw new Error("Season not found");

    const [zoneClaimPda] = this.zoneClaimPda(seasonPda, h3Index);
    const [operatorVaultPda] = this.operatorVaultPda(seasonPda, operator);
    const [passportPda] = this.passportPda(operator);

    const ix = await this.program.methods
      .claimZone(new BN(h3Index.toString()), facility)
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

    const tx = new Transaction().add(ix);
    tx.feePayer = operator;
    const { blockhash } = await this.program.provider.connection.getLatestBlockhash();
    tx.recentBlockhash = blockhash;

    return tx.serialize({ requireAllSignatures: false }).toString("base64");
  }

  async buildDelegateStakeTx(
    delegator: PublicKey,
    operator: PublicKey,
    seasonPda: PublicKey,
    amount: bigint,
    zoneTokenMint: PublicKey
  ): Promise<string> {
    const season = await this.getSeason(seasonPda);
    if (!season) throw new Error("Season not found");

    const [delegationPda] = this.delegationPda(seasonPda, operator, delegator);
    const [operatorVaultPda] = this.operatorVaultPda(seasonPda, operator);
    const [passportPda] = this.passportPda(delegator);
    const [operatorTokenVaultPda] = this.operatorTokenVaultPda(seasonPda, operator);
    const delegatorTokenAccount = await getAssociatedTokenAddress(zoneTokenMint, delegator);

    const ix = await this.program.methods
      .delegateStake(new BN(amount.toString()))
      .accounts({
        delegator,
        operator,
        season: seasonPda,
        delegationStake: delegationPda,
        operatorVault: operatorVaultPda,
        passport: passportPda,
        zoneTokenMint,
        delegatorTokenAccount,
        operatorTokenVault: operatorTokenVaultPda,
        tokenProgram: TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
        rent: anchor.web3.SYSVAR_RENT_PUBKEY,
      })
      .instruction();

    const tx = new Transaction().add(ix);
    tx.feePayer = delegator;
    const { blockhash } = await this.program.provider.connection.getLatestBlockhash();
    tx.recentBlockhash = blockhash;

    return tx.serialize({ requireAllSignatures: false }).toString("base64");
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
      .verifyZoneCoverage(
        new BN(h3Index.toString()),
        minEntries,
        new BN(minQualityFlags.toString())
      )
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

    const tx = new Transaction().add(ix);
    tx.feePayer = operator;
    const { blockhash } = await this.program.provider.connection.getLatestBlockhash();
    tx.recentBlockhash = blockhash;

    return tx.serialize({ requireAllSignatures: false }).toString("base64");
  }
}
