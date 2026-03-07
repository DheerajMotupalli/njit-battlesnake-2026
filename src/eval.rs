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

    // ── 1. Territory control (most important: ~40%) ──────────────────────────
    let our_territory = flood.territory[board.our_index] as f64;
    let territory_ratio = our_territory / total_cells;
    score += territory_ratio * 400.0;

    // Bonus for having more territory than all opponents
    for &oi in &alive_opponents {
        let opp_territory = flood.territory[oi] as f64;
        if our_territory > opp_territory {
            score += 20.0;
        } else if our_territory < opp_territory {
            score -= 30.0; // Penalise being behind more harshly
        }
    }

    // ── 2. Health & food management (~20%) ────────────────────────────────────
    let health = us.health as f64;

    // Health scoring with urgency curve
    if health < 10.0 {
        score -= (10.0 - health) * 15.0; // Critical: heavy penalty
    } else if health < 30.0 {
        score -= (30.0 - health) * 3.0; // Low: moderate penalty
    } else if health < 50.0 {
        score += health * 0.5; // Healthy: mild bonus
    } else {
        score += 25.0; // Full: flat bonus (don't over-value high health)
    }

    // Food proximity when hungry
    if health < 40.0 && flood.nearest_food_dist[board.our_index] < i32::MAX {
        let food_dist = flood.nearest_food_dist[board.our_index] as f64;
        // Closer food is better when hungry, scaled by urgency
        let urgency = (40.0 - health) / 40.0;
        score += (10.0 - food_dist.min(10.0)) * urgency * 10.0;
    }

    // Reachable food count
    score += flood.reachable_food[board.our_index] as f64 * 5.0;

    // ── 3. Length advantage (~15%) ────────────────────────────────────────────
    for &oi in &alive_opponents {
        let length_diff = us.length - board.snakes[oi].length;
        if length_diff > 0 {
            // We're longer: can win head-to-head
            score += (length_diff as f64).min(5.0) * 15.0;
        } else if length_diff < 0 {
            // They're longer: penalise
            score += length_diff as f64 * 8.0;
        }
        // Equal length: slight penalty (can't win h2h)
    }

    // ── 4. Aggression / kill potential (~10%) ─────────────────────────────────
    for &oi in &alive_opponents {
        let opp = &board.snakes[oi];
        if !opp.alive {
            continue;
        }

        // Reward being near shorter opponents (can kill them head-to-head)
        if us.length > opp.length {
            let dist = us.head.dist(&opp.head) as f64;
            if dist <= 4.0 {
                score += (5.0 - dist) * 12.0; // Chase smaller snakes
            }
        }

        // Reward opponent having few safe moves (cornered)
        let opp_safe = board.safe_moves(oi).len() as f64;
        if opp_safe <= 1.0 {
            score += 40.0; // Opponent is nearly trapped
        } else if opp_safe <= 2.0 {
            score += 15.0;
        }
    }

    // ── 5. Tail accessibility (~10%) ──────────────────────────────────────────
    if flood.can_reach_tail {
        score += 30.0; // Can always chase our tail as an escape
    } else {
        score -= 40.0; // Can't reach tail: we might get trapped
    }

    // Add mild bonus for being close to tail (escape route)
    if flood.tail_distance < i32::MAX {
        let td = flood.tail_distance as f64;
        score += (15.0 - td.min(15.0)) * 1.5;
    }

    // ── 6. Center control (~5%) ──────────────────────────────────────────────
    let center_x = board.width as f64 / 2.0;
    let center_y = board.height as f64 / 2.0;
    let dist_to_center =
        ((us.head.x as f64 - center_x).abs() + (us.head.y as f64 - center_y).abs()) / total_cells;
    score -= dist_to_center * 15.0;

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
