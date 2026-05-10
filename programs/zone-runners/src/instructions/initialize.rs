use anchor_lang::prelude::*;
use crate::state::*;

pub fn initialize_zone_config(
    ctx: Context<InitializeZoneConfig>,
    club_id: u64,
    oracle_program_id: Pubkey,
) -> Result<()> {
    let cfg = &mut ctx.accounts.zone_config;
    cfg.admin = ctx.accounts.admin.key();
    cfg.club_id = club_id.to_le_bytes();
    cfg.oracle_program_id = oracle_program_id;
    cfg.season_count = 0;
    cfg.bump = ctx.bumps.zone_config;
    Ok(())
}

#[derive(Accounts)]
#[instruction(club_id: u64)]
pub struct InitializeZoneConfig<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        init,
        payer = admin,
        space = 8 + ZoneConfig::MAX_SIZE,
        seeds = [ZONE_CONFIG_SEED, &club_id.to_le_bytes()],
        bump
    )]
    pub zone_config: Account<'info, ZoneConfig>,

    pub system_program: Program<'info, System>,
}
