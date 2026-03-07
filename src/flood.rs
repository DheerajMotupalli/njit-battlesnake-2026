use crate::board::{Cell, SimBoard, MAX_CELLS, MAX_DIM, MAX_SNAKES};
use crate::types::Coord;

/// Result of a flood fill / Voronoi partition from all snake heads.
#[derive(Debug, Clone)]
pub struct FloodResult {
    /// Territory size for each snake (number of reachable cells).
    pub territory: [u32; MAX_SNAKES],
    /// How many food items each snake can reach first.
    pub reachable_food: [u32; MAX_SNAKES],
    /// Distance to nearest food for each snake (i32::MAX if none).
    pub nearest_food_dist: [i32; MAX_SNAKES],
    /// Total reachable cells for our snake (including contested).
    pub our_reachable: u32,
    /// Can our snake reach its own tail?
    pub can_reach_tail: bool,
    /// Distance to our tail.
    pub tail_distance: i32,
}

/// Perform simultaneous BFS flood fill from all alive snake heads.
/// Each cell is claimed by the snake that reaches it first (Voronoi).
/// Ties go to the longer snake (advantage in head-to-head).
pub fn flood_fill(board: &SimBoard) -> FloodResult {
    let mut result = FloodResult {
        territory: [0; MAX_SNAKES],
        reachable_food: [0; MAX_SNAKES],
        nearest_food_dist: [i32::MAX; MAX_SNAKES],
        our_reachable: 0,
        can_reach_tail: false,
        tail_distance: i32::MAX,
    };

    // Owner of each cell: 255 = unclaimed
    let mut owner: [u8; MAX_CELLS] = [255; MAX_CELLS];
    // Distance to each cell from its owner
    let mut dist: [i32; MAX_CELLS] = [i32::MAX; MAX_CELLS];

    // BFS queue: (coord, snake_index, distance)
    let mut queue: Vec<(Coord, u8, i32)> = Vec::with_capacity(MAX_CELLS);
    let mut head = 0usize;

    // Seed BFS with all alive snake heads
    for si in 0..board.snake_count {
        if !board.snakes[si].alive {
            continue;
        }
        let hd = board.snakes[si].head;
        let idx = board.coord_to_idx(hd);
        if idx < MAX_CELLS {
            owner[idx] = si as u8;
            dist[idx] = 0;
            queue.push((hd, si as u8, 0));
        }
    }

    // BFS
    while head < queue.len() {
        let (coord, si, d) = queue[head];
        head += 1;

        let neighbors = if board.mode.is_wrapped() {
            [
                wrap_coord(Coord::new(coord.x, coord.y + 1), board.width, board.height),
                wrap_coord(Coord::new(coord.x, coord.y - 1), board.width, board.height),
                wrap_coord(Coord::new(coord.x - 1, coord.y), board.width, board.height),
                wrap_coord(Coord::new(coord.x + 1, coord.y), board.width, board.height),
            ]
        } else {
            coord.neighbors()
        };

        for nc in neighbors {
            if !board.mode.is_wrapped() && !nc.in_bounds(board.width, board.height) {
                continue;
            }

            let nidx = (nc.y as usize) * MAX_DIM + (nc.x as usize);
            if nidx >= MAX_CELLS {
                continue;
            }

            let nd = d + 1;

            // Check if cell is passable
            let cell = board.cells[nidx];
            match cell {
                Cell::SnakeBody(_, ttl) => {
                    // Can pass through body segments that will disappear before we arrive
                    if (ttl as i32) > nd {
                        continue;
                    }
                }
                Cell::SnakeHead(s) if s != si => continue,
                _ => {}
            }

            // Check if we can claim this cell
            if dist[nidx] < nd {
                continue; // Already claimed by someone closer
            }
            if dist[nidx] == nd && owner[nidx] != 255 {
                // Tie: longer snake wins
                let existing_len = board.snakes[owner[nidx] as usize].length;
                let our_len = board.snakes[si as usize].length;
                if existing_len >= our_len {
                    continue;
                }
                // We're longer, take it over (decrement old owner)
                let old_owner = owner[nidx] as usize;
                if result.territory[old_owner] > 0 {
                    result.territory[old_owner] -= 1;
                }
            }

            owner[nidx] = si;
            dist[nidx] = nd;
            result.territory[si as usize] += 1;

            // Check for food
            if matches!(cell, Cell::Food | Cell::HazardFood) {
                result.reachable_food[si as usize] += 1;
                if nd < result.nearest_food_dist[si as usize] {
                    result.nearest_food_dist[si as usize] = nd;
                }
            }

            queue.push((nc, si, nd));
        }
    }

    // Calculate our-specific stats
    let our_idx = board.our_index;
    result.our_reachable = result.territory[our_idx];

    // Check tail reachability
    let our_tail = board.snakes[our_idx].tail;
    let tail_idx = board.coord_to_idx(our_tail);
    if tail_idx < MAX_CELLS && (owner[tail_idx] == our_idx as u8 || dist[tail_idx] < i32::MAX) {
        result.can_reach_tail = true;
        result.tail_distance = dist[tail_idx];
    }

    result
}

/// Perform flood fill from a single point and return the number of reachable cells.
/// Used for quick area calculations.
pub fn flood_fill_area(board: &SimBoard, start: Coord) -> u32 {
    if !start.in_bounds(board.width, board.height) {
        return 0;
    }

    let mut visited = [false; MAX_CELLS];
    let mut count = 0u32;
    let mut stack = Vec::with_capacity(64);

    let start_idx = board.coord_to_idx(start);
    if start_idx >= MAX_CELLS {
        return 0;
    }
    visited[start_idx] = true;
    stack.push(start);
    count += 1;

    while let Some(coord) = stack.pop() {
        let neighbors = if board.mode.is_wrapped() {
            [
                wrap_coord(Coord::new(coord.x, coord.y + 1), board.width, board.height),
                wrap_coord(Coord::new(coord.x, coord.y - 1), board.width, board.height),
                wrap_coord(Coord::new(coord.x - 1, coord.y), board.width, board.height),
                wrap_coord(Coord::new(coord.x + 1, coord.y), board.width, board.height),
            ]
        } else {
            coord.neighbors()
        };

        for nc in neighbors {
            if !board.mode.is_wrapped() && !nc.in_bounds(board.width, board.height) {
                continue;
            }

            let nidx = (nc.y as usize) * MAX_DIM + (nc.x as usize);
            if nidx >= MAX_CELLS || visited[nidx] {
                continue;
            }

            match board.cells[nidx] {
                Cell::SnakeBody(_, ttl) if ttl > 1 => continue,
                Cell::SnakeHead(_) => continue,
                _ => {}
            }

            visited[nidx] = true;
            count += 1;
            stack.push(nc);
        }
    }

    count
}

#[inline]
fn wrap_coord(c: Coord, width: i32, height: i32) -> Coord {
    Coord::new(
        ((c.x % width) + width) % width,
        ((c.y % height) + height) % height,
    )
}
