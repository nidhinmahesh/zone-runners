import { PublicKey } from "@solana/web3.js";
import BN from "bn.js";

export interface ZoneConfigAccount {
  admin: PublicKey;
  clubId: number[];
  guageProgramId: PublicKey;
  zoneTokenMint: PublicKey;
  seasonCount: number;
  bump: number;
}

export interface SeasonAccount {
  zoneConfig: PublicKey;
  seasonIndex: number;
  networkName: string;
  h3Resolution: number;
  startTs: BN;
  endTs: BN;
  rewardPool: BN;
  zonesClaimed: number;
  zonesVerified: number;
  totalDelegated: BN;
  isSettled: boolean;
  bump: number;
}

export interface ZoneClaimAccount {
  season: PublicKey;
  h3Index: BN;
  operator: PublicKey;
  facility: PublicKey;
  claimedAt: BN;
  isVerified: boolean;
  verifiedAt: BN;
  snapshotBuffer: PublicKey;
  bump: number;
}

export interface OperatorVaultAccount {
  season: PublicKey;
  operator: PublicKey;
  totalDelegated: BN;
  zonesClaimed: number;
  zonesVerified: number;
  rewardsDistributed: BN;
  bump: number;
}

export interface DelegationStakeAccount {
  season: PublicKey;
  operator: PublicKey;
  delegator: PublicKey;
  amount: BN;
  delegatedAt: BN;
  isActive: boolean;
  rewardsClaimed: BN;
  bump: number;
}

export interface ContributionPassportAccount {
  authority: PublicKey;
  zonesClaimedTotal: number;
  zonesVerifiedTotal: number;
  seasonsParticipated: number;
  totalDelegatedEver: BN;
  delegationCount: number;
  currentTier: number;
  lastUpdated: BN;
  bump: number;
}

export const TIER_LABELS = ["Scout", "Runner", "Zone Lead", "Pioneer"] as const;
export type TierLabel = (typeof TIER_LABELS)[number];

export function tierLabel(tier: number): TierLabel {
  return TIER_LABELS[Math.min(tier, 3)] ?? "Scout";
}

export const ZONE_RUNNERS_PROGRAM_ID = "ZRuNrAtgJM4hG3YLtq6NmbHdtBqoNGbvRvbyLuLNoWf";
export const GUAGE_PROGRAM_ID = "4Ch9vYQJyXtyZ7Swr9EMU9xaCtpZDckv4E1thjX7FZjW";
