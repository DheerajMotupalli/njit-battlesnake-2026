# Changelog — Ouroboros Battlesnake

## 2026-03-07

### Anti-Circling & Persistent Food Priority (v1.4.0)

**Problem:** Snake gets trapped in tight 2×2 circular loops for dozens of turns and never breaks out, missing food opportunities. In one game, Ouroboros stayed at length 4 for 65+ turns while opponents grew to length 11. In another, the snake circled at turns 10-15 and 20-24 before eventually finding food.

**Root Cause (three issues):**

1. **No penalty for tiny territory:** A 2×2 loop gives only ~4 cells of territory on a 121-cell board. The evaluation saw this as "safe" because tail was reachable (+30) and tail distance was short (+21). Nothing punished the snake for self-trapping.

2. **Mid/late food priority gated on low health:** After the opening, food proximity was only rewarded when health dropped below 50. A snake at health 80+ had almost zero incentive to seek food — just +2/step towards nearest food. This meant the "safe loop" almost always beat "move toward food."

3. **No length reward after early game:** The absolute length bonus (rewarding eating food) stopped after turn 20. Mid/late game positions got no direct credit for having eaten food.

**Fixes applied (eval.rs):**

- **Anti-circling territory penalty:** When territory < max(length × 1.5, 8) cells, apply -25 points per deficit cell. A length-4 snake in a 2×2 loop (4 cells) gets -100 penalty, overwhelming the tail-safety bonus.

- **Always-on food priority:** Mid/late game food seeking no longer waits for health < 50:
    - Shorter than any opponent → +15/step closer, +30 bonus at distance ≤ 2
    - Health < 80 → scaled urgency from +4 to +16 per step
    - Health ≥ 80 → +4/step (doubled from v1.3.0's +2)

- **Absolute length bonus in all phases:** Opening +30/length, early +25/length, mid/late +10/length. This ensures eating food gives a tangible score advantage throughout the game.

**Files changed:** `src/eval.rs`

---

### Opening Overhaul — Eat First, Think Later (v1.3.0)

**Problem:** Snake died on turn 3 by reversing into its own neck. The body at `(1,8)` was marked with TTL=1, and `is_cell_safe` treats `ttl <= 1` as passable, so the snake walked right back into itself.

**Root Cause (two issues):**

1. **TTL off-by-one in `board.rs` `from_game_state()`** — Formula `(body_len - 1 - bi)` gave the neck a TTL of 1 for a 3-segment snake, making it appear safe to walk into.
2. **No reverse-direction guard in `safe_moves()`** — Nothing prevented the snake from choosing the direction back into its own neck.

**Fixes applied:**

- **`types.rs`**: Added `Direction::opposite()` method.
- **`board.rs` `from_game_state()`**: Changed TTL formula from `(body_len - 1 - bi)` to `(body_len - bi)`. Also added logic to keep the highest TTL when body segments overlap (stacked after eating).
- **`board.rs` `safe_moves()`**: Computes the head→neck direction and explicitly excludes it from candidate moves, preventing reversal regardless of TTL edge cases.

---

### Evaluation Rebalance: Early-Game Food Priority (v1.2.0)

**Problem:** In two 4-player test matches, the snake ignored nearby food (2 steps away) on turns 0–2. All three opponents ate immediately and gained a length advantage by turn 2 that persisted the entire game.

**Analysis of two replays (`Ouroboros_2026-03-07T19-04-35.json`, `Ouroboros_2026-03-07T19-08-17.json`):**

| Aspect                 | Pros                                                                  | Cons                                                                                   |
| ---------------------- | --------------------------------------------------------------------- | -------------------------------------------------------------------------------------- |
| Survival               | No more self-collision deaths after the reversal fix                  | —                                                                                      |
| Spatial awareness      | Avoided walls, didn't get trapped                                     | —                                                                                      |
| Head-to-head avoidance | Correctly dodged when opponent was adjacent with equal/greater length | —                                                                                      |
| Response time          | 35–56ms latency, well within 500ms timeout                            | —                                                                                      |
| Food seeking           | —                                                                     | Ignored food at (2,0) and (6,10) within 2 steps of spawn                               |
| Growth                 | —                                                                     | Still length 3 after 6 turns while all opponents were length 4+                        |
| Territory bias         | —                                                                     | 400-point territory weight pulled snake toward open space instead of nearby food       |
| Center pull            | —                                                                     | Center control bonus actively pushed snake away from corner/edge food                  |
| Length penalty         | —                                                                     | Being shorter than all opponents only cost `length_diff * 8.0` per opponent — too mild |

**Fixes applied to `eval.rs`:**

1. **Game-phase awareness** — Added `is_early_game` flag (turn < 15) and `shorter_than_all`/`shorter_than_any` checks.
2. **Territory weight reduced early** — 250 in early game vs 400 mid/late game, so food signals aren't drowned out.
3. **Food proximity always matters early** — No longer gated behind `health < 40`. Early game + shorter: `(12 - dist) * 18` bonus. Early game + equal: `(10 - dist) * 10`. Mid-game: urgency threshold raised to `health < 50`.
4. **Reachable food count doubled early** — Weight 10 in early game vs 5 later.
5. **Length penalty increased early** — `length_diff * 18` in early game vs `10` later. Extra −40 penalty for being shorter than ALL opponents.
6. **Center pull reduced early** — Weight 5 in early game vs 15 later, so corner food isn't penalised.

---

### Opening Overhaul: Eat First, Think Later (v1.3.0)

**Problem:** Replay `Ouroboros_2026-03-07T19-14-24.json` — snake STILL never ate food in 14 turns. Spawned at (9,1) with food at (10,2) just 2 steps away, moved DOWN away from it on turn 1. Passed within 1 step of food at (5,5) on turn 9 and moved away again. Ended cornered at (4,0) with health 86 while all opponents were length 4.

**Root Causes (three issues):**

1. **TTL inconsistency in `apply_moves()`** — Used `body_len - 1 - bi` formula while `from_game_state()` used `body_len - bi`. This made the search tree inaccurate at deeper depths, with cells appearing passable sooner than they should.
2. **v1.2.0 food weights still too weak** — Territory at 250 weight and tail-reach at ±30/40 pts still overwhelmed the food proximity bonus of 18 pts/step. The search preferred moves that preserved territory over moves that led to food.
3. **Paranoid search opponent model too hostile** — Equal-length opponents at distance 8+ were modeled as chasing us, making the search overly defensive even when opponents were nowhere near.

**Fixes applied:**

- **`board.rs` `apply_moves()`**: Fixed TTL formula from `(body_len - 1 - bi)` to `(body_len - bi)`, consistent with `from_game_state()`.
- **`eval.rs`**: Added `is_opening` phase (turn < 5) on top of `is_early_game` (turn < 20):
    - Territory weight: 80 opening / 200 early / 400 late
    - Food proximity: +35 pts/step closer in opening (was +18), +100 bonus for food 1 step away, +50 for 2 steps
    - Absolute length bonus: +25 per body segment in early game (directly rewards eating)
    - Length shortfall penalty: 25/step in opening, 18 early, 10 late
    - Shorter-than-all penalty: −60 opening / −50 later
    - Tail reachability: ±10 in opening (was ±30/40), so it can't override food signals
    - Center pull: 0 in opening, 10 early, 15 late
- **`search.rs` `paranoid_min()`**: Added early-game distance check — opponents > 4 steps away in the first 10 turns are modeled as moving toward center (realistic opener) instead of chasing us.
