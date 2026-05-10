use anchor_lang::prelude::*;
use crate::state::*;

/// Permissionless passport refresh — anyone can call to sync tier for any wallet.
/// The on-chain stats are the source of truth; this just recomputes the tier field.
pub fn update_passport(ctx: Context<UpdatePassport>) -> Result<()> {
    let now = Clock::get()?.unix_timestamp;
    let passport = &mut ctx.accounts.passport;

    // Ensure authority is set if passport was just created
    if passport.authority == Pubkey::default() {
        passport.authority = ctx.accounts.authority.key();
        passport.bump = ctx.bumps.passport;
    }

    passport.last_updated = now;
    passport.recompute_tier();

    emit!(PassportUpdatedEvent {
        authority: passport.authority,
        tier: passport.current_tier,
        zones_verified: passport.zones_verified_total,
        seasons_participated: passport.seasons_participated,
        updated_at: now,
    });

    Ok(())
}

/// Called by season instructions to record season participation.
/// Kept as a standalone instruction so API can call it for any user post-season.
pub fn record_season_participation(ctx: Context<RecordSeasonParticipation>) -> Result<()> {
    let now = Clock::get()?.unix_timestamp;
    let passport = &mut ctx.accounts.passport;

    if passport.authority == Pubkey::default() {
        passport.authority = ctx.accounts.authority.key();
        passport.bump = ctx.bumps.passport;
    }

    passport.seasons_participated = passport.seasons_participated.saturating_add(1);
    passport.last_updated = now;
    passport.recompute_tier();

    Ok(())
}

// ─── Accounts ────────────────────────────────────────────────────────────────

#[derive(Accounts)]
pub struct UpdatePassport<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    /// CHECK: the wallet whose passport we are updating; does not need to sign
    pub authority: AccountInfo<'info>,

    #[account(
        init_if_needed,
        payer = payer,
        space = 8 + ContributionPassport::MAX_SIZE,
        seeds = [PASSPORT_SEED, authority.key().as_ref()],
        bump
    )]
    pub passport: Account<'info, ContributionPassport>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct RecordSeasonParticipation<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    /// CHECK: the wallet whose passport we are updating
    pub authority: AccountInfo<'info>,

    #[account(
        init_if_needed,
        payer = payer,
        space = 8 + ContributionPassport::MAX_SIZE,
        seeds = [PASSPORT_SEED, authority.key().as_ref()],
        bump
    )]
    pub passport: Account<'info, ContributionPassport>,

    pub system_program: Program<'info, System>,
}

// ─── Events ──────────────────────────────────────────────────────────────────

#[event]
pub struct PassportUpdatedEvent {
    pub authority: Pubkey,
    pub tier: u8,
    pub zones_verified: u32,
    pub seasons_participated: u32,
    pub updated_at: i64,
}
