# Zone Runners

Zone Runners is a stake-and-challenge game built on top of existing DePIN infrastructure. Operators stake SOL to claim H3 geographic zones and prove their hardware covers them. Any other operator with better hardware coverage can challenge and take a zone by proving it on-chain. The challenger wins the defender's stake. Failed challenges cost only gas.

No new token. No oracle you have to trust. The proof is the hardware's own on-chain data feed, read directly.

Live: `https://api.getfexr.com/v1/clubs/6`

---

## What you get out of it

**You run a Helium hotspot, Hivemapper dashcam, or GEODNET station**

Stake SOL to claim the zones your hardware covers, verify them using your existing DePIN oracle data feed, and hold them. As long as your hardware keeps publishing quality data, nobody can take your zones. If a challenger shows up with a weaker signal, their challenge reverts on-chain and they only lose gas. You earn from successful defenses and from the season bounty pool. One device, multiple income streams.

**You want to compete without owning the best hardware in an area**

Find zones held by operators with low coverage scores. Put up matching SOL, point to your own SnapshotBuffer, and if your recent entry count beats theirs, the zone transfers to you instantly and you earn their stake (minus 5% protocol fee). The on-chain comparison is deterministic and instant. You know before submitting whether you'll win.

**You're a Solana user building on real-world data**

Every zone a wallet claims and defends writes to their Contribution Passport on-chain. That record accumulates across seasons and networks. A Pioneer passport with a long history of defended zones across multiple networks is a genuine credential that other protocols can read without permission. Getting in early builds history nobody who joins later can replicate.

---

## The problem it's solving

DePIN networks reward operators individually but have no competitive layer that pushes coverage to where it's most needed. The operator with mediocre hardware in a dense city earns the same as one in a gap nobody else covers. There's no mechanism that rewards quality over presence.

Zone Runners adds geographic competition on top. Operators with genuinely better hardware win zones and earn SOL from operators with worse coverage. The result is coverage quality improving over time without any central coordinator.

---

## How a season works

A season defines a time window targeting one DePIN network. Operators race to claim and defend zones. A bounty pool of SOL is distributed proportionally to verified zones at season end. Stakes are withdrawable after settlement.

```
Season: Helium · 90 days · 50 SOL bounty pool

  Operator A stakes 0.1 SOL to claim zone 617700169958293503 (Austin, TX)
    └── verify_zone_coverage reads Operator A's SnapshotBuffer from DePIN oracle
    └── finds 5 recent entries with quality_flags ≥ 1
    └── zone verified. coverage_score = 5 ✓

  Operator B challenges: coverage_score = 9
    └── B deposits 0.1 SOL (matching A's stake)
    └── on-chain: 9 > 5 → B wins
    └── B earns 0.095 SOL (A's stake minus 5% fee)
    └── B's 0.1 SOL becomes the new zone bond

  Season ends → settle_season() called by anyone
    └── Both operators earn from 50 SOL pool proportional to zones_verified
    └── Each operator withdraws their remaining zone stakes
```

---

## Why zone verification can't be faked

When `verify_zone_coverage` or `challenge_zone` is called, the program reads the operator's `SnapshotBuffer` account directly from DePIN oracle storage. No CPI, no oracle, no intermediary.

```rust
let data = ctx.accounts.snapshot_buffer.data.borrow();
let snapshot = SnapshotBufferView::try_from_slice(&data[8..])?;

// snapshot must belong to the facility the operator registered
require_keys_eq!(snapshot.facility, claim.facility, ZoneError::FacilityMismatch);

// count recent high-quality entries in the last 24h
let recent = snapshot.entries.iter()
    .filter(|e| e.created_at > now - 86_400 && e.quality_flags >= min_quality_flags)
    .count();
```

The `snapshot_buffer` account ownership is verified against the DePIN oracle program ID before deserialization. An operator can't pass in a fake account — Solana enforces account ownership.

---

## Challenge mechanic

```
Before:  ZoneClaim PDA holds defender_stake (e.g. 0.1 SOL), coverage_score = 5

challenge_zone called with challenger's SnapshotBuffer:
  1. Challenger deposits 0.1 SOL → ZoneClaim PDA (atomic, reverts if step 2 fails)
  2. On-chain: challenger_score (9) > coverage_score (5) ✓
  3. Transfer 0.095 SOL to challenger (95% of defender stake)
  4. Transfer 0.005 SOL to admin (5% fee)
  5. Challenger's 0.1 SOL stays in PDA as new bond
  6. ZoneClaim.operator = challenger, coverage_score = 9

If challenger_score ≤ coverage_score:
  → require! fails → TX reverts → challenger deposit undone → costs only gas
```

---

## Contribution Passport

Every wallet gets one `ContributionPassport` PDA that accumulates across seasons and networks.

| Tier | How you get there |
|------|------------------|
| Scout | Participate in any season |
| Runner | Verify at least 1 zone |
| Zone Lead | Verify 5+ zones across 2+ seasons |
| Pioneer | Verify 20+ zones |

Passport is public and composable. Any Solana program can read it without permission.

---

## Reward math

```
Season bounty pool = P (SOL)

operator_i share = P × (zones_verified_i / total_zones_verified)

Plus: challenger earnings from each successful challenge = defender_stake × 0.95
```

---

## Integrating your DePIN hardware

Zone Runners reads data from [DePIN oracle](https://github.com/fexr/solana-contracts).

1. Call `register_facility` on DePIN oracle with your node's metadata.
2. Publish data via `publish_snapshot` regularly. Each entry carries a `quality_flags` bitmask.
3. Use your `Facility` pubkey when calling `claim_zone`.

DePIN oracle devnet: `4Ch9vYQJyXtyZ7Swr9EMU9xaCtpZDckv4E1thjX7FZjW`

---

## TypeScript SDK

```typescript
import { ZoneRunnersClient } from "@zone-runners/sdk";

const client = new ZoneRunnersClient(provider, idl);

// check any wallet's passport
const passport = await client.getPassport(walletPublicKey);
console.log(passport.currentTier); // 2 = ZoneLead

// see zone claims with coverage scores
const claims = await client.getZoneClaims(seasonPda);
claims.forEach(c => console.log(`zone ${c.h3Index}: score=${c.coverageScore}, stake=${c.stakeLamports}`));

// claim a zone with SOL stake
const tx = await client.buildClaimZoneTx(
  operatorPublicKey,
  seasonPda,
  617700169958293503n,
  facilityPublicKey,
  10_000_000n, // 0.01 SOL stake
);

// challenge a zone
const challengeTx = await client.buildChallengeZoneTx(
  challengerPublicKey,
  seasonPda,
  617700169958293503n,
  challengerFacilityPublicKey,
  snapshotBufferPublicKey,
);
```

---

## REST API

`https://api.getfexr.com/v1`

| Endpoint | |
|----------|-|
| `GET /clubs/{id}/epochs` | all seasons |
| `GET /clubs/{id}/epochs/current` | active season with live stats |
| `GET /clubs/{id}/leaderboard` | top operators ranked by score |
| `GET /clubs/{id}/depin/zones` | zone claims with stake and coverage score |
| `POST /clubs/{id}/depin/zones/claim` | unsigned claim TX (includes stake_lamports) |
| `POST /clubs/{id}/depin/zones/verify` | unsigned verify TX |
| `POST /clubs/{id}/depin/zones/challenge` | unsigned challenge TX |
| `POST /clubs/{id}/depin/zones/withdraw-stake` | unsigned withdraw TX (after season settles) |
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
| `Season` | `["season", zone_config, season_index]` | campaign window and SOL bounty pool |
| `ZoneClaim` | `["zone-claim", season, h3_index]` | zone ownership, stake, and coverage score |
| `OperatorVault` | `["op-vault", season, operator]` | operator stats per season |
| `ContributionPassport` | `["passport", authority]` | cross-season identity |

---

## Deployed addresses

| | |
|-|-|
| Zone Runners (devnet) | `ZRuNrAtgJM4hG3YLtq6NmbHdtBqoNGbvRvbyLuLNoWf` |
| DePIN oracle (devnet) | `4Ch9vYQJyXtyZ7Swr9EMU9xaCtpZDckv4E1thjX7FZjW` |

---

## What's next

- Multi-network seasons spanning Helium and GEODNET simultaneously
- Slash mechanics for false claims challenged after the fact
- A standard interface for other programs to query Passport tier
- Zone claim history as compressed NFT collectibles

---

## License

Apache 2.0
