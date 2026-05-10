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
    #[msg("Zone is not verified; cannot be challenged or rewarded")]
    ZoneNotVerified,
    #[msg("The snapshot_buffer is not owned by the expected guage-commons program")]
    InvalidSnapshotOwner,
    #[msg("The snapshot_buffer facility does not match the zone claim facility")]
    FacilityMismatch,
    #[msg("Not enough recent high-quality snapshots to verify this zone")]
    InsufficientCoverage,
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
    #[msg("Challenger coverage score must strictly exceed the current owner's score")]
    InsufficientCoverageToChallenge,
    #[msg("Cannot challenge your own zone")]
    CannotChallengeSelf,
    #[msg("Stake amount is below the minimum required (0.01 SOL)")]
    StakeBelowMinimum,
    #[msg("Zone stake has already been withdrawn")]
    StakeAlreadyWithdrawn,
    #[msg("Season must be settled before withdrawing stake")]
    SeasonNotSettledForWithdraw,
}
