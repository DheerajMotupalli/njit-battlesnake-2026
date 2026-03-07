use serde::{Deserialize, Serialize};
use std::fmt;

// ── Coordinates ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub struct Coord {
    pub x: i32,
    pub y: i32,
}

impl Coord {
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    /// Manhattan distance to another coordinate.
    pub fn dist(&self, other: &Coord) -> i32 {
        (self.x - other.x).abs() + (self.y - other.y).abs()
    }

    /// Move one step in the given direction.
    pub fn apply_move(&self, dir: Direction) -> Coord {
        match dir {
            Direction::Up => Coord::new(self.x, self.y + 1),
            Direction::Down => Coord::new(self.x, self.y - 1),
            Direction::Left => Coord::new(self.x - 1, self.y),
            Direction::Right => Coord::new(self.x + 1, self.y),
        }
    }

    /// Move one step in the given direction, wrapping around the board edges.
    pub fn apply_move_wrapped(&self, dir: Direction, width: i32, height: i32) -> Coord {
        let mut c = self.apply_move(dir);
        c.x = ((c.x % width) + width) % width;
        c.y = ((c.y % height) + height) % height;
        c
    }

    /// All 4 cardinal neighbors (no bounds checking).
    pub fn neighbors(&self) -> [Coord; 4] {
        [
            Coord::new(self.x, self.y + 1), // Up
            Coord::new(self.x, self.y - 1), // Down
            Coord::new(self.x - 1, self.y), // Left
            Coord::new(self.x + 1, self.y), // Right
        ]
    }

    /// Check if within bounds of a board.
    pub fn in_bounds(&self, width: i32, height: i32) -> bool {
        self.x >= 0 && self.x < width && self.y >= 0 && self.y < height
    }
}

// ── Direction ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}

impl Direction {
    pub const ALL: [Direction; 4] = [
        Direction::Up,
        Direction::Down,
        Direction::Left,
        Direction::Right,
    ];

    /// Return the opposite direction.
    pub fn opposite(self) -> Direction {
        match self {
            Direction::Up => Direction::Down,
            Direction::Down => Direction::Up,
            Direction::Left => Direction::Right,
            Direction::Right => Direction::Left,
        }
    }
}

impl fmt::Display for Direction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Direction::Up => write!(f, "up"),
            Direction::Down => write!(f, "down"),
            Direction::Left => write!(f, "left"),
            Direction::Right => write!(f, "right"),
        }
    }
}

impl Serialize for Direction {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

// ── Battlesnake API Types ────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct GameState {
    pub game: Game,
    pub turn: u32,
    pub board: Board,
    pub you: Snake,
}

#[derive(Debug, Deserialize)]
pub struct Game {
    pub id: String,
    pub ruleset: Ruleset,
    pub timeout: u32,
}

#[derive(Debug, Deserialize)]
pub struct Ruleset {
    pub name: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub settings: Option<RulesetSettings>,
}

#[derive(Debug, Deserialize)]
pub struct RulesetSettings {
    #[serde(default)]
    pub hazard_damage_per_turn: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct Board {
    pub height: i32,
    pub width: i32,
    pub food: Vec<Coord>,
    pub hazards: Vec<Coord>,
    pub snakes: Vec<Snake>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Snake {
    pub id: String,
    pub name: String,
    pub health: i32,
    pub body: Vec<Coord>,
    pub head: Coord,
    pub length: i32,
    pub shout: Option<String>,
}

impl Snake {
    pub fn tail(&self) -> Coord {
        *self.body.last().unwrap()
    }

    pub fn is_alive(&self) -> bool {
        self.health > 0 && !self.body.is_empty()
    }
}

// ── API Response Types ───────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct InfoResponse {
    pub apiversion: &'static str,
    pub author: &'static str,
    pub color: &'static str,
    pub head: &'static str,
    pub tail: &'static str,
    pub version: &'static str,
}

#[derive(Serialize)]
pub struct MoveResponse {
    #[serde(rename = "move")]
    pub mv: Direction,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shout: Option<String>,
}

// ── Game Mode ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameMode {
    Standard,
    Royale,
    Constrictor,
    Wrapped,
    Solo,
}

impl GameMode {
    pub fn from_ruleset(name: &str) -> Self {
        match name {
            "royale" => GameMode::Royale,
            "constrictor" => GameMode::Constrictor,
            "wrapped" => GameMode::Wrapped,
            "solo" => GameMode::Solo,
            _ => GameMode::Standard,
        }
    }

    pub fn is_wrapped(&self) -> bool {
        *self == GameMode::Wrapped
    }
}
