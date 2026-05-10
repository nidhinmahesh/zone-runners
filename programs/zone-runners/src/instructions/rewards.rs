use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount, Transfer};
use crate::errors::ZoneError;
use crate::state::*;

/// Marks the season as settled and computes reward allocations.
/// Must be called after end_ts. Anyone can call (permissionless settlement).
pub fn settle_season(ctx: Context<SettleSeason>) -> Result<()> {
    let now = Clock::get()?.unix_timestamp;
    let season = &mut ctx.accounts.season;

    require!(now >= season.end_ts, ZoneError::SeasonStillActive);
    require!(!season.is_settled, ZoneError::SeasonAlreadySettled);
    require!(season.reward_pool > 0, ZoneError::EmptyRewardPool);
    require!(season.zones_verified > 0, ZoneError::NoVerifiedZones);

    season.is_settled = true;

    emit!(SeasonSettledEvent {
        season: ctx.accounts.season.key(),
        zones_verified: season.zones_verified,
        total_delegated: season.total_delegated,
        reward_pool: season.reward_pool,
    });

    Ok(())
}

/// Operator claims their 70% share, pro-rated by zones_verified.
pub fn claim_operator_rewards(ctx: Context<ClaimOperatorRewards>) -> Result<()> {
    let season = &ctx.accounts.season;
    require!(season.is_settled, ZoneError::SeasonStillActive);

    let vault = &mut ctx.accounts.operator_vault;
    require!(vault.zones_verified > 0, ZoneError::ZoneNotVerified);
    require!(vault.rewards_distributed == 0, ZoneError::RewardsAlreadyClaimed);

    // operator_share = pool * 0.70 * (vault.zones_verified / season.zones_verified)
    let operator_pool = season
        .reward_pool
        .checked_mul(OPERATOR_SHARE_BPS)
        .ok_or(ZoneError::MathOverflow)?
        .checked_div(BPS_DENOMINATOR)
        .ok_or(ZoneError::MathOverflow)?;

    let reward = operator_pool
        .checked_mul(vault.zones_verified as u64)
        .ok_or(ZoneError::MathOverflow)?
        .checked_div(season.zones_verified as u64)
        .ok_or(ZoneError::MathOverflow)?;

    vault.rewards_distributed = reward;

    // PDA signer — season is the authority on the season_token_vault
    let season_index_bytes = season.season_index.to_be_bytes();
    let season_vault_seeds: &[&[u8]] = &[
        SEASON_SEED,
        season.zone_config.as_ref(),
        &season_index_bytes,
        &[season.bump],
    ];
    let signer = &[season_vault_seeds];

    let cpi_ctx = CpiContext::new_with_signer(
        ctx.accounts.token_program.to_account_info(),
        Transfer {
            from: ctx.accounts.season_token_vault.to_account_info(),
            to: ctx.accounts.operator_token_account.to_account_info(),
            authority: ctx.accounts.season.to_account_info(),
        },
        signer,
    );
    token::transfer(cpi_ctx, reward)?;

    emit!(RewardsClaimedEvent {
        season: season.key(),
        claimant: ctx.accounts.operator.key(),
        amount: reward,
        claim_type: 0, // operator
    });

    Ok(())
}

/// Delegator claims their share of the 30% delegator pool.
/// Share = 30% of pool * (their delegation / operator's total_delegated)
pub fn claim_delegator_rewards(ctx: Context<ClaimDelegatorRewards>) -> Result<()> {
    let season = &ctx.accounts.season;
    require!(season.is_settled, ZoneError::SeasonStillActive);

    let stake = &mut ctx.accounts.delegation_stake;
    require!(stake.is_active || season.is_settled, ZoneError::DelegationNotActive);
    require!(stake.rewards_claimed == 0, ZoneError::RewardsAlreadyClaimed);

    let vault = &ctx.accounts.operator_vault;
    require!(vault.total_delegated > 0, ZoneError::ZeroDelegation);

    // Only delegators backing operators with verified zones earn rewards
    let operator_pool = season
        .reward_pool
        .checked_mul(OPERATOR_SHARE_BPS)
        .ok_or(ZoneError::MathOverflow)?
        .checked_div(BPS_DENOMINATOR)
        .ok_or(ZoneError::MathOverflow)?;

    let operator_share = operator_pool
        .checked_mul(vault.zones_verified as u64)
        .ok_or(ZoneError::MathOverflow)?
        .checked_div(season.zones_verified.max(1) as u64)
        .ok_or(ZoneError::MathOverflow)?;

    let delegator_cut_of_operator = operator_share
        .checked_mul(DELEGATOR_SHARE_BPS)
        .ok_or(ZoneError::MathOverflow)?
        .checked_div(BPS_DENOMINATOR)
        .ok_or(ZoneError::MathOverflow)?;

    let reward = delegator_cut_of_operator
        .checked_mul(stake.amount)
        .ok_or(ZoneError::MathOverflow)?
        .checked_div(vault.total_delegated)
        .ok_or(ZoneError::MathOverflow)?;

    stake.rewards_claimed = reward;

    let season_index_bytes = season.season_index.to_be_bytes();
    let season_vault_seeds: &[&[u8]] = &[
        SEASON_SEED,
        season.zone_config.as_ref(),
        &season_index_bytes,
        &[season.bump],
    ];
    let signer = &[season_vault_seeds];

    let cpi_ctx = CpiContext::new_with_signer(
        ctx.accounts.token_program.to_account_info(),
        Transfer {
            from: ctx.accounts.season_token_vault.to_account_info(),
            to: ctx.accounts.delegator_token_account.to_account_info(),
            authority: ctx.accounts.season.to_account_info(),
        },
        signer,
    );
    token::transfer(cpi_ctx, reward)?;

    emit!(RewardsClaimedEvent {
        season: season.key(),
        claimant: ctx.accounts.delegator.key(),
        amount: reward,
        claim_type: 1, // delegator
    });

    Ok(())
}

// ─── Accounts ────────────────────────────────────────────────────────────────

#[derive(Accounts)]
pub struct SettleSeason<'info> {
    /// Anyone can settle — permissionless after end_ts
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

    pub zone_token_mint: Account<'info, Mint>,

    #[account(
        mut,
        associated_token::mint = zone_token_mint,
        associated_token::authority = operator,
    )]
    pub operator_token_account: Account<'info, TokenAccount>,

    #[account(
        mut,
        seeds = [SEASON_TOKEN_VAULT_SEED, season.key().as_ref()],
        bump,
        token::mint = zone_token_mint,
        token::authority = season,
    )]
    pub season_token_vault: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct ClaimDelegatorRewards<'info> {
    #[account(mut)]
    pub delegator: Signer<'info>,

    /// CHECK: used for PDA derivation
    pub operator: AccountInfo<'info>,

    #[account(
        seeds = [SEASON_SEED, season.zone_config.as_ref(), &season.season_index.to_be_bytes()],
        bump = season.bump,
    )]
    pub season: Account<'info, Season>,

    #[account(
        mut,
        seeds = [DELEGATION_SEED, season.key().as_ref(), operator.key().as_ref(), delegator.key().as_ref()],
        bump = delegation_stake.bump,
        constraint = delegation_stake.delegator == delegator.key() @ ZoneError::Unauthorized,
    )]
    pub delegation_stake: Account<'info, DelegationStake>,

    #[account(
        seeds = [OP_VAULT_SEED, season.key().as_ref(), operator.key().as_ref()],
        bump = operator_vault.bump,
    )]
    pub operator_vault: Account<'info, OperatorVault>,

    pub zone_token_mint: Account<'info, Mint>,

    #[account(
        mut,
        associated_token::mint = zone_token_mint,
        associated_token::authority = delegator,
    )]
    pub delegator_token_account: Account<'info, TokenAccount>,

    #[account(
        mut,
        seeds = [SEASON_TOKEN_VAULT_SEED, season.key().as_ref()],
        bump,
        token::mint = zone_token_mint,
        token::authority = season,
    )]
    pub season_token_vault: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
}

// ─── Events ──────────────────────────────────────────────────────────────────

#[event]
pub struct SeasonSettledEvent {
    pub season: Pubkey,
    pub zones_verified: u32,
    pub total_delegated: u64,
    pub reward_pool: u64,
}

#[event]
pub struct RewardsClaimedEvent {
    pub season: Pubkey,
    pub claimant: Pubkey,
    pub amount: u64,
    /// 0 = operator, 1 = delegator
    pub claim_type: u8,
}
