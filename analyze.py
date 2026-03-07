#!/usr/bin/env python3
"""
analyze.py — Analyze Battlesnake tournament results.

Reads NDJSON game files produced by the Battlesnake CLI and produces
a comprehensive statistical summary.

Usage:
    python3 analyze.py tournament/results/
    python3 analyze.py tournament/results/ --json    # machine-readable output
"""

import json
import os
import sys
from collections import defaultdict
from pathlib import Path


def parse_game(filepath: str) -> dict | None:
    """Parse a single game NDJSON file and extract key stats."""
    try:
        with open(filepath) as f:
            lines = f.readlines()
    except (IOError, OSError):
        return None

    if len(lines) < 3:
        return None

    try:
        game_info = json.loads(lines[0])   # Game config
        turn0 = json.loads(lines[1])       # Turn 0 state
        result = json.loads(lines[-1])     # Winner info
    except json.JSONDecodeError:
        return None

    # Get snake roster from turn 0
    snakes_t0 = {s["id"]: s["name"] for s in turn0["board"]["snakes"]}

    # Find each snake's last alive state by scanning backwards
    snake_stats = {}
    total_turns = 0

    # Parse intermediate turns to track when each snake died
    # and their max length
    alive_tracker = {sid: True for sid in snakes_t0}
    last_alive_turn = {sid: 0 for sid in snakes_t0}
    max_length = {sid: 3 for sid in snakes_t0}
    death_turn = {}
    death_cause = {}

    # Track previous turn state for death cause inference
    prev_board_snakes = {}

    for line_idx in range(1, len(lines) - 1):  # Skip game info + result
        try:
            turn_data = json.loads(lines[line_idx])
        except json.JSONDecodeError:
            continue

        turn = turn_data.get("turn", 0)
        total_turns = max(total_turns, turn)
        board_snakes = {s["id"]: s for s in turn_data["board"]["snakes"]}
        board_w = turn_data["board"]["width"]
        board_h = turn_data["board"]["height"]

        # Track alive snakes and max length
        for sid, name in snakes_t0.items():
            if sid in board_snakes:
                s = board_snakes[sid]
                length = s.get("length", len(s.get("body", [])))
                max_length[sid] = max(max_length[sid], length)
                if s.get("health", 0) > 0 and len(s.get("body", [])) > 0:
                    last_alive_turn[sid] = turn

        # Detect snakes that disappeared this turn
        if prev_board_snakes:
            disappeared_this_turn = set()
            for sid in snakes_t0:
                if sid in prev_board_snakes and sid not in board_snakes:
                    if alive_tracker.get(sid, False):
                        alive_tracker[sid] = False
                        death_turn[sid] = turn
                        disappeared_this_turn.add(sid)

            # Determine death causes for disappeared snakes
            for sid in disappeared_this_turn:
                prev_snake = prev_board_snakes[sid]
                prev_health = prev_snake.get("health", 0)

                # Starvation: health was 1 last turn (drops to 0 this turn)
                if prev_health <= 1:
                    death_cause[sid] = "starvation"
                    continue

                # Check 'you' field - if the dead snake is 'you', we get
                # its final position (may be out of bounds)
                you = turn_data.get("you", {})
                if you.get("id") == sid:
                    head = you.get("head", {})
                    hx, hy = head.get("x", -1), head.get("y", -1)
                    if hx < 0 or hx >= board_w or hy < 0 or hy >= board_h:
                        death_cause[sid] = "wall"
                        continue

                # Head-to-head: multiple snakes disappeared same turn
                if len(disappeared_this_turn) > 1:
                    death_cause[sid] = "head-to-head"
                    continue

                # Default: collision (body or head-to-head with larger snake)
                death_cause[sid] = "collision"

        prev_board_snakes = board_snakes

    # Build per-snake stats
    winner_id = result.get("winnerId", "")
    winner_name = result.get("winnerName", "")
    is_draw = result.get("isDraw", False)

    for sid, name in snakes_t0.items():
        won = (not is_draw) and (sid == winner_id)
        snake_stats[sid] = {
            "name": name,
            "won": won,
            "draw": is_draw,
            "survived_turns": last_alive_turn.get(sid, 0),
            "max_length": max_length.get(sid, 3),
            "death_cause": death_cause.get(sid, "survived" if won else "unknown"),
            "death_turn": death_turn.get(sid, total_turns),
        }

    return {
        "game_id": game_info.get("id", ""),
        "total_turns": total_turns,
        "is_draw": is_draw,
        "winner_name": winner_name,
        "snakes": snake_stats,
    }


def analyze_tournament(results_dir: str, output_json: bool = False):
    """Analyze all game files in the results directory."""
    results_path = Path(results_dir)
    game_files = sorted(results_path.glob("game_*.json"))

    if not game_files:
        print(f"No game files found in {results_dir}")
        return

    # Aggregate stats per snake name
    stats = defaultdict(lambda: {
        "games": 0,
        "wins": 0,
        "draws": 0,
        "losses": 0,
        "total_survival_turns": 0,
        "total_max_length": 0,
        "death_causes": defaultdict(int),
        "survival_turns_list": [],
        "max_length_list": [],
        "win_lengths": [],
    })

    total_games = 0
    total_turns_all = 0
    parse_errors = 0

    for gf in game_files:
        result = parse_game(str(gf))
        if result is None:
            parse_errors += 1
            continue

        total_games += 1
        total_turns_all += result["total_turns"]

        for sid, snake in result["snakes"].items():
            name = snake["name"]
            s = stats[name]
            s["games"] += 1
            if snake["won"]:
                s["wins"] += 1
                s["win_lengths"].append(snake["max_length"])
            elif snake["draw"]:
                s["draws"] += 1
            else:
                s["losses"] += 1
            s["total_survival_turns"] += snake["survived_turns"]
            s["total_max_length"] += snake["max_length"]
            s["death_causes"][snake["death_cause"]] += 1
            s["survival_turns_list"].append(snake["survived_turns"])
            s["max_length_list"].append(snake["max_length"])

    if total_games == 0:
        print("No valid games found.")
        return

    # ── JSON Output ───────────────────────────────────────────────────────
    if output_json:
        json_out = {
            "total_games": total_games,
            "parse_errors": parse_errors,
            "avg_game_length": round(total_turns_all / total_games, 1),
            "snakes": {},
        }
        for name, s in sorted(stats.items(), key=lambda x: x[1]["wins"], reverse=True):
            json_out["snakes"][name] = {
                "games": s["games"],
                "wins": s["wins"],
                "draws": s["draws"],
                "losses": s["losses"],
                "win_rate": round(s["wins"] / s["games"] * 100, 1) if s["games"] else 0,
                "avg_survival_turns": round(s["total_survival_turns"] / s["games"], 1),
                "avg_max_length": round(s["total_max_length"] / s["games"], 1),
                "death_causes": dict(s["death_causes"]),
            }
        print(json.dumps(json_out, indent=2))
        return

    # ── Pretty Output ─────────────────────────────────────────────────────
    print()
    print("=" * 72)
    print(f"  TOURNAMENT RESULTS — {total_games} games analyzed")
    print(f"  Average game length: {total_turns_all / total_games:.0f} turns")
    if parse_errors:
        print(f"  ⚠ {parse_errors} game files could not be parsed")
    print("=" * 72)

    # Sort by win rate descending
    sorted_snakes = sorted(
        stats.items(),
        key=lambda x: x[1]["wins"] / max(x[1]["games"], 1),
        reverse=True,
    )

    for rank, (name, s) in enumerate(sorted_snakes, 1):
        games = s["games"]
        win_rate = s["wins"] / games * 100 if games else 0
        avg_survival = s["total_survival_turns"] / games if games else 0
        avg_length = s["total_max_length"] / games if games else 0

        # Median survival
        surv_sorted = sorted(s["survival_turns_list"])
        median_survival = surv_sorted[len(surv_sorted) // 2] if surv_sorted else 0

        # Median max length
        len_sorted = sorted(s["max_length_list"])
        median_length = len_sorted[len(len_sorted) // 2] if len_sorted else 0

        # Avg win length
        avg_win_len = (
            sum(s["win_lengths"]) / len(s["win_lengths"])
            if s["win_lengths"]
            else 0
        )

        print()
        # Highlight #1 rank
        marker = "👑" if rank == 1 else f"#{rank}"
        print(f"  {marker}  {name}")
        print(f"  {'─' * 50}")

        # Win/loss bar
        bar_width = 40
        win_bar = int(win_rate / 100 * bar_width)
        loss_bar = bar_width - win_bar
        bar = "█" * win_bar + "░" * loss_bar
        print(f"  Win Rate:   {win_rate:5.1f}%  [{bar}]")
        print(f"  Record:     {s['wins']}W / {s['draws']}D / {s['losses']}L  ({games} games)")
        print(f"  Avg Survival:  {avg_survival:.1f} turns  (median: {median_survival})")
        print(f"  Avg Length:    {avg_length:.1f}  (median: {median_length}, avg win length: {avg_win_len:.1f})")

        # Death causes breakdown
        causes = sorted(s["death_causes"].items(), key=lambda x: -x[1])
        causes_str = "  Deaths:     "
        cause_parts = []
        for cause, count in causes:
            pct = count / games * 100
            cause_parts.append(f"{cause}: {count} ({pct:.0f}%)")
        print(causes_str + "  |  ".join(cause_parts))

    print()
    print("=" * 72)

    # ── Head-to-Head Matrix (if 2-4 snakes) ───────────────────────────────
    snake_names = [name for name, _ in sorted_snakes]
    if 2 <= len(snake_names) <= 6:
        print()
        print("  HEAD-TO-HEAD WIN RATE MATRIX")
        print("  (row beat column)")
        print()

        # Reparse for head-to-head data
        h2h = defaultdict(lambda: defaultdict(lambda: {"wins": 0, "games": 0}))

        for gf in game_files:
            result = parse_game(str(gf))
            if result is None:
                continue
            for sid1, s1 in result["snakes"].items():
                for sid2, s2 in result["snakes"].items():
                    if s1["name"] == s2["name"]:
                        continue
                    h2h[s1["name"]][s2["name"]]["games"] += 1
                    # Count as win if s1 survived longer
                    if s1["survived_turns"] > s2["survived_turns"]:
                        h2h[s1["name"]][s2["name"]]["wins"] += 1

        # Print matrix
        max_name = max(len(n) for n in snake_names)
        header = " " * (max_name + 4)
        for n in snake_names:
            header += f"{n[:8]:>10}"
        print(f"  {header}")

        for row_name in snake_names:
            row = f"  {row_name:<{max_name + 2}}"
            for col_name in snake_names:
                if row_name == col_name:
                    row += f"{'---':>10}"
                else:
                    d = h2h[row_name][col_name]
                    if d["games"] > 0:
                        pct = d["wins"] / d["games"] * 100
                        row += f"{pct:>8.0f}% "
                    else:
                        row += f"{'N/A':>10}"
            print(row)

    print()
    print("=" * 72)
    print()


if __name__ == "__main__":
    if len(sys.argv) < 2:
        print("Usage: python3 analyze.py <results_dir> [--json]")
        sys.exit(1)

    results_dir = sys.argv[1]
    json_mode = "--json" in sys.argv

    analyze_tournament(results_dir, output_json=json_mode)
