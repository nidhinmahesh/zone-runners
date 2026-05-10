# Zone Runners

**A competitive coordination layer for DePIN networks on Solana.**

DePIN hardware operators — running Helium hotspots, Hivemapper dashcams, GEODNET base stations — already generate real-world data on-chain. Zone Runners turns that data into a territorial game: operators race to claim geographic zones, prove coverage with verifiable on-chain proof, and earn yield from a shared pool. Capital holders delegate $ZONE to back the operators they believe in and earn automatically without owning a single device.

No synthetic proofs. No trusted oracle bridges. The proof is the data already on-chain.

---

## Who this is for

**DePIN node operators** — Earn additional yield on top of your existing network rewards by racing to claim and verify zones in active seasons. Your hardware's on-chain data is the proof.

**Capital holders** — Delegate $ZONE to operators you believe in. Their zone verification performance determines your yield. No hardware required, no manual claiming.

**Protocol developers** — Zone Runners is an open coordination primitive. Any project deploying physical infrastructure on Solana can run seasons, issue zones, and reward operators through the same program.

---

## The problem

DePIN networks reward operators for contributing infrastructure, but those rewards exist in silos. A Helium operator earns HNT. A Hivemapper driver earns HONEY. There is no cross-network identity layer, no competitive incentive to expand into uncovered areas, and no easy way for capital to flow to the best operators without owning hardware.

Zone Runners solves all three:
- It creates a **competitive geographic game** that incentivises operators to expand coverage into uncovered zones
- It gives **capital holders a yield-bearing route** into DePIN without hardware
- It builds a **portable on-chain identity** (Contribution Passport) that operators carry across networks and seasons

---

## How it works

The world is divided into [H3 hexagonal cells](https://h3geo.org/) at configurable resolution. Each season targets one DePIN network. Operators claim cells, prove coverage with real data, and earn from the pool. Delegators back operators and earn a share automatically.

```
Season starts (network: helium, h3_resolution: 7, pool: 100,000 $ZONE)
    │
    ├── Operator A: claim_zone(h3_index, facility_pubkey)
    │       └── verify_zone_coverage(h3_index, snapshot_buffer)
    │               └── reads guage-commons SnapshotBuffer on-chain
    │               └── requires ≥ 3 recent entries with quality_flags ≥ 1
    │               └── zone marked verified ✓
    │
    ├── Delegator X: delegate_stake(operator_A, 5000 $ZONE)
    │       └── tokens held in operator vault PDA
    │       └── earns proportional share of operator A's reward
    │
    └── Season ends → settle_season() (permissionless)
            └── Operator A claims: pool × 0.70 × (zones_verified_A / total)
            └── Delegator X claims: operator_A_pool × 0.30 × (5000 / operator_A_total)
```

### The oracle layer: guage-commons

Zone Runners does not ship its own data oracle. It reads directly from [guage-commons](https://github.com/fexr/solana-contracts), a deployed Solana program that manages physical infrastructure data feeds:

- **Facility** — a registered DePIN node (hotspot, dashcam, base station)
- **Metric** — a data feed from that node (coverage, signal quality, GPS accuracy)
- **SnapshotBuffer** — a 64-entry ring buffer of time-series readings from that metric

When an operator calls `verify_zone_coverage`, the Zone Runners program reads the operator's `SnapshotBuffer` account **directly** — no CPI, no bridge, no trust assumption — and checks for recent entries with sufficient quality. The zone is verified if and only if the data is there.

```rust
// verify_zone_coverage — the core of the protocol
let data = ctx.accounts.snapshot_buffer.data.borrow();
let snapshot = SnapshotBufferView::try_from_slice(&data[8..])?; // skip Anchor discriminator

// Facility on the snapshot must match what the operator registered
require_keys_eq!(snapshot.facility, claim.facility, ZoneError::FacilityMismatch);

// Real recent data required: >= min_entries in last 24h with quality_flags >= threshold
let recent = snapshot.entries.iter()
    .filter(|e| e.created_at > now - 86_400 && e.quality_flags >= min_quality_flags)
    .count();
require!(recent >= min_entries as usize, ZoneError::InsufficientCoverage);
```

This means **a zone cannot be gamed by submitting fake transactions**. The proof is the data that the operator's hardware has already been publishing to guage-commons. If the data is not there, the zone does not verify.

---

## Contribution Passport

Every wallet has a single `ContributionPassport` PDA that accumulates stats across all seasons and networks. It upgrades automatically when thresholds are crossed — no manual action required.

| Tier | How to reach it |
|------|----------------|
| **Scout** | Join any season |
| **Runner** | Verify ≥ 1 zone OR delegate ≥ 5,000 $ZONE total |
| **Zone Lead** | Verify ≥ 5 zones AND participate in ≥ 2 seasons |
| **Pioneer** | Verify ≥ 20 zones OR delegate ≥ 100,000 $ZONE total |

The passport is a public, immutable, composable record. Any protocol on Solana can read it to gate access, assign roles, determine credit limits, or issue airdrops based on verified DePIN contribution history — without asking Zone Runners for permission.

---

## Reward distribution

```
Season pool = P  (funded in $ZONE before season starts)

Operator share (70% of pool):
    operator_i = P × 0.70 × (zones_verified_i / total_zones_verified_in_season)

Delegator share (30% of pool, distributed through operators):
    delegator_j backing operator_i =
        (operator_i_pool_share × 0.30) × (delegation_j / total_delegated_to_operator_i)
```

Settlement is **permissionless** — anyone can call `settle_season()` after `end_ts`. Reward claims are individual, so operators and delegators claim at their own pace.

---

## Program accounts

| Account | PDA seeds | Description |
|---------|-----------|-------------|
| `ZoneConfig` | `["zone-config", club_id_le]` | Protocol config for a deployment: admin, token mint, oracle program |
| `Season` | `["season", zone_config, season_index_be]` | Campaign: network, H3 resolution, window, reward pool |
| `ZoneClaim` | `["zone-claim", season, h3_index_be]` | First-claim ownership of an H3 cell |
| `OperatorVault` | `["op-vault", season, operator]` | Per-operator stats and delegation total |
| `DelegationStake` | `["delegation", season, operator, delegator]` | An individual delegation record |
| `ContributionPassport` | `["passport", authority]` | Cross-season on-chain identity |

Season vault token account: `["season-token-vault", season]`  
Operator vault token account: `["op-token-vault", season, operator]`

---

## Getting started

**Prerequisites:** Anchor CLI 0.29.0 · Rust 1.89.0 · Solana CLI · Node 18+

```bash
git clone https://github.com/fexr/zone-runners
cd zone-runners
yarn install

# Build the program
anchor build

# Run tests on localnet
anchor test

# Deploy to devnet
anchor deploy --provider.cluster devnet
```

### TypeScript SDK

```typescript
import { ZoneRunnersClient, tierLabel } from "@zone-runners/sdk";

const client = new ZoneRunnersClient(provider, idl);

// Read any wallet's Contribution Passport
const passport = await client.getPassport(walletPublicKey);
console.log(`${walletPublicKey.toBase58()} is a ${tierLabel(passport.currentTier)}`);
// → "HN7cAB...WrH is a Runner"

// Get all zone claims for the current season
const claims = await client.getZoneClaims(seasonPda);
const verified = claims.filter(c => c.isVerified);
console.log(`${verified.length} / ${claims.length} zones verified`);

// Build an unsigned claim transaction (user signs in wallet)
const claimTx = await client.buildClaimZoneTx(
  operatorPublicKey,
  seasonPda,
  617700169958293503n,  // H3 cell index (res-7 cell in Austin, TX)
  facilityPublicKey,
);
// → base64-encoded unsigned transaction, send to wallet adapter

// Build a verify transaction once the SnapshotBuffer has data
const verifyTx = await client.buildVerifyCoverageTx(
  operatorPublicKey,
  seasonPda,
  617700169958293503n,
  snapshotBufferPublicKey, // guage-commons SnapshotBuffer for this facility
);

// Build a delegation transaction (no hardware needed)
const delegateTx = await client.buildDelegateStakeTx(
  delegatorPublicKey,
  seasonPda,
  operatorPublicKey,
  5_000_000_000n, // 5,000 $ZONE (6 decimals)
  zoneTokenMint,
);
```

---

## Integrating with guage-commons

If you operate DePIN hardware and want your infrastructure to be verifiable by Zone Runners:

1. **Register your facility** on guage-commons using `register_facility`. Your facility pubkey becomes the link between your hardware and your zone claims.

2. **Publish snapshots** via `publish_snapshot` on a regular cadence. Each snapshot entry needs a `quality_flags` value indicating data quality. Zone Runners requires a minimum number of recent entries above a quality threshold to verify a zone.

3. **Claim zones** in Zone Runners using your facility pubkey. When you call `verify_zone_coverage`, Zone Runners reads your `SnapshotBuffer` directly and verifies on-chain that your hardware was active.

guage-commons is deployed at `4Ch9vYQJyXtyZ7Swr9EMU9xaCtpZDckv4E1thjX7FZjW` on Solana devnet.

---

## REST API

A hosted REST API is available for applications that want to read Zone Runners state without running a Solana node.

**Base URL:** `https://api.getfexr.com/v1`

| Endpoint | Description |
|----------|-------------|
| `GET /clubs/{id}/epochs` | List all seasons for a deployment |
| `GET /clubs/{id}/epochs/current` | Active season with live stats |
| `GET /clubs/{id}/epochs/{epoch_id}/leaderboard` | Ranked operators by zones verified |
| `GET /clubs/{id}/depin/zones` | All zone claims, filterable by operator and verified status |
| `POST /clubs/{id}/depin/zones/claim` | Get unsigned claim TX for a given H3 index |
| `POST /clubs/{id}/depin/zones/verify` | Get unsigned verify TX for a SnapshotBuffer |
| `GET /clubs/{id}/depin/delegation` | Operator delegation pools ranked by $ZONE |
| `POST /clubs/{id}/depin/delegation` | Get unsigned delegation TX |
| `GET /clubs/{id}/depin/passport` | Authenticated user's Contribution Passport |
| `GET /clubs/{id}/depin/passport/{wallet}` | Any wallet's public passport |

---

## Repository structure

```
zone-runners/
├── programs/zone-runners/
│   └── src/
│       ├── lib.rs                 program entry, instruction dispatch
│       ├── state.rs               all account structs + tier logic
│       ├── errors.rs
│       └── instructions/
│           ├── initialize.rs      initialize_zone_config
│           ├── season.rs          create_season, fund_season_pool
│           ├── zone.rs            claim_zone, verify_zone_coverage
│           ├── delegation.rs      delegate_stake, undelegate_stake
│           ├── rewards.rs         settle_season, claim_*_rewards
│           └── passport.rs        update_passport, record_season_participation
│
├── app/src/                       TypeScript SDK
│   ├── client.ts                  ZoneRunnersClient — PDA helpers + TX builders
│   ├── types.ts                   TypeScript types + tier helpers
│   └── index.ts
│
└── tests/
    └── zone-runners.ts            Anchor integration tests
```

---

## Deployed addresses

| | Program ID |
|--|-----------|
| Zone Runners (devnet) | `ZRuNrAtgJM4hG3YLtq6NmbHdtBqoNGbvRvbyLuLNoWf` |
| guage-commons oracle (devnet) | `4Ch9vYQJyXtyZ7Swr9EMU9xaCtpZDckv4E1thjX7FZjW` |

---

## What comes next

- **Multi-network seasons** — a single season spanning Helium + GEODNET simultaneously, with zone weights based on data quality across both networks
- **Slash mechanics** — operators who submit false zone claims can be challenged; successful challenges earn the challenger a share of the operator's staked collateral
- **Passport composability** — standard interface for other Solana programs to query passport tier without importing Zone Runners as a dependency
- **Zone NFTs** — verified zone claims as tradeable NFTs that carry historical performance data

---

## License

Apache 2.0
