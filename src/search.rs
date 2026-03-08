use crate::board::SimBoard;
use crate::eval::evaluate;
use crate::types::{Coord, Direction};
use std::time::Instant;

/// Result of the search: best move and its score.
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub best_move: Direction,
    pub score: f64,
    pub depth_reached: u32,
    pub nodes_searched: u64,
}

/// Iterative deepening minimax with alpha-beta pruning.
/// Searches until the time budget is exhausted.
pub fn iterative_deepening_search(
    board: &SimBoard,
    time_budget_ms: u64,
) -> SearchResult {
    let start = Instant::now();
    let deadline = start + std::time::Duration::from_millis(time_budget_ms);

    let safe = board.smart_safe_moves(board.our_index);

    // If only one safe move, don't waste time searching
    if safe.len() == 1 {
        return SearchResult {
            best_move: safe[0].0,
            score: 0.0,
            depth_reached: 0,
            nodes_searched: 1,
        };
    }

    // If no safe moves, just go up (we're dead anyway)
    if safe.is_empty() {
        return SearchResult {
            best_move: Direction::Up,
            score: f64::NEG_INFINITY,
            depth_reached: 0,
            nodes_searched: 0,
        };
    }

    let mut best_result = SearchResult {
        best_move: safe[0].0,
        score: f64::NEG_INFINITY,
        depth_reached: 0,
        nodes_searched: 0,
    };

    // Iterative deepening
    let max_depth = if board.snake_count <= 2 { 15 } else { 8 };

    for depth in 1..=max_depth {
        if Instant::now() >= deadline {
            break;
        }

        let mut search_state = SearchState {
            nodes: 0,
            deadline,
            timed_out: false,
        };

        let result = if board.alive_count() <= 2 {
            // 1v1 or solo: standard minimax
            search_1v1(board, depth, &mut search_state)
        } else {
            // Multiplayer: paranoid search
            search_multiplayer(board, depth, &mut search_state)
        };

        if !search_state.timed_out {
            best_result = SearchResult {
                best_move: result.0,
                score: result.1,
                depth_reached: depth,
                nodes_searched: search_state.nodes,
            };

            // If we found a winning move, stop searching
            if result.1 == f64::INFINITY {
                break;
            }
        } else {
            // Timed out: use previous depth's result
            break;
        }
    }

    best_result
}

struct SearchState {
    nodes: u64,
    deadline: Instant,
    timed_out: bool,
}

impl SearchState {
    fn check_time(&mut self) -> bool {
        self.nodes += 1;
        // Check every 256 nodes instead of 1024 to catch timeouts sooner.
        // This is critical when search complexity spikes (e.g., switching
        // from paranoid multiplayer to 1v1 minimax).
        if self.nodes % 256 == 0 && Instant::now() >= self.deadline {
            self.timed_out = true;
            return true;
        }
        false
    }
}

/// 1v1 minimax with alpha-beta pruning.
/// Returns (best_direction, score).
fn search_1v1(
    board: &SimBoard,
    max_depth: u32,
    state: &mut SearchState,
) -> (Direction, f64) {
    let our_moves = board.safe_moves(board.our_index);
    if our_moves.is_empty() {
        return (Direction::Up, f64::NEG_INFINITY);
    }

    // Find the opponent index
    let opp_index = (0..board.snake_count)
        .find(|&i| i != board.our_index && board.snakes[i].alive)
        .unwrap_or(board.our_index);

    let mut best_move = our_moves[0].0;
    let mut best_score = f64::NEG_INFINITY;
    let mut alpha = f64::NEG_INFINITY;
    let beta = f64::INFINITY;

    // Order moves: prefer non-risky moves first
    let mut ordered_moves = board.smart_safe_moves(board.our_index);
    ordered_moves.sort_by(|a, b| a.2.cmp(&b.2)); // non-risky first

    for (our_dir, _, _) in &ordered_moves {
        if state.check_time() {
            break;
        }

        // Try each opponent move (min player)
        let opp_moves = board.safe_moves(opp_index);
        let mut min_score = f64::INFINITY;

        if opp_moves.is_empty() {
            // Opponent has no safe moves - apply only our move
            let new_board = board.apply_moves(&[(board.our_index, *our_dir)]);
            min_score = minimax_min(
                &new_board,
                max_depth - 1,
                alpha,
                beta,
                opp_index,
                state,
            );
        } else {
            for (opp_dir, _) in &opp_moves {
                if state.timed_out {
                    break;
                }

                let new_board = board.apply_moves(&[
                    (board.our_index, *our_dir),
                    (opp_index, *opp_dir),
                ]);

                let score = minimax_max(
                    &new_board,
                    max_depth - 1,
                    alpha,
                    beta,
                    opp_index,
                    state,
                );

                if score < min_score {
                    min_score = score;
                }

                if min_score <= alpha {
                    break; // Alpha cutoff
                }
            }
        }

        if min_score > best_score {
            best_score = min_score;
            best_move = *our_dir;
        }
        if best_score > alpha {
            alpha = best_score;
        }
    }

    (best_move, best_score)
}

/// Maximizing player (us).
fn minimax_max(
    board: &SimBoard,
    depth: u32,
    mut alpha: f64,
    beta: f64,
    opp_index: usize,
    state: &mut SearchState,
) -> f64 {
    if state.check_time() {
        return evaluate(board);
    }
    if depth == 0 || board.we_died() || board.we_won() {
        return evaluate(board);
    }

    let our_moves = board.safe_moves(board.our_index);
    if our_moves.is_empty() {
        return f64::NEG_INFINITY;
    }

    let mut max_score = f64::NEG_INFINITY;

    for (our_dir, _) in &our_moves {
        let opp_moves = board.safe_moves(opp_index);
        let mut min_score = f64::INFINITY;

        if opp_moves.is_empty() {
            let new_board = board.apply_moves(&[(board.our_index, *our_dir)]);
            min_score = minimax_max(&new_board, depth - 1, alpha, beta, opp_index, state);
        } else {
            for (opp_dir, _) in &opp_moves {
                if state.timed_out {
                    break;
                }

                let new_board = board.apply_moves(&[
                    (board.our_index, *our_dir),
                    (opp_index, *opp_dir),
                ]);

                let score = minimax_max(&new_board, depth - 1, alpha, beta, opp_index, state);

                if score < min_score {
                    min_score = score;
                }
                if min_score <= alpha {
                    break;
                }
            }
        }

        if min_score > max_score {
            max_score = min_score;
        }
        if max_score > alpha {
            alpha = max_score;
        }
        if alpha >= beta {
            break; // Beta cutoff
        }
    }

    max_score
}

/// Minimizing player (opponent).
fn minimax_min(
    board: &SimBoard,
    depth: u32,
    alpha: f64,
    mut beta: f64,
    opp_index: usize,
    state: &mut SearchState,
) -> f64 {
    if state.check_time() {
        return evaluate(board);
    }
    if depth == 0 || board.we_died() || board.we_won() {
        return evaluate(board);
    }

    let opp_moves = board.safe_moves(opp_index);
    if opp_moves.is_empty() {
        return evaluate(board);
    }

    let our_moves = board.safe_moves(board.our_index);
    if our_moves.is_empty() {
        return f64::NEG_INFINITY;
    }

    let mut min_score = f64::INFINITY;

    for (opp_dir, _) in &opp_moves {
        let mut max_score = f64::NEG_INFINITY;

        for (our_dir, _) in &our_moves {
            if state.timed_out {
                break;
            }

            let new_board = board.apply_moves(&[
                (board.our_index, *our_dir),
                (opp_index, *opp_dir),
            ]);

            let score = minimax_min(&new_board, depth - 1, alpha, beta, opp_index, state);

            if score > max_score {
                max_score = score;
            }
            if max_score >= beta {
                break;
            }
        }

        if max_score < min_score {
            min_score = max_score;
        }
        if min_score < beta {
            beta = min_score;
        }
        if alpha >= beta {
            break; // Alpha cutoff
        }
    }

    min_score
}

/// Multiplayer paranoid search: all opponents treated as trying to minimize our score.
fn search_multiplayer(
    board: &SimBoard,
    max_depth: u32,
    state: &mut SearchState,
) -> (Direction, f64) {
    let our_moves = board.smart_safe_moves(board.our_index);
    if our_moves.is_empty() {
        return (Direction::Up, f64::NEG_INFINITY);
    }

    let mut best_move = our_moves[0].0;
    let mut best_score = f64::NEG_INFINITY;

    // Gather alive opponent indices
    let opponents: Vec<usize> = (0..board.snake_count)
        .filter(|&i| i != board.our_index && board.snakes[i].alive)
        .collect();

    // Order moves: prefer non-risky moves first
    let mut ordered_moves = our_moves;
    ordered_moves.sort_by(|a, b| a.2.cmp(&b.2));

    for (our_dir, _, _) in &ordered_moves {
        if state.check_time() {
            break;
        }

        // For each of our moves, assume each opponent makes the worst move for us.
        // We enumerate all opponent move combinations, but limit to keep it tractable.
        let score = paranoid_min(
            board,
            *our_dir,
            &opponents,
            max_depth,
            f64::NEG_INFINITY,
            f64::INFINITY,
            state,
        );

        if score > best_score {
            best_score = score;
            best_move = *our_dir;
        }
    }

    (best_move, best_score)
}

/// Minimizing step of paranoid search: each opponent picks the move that hurts us most.
fn paranoid_min(
    board: &SimBoard,
    our_dir: Direction,
    opponents: &[usize],
    depth: u32,
    alpha: f64,
    beta: f64,
    state: &mut SearchState,
) -> f64 {
    if state.check_time() {
        return evaluate(board);
    }

    // Generate one move per opponent (pick worst for us)
    // For tractability with many opponents, pick a reasonable default for each
    let mut moves: Vec<(usize, Direction)> = vec![(board.our_index, our_dir)];

    for &oi in opponents {
        let opp_safe = board.safe_moves(oi);
        if opp_safe.is_empty() {
            continue;
        }
        // Pick the opponent move that leads to the worst position for us
        // For depth > 2, just pick a reasonable heuristic move (saves time)
        if depth <= 2 || opponents.len() > 2 {
            // Heuristic: opponent moves toward us if shorter, away if longer
            let our_head = board.snakes[board.our_index].head;
            let opp_len = board.snakes[oi].length;
            let our_len = board.snakes[board.our_index].length;

            let dist_to_us = board.snakes[oi].head.dist(&our_head);
            // Distance-based opponent model (no board.turn dependency).
            // Previously equal-length opponents always chased us, making
            // the paranoid search see walls as death traps (3 snakes
            // converging). Now only strictly-bigger + close opponents
            // chase — much more realistic for 4-player games.
            let best_opp_move = if dist_to_us > 5 {
                // Far away: opponent plays their own game (seek center)
                let center = Coord::new(board.width / 2, board.height / 2);
                opp_safe
                    .iter()
                    .min_by_key(|(_, c)| c.dist(&center))
                    .unwrap()
                    .0
            } else if opp_len > our_len {
                // Strictly bigger + close: they chase us (head-to-head advantage)
                opp_safe
                    .iter()
                    .min_by_key(|(_, c)| c.dist(&our_head))
                    .unwrap()
                    .0
            } else if opp_len < our_len {
                // Smaller + close: they avoid us
                opp_safe
                    .iter()
                    .max_by_key(|(_, c)| c.dist(&our_head))
                    .unwrap()
                    .0
            } else {
                // Equal length + close: seek center (both avoid head-to-head)
                let center = Coord::new(board.width / 2, board.height / 2);
                opp_safe
                    .iter()
                    .min_by_key(|(_, c)| c.dist(&center))
                    .unwrap()
                    .0
            };
            moves.push((oi, best_opp_move));
        } else {
            // Try each opponent move and pick the worst for us
            let mut worst_score = f64::INFINITY;
            let mut worst_dir = opp_safe[0].0;

            for (opp_dir, _) in &opp_safe {
                let mut test_moves = moves.clone();
                test_moves.push((oi, *opp_dir));
                // Quick eval
                let new_board = board.apply_moves(&test_moves);
                let score = evaluate(&new_board);
                if score < worst_score {
                    worst_score = score;
                    worst_dir = *opp_dir;
                }
            }
            moves.push((oi, worst_dir));
        }
    }

    let new_board = board.apply_moves(&moves);

    if depth <= 1 || new_board.we_died() || new_board.we_won() || new_board.alive_count() <= 1 {
        return evaluate(&new_board);
    }

    // Continue searching from new position (our turn again)
    let next_our_moves = new_board.safe_moves(new_board.our_index);
    if next_our_moves.is_empty() {
        return f64::NEG_INFINITY;
    }

    let next_opponents: Vec<usize> = (0..new_board.snake_count)
        .filter(|&i| i != new_board.our_index && new_board.snakes[i].alive)
        .collect();

    let mut max_score = f64::NEG_INFINITY;
    let mut alpha = alpha;

    for (next_dir, _) in &next_our_moves {
        if state.timed_out {
            break;
        }

        let score = paranoid_min(
            &new_board,
            *next_dir,
            &next_opponents,
            depth - 1,
            alpha,
            beta,
            state,
        );

        if score > max_score {
            max_score = score;
        }
        if max_score > alpha {
            alpha = max_score;
        }
        if alpha >= beta {
            break;
        }
    }

    max_score
}
