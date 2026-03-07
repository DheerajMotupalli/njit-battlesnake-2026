# Battlesnake — Hackathon Champion

A high-performance Battlesnake server written in Rust with iterative-deepening minimax search, alpha-beta pruning, and Voronoi flood fill territory evaluation.

## Features

- **Minimax with Alpha-Beta Pruning** — Deep search for 1v1 games
- **Paranoid Search** — Multiplayer support (3-4 snake games)
- **Iterative Deepening** — Uses full time budget, returns best move found
- **Voronoi Flood Fill** — Territory control + food reachability analysis
- **Game Mode Support** — Standard, Royale, Constrictor, Wrapped, Solo
- **Smart Move Ordering** — Prunes risky moves first for faster cutoffs
- **Zero-allocation hot path** — Stack-allocated board arrays

## Quick Start

```bash
# Build release (optimized)
cargo build --release

# Run server
PORT=8080 ./target/release/battlesnake

# Test the info endpoint
curl http://localhost:8080/
```

## Deploy to Fly.io

```bash
fly launch
fly deploy
```

## Architecture

```
src/
├── main.rs    — HTTP server (axum), routes
├── types.rs   — API types (GameState, Coord, Direction, etc.)
├── board.rs   — Compact board representation for simulation
├── flood.rs   — Voronoi flood fill + area calculation
├── eval.rs    — Position evaluation (territory, food, aggression, etc.)
├── search.rs  — Iterative deepening minimax + paranoid search
└── logic.rs   — Move selection entry point
```

## Evaluation Weights

| Factor            | Weight | Description                            |
| ----------------- | ------ | -------------------------------------- |
| Territory control | ~40%   | Voronoi area from flood fill           |
| Health/food       | ~20%   | Urgency-based food seeking             |
| Length advantage  | ~15%   | Head-to-head win potential             |
| Aggression        | ~10%   | Chase smaller snakes, corner opponents |
| Tail access       | ~10%   | Escape route guarantee                 |
| Center control    | ~5%    | Board position quality                 |
