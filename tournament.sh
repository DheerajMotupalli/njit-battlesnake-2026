#!/usr/bin/env bash
# ──────────────────────────────────────────────────────────────────────────────
# tournament.sh — Run N Battlesnake games locally and collect results
#
# Usage:
#   ./tournament.sh                      # 100 games, 4 copies of Ouroboros
#   ./tournament.sh 500                  # 500 games
#   ./tournament.sh 100 2               # 100 games, 2 snakes
#   PORTS="8080 8081" ./tournament.sh 50 # 50 games, snake A on 8080 vs B on 8081
#
# Prerequisites:
#   - Battlesnake CLI at ~/rules/battlesnake
#   - Built binary at ./target/release/battlesnake
#
# Output:
#   tournament/results/  — one JSON per game
#   tournament/summary.txt — human-readable stats
# ──────────────────────────────────────────────────────────────────────────────
set -euo pipefail

# ── Configuration ─────────────────────────────────────────────────────────────
NUM_GAMES="${1:-100}"
NUM_SNAKES="${2:-4}"
BOARD_WIDTH="${BOARD_WIDTH:-11}"
BOARD_HEIGHT="${BOARD_HEIGHT:-11}"
GAME_TYPE="${GAME_TYPE:-standard}"
TIMEOUT="${TIMEOUT:-500}"
CLI="${CLI:-$HOME/rules/battlesnake}"
BINARY="./target/release/battlesnake"
BASE_PORT="${BASE_PORT:-9000}"
RESULTS_DIR="./tournament/results"

# Allow custom port assignments: PORTS="8080 8081 8082 8083"
CUSTOM_PORTS="${PORTS:-}"

# ── Colours ───────────────────────────────────────────────────────────────────
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m'

log()  { echo -e "${CYAN}[tournament]${NC} $*"; }
warn() { echo -e "${YELLOW}[tournament]${NC} $*"; }
err()  { echo -e "${RED}[tournament]${NC} $*" >&2; }
ok()   { echo -e "${GREEN}[tournament]${NC} $*"; }

# ── Cleanup ───────────────────────────────────────────────────────────────────
SERVER_PIDS=()
cleanup() {
    log "Cleaning up..."
    for pid in "${SERVER_PIDS[@]}"; do
        kill "$pid" 2>/dev/null || true
    done
    wait 2>/dev/null || true
}
trap cleanup EXIT

# ── Validation ────────────────────────────────────────────────────────────────
if [[ ! -x "$CLI" ]]; then
    err "Battlesnake CLI not found at $CLI"
    exit 1
fi

if [[ ! -f "$BINARY" ]]; then
    log "Building release binary..."
    cargo build --release
fi

# ── Start Snake Servers ───────────────────────────────────────────────────────
mkdir -p "$RESULTS_DIR"
rm -f "$RESULTS_DIR"/*.json

SNAKE_URLS=()
SNAKE_NAMES=()

if [[ -n "$CUSTOM_PORTS" ]]; then
    i=0
    for port in $CUSTOM_PORTS; do
        SNAKE_URLS+=("http://localhost:$port")
        name=$(curl -s "http://localhost:$port/" 2>/dev/null | python3 -c "import json,sys; print(json.load(sys.stdin).get('author','snake_$i'))" 2>/dev/null || echo "snake_$i")
        SNAKE_NAMES+=("$name")
        ((i++)) || true
    done
    NUM_SNAKES=${#SNAKE_URLS[@]}
    log "Using ${NUM_SNAKES} pre-running snakes on custom ports"
else
    LABELS=("Ouroboros" "Opponent_A" "Opponent_B" "Opponent_C" "Opponent_D" "Opponent_E" "Opponent_F" "Opponent_G")

    for ((i = 0; i < NUM_SNAKES; i++)); do
        port=$((BASE_PORT + i))
        PORT=$port "$BINARY" > /dev/null 2>&1 &
        SERVER_PIDS+=($!)
        SNAKE_URLS+=("http://localhost:$port")
        SNAKE_NAMES+=("${LABELS[$i]:-Snake_$i}")
    done

    log "Waiting for ${NUM_SNAKES} servers to start..."
    sleep 2

    for ((i = 0; i < NUM_SNAKES; i++)); do
        if ! curl -s "${SNAKE_URLS[$i]}/" > /dev/null 2>&1; then
            err "Server on ${SNAKE_URLS[$i]} not responding!"
            exit 1
        fi
    done
    ok "All ${NUM_SNAKES} servers ready"
fi

# ── Run Tournament ───────────────────────────────────────────────────────────
log "Starting tournament: ${NUM_GAMES} games, ${NUM_SNAKES} snakes, ${BOARD_WIDTH}x${BOARD_HEIGHT} ${GAME_TYPE}"

COMPLETED=0
FAILED=0
START_TIME=$(date +%s)

for ((g = 1; g <= NUM_GAMES; g++)); do
    seed=$((RANDOM * 32768 + RANDOM + g))
    outfile="$RESULTS_DIR/game_${g}.json"

    args=()
    for ((i = 0; i < NUM_SNAKES; i++)); do
        args+=(-n "${SNAKE_NAMES[$i]}" -u "${SNAKE_URLS[$i]}")
    done
    args+=(-W "$BOARD_WIDTH" -H "$BOARD_HEIGHT" -g "$GAME_TYPE" -t "$TIMEOUT" -r "$seed" -o "$outfile")

    if "$CLI" play "${args[@]}" > /dev/null 2>&1; then
        ((COMPLETED++)) || true
    else
        ((FAILED++)) || true
        warn "Game $g failed"
    fi

    # Progress
    elapsed=$(($(date +%s) - START_TIME))
    if [[ $elapsed -eq 0 ]]; then elapsed=1; fi
    total=$((COMPLETED + FAILED))
    pct=$((total * 100 / NUM_GAMES))
    remaining_games=$((NUM_GAMES - total))

    # Rate: show seconds/game when rate < 1 game/s 
    rate_x10=$((total * 10 / elapsed))
    if [[ $rate_x10 -gt 0 ]]; then
        rate_whole=$((rate_x10 / 10))
        rate_frac=$((rate_x10 % 10))
        rate_str="${rate_whole}.${rate_frac} games/s"
        eta=$((remaining_games * 10 / rate_x10))
    else
        spg=$((elapsed / (total > 0 ? total : 1)))
        rate_str="${spg}s/game"
        eta=$((remaining_games * spg))
    fi
    printf "\r  Game %d/%d (%d%%)  %s  ETA: %ss  %d failed   " \
        "$total" "$NUM_GAMES" "$pct" "$rate_str" "$eta" "$FAILED"
done

echo ""
elapsed=$(($(date +%s) - START_TIME))
ok "Tournament complete: ${COMPLETED} games in ${elapsed}s (${FAILED} failed)"

# ── Analyze ──────────────────────────────────────────────────────────────────
log "Analyzing results..."
echo ""
python3 ./analyze.py "$RESULTS_DIR" 2>&1 | tee "$RESULTS_DIR/../summary.txt"

ok "Summary saved to tournament/summary.txt"
