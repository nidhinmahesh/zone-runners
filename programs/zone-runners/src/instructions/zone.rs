use anchor_lang::prelude::*;
use anchor_lang::system_program;
use crate::errors::ZoneError;
use crate::state::*;

pub fn claim_zone(
    ctx: Context<ClaimZone>,
    h3_index: u64,
    facility: Pubkey,
    stake_lamports: u64,
) -> Result<()> {
    let now = Clock::get()?.unix_timestamp;
    let season = &mut ctx.accounts.season;

    require!(now >= season.start_ts, ZoneError::SeasonNotStarted);
    require!(now < season.end_ts, ZoneError::SeasonEnded);
    require!(!season.is_settled, ZoneError::SeasonAlreadySettled);
    require!(stake_lamports >= MIN_ZONE_STAKE, ZoneError::StakeBelowMinimum);

    let claim = &mut ctx.accounts.zone_claim;
    claim.season = season.key();
    claim.h3_index = h3_index;
    claim.operator = ctx.accounts.operator.key();
    claim.facility = facility;
    claim.claimed_at = now;
    claim.is_verified = false;
    claim.verified_at = 0;
    claim.snapshot_buffer = Pubkey::default();
    claim.stake_lamports = stake_lamports;
    claim.coverage_score = 0;
    claim.challenge_count = 0;
    claim.bump = ctx.bumps.zone_claim;

    // Transfer stake SOL from operator → ZoneClaim PDA
    let cpi_ctx = CpiContext::new(
        ctx.accounts.system_program.to_account_info(),
        system_program::Transfer {
            from: ctx.accounts.operator.to_account_info(),
            to: ctx.accounts.zone_claim.to_account_info(),
        },
    );
    system_program::transfer(cpi_ctx, stake_lamports)?;

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
        stake_lamports,
        claimed_at: now,
    });

    Ok(())
}

/// Verifies zone coverage by reading a guage-commons SnapshotBuffer account.
/// No CPI — direct cross-program account read. Records coverage_score for future challenges.
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
    require_keys_eq!(
        ctx.accounts.snapshot_buffer.owner.key(),
        zone_config.guage_program_id,
        ZoneError::InvalidSnapshotOwner
    );

    let snapshot_data = ctx.accounts.snapshot_buffer.data.borrow();
    let snapshot =
        SnapshotBufferView::try_from_slice(&snapshot_data[8..]).map_err(|_| ZoneError::InvalidSnapshotOwner)?;

    require_keys_eq!(snapshot.facility, claim.facility, ZoneError::FacilityMismatch);

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

    claim.is_verified = true;
    claim.verified_at = now;
    claim.snapshot_buffer = ctx.accounts.snapshot_buffer.key();
    // Record score — challengers must strictly beat this to take the zone
    claim.coverage_score = recent_count as u32;

    let vault = &mut ctx.accounts.operator_vault;
    vault.zones_verified = vault.zones_verified.checked_add(1).ok_or(ZoneError::MathOverflow)?;

    let season = &mut ctx.accounts.season;
    season.zones_verified = season.zones_verified.checked_add(1).ok_or(ZoneError::MathOverflow)?;

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
        coverage_score: recent_count as u32,
        verified_at: now,
        new_tier: passport.current_tier,
    });

    Ok(())
}

/// Challenge a verified zone. Reads challenger's SnapshotBuffer on-chain.
/// If challenger's coverage_score strictly exceeds the current owner's, the challenge
/// succeeds instantly: challenger takes the defender's stake (minus 5% fee) and owns the zone.
/// A failed challenge costs only gas — Solana atomicity reverts the stake deposit.
pub fn challenge_zone(
    ctx: Context<ChallengeZone>,
    h3_index: u64,
    facility: Pubkey,
    min_entries: u8,
    min_quality_flags: u64,
) -> Result<()> {
    let now = Clock::get()?.unix_timestamp;
    let zone_config = &ctx.accounts.zone_config;
    let claim = &mut ctx.accounts.zone_claim;

    require!(claim.is_verified, ZoneError::ZoneNotVerified);
    require!(ctx.accounts.challenger.key() != claim.operator, ZoneError::CannotChallengeSelf);
    require_keys_eq!(claim.h3_index, h3_index);

    // ── Cross-program account read (same pattern as verify_zone_coverage) ────
    require_keys_eq!(
        ctx.accounts.snapshot_buffer.owner.key(),
        zone_config.guage_program_id,
        ZoneError::InvalidSnapshotOwner
    );

    let snapshot_data = ctx.accounts.snapshot_buffer.data.borrow();
    let snapshot =
        SnapshotBufferView::try_from_slice(&snapshot_data[8..]).map_err(|_| ZoneError::InvalidSnapshotOwner)?;

    require_keys_eq!(snapshot.facility, facility, ZoneError::FacilityMismatch);

    let challenger_score = snapshot
        .entries
        .iter()
        .filter(|e| {
            e.created_at != 0
                && e.created_at > now - 86_400
                && e.quality_flags >= min_quality_flags
        })
        .count() as u32;
    // ────────────────────────────────────────────────────────────────────────

    let defender_score = claim.coverage_score;
    let stake = claim.stake_lamports;

    // Deposit challenger's matching stake into ZoneClaim PDA.
    // If the require! below fails, the entire TX reverts and the deposit is undone.
    let cpi_ctx = CpiContext::new(
        ctx.accounts.system_program.to_account_info(),
        system_program::Transfer {
            from: ctx.accounts.challenger.to_account_info(),
            to: ctx.accounts.zone_claim.to_account_info(),
        },
    );
    system_program::transfer(cpi_ctx, stake)?;

    // Must beat — not just tie — the current owner's score.
    require!(
        challenger_score > defender_score,
        ZoneError::InsufficientCoverageToChallenge
    );

    // Challenger wins. Transfer defender's original stake to challenger minus fee.
    let fee = stake
        .checked_mul(CHALLENGE_FEE_BPS)
        .ok_or(ZoneError::MathOverflow)?
        .checked_div(BPS_BASE)
        .ok_or(ZoneError::MathOverflow)?;
    let profit = stake.checked_sub(fee).ok_or(ZoneError::MathOverflow)?;

    // ZoneClaim PDA now holds 2 × stake. Transfer profit to challenger, fee to admin.
    // Challenger's own stake stays in PDA as the new bond.
    let zone_claim_info = ctx.accounts.zone_claim.to_account_info();
    let challenger_info = ctx.accounts.challenger.to_account_info();
    let admin_info = ctx.accounts.admin.to_account_info();

    **zone_claim_info.try_borrow_mut_lamports()? -= profit + fee;
    **challenger_info.try_borrow_mut_lamports()? += profit;
    **admin_info.try_borrow_mut_lamports()? += fee;

    let former_operator = claim.operator;

    // Transfer zone ownership to challenger
    claim.operator = ctx.accounts.challenger.key();
    claim.facility = facility;
    claim.snapshot_buffer = ctx.accounts.snapshot_buffer.key();
    claim.coverage_score = challenger_score;
    claim.challenge_count = claim.challenge_count.saturating_add(1);
    // stake_lamports unchanged — challenger's deposit is now the bond

    // Update challenger passport
    let passport = &mut ctx.accounts.challenger_passport;
    if passport.authority == Pubkey::default() {
        passport.authority = ctx.accounts.challenger.key();
        passport.bump = ctx.bumps.challenger_passport;
    }
    passport.zones_claimed_total = passport
        .zones_claimed_total
        .checked_add(1)
        .ok_or(ZoneError::MathOverflow)?;
    passport.zones_verified_total = passport
        .zones_verified_total
        .checked_add(1)
        .ok_or(ZoneError::MathOverflow)?;
    passport.last_updated = now;
    passport.recompute_tier();

    emit!(ChallengeWonEvent {
        season: ctx.accounts.season.key(),
        h3_index,
        challenger: ctx.accounts.challenger.key(),
        former_operator,
        stake_transferred: profit,
        fee,
        challenger_score,
        defender_score,
        won_at: now,
    });

    Ok(())
}

/// Return staked SOL to the zone owner after the season is settled.
pub fn withdraw_zone_stake(ctx: Context<WithdrawZoneStake>, h3_index: u64) -> Result<()> {
    let claim = &mut ctx.accounts.zone_claim;

    require!(ctx.accounts.season.is_settled, ZoneError::SeasonNotSettledForWithdraw);
    require!(claim.operator == ctx.accounts.operator.key(), ZoneError::Unauthorized);
    require_keys_eq!(claim.h3_index, h3_index);
    require!(claim.stake_lamports > 0, ZoneError::StakeAlreadyWithdrawn);

    let stake = claim.stake_lamports;
    claim.stake_lamports = 0;

    **ctx.accounts.zone_claim.to_account_info().try_borrow_mut_lamports()? -= stake;
    **ctx.accounts.operator.to_account_info().try_borrow_mut_lamports()? += stake;

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

#[derive(Accounts)]
#[instruction(h3_index: u64)]
pub struct ChallengeZone<'info> {
    #[account(mut)]
    pub challenger: Signer<'info>,

    #[account(
        seeds = [ZONE_CONFIG_SEED, &zone_config.club_id],
        bump = zone_config.bump,
    )]
    pub zone_config: Account<'info, ZoneConfig>,

    #[account(
        seeds = [SEASON_SEED, season.zone_config.as_ref(), &season.season_index.to_be_bytes()],
        bump = season.bump,
    )]
    pub season: Account<'info, Season>,

    #[account(
        mut,
        seeds = [ZONE_CLAIM_SEED, season.key().as_ref(), &h3_index.to_be_bytes()],
        bump = zone_claim.bump,
    )]
    pub zone_claim: Account<'info, ZoneClaim>,

    /// The current zone owner — receives nothing on loss (their stake stays as challenger's bond)
    /// CHECK: validated via zone_claim.operator in instruction body
    #[account(
        mut,
        constraint = operator.key() == zone_claim.operator @ ZoneError::Unauthorized
    )]
    pub operator: AccountInfo<'info>,

    /// Protocol fee recipient
    /// CHECK: validated via zone_config.admin
    #[account(
        mut,
        constraint = admin.key() == zone_config.admin @ ZoneError::Unauthorized
    )]
    pub admin: AccountInfo<'info>,

    #[account(
        init_if_needed,
        payer = challenger,
        space = 8 + ContributionPassport::MAX_SIZE,
        seeds = [PASSPORT_SEED, challenger.key().as_ref()],
        bump
    )]
    pub challenger_passport: Account<'info, ContributionPassport>,

    /// CHECK: owned-by check enforced in instruction body against zone_config.guage_program_id
    pub snapshot_buffer: AccountInfo<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(h3_index: u64)]
pub struct WithdrawZoneStake<'info> {
    #[account(mut)]
    pub operator: Signer<'info>,

    #[account(
        seeds = [SEASON_SEED, season.zone_config.as_ref(), &season.season_index.to_be_bytes()],
        bump = season.bump,
    )]
    pub season: Account<'info, Season>,

    #[account(
        mut,
        seeds = [ZONE_CLAIM_SEED, season.key().as_ref(), &h3_index.to_be_bytes()],
        bump = zone_claim.bump,
    )]
    pub zone_claim: Account<'info, ZoneClaim>,
}

// ─── Events ──────────────────────────────────────────────────────────────────

#[event]
pub struct ZoneClaimedEvent {
    pub season: Pubkey,
    pub h3_index: u64,
    pub operator: Pubkey,
    pub facility: Pubkey,
    pub stake_lamports: u64,
    pub claimed_at: i64,
}

#[event]
pub struct ZoneVerifiedEvent {
    pub season: Pubkey,
    pub h3_index: u64,
    pub operator: Pubkey,
    pub snapshot_buffer: Pubkey,
    pub coverage_score: u32,
    pub verified_at: i64,
    pub new_tier: u8,
}

#[event]
pub struct ChallengeWonEvent {
    pub season: Pubkey,
    pub h3_index: u64,
    pub challenger: Pubkey,
    pub former_operator: Pubkey,
    pub stake_transferred: u64,
    pub fee: u64,
    pub challenger_score: u32,
    pub defender_score: u32,
    pub won_at: i64,
}
