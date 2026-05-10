use anchor_lang::prelude::*;
use anchor_lang::system_program;
use crate::errors::ZoneError;
use crate::state::*;

pub fn create_season(
    ctx: Context<CreateSeason>,
    network_name: String,
    h3_resolution: u8,
    start_ts: i64,
    end_ts: i64,
) -> Result<()> {
    require!(network_name.len() <= MAX_NETWORK_NAME_LEN, ZoneError::FieldTooLong);
    require!(h3_resolution >= 5 && h3_resolution <= 12, ZoneError::InvalidH3Resolution);
    require!(end_ts > start_ts, ZoneError::InvalidSeasonWindow);

    let cfg = &mut ctx.accounts.zone_config;
    let season = &mut ctx.accounts.season;

    season.zone_config = cfg.key();
    season.season_index = cfg.season_count;
    season.network_name = network_name;
    season.h3_resolution = h3_resolution;
    season.start_ts = start_ts;
    season.end_ts = end_ts;
    season.bounty_pool = 0;
    season.zones_claimed = 0;
    season.zones_verified = 0;
    season.is_settled = false;
    season.bump = ctx.bumps.season;

    cfg.season_count = cfg.season_count.checked_add(1).ok_or(ZoneError::MathOverflow)?;

    Ok(())
}

/// Deposit SOL into the season bounty pool. Anyone can fund a season.
/// Coverage buyers — businesses, protocols, researchers — use this to put
/// real money behind the geographic coverage they want proven.
pub fn fund_season_pool(ctx: Context<FundSeasonPool>, amount: u64) -> Result<()> {
    let season = &mut ctx.accounts.season;
    require!(!season.is_settled, ZoneError::SeasonAlreadySettled);
    require!(amount > 0, ZoneError::EmptyRewardPool);

    // Transfer SOL from funder to the Season PDA
    let cpi_ctx = CpiContext::new(
        ctx.accounts.system_program.to_account_info(),
        system_program::Transfer {
            from: ctx.accounts.funder.to_account_info(),
            to: ctx.accounts.season.to_account_info(),
        },
    );
    system_program::transfer(cpi_ctx, amount)?;

    season.bounty_pool = season
        .bounty_pool
        .checked_add(amount)
        .ok_or(ZoneError::MathOverflow)?;

    Ok(())
}

// ─── Accounts ────────────────────────────────────────────────────────────────

#[derive(Accounts)]
pub struct CreateSeason<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        mut,
        seeds = [ZONE_CONFIG_SEED, &zone_config.club_id],
        bump = zone_config.bump,
        has_one = admin @ ZoneError::Unauthorized,
    )]
    pub zone_config: Account<'info, ZoneConfig>,

    #[account(
        init,
        payer = admin,
        space = 8 + Season::MAX_SIZE,
        seeds = [SEASON_SEED, zone_config.key().as_ref(), &zone_config.season_count.to_be_bytes()],
        bump
    )]
    pub season: Account<'info, Season>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct FundSeasonPool<'info> {
    #[account(mut)]
    pub funder: Signer<'info>,

    #[account(
        mut,
        seeds = [SEASON_SEED, season.zone_config.as_ref(), &season.season_index.to_be_bytes()],
        bump = season.bump,
    )]
    pub season: Account<'info, Season>,

    pub system_program: Program<'info, System>,
}
