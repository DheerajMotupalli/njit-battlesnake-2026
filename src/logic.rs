use crate::board::SimBoard;
use crate::flood::flood_fill_area;
use crate::search::iterative_deepening_search;
use crate::types::{Direction, GameState};
use tracing::info;

/// Main entry point: given a game state, return the best move.
pub fn get_move(state: &GameState) -> Direction {
    let board = SimBoard::from_game_state(state);

    // Calculate time budget: use 70% of timeout to leave margin for
    // HTTP response serialization and network latency. Previous 85%
    // caused timeouts when transitioning from multiplayer to 1v1.
    let timeout_ms = state.game.timeout as u64;
    let budget_ms = (timeout_ms * 70 / 100).max(50);

    info!(
        turn = state.turn,
        health = state.you.health,
        length = state.you.length,
        snakes_alive = board.alive_count(),
        mode = ?board.mode,
        budget_ms,
        "Processing move"
    );

    // Quick safety check: if only 0-1 safe moves, handle immediately
    let safe = board.smart_safe_moves(board.our_index);

    if safe.is_empty() {
        // No safe moves at all — pick the least bad option
        info!("No safe moves! Picking least bad direction.");
        return emergency_move(&board);
    }

    if safe.len() == 1 {
        info!(direction = %safe[0].0, "Only one safe move");
        return safe[0].0;
    }

    // If exactly 2+ safe moves but some are risky (head-to-head), prefer safe ones
    let non_risky: Vec<_> = safe.iter().filter(|m| !m.2).collect();
    if non_risky.len() == 1 {
        // Only one truly safe move
        // But still check if the risky moves lead to significantly better territory
        let safe_area = flood_fill_area(&board, non_risky[0].1);
        let our_length = board.snakes[board.our_index].length as u32;

        // If safe move gives us enough space, take it
        if safe_area > our_length * 2 {
            info!(
                direction = %non_risky[0].0,
                area = safe_area,
                "One non-risky move with adequate space"
            );
            return non_risky[0].0;
        }
        // Otherwise, fall through to search (might need to risk it)
    }

    // Run iterative deepening search
    let result = iterative_deepening_search(&board, budget_ms);

    info!(
        direction = %result.best_move,
        score = result.score,
        depth = result.depth_reached,
        nodes = result.nodes_searched,
        "Search complete"
    );

    result.best_move
}

/// Emergency move when no safe moves exist.
/// Pick the direction that leads to the most open space (least bad death).
fn emergency_move(board: &SimBoard) -> Direction {
    let head = board.snakes[board.our_index].head;

    Direction::ALL
        .iter()
        .max_by_key(|&&dir| {
            let new_pos = board.move_coord(head, dir);
            if !new_pos.in_bounds(board.width, board.height) {
                return 0;
            }
            flood_fill_area(board, new_pos) as i64
        })
        .copied()
        .unwrap_or(Direction::Up)
}
