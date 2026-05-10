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
    /// Must be called by the club admin before any seasons can be created.
    pub fn initialize_zone_config(
        ctx: Context<InitializeZoneConfig>,
        club_id: u64,
        guage_program_id: Pubkey,
    ) -> Result<()> {
        initialize::initialize_zone_config(ctx, club_id, guage_program_id)
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

    /// Deposit $ZONE tokens into the season reward pool.
    pub fn fund_season_pool(ctx: Context<FundSeasonPool>, amount: u64) -> Result<()> {
        season::fund_season_pool(ctx, amount)
    }

    // ── Zone racing ──────────────────────────────────────────────────────────

    /// Operator claims an H3 geographic zone for the current season.
    pub fn claim_zone(
        ctx: Context<ClaimZone>,
        h3_index: u64,
        facility: Pubkey,
    ) -> Result<()> {
        zone::claim_zone(ctx, h3_index, facility)
    }

    /// Verify a zone claim by reading coverage data from a guage-commons SnapshotBuffer.
    /// Cross-program account read — no CPI. The proof is the real-world data.
    pub fn verify_zone_coverage(
        ctx: Context<VerifyZoneCoverage>,
        h3_index: u64,
        min_entries: u8,
        min_quality_flags: u64,
    ) -> Result<()> {
        zone::verify_zone_coverage(ctx, h3_index, min_entries, min_quality_flags)
    }

    // ── Delegation ───────────────────────────────────────────────────────────

    /// Delegate $ZONE to an operator for the current season.
    /// Tokens are held in the operator's vault PDA until undelegated or rewards claimed.
    pub fn delegate_stake(ctx: Context<DelegateStake>, amount: u64) -> Result<()> {
        delegation::delegate_stake(ctx, amount)
    }

    /// Return principal to delegator after the season ends.
    pub fn undelegate_stake(ctx: Context<UndelegateStake>) -> Result<()> {
        delegation::undelegate_stake(ctx)
    }

    // ── Rewards ──────────────────────────────────────────────────────────────

    /// Settle the season after end_ts. Permissionless — anyone can call.
    pub fn settle_season(ctx: Context<SettleSeason>) -> Result<()> {
        rewards::settle_season(ctx)
    }

    /// Operator claims their 70% reward share, pro-rated by zones_verified.
    pub fn claim_operator_rewards(ctx: Context<ClaimOperatorRewards>) -> Result<()> {
        rewards::claim_operator_rewards(ctx)
    }

    /// Delegator claims their share of the 30% delegator pool.
    pub fn claim_delegator_rewards(ctx: Context<ClaimDelegatorRewards>) -> Result<()> {
        rewards::claim_delegator_rewards(ctx)
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
