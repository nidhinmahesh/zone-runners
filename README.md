# Zone Runners

Zone Runners is an app built on top of existing DePIN infrastructure. It lets anyone pay in SOL to get trustless, on-chain proof that a geographic area has active coverage from real hardware. Operators running Helium hotspots, Hivemapper dashcams, or GEODNET base stations earn that SOL by proving their hardware was actually there and running.

No new token. No oracle you have to trust. The proof is the hardware's own data feed, read directly from the chain.

Live: `https://api.getfexr.com/v1/clubs/6`

---

## What you get out of it

**You run a Helium hotspot, Hivemapper dashcam, or GEODNET station**

Your hardware earns HNT, HONEY, or GEOD from its own network. Zone Runners adds a second income stream on top, paid in SOL. When a season has a bounty on a geographic area, you claim the zones your hardware covers, verify them, and earn a share of the SOL pool proportional to how many zones you proved. No new setup, no extra hardware. One device, two income streams.

**You need to know if coverage actually exists somewhere**

A telecom doing competitive analysis, a logistics company planning IoT deployment, a protocol that needs a geographic condition verified before executing, a researcher mapping real-world DePIN density. You deposit SOL into a season bounty targeting the area and network you care about. Operators with hardware in that area prove it, and you get an on-chain proof that can't be forged. Not an operator's word. Not a bridge you have to trust. The hardware's own data.

**You're a Solana user building on real-world data**

Every zone an operator verifies writes to their Contribution Passport on-chain. That record builds over time across seasons and networks. It can't be bought retroactively, only earned by running hardware that publishes real data. Any Solana program can read a Passport without permission from Zone Runners. Getting in early means a longer track record than anyone who joins later.

---

## The problem it's solving

There's no trustless way to answer "does active DePIN coverage exist at location X right now?" You either trust an operator's claim, trust a bridge, or trust an oracle. All of those have a point of failure.

Zone Runners removes the trust requirement. It reads the operator's existing data feeds from the chain directly, without CPI, without an intermediary. If the hardware was running and publishing data, the proof is already there.

---

## How a season works

A season is a coverage campaign targeting one DePIN network over a fixed time window. Anyone can deposit SOL to fund the bounty. Operators claim H3 hexagonal zones and verify them against their live data. When the season ends, SOL flows to operators proportional to verified zones.

```
Season: Helium · 90 days · 50 SOL bounty pool

  Coverage buyer deposits 50 SOL into the season
    └── looking for proof of Helium coverage across Austin, TX

  Operator A claims zone 617700169958293503 (Austin, TX)
    └── verify_zone_coverage reads Operator A's SnapshotBuffer from guage-commons
    └── finds 5 recent entries with quality_flags ≥ 1 in the last 24h
    └── zone verified, proof written on-chain ✓

  Operator B verifies 3 zones. Operator A verifies 7.

  Season ends → settle_season() called by anyone
    └── Operator A earns: 50 SOL × (7 / 10) = 35 SOL
    └── Operator B earns: 50 SOL × (3 / 10) = 15 SOL
```

Settlement is permissionless. Rewards are claimed individually.

---

## Why coverage proofs can't be faked

When `verify_zone_coverage` is called, the program reads the operator's `SnapshotBuffer` account directly from guage-commons storage. No CPI, no oracle call, no intermediary.

```rust
let data = ctx.accounts.snapshot_buffer.data.borrow();
let snapshot = SnapshotBufferView::try_from_slice(&data[8..])?;

// the snapshot must belong to the facility the operator registered
require_keys_eq!(snapshot.facility, claim.facility, ZoneError::FacilityMismatch);

// need real recent data: at least min_entries in the last 24h above quality threshold
let recent = snapshot.entries.iter()
    .filter(|e| e.created_at > now - 86_400 && e.quality_flags >= min_quality_flags)
    .count();
require!(recent >= min_entries as usize, ZoneError::InsufficientCoverage);
```

The `snapshot_buffer` account's owner is verified against the guage-commons program ID before deserialization. An operator can't pass in a fake account — Solana enforces account ownership. If the hardware wasn't publishing data, there's nothing to read, and the zone doesn't verify.

---

## Contribution Passport

Every operator wallet gets one `ContributionPassport` PDA that updates automatically across seasons and networks.

| Tier | How you get there |
|------|------------------|
| Scout | Participate in any season |
| Runner | Verify at least 1 zone |
| Zone Lead | Verify 5+ zones across 2+ seasons |
| Pioneer | Verify 20+ zones |

The passport is public and composable. Any Solana program can read it without permission from Zone Runners. A Pioneer passport with a long history across multiple networks becomes a meaningful credential for airdrops, whitelist access, and credit scoring by other protocols.

---

## Reward math

```
Season bounty pool = P (in SOL lamports)

Each operator's share:
  operator_i = P × (zones_verified_i / total_zones_verified)
```

100% of the pool goes to operators who proved real coverage. No platform fee, no token middleman.

---

## Integrating your DePIN hardware

Zone Runners reads data from [guage-commons](https://github.com/fexr/solana-contracts), a Solana program for managing physical infrastructure data feeds.

1. Call `register_facility` on guage-commons with your node's metadata. This gives you a `Facility` pubkey that becomes the link between your hardware and your zone claims.

2. Publish data via `publish_snapshot` regularly. Each entry carries a `quality_flags` bitmask. Zone Runners checks that enough recent entries exceed a quality threshold.

3. Use your `Facility` pubkey when calling `claim_zone`. Zone Runners will check the corresponding `SnapshotBuffer` on verification.

guage-commons devnet: `4Ch9vYQJyXtyZ7Swr9EMU9xaCtpZDckv4E1thjX7FZjW`

---

## TypeScript SDK

```typescript
import { ZoneRunnersClient } from "@zone-runners/sdk";

const client = new ZoneRunnersClient(provider, idl);

// check any wallet's passport
const passport = await client.getPassport(walletPublicKey);
console.log(passport.currentTier); // 2 = ZoneLead

// see what's been verified this season
const claims = await client.getZoneClaims(seasonPda);
console.log(`${claims.filter(c => c.isVerified).length} zones verified`);

// build a claim TX, user signs it in their wallet
const tx = await client.buildClaimZoneTx(
  operatorPublicKey,
  seasonPda,
  617700169958293503n,
  facilityPublicKey,
);

// fund a coverage bounty with SOL
const fundTx = await client.buildFundSeasonTx(
  funderPublicKey,
  seasonPda,
  0.5 * 1e9, // 0.5 SOL in lamports
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
| `GET /clubs/{id}/depin/zones` | zone claims, filterable by operator or verified status |
| `POST /clubs/{id}/depin/zones/claim` | unsigned claim TX |
| `POST /clubs/{id}/depin/zones/verify` | unsigned verify TX |
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
| `ZoneConfig` | `["zone-config", club_id]` | admin and oracle program |
| `Season` | `["season", zone_config, season_index]` | campaign window and SOL bounty |
| `ZoneClaim` | `["zone-claim", season, h3_index]` | territorial ownership of one cell |
| `OperatorVault` | `["op-vault", season, operator]` | operator stats per season |
| `ContributionPassport` | `["passport", authority]` | cross-season identity |

---

## Deployed addresses

| | |
|-|-|
| Zone Runners (devnet) | `ZRuNrAtgJM4hG3YLtq6NmbHdtBqoNGbvRvbyLuLNoWf` |
| guage-commons (devnet) | `4Ch9vYQJyXtyZ7Swr9EMU9xaCtpZDckv4E1thjX7FZjW` |

---

## What's next

- Multi-network seasons where a single campaign spans Helium and GEODNET simultaneously
- Slash mechanics so false claims can be challenged on-chain
- A standard interface for other programs to query Passport tier without a Zone Runners dependency
- Direct API access for coverage proofs, so non-Solana applications can pay for verified data

---

## License

Apache 2.0
