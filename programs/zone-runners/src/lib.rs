use anchor_lang::prelude::*;

pub mod errors;
pub mod instructions;
pub mod state;

use instructions::*;

// Replace with: `anchor keys list` output after `anchor build`
declare_id!("ZRuNrAtgJM4hG3YLtq6NmbHdtBqoNGbvRvbyLuLNoWf");

#[program]
pub mod zone_runners {
    use super::*;

    // ── Config ───────────────────────────────────────────────────────────────

    /// Initialize a ZoneConfig for a fexrapi club.
    pub fn initialize_zone_config(
        ctx: Context<InitializeZoneConfig>,
        club_id: u64,
        oracle_program_id: Pubkey,
    ) -> Result<()> {
        initialize::initialize_zone_config(ctx, club_id, oracle_program_id)
    }

    // ── Seasons ──────────────────────────────────────────────────────────────

    /// Create a new season campaign targeting a specific DePIN network.
    pub fn create_season(
        ctx: Context<CreateSeason>,
        network_name: String,
        h3_resolution: u8,
        start_ts: i64,
        end_ts: i64,
    ) -> Result<()> {
        season::create_season(ctx, network_name, h3_resolution, start_ts, end_ts)
    }

    /// Deposit SOL into the season bounty pool.
    /// Coverage buyers fund campaigns for geographic areas they need proven.
    pub fn fund_season_pool(ctx: Context<FundSeasonPool>, amount: u64) -> Result<()> {
        season::fund_season_pool(ctx, amount)
    }

    // ── Zone coverage ────────────────────────────────────────────────────────

    /// Operator claims an H3 geographic zone for the current season.
    /// Requires staking SOL (min 0.01 SOL) as a security bond. The stake is
    /// locked until the season is settled or lost to a successful challenger.
    pub fn claim_zone(
        ctx: Context<ClaimZone>,
        h3_index: u64,
        facility: Pubkey,
        stake_lamports: u64,
    ) -> Result<()> {
        zone::claim_zone(ctx, h3_index, facility, stake_lamports)
    }

    /// Verify a zone claim by reading coverage data from a DePIN oracle SnapshotBuffer.
    /// Cross-program account read — no CPI, no oracle. The proof is the hardware's own data.
    pub fn verify_zone_coverage(
        ctx: Context<VerifyZoneCoverage>,
        h3_index: u64,
        min_entries: u8,
        min_quality_flags: u64,
    ) -> Result<()> {
        zone::verify_zone_coverage(ctx, h3_index, min_entries, min_quality_flags)
    }

    // ── Rewards ──────────────────────────────────────────────────────────────

    /// Settle the season after end_ts. Permissionless — anyone can call.
    pub fn settle_season(ctx: Context<SettleSeason>) -> Result<()> {
        rewards::settle_season(ctx)
    }

    /// Operator claims their SOL share of the bounty pool, pro-rated by zones verified.
    pub fn claim_operator_rewards(ctx: Context<ClaimOperatorRewards>) -> Result<()> {
        rewards::claim_operator_rewards(ctx)
    }

    // ── Zone contention ──────────────────────────────────────────────────────

    /// Challenge a verified zone by proving strictly better coverage.
    /// Challenger deposits matching SOL. If their SnapshotBuffer shows more
    /// recent high-quality entries, they instantly take the defender's stake
    /// (minus 5% fee) and own the zone. A failed challenge costs only gas.
    pub fn challenge_zone(
        ctx: Context<ChallengeZone>,
        h3_index: u64,
        facility: Pubkey,
        min_entries: u8,
        min_quality_flags: u64,
    ) -> Result<()> {
        zone::challenge_zone(ctx, h3_index, facility, min_entries, min_quality_flags)
    }

    /// Return staked SOL to the zone owner after the season is settled.
    pub fn withdraw_zone_stake(ctx: Context<WithdrawZoneStake>, h3_index: u64) -> Result<()> {
        zone::withdraw_zone_stake(ctx, h3_index)
    }

    // ── Passport ─────────────────────────────────────────────────────────────

    /// Permissionless: recompute tier for any wallet's ContributionPassport.
    pub fn update_passport(ctx: Context<UpdatePassport>) -> Result<()> {
        passport::update_passport(ctx)
    }

    /// Record that a wallet participated in a season (called after season ends).
    pub fn record_season_participation(ctx: Context<RecordSeasonParticipation>) -> Result<()> {
        passport::record_season_participation(ctx)
    }
}
