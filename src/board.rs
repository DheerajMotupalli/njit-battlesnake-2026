use crate::types::{Coord, Direction, GameMode, GameState};

/// Maximum board dimension we support (19x19 + some margin).
pub const MAX_DIM: usize = 21;
pub const MAX_CELLS: usize = MAX_DIM * MAX_DIM;
pub const MAX_SNAKES: usize = 8;

/// What occupies a cell on the board.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cell {
    Empty,
    Food,
    /// Snake body segment. (snake_index, turns_until_disappears)
    /// turns_until_disappears == 0 means it's the tail and will move next turn
    SnakeBody(u8, u16),
    /// Snake head. (snake_index)
    SnakeHead(u8),
    Hazard,
    /// Food on a hazard cell
    HazardFood,
}

/// Compact board representation optimised for simulation.
#[derive(Clone)]
pub struct SimBoard {
    pub width: i32,
    pub height: i32,
    pub cells: [Cell; MAX_CELLS],
    pub snakes: [SimSnake; MAX_SNAKES],
    pub snake_count: usize,
    pub our_index: usize,
    pub mode: GameMode,
    pub hazard_damage: i32,
    pub turn: u32,
}

#[derive(Clone, Debug)]
pub struct SimSnake {
    pub alive: bool,
    pub health: i32,
    pub head: Coord,
    pub tail: Coord,
    pub length: i32,
    pub body: Vec<Coord>,
}

impl Default for SimSnake {
    fn default() -> Self {
        Self {
            alive: false,
            health: 0,
            head: Coord::new(-1, -1),
            tail: Coord::new(-1, -1),
            length: 0,
            body: Vec::new(),
        }
    }
}

impl SimBoard {
    /// Convert from the API GameState into our compact representation.
    pub fn from_game_state(state: &GameState) -> Self {
        let width = state.board.width;
        let height = state.board.height;
        let mode = GameMode::from_ruleset(&state.game.ruleset.name);
        let hazard_damage = state
            .game
            .ruleset
            .settings
            .as_ref()
            .and_then(|s| s.hazard_damage_per_turn)
            .unwrap_or(14);

        let mut board = SimBoard {
            width,
            height,
            cells: [Cell::Empty; MAX_CELLS],
            snakes: std::array::from_fn(|_| SimSnake::default()),
            snake_count: state.board.snakes.len(),
            our_index: 0,
            mode,
            hazard_damage,
            turn: state.turn,
        };

        // Place hazards first (before food, so we can make HazardFood)
        for h in &state.board.hazards {
            let idx = board.coord_to_idx(*h);
            if idx < MAX_CELLS {
                board.cells[idx] = Cell::Hazard;
            }
        }

        // Place food
        for f in &state.board.food {
            let idx = board.coord_to_idx(*f);
            if idx < MAX_CELLS {
                board.cells[idx] = if board.cells[idx] == Cell::Hazard {
                    Cell::HazardFood
                } else {
                    Cell::Food
                };
            }
        }

        // Place snakes
        for (si, snake) in state.board.snakes.iter().enumerate() {
            if si >= MAX_SNAKES {
                break;
            }
            if snake.id == state.you.id {
                board.our_index = si;
            }

            board.snakes[si] = SimSnake {
                alive: snake.is_alive(),
                health: snake.health,
                head: snake.head,
                tail: snake.tail(),
                length: snake.length,
                body: snake.body.clone(),
            };

            // Place head
            let head_idx = board.coord_to_idx(snake.head);
            if head_idx < MAX_CELLS {
                board.cells[head_idx] = Cell::SnakeHead(si as u8);
            }

            // Place body (skip head which is body[0])
            let body_len = snake.body.len();
            for (bi, &seg) in snake.body.iter().enumerate().skip(1) {
                let idx = board.coord_to_idx(seg);
                if idx < MAX_CELLS {
                    // turns_until_disappears: the segment at position bi disappears
                    // after (body_len - bi) turns. Tail (bi = body_len-1) gets TTL=1.
                    let ttl = (body_len - bi) as u16;
                    // For stacked segments (e.g. after eating), keep the highest TTL
                    match board.cells[idx] {
                        Cell::SnakeBody(s, existing_ttl) if s == si as u8 && existing_ttl > ttl => {
                            // Keep the higher TTL already written
                        }
                        _ => {
                            board.cells[idx] = Cell::SnakeBody(si as u8, ttl);
                        }
                    }
                }
            }
        }

        board
    }

    #[inline]
    pub fn coord_to_idx(&self, c: Coord) -> usize {
        (c.y as usize) * MAX_DIM + (c.x as usize)
    }

    #[inline]
    pub fn idx_to_coord(&self, idx: usize) -> Coord {
        Coord::new((idx % MAX_DIM) as i32, (idx / MAX_DIM) as i32)
    }

    #[inline]
    pub fn get_cell(&self, c: Coord) -> Cell {
        if !c.in_bounds(self.width, self.height) {
            return Cell::SnakeBody(255, 999); // Treat out of bounds as impassable
        }
        self.cells[self.coord_to_idx(c)]
    }

    /// Apply a move for the given direction. Returns the new head coordinate.
    pub fn move_coord(&self, head: Coord, dir: Direction) -> Coord {
        if self.mode.is_wrapped() {
            head.apply_move_wrapped(dir, self.width, self.height)
        } else {
            head.apply_move(dir)
        }
    }

    /// Check if a cell is safe to move into (no wall, no body that won't disappear).
    pub fn is_cell_safe(&self, c: Coord) -> bool {
        if !self.mode.is_wrapped() && !c.in_bounds(self.width, self.height) {
            return false;
        }
        let c = if self.mode.is_wrapped() {
            Coord::new(
                ((c.x % self.width) + self.width) % self.width,
                ((c.y % self.height) + self.height) % self.height,
            )
        } else {
            c
        };
        match self.get_cell(c) {
            Cell::Empty | Cell::Food | Cell::Hazard | Cell::HazardFood => true,
            Cell::SnakeBody(_, ttl) => ttl <= 1, // Tail will move away
            Cell::SnakeHead(_) => false,
        }
    }

    /// Get safe moves for a snake by index, also considering head-to-head risk.
    pub fn safe_moves(&self, snake_idx: usize) -> Vec<(Direction, Coord)> {
        let snake = &self.snakes[snake_idx];
        if !snake.alive {
            return Vec::new();
        }

        // Determine the reverse direction (moving back into the neck) to exclude it.
        let reverse_dir = if snake.body.len() >= 2 {
            let neck = snake.body[1];
            let dx = neck.x - snake.head.x;
            let dy = neck.y - snake.head.y;
            match (dx, dy) {
                (1, 0) => Some(Direction::Right),
                (-1, 0) => Some(Direction::Left),
                (0, 1) => Some(Direction::Up),
                (0, -1) => Some(Direction::Down),
                _ => None, // Stacked head/neck at start; no reverse to skip
            }
        } else {
            None
        };

        let mut moves = Vec::with_capacity(4);
        for dir in Direction::ALL {
            if Some(dir) == reverse_dir {
                continue;
            }
            let new_head = self.move_coord(snake.head, dir);
            if self.is_cell_safe(new_head) {
                moves.push((dir, new_head));
            }
        }
        moves
    }

    /// Safe moves that also avoid positions where a larger or equal snake could head-to-head us.
    pub fn smart_safe_moves(&self, snake_idx: usize) -> Vec<(Direction, Coord, bool)> {
        let our_len = self.snakes[snake_idx].length;
        let basic = self.safe_moves(snake_idx);

        basic
            .into_iter()
            .map(|(dir, new_head)| {
                let mut risky = false;
                // Check if any other snake's head is adjacent to our new head
                for (si, s) in self.snakes.iter().enumerate() {
                    if si == snake_idx || !s.alive {
                        continue;
                    }
                    if s.head.dist(&new_head) == 1 && s.length >= our_len {
                        risky = true;
                        break;
                    }
                }
                (dir, new_head, risky)
            })
            .collect()
    }

    /// Apply moves for all snakes and return a new board state.
    /// `moves` maps snake_index -> direction.
    pub fn apply_moves(&self, moves: &[(usize, Direction)]) -> SimBoard {
        let mut new_board = self.clone();
        new_board.turn += 1;

        // Track new heads and which snakes ate food
        let mut new_heads: [(Coord, bool); MAX_SNAKES] = [(Coord::new(-1, -1), false); MAX_SNAKES];
        let mut ate_food = [false; MAX_SNAKES];

        // 1. Compute new head positions
        for &(si, dir) in moves {
            if !new_board.snakes[si].alive {
                continue;
            }
            let new_head = self.move_coord(self.snakes[si].head, dir);
            new_heads[si] = (new_head, true);
        }

        // 2. Clear old snake cells from board
        for si in 0..self.snake_count {
            if !self.snakes[si].alive {
                continue;
            }
            for seg in &self.snakes[si].body {
                let idx = new_board.coord_to_idx(*seg);
                if idx < MAX_CELLS {
                    // Only clear if it belongs to this snake
                    match new_board.cells[idx] {
                        Cell::SnakeHead(s) if s == si as u8 => {
                            new_board.cells[idx] = Cell::Empty;
                        }
                        Cell::SnakeBody(s, _) if s == si as u8 => {
                            new_board.cells[idx] = Cell::Empty;
                        }
                        _ => {}
                    }
                }
            }
        }

        // 3. Check food consumption and update bodies
        for si in 0..self.snake_count {
            if !new_heads[si].1 || !self.snakes[si].alive {
                continue;
            }
            let head = new_heads[si].0;

            // Check bounds
            if !self.mode.is_wrapped() && !head.in_bounds(self.width, self.height) {
                new_board.snakes[si].alive = false;
                continue;
            }

            // Check food
            let cell = self.get_cell(head);
            ate_food[si] = matches!(cell, Cell::Food | Cell::HazardFood);

            // Update snake body
            let snake = &mut new_board.snakes[si];
            snake.body.insert(0, head);
            snake.head = head;

            if ate_food[si] {
                snake.health = 100;
                snake.length += 1;
            } else {
                snake.body.pop();
                snake.health -= 1;

                // Hazard damage
                if matches!(cell, Cell::Hazard | Cell::HazardFood) {
                    snake.health -= self.hazard_damage;
                }
            }

            if snake.health <= 0 {
                snake.alive = false;
            }

            if let Some(&t) = snake.body.last() {
                snake.tail = t;
            }
        }

        // 4. Resolve head-to-head collisions
        for si in 0..self.snake_count {
            if !new_board.snakes[si].alive || !new_heads[si].1 {
                continue;
            }
            for sj in (si + 1)..self.snake_count {
                if !new_board.snakes[sj].alive || !new_heads[sj].1 {
                    continue;
                }
                if new_heads[si].0 == new_heads[sj].0 {
                    // Head-to-head collision
                    let len_i = new_board.snakes[si].length;
                    let len_j = new_board.snakes[sj].length;
                    if len_i > len_j {
                        new_board.snakes[sj].alive = false;
                    } else if len_j > len_i {
                        new_board.snakes[si].alive = false;
                    } else {
                        new_board.snakes[si].alive = false;
                        new_board.snakes[sj].alive = false;
                    }
                }
            }
        }

        // 5. Check body collisions for alive snakes
        for si in 0..self.snake_count {
            if !new_board.snakes[si].alive || !new_heads[si].1 {
                continue;
            }
            let head = new_heads[si].0;
            // Check against all other snake bodies (not heads, already handled)
            for sj in 0..self.snake_count {
                if !new_board.snakes[sj].alive {
                    continue;
                }
                if si == sj {
                    continue;
                }
                // Check if our head collides with their body (skip their head at index 0)
                for seg in new_board.snakes[sj].body.iter().skip(1) {
                    if head == *seg {
                        new_board.snakes[si].alive = false;
                        break;
                    }
                }
            }
        }

        // 6. Place surviving snakes back on the board
        for si in 0..new_board.snake_count {
            if !new_board.snakes[si].alive {
                continue;
            }
            let body_len = new_board.snakes[si].body.len();
            for (bi, &seg) in new_board.snakes[si].body.iter().enumerate() {
                let idx = new_board.coord_to_idx(seg);
                if idx < MAX_CELLS {
                    if bi == 0 {
                        new_board.cells[idx] = Cell::SnakeHead(si as u8);
                    } else {
                        // Match from_game_state: TTL = body_len - bi
                        // Tail (bi = body_len-1) gets TTL=1 (will move next turn)
                        let ttl = (body_len - bi) as u16;
                        new_board.cells[idx] = Cell::SnakeBody(si as u8, ttl);
                    }
                }
            }
        }

        // 7. Remove eaten food from the board
        for si in 0..self.snake_count {
            if ate_food[si] && new_heads[si].1 {
                let idx = new_board.coord_to_idx(new_heads[si].0);
                if idx < MAX_CELLS {
                    match new_board.cells[idx] {
                        Cell::SnakeHead(_) | Cell::SnakeBody(_, _) => {} // already occupied
                        _ => {}
                    }
                }
            }
        }

        new_board
    }

    /// Count alive snakes.
    pub fn alive_count(&self) -> usize {
        self.snakes[..self.snake_count]
            .iter()
            .filter(|s| s.alive)
            .count()
    }

    /// Check if we won.
    pub fn we_won(&self) -> bool {
        let us = &self.snakes[self.our_index];
        us.alive && self.alive_count() == 1
    }

    /// Check if we died.
    pub fn we_died(&self) -> bool {
        !self.snakes[self.our_index].alive
    }
}
