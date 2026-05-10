use anchor_lang::prelude::*;
use crate::errors::ZoneError;
use crate::state::*;

pub fn claim_zone(ctx: Context<ClaimZone>, h3_index: u64, facility: Pubkey) -> Result<()> {
    let now = Clock::get()?.unix_timestamp;
    let season = &mut ctx.accounts.season;

    require!(now >= season.start_ts, ZoneError::SeasonNotStarted);
    require!(now < season.end_ts, ZoneError::SeasonEnded);
    require!(!season.is_settled, ZoneError::SeasonAlreadySettled);

    let claim = &mut ctx.accounts.zone_claim;
    claim.season = season.key();
    claim.h3_index = h3_index;
    claim.operator = ctx.accounts.operator.key();
    claim.facility = facility;
    claim.claimed_at = now;
    claim.is_verified = false;
    claim.verified_at = 0;
    claim.snapshot_buffer = Pubkey::default();
    claim.bump = ctx.bumps.zone_claim;

    // Init or update operator vault
    let vault = &mut ctx.accounts.operator_vault;
    if vault.season == Pubkey::default() {
        vault.season = season.key();
        vault.operator = ctx.accounts.operator.key();
        vault.zones_claimed = 0;
        vault.zones_verified = 0;
        vault.rewards_claimed = 0;
        vault.bump = ctx.bumps.operator_vault;
    }
    vault.zones_claimed = vault.zones_claimed.checked_add(1).ok_or(ZoneError::MathOverflow)?;

    season.zones_claimed = season.zones_claimed.checked_add(1).ok_or(ZoneError::MathOverflow)?;

    // Ensure operator passport exists
    let passport = &mut ctx.accounts.passport;
    if passport.authority == Pubkey::default() {
        passport.authority = ctx.accounts.operator.key();
        passport.bump = ctx.bumps.passport;
    }
    passport.zones_claimed_total = passport
        .zones_claimed_total
        .checked_add(1)
        .ok_or(ZoneError::MathOverflow)?;
    passport.last_updated = now;
    passport.recompute_tier();

    emit!(ZoneClaimedEvent {
        season: season.key(),
        h3_index,
        operator: ctx.accounts.operator.key(),
        facility,
        claimed_at: now,
    });

    Ok(())
}

/// Verifies zone coverage by reading a guage-commons SnapshotBuffer account.
/// No CPI — we deserialize the account data directly using matching borsh layout.
pub fn verify_zone_coverage(
    ctx: Context<VerifyZoneCoverage>,
    h3_index: u64,
    min_entries: u8,
    min_quality_flags: u64,
) -> Result<()> {
    let now = Clock::get()?.unix_timestamp;
    let zone_config = &ctx.accounts.zone_config;
    let claim = &mut ctx.accounts.zone_claim;

    require!(!claim.is_verified, ZoneError::ZoneAlreadyClaimed);
    require_keys_eq!(claim.h3_index, h3_index);

    // ── Cross-program account read ───────────────────────────────────────────
    // Verify snapshot_buffer is owned by guage-commons (not spoofable).
    require_keys_eq!(
        ctx.accounts.snapshot_buffer.owner.key(),
        zone_config.guage_program_id,
        ZoneError::InvalidSnapshotOwner
    );

    // Deserialize the SnapshotBuffer — skip 8-byte Anchor discriminator.
    let snapshot_data = ctx.accounts.snapshot_buffer.data.borrow();
    let snapshot =
        SnapshotBufferView::try_from_slice(&snapshot_data[8..]).map_err(|_| ZoneError::InvalidSnapshotOwner)?;

    // Facility on the snapshot must match what the operator registered.
    require_keys_eq!(snapshot.facility, claim.facility, ZoneError::FacilityMismatch);

    // Count recent entries with sufficient quality in the last 24 hours.
    let recent_count = snapshot
        .entries
        .iter()
        .filter(|e| {
            e.created_at != 0
                && e.created_at > now - 86_400
                && e.quality_flags >= min_quality_flags
        })
        .count();

    require!(
        recent_count >= min_entries as usize,
        ZoneError::InsufficientCoverage
    );
    // ────────────────────────────────────────────────────────────────────────

    // Mark claim as verified
    claim.is_verified = true;
    claim.verified_at = now;
    claim.snapshot_buffer = ctx.accounts.snapshot_buffer.key();

    // Update operator vault
    let vault = &mut ctx.accounts.operator_vault;
    vault.zones_verified = vault.zones_verified.checked_add(1).ok_or(ZoneError::MathOverflow)?;

    // Update season counter
    let season = &mut ctx.accounts.season;
    season.zones_verified = season.zones_verified.checked_add(1).ok_or(ZoneError::MathOverflow)?;

    // Update passport
    let passport = &mut ctx.accounts.passport;
    passport.zones_verified_total = passport
        .zones_verified_total
        .checked_add(1)
        .ok_or(ZoneError::MathOverflow)?;
    passport.last_updated = now;
    passport.recompute_tier();

    emit!(ZoneVerifiedEvent {
        season: season.key(),
        h3_index,
        operator: ctx.accounts.operator.key(),
        snapshot_buffer: ctx.accounts.snapshot_buffer.key(),
        verified_at: now,
        new_tier: passport.current_tier,
    });

    Ok(())
}

// ─── Accounts ────────────────────────────────────────────────────────────────

#[derive(Accounts)]
#[instruction(h3_index: u64)]
pub struct ClaimZone<'info> {
    #[account(mut)]
    pub operator: Signer<'info>,

    #[account(
        seeds = [ZONE_CONFIG_SEED, &zone_config.club_id],
        bump = zone_config.bump,
    )]
    pub zone_config: Account<'info, ZoneConfig>,

    #[account(
        mut,
        seeds = [SEASON_SEED, season.zone_config.as_ref(), &season.season_index.to_be_bytes()],
        bump = season.bump,
        constraint = season.zone_config == zone_config.key(),
    )]
    pub season: Account<'info, Season>,

    #[account(
        init,
        payer = operator,
        space = 8 + ZoneClaim::MAX_SIZE,
        seeds = [ZONE_CLAIM_SEED, season.key().as_ref(), &h3_index.to_be_bytes()],
        bump
    )]
    pub zone_claim: Account<'info, ZoneClaim>,

    #[account(
        init_if_needed,
        payer = operator,
        space = 8 + OperatorVault::MAX_SIZE,
        seeds = [OP_VAULT_SEED, season.key().as_ref(), operator.key().as_ref()],
        bump
    )]
    pub operator_vault: Account<'info, OperatorVault>,

    #[account(
        init_if_needed,
        payer = operator,
        space = 8 + ContributionPassport::MAX_SIZE,
        seeds = [PASSPORT_SEED, operator.key().as_ref()],
        bump
    )]
    pub passport: Account<'info, ContributionPassport>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(h3_index: u64)]
pub struct VerifyZoneCoverage<'info> {
    #[account(mut)]
    pub operator: Signer<'info>,

    #[account(
        seeds = [ZONE_CONFIG_SEED, &zone_config.club_id],
        bump = zone_config.bump,
    )]
    pub zone_config: Account<'info, ZoneConfig>,

    #[account(
        mut,
        seeds = [SEASON_SEED, season.zone_config.as_ref(), &season.season_index.to_be_bytes()],
        bump = season.bump,
    )]
    pub season: Account<'info, Season>,

    #[account(
        mut,
        seeds = [ZONE_CLAIM_SEED, season.key().as_ref(), &h3_index.to_be_bytes()],
        bump = zone_claim.bump,
        constraint = zone_claim.operator == operator.key() @ ZoneError::Unauthorized,
    )]
    pub zone_claim: Account<'info, ZoneClaim>,

    #[account(
        mut,
        seeds = [OP_VAULT_SEED, season.key().as_ref(), operator.key().as_ref()],
        bump = operator_vault.bump,
    )]
    pub operator_vault: Account<'info, OperatorVault>,

    #[account(
        mut,
        seeds = [PASSPORT_SEED, operator.key().as_ref()],
        bump = passport.bump,
    )]
    pub passport: Account<'info, ContributionPassport>,

    /// CHECK: owned-by check enforced in instruction body against zone_config.guage_program_id
    pub snapshot_buffer: AccountInfo<'info>,
}

// ─── Events ──────────────────────────────────────────────────────────────────

#[event]
pub struct ZoneClaimedEvent {
    pub season: Pubkey,
    pub h3_index: u64,
    pub operator: Pubkey,
    pub facility: Pubkey,
    pub claimed_at: i64,
}

#[event]
pub struct ZoneVerifiedEvent {
    pub season: Pubkey,
    pub h3_index: u64,
    pub operator: Pubkey,
    pub snapshot_buffer: Pubkey,
    pub verified_at: i64,
    pub new_tier: u8,
}
