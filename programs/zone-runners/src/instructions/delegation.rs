use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount, Transfer};
use crate::errors::ZoneError;
use crate::state::*;

pub fn delegate_stake(ctx: Context<DelegateStake>, amount: u64) -> Result<()> {
    require!(amount > 0, ZoneError::ZeroDelegation);

    let now = Clock::get()?.unix_timestamp;
    let season = &mut ctx.accounts.season;

    require!(now >= season.start_ts, ZoneError::SeasonNotStarted);
    require!(now < season.end_ts, ZoneError::SeasonEnded);
    require!(!season.is_settled, ZoneError::SeasonAlreadySettled);

    // Transfer $ZONE delegator → operator token vault
    let cpi_ctx = CpiContext::new(
        ctx.accounts.token_program.to_account_info(),
        Transfer {
            from: ctx.accounts.delegator_token_account.to_account_info(),
            to: ctx.accounts.operator_token_vault.to_account_info(),
            authority: ctx.accounts.delegator.to_account_info(),
        },
    );
    token::transfer(cpi_ctx, amount)?;

    // Record delegation
    let stake = &mut ctx.accounts.delegation_stake;
    stake.season = season.key();
    stake.operator = ctx.accounts.operator.key();
    stake.delegator = ctx.accounts.delegator.key();
    stake.amount = amount;
    stake.delegated_at = now;
    stake.is_active = true;
    stake.rewards_claimed = 0;
    stake.bump = ctx.bumps.delegation_stake;

    // Update operator vault totals
    let vault = &mut ctx.accounts.operator_vault;
    if vault.season == Pubkey::default() {
        vault.season = season.key();
        vault.operator = ctx.accounts.operator.key();
        vault.bump = ctx.bumps.operator_vault;
    }
    vault.total_delegated = vault
        .total_delegated
        .checked_add(amount)
        .ok_or(ZoneError::MathOverflow)?;

    // Update season totals
    season.total_delegated = season
        .total_delegated
        .checked_add(amount)
        .ok_or(ZoneError::MathOverflow)?;

    // Update delegator passport
    let passport = &mut ctx.accounts.passport;
    if passport.authority == Pubkey::default() {
        passport.authority = ctx.accounts.delegator.key();
        passport.bump = ctx.bumps.passport;
    }
    passport.total_delegated_ever = passport
        .total_delegated_ever
        .checked_add(amount)
        .ok_or(ZoneError::MathOverflow)?;
    passport.delegation_count = passport
        .delegation_count
        .checked_add(1)
        .ok_or(ZoneError::MathOverflow)?;
    passport.last_updated = now;
    passport.recompute_tier();

    emit!(DelegationStakedEvent {
        season: season.key(),
        operator: ctx.accounts.operator.key(),
        delegator: ctx.accounts.delegator.key(),
        amount,
        new_tier: passport.current_tier,
    });

    Ok(())
}

/// Returns delegated tokens after season ends. Rewards are claimed separately.
pub fn undelegate_stake(ctx: Context<UndelegateStake>) -> Result<()> {
    let now = Clock::get()?.unix_timestamp;
    let season = &ctx.accounts.season;

    // Can only undelegate after season ends
    require!(now >= season.end_ts || season.is_settled, ZoneError::CannotUndelegateActive);

    let stake = &mut ctx.accounts.delegation_stake;
    require!(stake.is_active, ZoneError::DelegationNotActive);

    let amount = stake.amount;
    stake.is_active = false;

    // PDA signer seeds for operator_token_vault (authority = operator_vault PDA)
    let season_key = season.key();
    let operator_key = ctx.accounts.operator.key();
    let vault_seeds: &[&[u8]] = &[
        OP_VAULT_SEED,
        season_key.as_ref(),
        operator_key.as_ref(),
        &[ctx.accounts.operator_vault.bump],
    ];
    let signer = &[vault_seeds];

    let cpi_ctx = CpiContext::new_with_signer(
        ctx.accounts.token_program.to_account_info(),
        Transfer {
            from: ctx.accounts.operator_token_vault.to_account_info(),
            to: ctx.accounts.delegator_token_account.to_account_info(),
            authority: ctx.accounts.operator_vault.to_account_info(),
        },
        signer,
    );
    token::transfer(cpi_ctx, amount)?;

    Ok(())
}

// ─── Accounts ────────────────────────────────────────────────────────────────

#[derive(Accounts)]
pub struct DelegateStake<'info> {
    #[account(mut)]
    pub delegator: Signer<'info>,

    /// CHECK: operator is just a pubkey target; no signing required for delegation
    pub operator: AccountInfo<'info>,

    #[account(
        mut,
        seeds = [SEASON_SEED, season.zone_config.as_ref(), &season.season_index.to_be_bytes()],
        bump = season.bump,
    )]
    pub season: Account<'info, Season>,

    #[account(
        init,
        payer = delegator,
        space = 8 + DelegationStake::MAX_SIZE,
        seeds = [DELEGATION_SEED, season.key().as_ref(), operator.key().as_ref(), delegator.key().as_ref()],
        bump
    )]
    pub delegation_stake: Account<'info, DelegationStake>,

    #[account(
        init_if_needed,
        payer = delegator,
        space = 8 + OperatorVault::MAX_SIZE,
        seeds = [OP_VAULT_SEED, season.key().as_ref(), operator.key().as_ref()],
        bump
    )]
    pub operator_vault: Account<'info, OperatorVault>,

    #[account(
        init_if_needed,
        payer = delegator,
        space = 8 + ContributionPassport::MAX_SIZE,
        seeds = [PASSPORT_SEED, delegator.key().as_ref()],
        bump
    )]
    pub passport: Account<'info, ContributionPassport>,

    pub zone_token_mint: Account<'info, Mint>,

    #[account(
        mut,
        associated_token::mint = zone_token_mint,
        associated_token::authority = delegator,
    )]
    pub delegator_token_account: Account<'info, TokenAccount>,

    /// Operator vault token account — authority is the operator_vault PDA
    #[account(
        init_if_needed,
        payer = delegator,
        seeds = [OP_TOKEN_VAULT_SEED, season.key().as_ref(), operator.key().as_ref()],
        bump,
        token::mint = zone_token_mint,
        token::authority = operator_vault,
    )]
    pub operator_token_vault: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    /// CHECK: required by anchor_spl
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct UndelegateStake<'info> {
    #[account(mut)]
    pub delegator: Signer<'info>,

    /// CHECK: used only as a key reference for PDA derivation
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
        mut,
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
        seeds = [OP_TOKEN_VAULT_SEED, season.key().as_ref(), operator.key().as_ref()],
        bump,
        token::mint = zone_token_mint,
        token::authority = operator_vault,
    )]
    pub operator_token_vault: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

// ─── Events ──────────────────────────────────────────────────────────────────

#[event]
pub struct DelegationStakedEvent {
    pub season: Pubkey,
    pub operator: Pubkey,
    pub delegator: Pubkey,
    pub amount: u64,
    pub new_tier: u8,
}
