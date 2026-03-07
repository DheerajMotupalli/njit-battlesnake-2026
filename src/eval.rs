use crate::board::SimBoard;
use crate::flood::flood_fill;
use crate::types::GameMode;

/// Evaluate a board position from the perspective of our snake.
/// Returns a score where higher is better for us.
/// Returns f64::NEG_INFINITY if we're dead, f64::INFINITY if we won.
pub fn evaluate(board: &SimBoard) -> f64 {
    let us = &board.snakes[board.our_index];

    // Terminal states
    if !us.alive {
        return f64::NEG_INFINITY;
    }
    if board.we_won() {
        return f64::INFINITY;
    }

    let alive_opponents: Vec<usize> = (0..board.snake_count)
        .filter(|&i| i != board.our_index && board.snakes[i].alive)
        .collect();

    // Solo mode: just survive and eat
    if alive_opponents.is_empty() && board.mode == GameMode::Solo {
        return eval_solo(board);
    }

    let flood = flood_fill(board);
    let total_cells = (board.width * board.height) as f64;

    let mut score = 0.0;

    // Determine game phase
    let is_opening = board.turn < 5; // First ~5 turns: must eat immediately
    let is_early_game = board.turn < 20; // First ~20 turns: growth is critical
    let shorter_than_all = alive_opponents
        .iter()
        .all(|&oi| board.snakes[oi].length > us.length);
    let shorter_than_any = alive_opponents
        .iter()
        .any(|&oi| board.snakes[oi].length > us.length);

    // ── 1. Territory control ─────────────────────────────────────────────────
    let our_territory = flood.territory[board.our_index] as f64;
    let territory_ratio = our_territory / total_cells;
    // Territory is much less important in the opening when eating should dominate
    let territory_weight = if is_opening {
        80.0
    } else if is_early_game {
        200.0
    } else {
        400.0
    };
    score += territory_ratio * territory_weight;

    for &oi in &alive_opponents {
        let opp_territory = flood.territory[oi] as f64;
        if our_territory > opp_territory {
            score += 15.0;
        } else if our_territory < opp_territory {
            score -= 20.0;
        }
    }

    // ── 2. Health & food management ──────────────────────────────────────────
    let health = us.health as f64;

    if health < 10.0 {
        score -= (10.0 - health) * 15.0;
    } else if health < 30.0 {
        score -= (30.0 - health) * 3.0;
    } else if health < 50.0 {
        score += health * 0.5;
    } else {
        score += 25.0;
    }

    // Food proximity — THE dominant signal in the opening
    if flood.nearest_food_dist[board.our_index] < i32::MAX {
        let food_dist = flood.nearest_food_dist[board.our_index] as f64;

        if is_opening {
            // Opening: food is the TOP priority — each step closer ~35 pts
            score += (15.0 - food_dist.min(15.0)) * 35.0;
            // Massive bonus for being about to eat
            if food_dist <= 1.0 {
                score += 100.0;
            } else if food_dist <= 2.0 {
                score += 50.0;
            }
        } else if is_early_game && shorter_than_any {
            // Early game + we're shorter: aggressively chase food
            score += (12.0 - food_dist.min(12.0)) * 25.0;
            if food_dist <= 2.0 {
                score += 50.0;
            }
        } else if is_early_game {
            // Early game, equal/longer: still value food proximity
            score += (10.0 - food_dist.min(10.0)) * 12.0;
        } else if health < 50.0 {
            let urgency = (50.0 - health) / 50.0;
            score += (10.0 - food_dist.min(10.0)) * urgency * 12.0;
        } else {
            score += (8.0 - food_dist.min(8.0)) * 2.0;
        }
    }

    // Reachable food count
    let food_count_weight = if is_opening {
        15.0
    } else if is_early_game {
        10.0
    } else {
        5.0
    };
    score += flood.reachable_food[board.our_index] as f64 * food_count_weight;

    // ── 3. Length advantage ──────────────────────────────────────────────────
    // Absolute length bonus in early game: directly rewards eating
    if is_early_game {
        score += us.length as f64 * 25.0;
    }

    for &oi in &alive_opponents {
        let length_diff = us.length - board.snakes[oi].length;
        if length_diff > 0 {
            score += (length_diff as f64).min(5.0) * 15.0;
        } else if length_diff < 0 {
            let penalty = if is_opening {
                25.0
            } else if is_early_game {
                18.0
            } else {
                10.0
            };
            score += length_diff as f64 * penalty;
        }
    }

    // Extra penalty for being shorter than ALL opponents
    if shorter_than_all && !alive_opponents.is_empty() {
        score -= if is_opening { 60.0 } else { 50.0 };
    }

    // ── 4. Aggression / kill potential ────────────────────────────────────────
    for &oi in &alive_opponents {
        let opp = &board.snakes[oi];
        if !opp.alive {
            continue;
        }

        if us.length > opp.length {
            let dist = us.head.dist(&opp.head) as f64;
            if dist <= 4.0 {
                score += (5.0 - dist) * 12.0;
            }
        }

        let opp_safe = board.safe_moves(oi).len() as f64;
        if opp_safe <= 1.0 {
            score += 40.0;
        } else if opp_safe <= 2.0 {
            score += 15.0;
        }
    }

    // ── 5. Tail accessibility ────────────────────────────────────────────────
    // Reduce tail weight during opening so it doesn't override food-seeking
    if flood.can_reach_tail {
        score += if is_opening { 10.0 } else { 30.0 };
    } else {
        score -= if is_opening { 10.0 } else { 40.0 };
    }

    if flood.tail_distance < i32::MAX {
        let td = flood.tail_distance as f64;
        score += (15.0 - td.min(15.0)) * 1.5;
    }

    // ── 6. Center control ────────────────────────────────────────────────────
    let center_x = board.width as f64 / 2.0;
    let center_y = board.height as f64 / 2.0;
    let dist_to_center =
        ((us.head.x as f64 - center_x).abs() + (us.head.y as f64 - center_y).abs()) / total_cells;
    // No center pull in opening (don't fight food-seeking)
    let center_weight = if is_opening {
        0.0
    } else if is_early_game {
        10.0
    } else {
        15.0
    };
    score -= dist_to_center * center_weight;

    // ── 7. Royale-specific: avoid hazards ────────────────────────────────────
    if board.mode == GameMode::Royale {
        use crate::board::Cell;
        // Penalise if head is on hazard
        let head_cell = board.get_cell(us.head);
        if matches!(head_cell, Cell::Hazard | Cell::HazardFood) {
            score -= 30.0;
        }

        // Bonus for being away from edges in royale (hazards come from edges)
        let edge_dist = us
            .head
            .x
            .min(us.head.y)
            .min(board.width - 1 - us.head.x)
            .min(board.height - 1 - us.head.y) as f64;
        score += edge_dist * 5.0;
    }

    score
}

/// Simple evaluation for solo mode (no opponents).
fn eval_solo(board: &SimBoard) -> f64 {
    let us = &board.snakes[board.our_index];
    let mut score = 1000.0; // Base survival score

    // Health is critical in solo
    let health = us.health as f64;
    if health < 15.0 {
        score -= (15.0 - health) * 20.0;
    }

    // Find food quickly
    let flood = flood_fill(board);
    if flood.nearest_food_dist[board.our_index] < i32::MAX {
        let food_dist = flood.nearest_food_dist[board.our_index] as f64;
        score += (20.0 - food_dist.min(20.0)) * 10.0;
    }

    // Territory (don't trap yourself)
    score += flood.territory[board.our_index] as f64 * 2.0;

    // Length bonus
    score += us.length as f64 * 5.0;

    score
}
