use anchor_lang::prelude::*;

#[error_code]
pub enum ZoneError {
    #[msg("Unauthorized: signer is not the admin or authority")]
    Unauthorized,
    #[msg("Season has not started yet")]
    SeasonNotStarted,
    #[msg("Season has already ended")]
    SeasonEnded,
    #[msg("Season is still active and cannot be settled")]
    SeasonStillActive,
    #[msg("Season has already been settled")]
    SeasonAlreadySettled,
    #[msg("This H3 zone has already been claimed in this season")]
    ZoneAlreadyClaimed,
    #[msg("Zone is not verified; cannot claim rewards")]
    ZoneNotVerified,
    #[msg("The snapshot_buffer is not owned by the expected guage-commons program")]
    InvalidSnapshotOwner,
    #[msg("The snapshot_buffer facility does not match the zone claim facility")]
    FacilityMismatch,
    #[msg("Not enough recent high-quality snapshots to verify this zone")]
    InsufficientCoverage,
    #[msg("Delegation amount must be greater than zero")]
    ZeroDelegation,
    #[msg("This delegation stake is not active")]
    DelegationNotActive,
    #[msg("Cannot undelegate while season is still active")]
    CannotUndelegateActive,
    #[msg("Rewards have already been claimed for this position")]
    RewardsAlreadyClaimed,
    #[msg("Season pool is empty; no rewards to distribute")]
    EmptyRewardPool,
    #[msg("No verified zones in season; cannot compute reward shares")]
    NoVerifiedZones,
    #[msg("Field value exceeds maximum allowed length")]
    FieldTooLong,
    #[msg("H3 resolution must be between 5 and 12")]
    InvalidH3Resolution,
    #[msg("Season end time must be after start time")]
    InvalidSeasonWindow,
    #[msg("Arithmetic overflow in reward calculation")]
    MathOverflow,
}
