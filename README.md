# Zone Runners

Zone Runners is an app built on top of existing DePIN infrastructure that turns it into a competitive game. Operators running Helium hotspots, Hivemapper dashcams, or GEODNET base stations race to claim geographic zones on Solana and earn $ZONE on top of whatever their hardware already earns. People without hardware can delegate $ZONE to operators they trust and earn a cut automatically.

The verification is the interesting part. Zone Runners doesn't use a trusted bridge or an oracle you have to believe. It reads the operator's existing on-chain data feeds directly. If the hardware was running, the proof is already there.

Live: `https://api.getfexr.com/v1/clubs/6`

---

## What you get out of it

**You run a Helium hotspot, Hivemapper dashcam, or GEODNET station**

Your hardware earns HNT, HONEY, or GEOD from its own network. Zone Runners adds a second layer of yield on top, paid in $ZONE, for coverage your hardware is already providing. You register your node, claim the zones it covers, and the data it's already publishing verifies the claim. No new setup, no extra hardware. One device, two income streams.

**You have capital but no hardware**

Browse the operator leaderboard, pick someone with strong coverage in an active season, and delegate $ZONE to them. You earn a share of their verified zone rewards automatically. When the season ends you get your principal back and can move to a different operator. You never touch a physical device.

**You're a Solana user with neither**

Every season you touch, whether as an operator, delegator, or both, writes permanently to your Contribution Passport on-chain. That record builds over time: zones verified, $ZONE delegated, seasons completed. It can't be bought retroactively, only earned. Protocols across Solana can already read it for airdrops, whitelist access, and credit scoring. Getting in early means a longer history than anyone who joins later.

---

## The problem it's solving

Helium, Hivemapper, and GEODNET each reward their operators in isolation. The operator running a hotspot in an underserved city gets the same HNT per hour as one in a saturated area. There's no incentive to expand into gaps, no way for capital to flow to the best operators without owning hardware yourself, and no shared identity layer that follows an operator across networks.

Zone Runners puts geographic competition and capital coordination on top of existing DePIN networks. It doesn't replace them.

---

## How a season works

Each season runs for a fixed window targeting one DePIN network. A reward pool of $ZONE is deposited before the season starts. Operators race to claim H3 hexagonal zones and verify them. When the season ends, anyone can settle it and rewards flow proportionally.

```
Season: Helium · 90 days · 100,000 $ZONE pool

  Operator A claims zone 617700169958293503 (Austin, TX)
    └── verify_zone_coverage reads Operator A's SnapshotBuffer from guage-commons
    └── finds 5 recent entries with quality_flags ≥ 1
    └── zone verified ✓

  Delegator X puts 5,000 $ZONE behind Operator A
    └── held in Operator A's vault PDA until season ends

  Season ends → settle_season() called by anyone
    └── Operator A earns: 100k × 0.70 × (A's zones / total zones)
    └── Delegator X earns: A's pool share × 0.30 × (5k / A's total delegation)
```

Rewards are claimed individually. Settlement is permissionless.

---

## Why zone verification can't be faked

This is worth understanding. Zone Runners doesn't trust operators. When `verify_zone_coverage` is called, the program reads the operator's `SnapshotBuffer` account directly from guage-commons storage. No CPI, no oracle call, no intermediary.

```rust
let data = ctx.accounts.snapshot_buffer.data.borrow();
let snapshot = SnapshotBufferView::try_from_slice(&data[8..])?;

// the facility on the snapshot must match the one registered on the claim
require_keys_eq!(snapshot.facility, claim.facility, ZoneError::FacilityMismatch);

// need real recent data: at least min_entries in the last 24h above the quality threshold
let recent = snapshot.entries.iter()
    .filter(|e| e.created_at > now - 86_400 && e.quality_flags >= min_quality_flags)
    .count();
require!(recent >= min_entries as usize, ZoneError::InsufficientCoverage);
```

If the hardware wasn't publishing data, there's nothing to read, and the zone doesn't verify. The proof is the data the hardware already produced, not a transaction the operator submitted.

---

## Contribution Passport

Every wallet gets one `ContributionPassport` PDA that updates automatically across seasons and networks.

| Tier | How you get there |
|------|------------------|
| Scout | Participate in any season |
| Runner | Verify at least 1 zone, or delegate at least 5,000 $ZONE total |
| Zone Lead | Verify 5+ zones across 2+ seasons |
| Pioneer | Verify 20+ zones, or 100,000+ $ZONE delegated total |

The passport is public and composable. Any Solana program can read it without permission from Zone Runners. It's designed to become useful collateral. The longer you hold a Pioneer passport with a deep history, the more other protocols will weight it.

---

## Reward math

```
Season pool = P

Operators split 70%:
  operator_i share = P × 0.70 × (zones_verified_i / total_zones_verified)

Delegators split 30%, routed through their operator:
  delegator_j share = (operator_i's 70% share × 0.30) × (delegation_j / operator_i_total_delegated)
```

---

## Integrating your DePIN hardware

Zone Runners reads data from [guage-commons](https://github.com/fexr/solana-contracts), a Solana program for managing physical infrastructure data feeds. To make your hardware verifiable:

1. Call `register_facility` on guage-commons with your node's metadata. This gives you a `Facility` pubkey that becomes the link between your hardware and your zone claims.

2. Publish data via `publish_snapshot` regularly. Each entry carries a `quality_flags` bitmask. Zone Runners checks that enough recent entries exceed a quality threshold. Set this to whatever makes sense for your network.

3. Use your `Facility` pubkey when calling `claim_zone`. Zone Runners will check the corresponding `SnapshotBuffer` on verification.

guage-commons devnet: `4Ch9vYQJyXtyZ7Swr9EMU9xaCtpZDckv4E1thjX7FZjW`

---

## TypeScript SDK

```typescript
import { ZoneRunnersClient, tierLabel } from "@zone-runners/sdk";

const client = new ZoneRunnersClient(provider, idl);

// check any wallet's passport
const passport = await client.getPassport(walletPublicKey);
console.log(tierLabel(passport.currentTier)); // "Runner"

// see what's been claimed this season
const claims = await client.getZoneClaims(seasonPda);
console.log(`${claims.filter(c => c.isVerified).length} zones verified`);

// build a claim TX, user signs it in their wallet
const tx = await client.buildClaimZoneTx(
  operatorPublicKey,
  seasonPda,
  617700169958293503n,
  facilityPublicKey,
);

// delegate without hardware
const delegateTx = await client.buildDelegateStakeTx(
  delegatorPublicKey,
  seasonPda,
  operatorPublicKey,
  5_000_000_000n, // 5,000 $ZONE
  zoneTokenMint,
);
```

---

## REST API

`https://api.getfexr.com/v1`

| Endpoint | |
|----------|-|
| `GET /clubs/{id}/epochs` | all seasons |
| `GET /clubs/{id}/epochs/current` | active season with live stats |
| `GET /clubs/{id}/epochs/{id}/leaderboard` | operators ranked by zones verified |
| `GET /clubs/{id}/depin/zones` | zone claims, filterable by operator / verified |
| `POST /clubs/{id}/depin/zones/claim` | unsigned claim TX |
| `POST /clubs/{id}/depin/zones/verify` | unsigned verify TX |
| `GET /clubs/{id}/depin/delegation` | operator pools |
| `POST /clubs/{id}/depin/delegation` | unsigned delegation TX |
| `GET /clubs/{id}/depin/passport` | your passport |
| `GET /clubs/{id}/depin/passport/{wallet}` | any wallet's passport |

---

## Building and deploying

```bash
git clone https://github.com/nidhinmahesh/zone-runners
cd zone-runners
yarn install
anchor build
anchor test
anchor deploy --provider.cluster devnet
```

Prerequisites: Anchor 0.29.0, Rust 1.89.0, Solana CLI, Node 18+

---

## Program accounts

| Account | Seeds | |
|---------|-------|-|
| `ZoneConfig` | `["zone-config", club_id]` | admin, token mint, oracle program |
| `Season` | `["season", zone_config, season_index]` | campaign window and pool |
| `ZoneClaim` | `["zone-claim", season, h3_index]` | territorial ownership of one cell |
| `OperatorVault` | `["op-vault", season, operator]` | stats and delegation total |
| `DelegationStake` | `["delegation", season, operator, delegator]` | one delegation record |
| `ContributionPassport` | `["passport", authority]` | cross-season identity |

---

## Deployed addresses

| | |
|-|-|
| Zone Runners (devnet) | `ZRuNrAtgJM4hG3YLtq6NmbHdtBqoNGbvRvbyLuLNoWf` |
| guage-commons (devnet) | `4Ch9vYQJyXtyZ7Swr9EMU9xaCtpZDckv4E1thjX7FZjW` |

---

## What's next

- Multi-network seasons where a single campaign spans Helium and GEODNET simultaneously, weighting zones by cross-network data quality
- Slash mechanics so false claims can be challenged on-chain
- A standard interface for other programs to query passport tier without a Zone Runners dependency
- Zone claim NFTs carrying verified coverage history

---

## License

Apache 2.0
