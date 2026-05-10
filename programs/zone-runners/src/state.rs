use anchor_lang::prelude::*;

// ─── constants ───────────────────────────────────────────────────────────────

pub const MAX_NETWORK_NAME_LEN: usize = 32;
pub const ZONE_CONFIG_SEED: &[u8] = b"zone-config";
pub const SEASON_SEED: &[u8] = b"season";
pub const ZONE_CLAIM_SEED: &[u8] = b"zone-claim";
pub const OP_VAULT_SEED: &[u8] = b"op-vault";
pub const PASSPORT_SEED: &[u8] = b"passport";

pub const RUNNER_MIN_ZONES: u32 = 1;
pub const ZONE_LEAD_MIN_ZONES: u32 = 5;
pub const ZONE_LEAD_MIN_SEASONS: u32 = 2;
pub const PIONEER_MIN_ZONES: u32 = 20;

// ─── ZoneConfig ──────────────────────────────────────────────────────────────
// PDA seeds: [ZONE_CONFIG_SEED, club_id]

#[account]
pub struct ZoneConfig {
    pub admin: Pubkey,
    /// 8-byte little-endian encoding of fexrapi club_id
    pub club_id: [u8; 8],
    /// guage-commons program ID (4Ch9vYQJyXtyZ7Swr9EMU9xaCtpZDckv4E1thjX7FZjW)
    pub guage_program_id: Pubkey,
    pub season_count: u32,
    pub bump: u8,
}

impl ZoneConfig {
    pub const MAX_SIZE: usize = 32 + 8 + 32 + 4 + 1 + 8;
}

// ─── Season ───────────────────────────────────────────────────────────────────
// PDA seeds: [SEASON_SEED, zone_config, season_index_be(4)]
// The Season PDA holds the SOL bounty pool as its lamports.

#[account]
pub struct Season {
    pub zone_config: Pubkey,
    pub season_index: u32,
    pub network_name: String, // "helium" | "hivemapper" | "geodnet" | …
    pub h3_resolution: u8,    // H3 cell resolution (5–12)
    pub start_ts: i64,
    pub end_ts: i64,
    pub bounty_pool: u64,     // SOL lamports deposited as coverage bounty
    pub zones_claimed: u32,
    pub zones_verified: u32,
    pub is_settled: bool,
    pub bump: u8,
}

impl Season {
    pub const MAX_SIZE: usize =
        32 + 4 + (4 + MAX_NETWORK_NAME_LEN) + 1 + 8 + 8 + 8 + 4 + 4 + 1 + 1 + 8;
}

// ─── ZoneClaim ────────────────────────────────────────────────────────────────
// PDA seeds: [ZONE_CLAIM_SEED, season, h3_index.to_be_bytes()]
// One per (season, H3 cell). First operator to claim wins.

#[account]
pub struct ZoneClaim {
    pub season: Pubkey,
    /// H3 cell index stored as raw u64 (big-endian bytes used in PDA seed)
    pub h3_index: u64,
    pub operator: Pubkey,
    /// The guage-commons Facility account that backs this zone
    pub facility: Pubkey,
    pub claimed_at: i64,
    pub is_verified: bool,
    pub verified_at: i64,
    /// The guage-commons SnapshotBuffer that provided proof of coverage
    pub snapshot_buffer: Pubkey,
    pub bump: u8,
}

impl ZoneClaim {
    pub const MAX_SIZE: usize = 32 + 8 + 32 + 32 + 8 + 1 + 8 + 32 + 1 + 8;
}

// ─── OperatorVault ────────────────────────────────────────────────────────────
// PDA seeds: [OP_VAULT_SEED, season, operator]
// Tracks an operator's aggregate stats per season.

#[account]
pub struct OperatorVault {
    pub season: Pubkey,
    pub operator: Pubkey,
    pub zones_claimed: u32,
    pub zones_verified: u32,
    pub rewards_claimed: u64,
    pub bump: u8,
}

impl OperatorVault {
    pub const MAX_SIZE: usize = 32 + 32 + 4 + 4 + 8 + 1 + 8;
}

// ─── ContributionPassport ─────────────────────────────────────────────────────
// PDA seeds: [PASSPORT_SEED, authority]
// One per wallet. Accumulates stats across all seasons and networks.

#[account]
pub struct ContributionPassport {
    pub authority: Pubkey,
    pub zones_claimed_total: u32,
    pub zones_verified_total: u32,
    pub seasons_participated: u32,
    /// 0=Scout  1=Runner  2=ZoneLead  3=Pioneer
    pub current_tier: u8,
    pub last_updated: i64,
    pub bump: u8,
}

impl ContributionPassport {
    pub const MAX_SIZE: usize = 32 + 4 + 4 + 4 + 1 + 8 + 1 + 8;

    pub fn recompute_tier(&mut self) {
        self.current_tier = if self.zones_verified_total >= PIONEER_MIN_ZONES {
            3 // Pioneer
        } else if self.zones_verified_total >= ZONE_LEAD_MIN_ZONES
            && self.seasons_participated >= ZONE_LEAD_MIN_SEASONS
        {
            2 // ZoneLead
        } else if self.zones_verified_total >= RUNNER_MIN_ZONES {
            1 // Runner
        } else {
            0 // Scout
        };
    }
}

// ─── Shadow structs for guage-commons cross-program reads ─────────────────────
// We read SnapshotBuffer accounts owned by the guage-commons program without CPI.
// These mirror the on-chain layout exactly (Anchor borsh serialization).

#[derive(AnchorDeserialize, Clone, Default)]
pub struct SnapshotEntryView {
    pub time_bucket_start: i64,
    pub value: i128,
    pub hash_of_raw_batch: [u8; 32],
    pub quality_flags: u64,
    pub publisher: Pubkey,
    pub created_at: i64,
}

#[derive(AnchorDeserialize)]
pub struct SnapshotBufferView {
    pub facility: Pubkey,
    pub metric: Pubkey,
    pub head: u16,
    pub count: u16,
    pub capacity: u16,
    pub entries: Vec<SnapshotEntryView>,
    pub bump: u8,
}
