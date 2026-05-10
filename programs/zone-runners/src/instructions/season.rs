use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount, Transfer};
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
    season.reward_pool = 0;
    season.zones_claimed = 0;
    season.zones_verified = 0;
    season.total_delegated = 0;
    season.is_settled = false;
    season.bump = ctx.bumps.season;

    cfg.season_count = cfg.season_count.checked_add(1).ok_or(ZoneError::MathOverflow)?;

    Ok(())
}

pub fn fund_season_pool(ctx: Context<FundSeasonPool>, amount: u64) -> Result<()> {
    let season = &mut ctx.accounts.season;
    require!(!season.is_settled, ZoneError::SeasonAlreadySettled);

    // Transfer $ZONE from funder → season vault
    let cpi_ctx = CpiContext::new(
        ctx.accounts.token_program.to_account_info(),
        Transfer {
            from: ctx.accounts.funder_token_account.to_account_info(),
            to: ctx.accounts.season_token_vault.to_account_info(),
            authority: ctx.accounts.funder.to_account_info(),
        },
    );
    token::transfer(cpi_ctx, amount)?;

    season.reward_pool = season
        .reward_pool
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

    pub zone_token_mint: Account<'info, Mint>,

    #[account(
        mut,
        associated_token::mint = zone_token_mint,
        associated_token::authority = funder,
    )]
    pub funder_token_account: Account<'info, TokenAccount>,

    /// Season vault PDA holds pooled $ZONE rewards
    #[account(
        init_if_needed,
        payer = funder,
        seeds = [SEASON_TOKEN_VAULT_SEED, season.key().as_ref()],
        bump,
        token::mint = zone_token_mint,
        token::authority = season,
    )]
    pub season_token_vault: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    /// CHECK: required by anchor_spl associated_token
    pub rent: Sysvar<'info, Rent>,
}
