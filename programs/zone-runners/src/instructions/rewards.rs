use anchor_lang::prelude::*;
use crate::errors::ZoneError;
use crate::state::*;

/// Marks the season as settled. Permissionless — anyone can call after end_ts.
pub fn settle_season(ctx: Context<SettleSeason>) -> Result<()> {
    let now = Clock::get()?.unix_timestamp;
    let season = &mut ctx.accounts.season;

    require!(now >= season.end_ts, ZoneError::SeasonStillActive);
    require!(!season.is_settled, ZoneError::SeasonAlreadySettled);
    require!(season.bounty_pool > 0, ZoneError::EmptyRewardPool);
    require!(season.zones_verified > 0, ZoneError::NoVerifiedZones);

    season.is_settled = true;

    emit!(SeasonSettledEvent {
        season: ctx.accounts.season.key(),
        zones_verified: season.zones_verified,
        bounty_pool: season.bounty_pool,
    });

    Ok(())
}

/// Operator claims their SOL reward share, pro-rated by zones_verified.
/// reward = bounty_pool * (operator_zones_verified / total_zones_verified)
pub fn claim_operator_rewards(ctx: Context<ClaimOperatorRewards>) -> Result<()> {
    let season = &ctx.accounts.season;
    require!(season.is_settled, ZoneError::SeasonStillActive);

    let vault = &mut ctx.accounts.operator_vault;
    require!(vault.zones_verified > 0, ZoneError::ZoneNotVerified);
    require!(vault.rewards_claimed == 0, ZoneError::RewardsAlreadyClaimed);

    let reward = season
        .bounty_pool
        .checked_mul(vault.zones_verified as u64)
        .ok_or(ZoneError::MathOverflow)?
        .checked_div(season.zones_verified as u64)
        .ok_or(ZoneError::MathOverflow)?;

    vault.rewards_claimed = reward;

    // Transfer SOL from Season PDA to operator.
    // The program owns the Season PDA so we can manipulate lamports directly.
    let season_info = ctx.accounts.season.to_account_info();
    let operator_info = ctx.accounts.operator.to_account_info();
    **season_info.try_borrow_mut_lamports()? -= reward;
    **operator_info.try_borrow_mut_lamports()? += reward;

    emit!(RewardsClaimedEvent {
        season: season.key(),
        claimant: ctx.accounts.operator.key(),
        amount: reward,
    });

    Ok(())
}

// ─── Accounts ────────────────────────────────────────────────────────────────

#[derive(Accounts)]
pub struct SettleSeason<'info> {
    pub settler: Signer<'info>,

    #[account(
        mut,
        seeds = [SEASON_SEED, season.zone_config.as_ref(), &season.season_index.to_be_bytes()],
        bump = season.bump,
    )]
    pub season: Account<'info, Season>,
}

#[derive(Accounts)]
pub struct ClaimOperatorRewards<'info> {
    #[account(mut)]
    pub operator: Signer<'info>,

    #[account(
        mut,
        seeds = [SEASON_SEED, season.zone_config.as_ref(), &season.season_index.to_be_bytes()],
        bump = season.bump,
    )]
    pub season: Account<'info, Season>,

    #[account(
        mut,
        seeds = [OP_VAULT_SEED, season.key().as_ref(), operator.key().as_ref()],
        bump = operator_vault.bump,
        constraint = operator_vault.operator == operator.key() @ ZoneError::Unauthorized,
    )]
    pub operator_vault: Account<'info, OperatorVault>,
}

// ─── Events ──────────────────────────────────────────────────────────────────

#[event]
pub struct SeasonSettledEvent {
    pub season: Pubkey,
    pub zones_verified: u32,
    pub bounty_pool: u64,
}

#[event]
pub struct RewardsClaimedEvent {
    pub season: Pubkey,
    pub claimant: Pubkey,
    pub amount: u64,
}
