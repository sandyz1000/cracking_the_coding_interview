use std::collections::HashMap;
use std::fmt;
use std::io;

fn index(x: u8, y: u8) -> u8 {
    x + (8 * y)
}

fn in_bounds(x: u8, y: u8) -> bool {
    x < 8 && y < 8
}

fn abs_dif(x: u8, y: u8) -> u8 {
    (x as i16 - y as i16).unsigned_abs() as u8
}

/// -1, 0 or 1: the direction to walk from `from` towards `to`.
fn step(from: u8, to: u8) -> i8 {
    match to.cmp(&from) {
        std::cmp::Ordering::Greater => 1,
        std::cmp::Ordering::Less => -1,
        std::cmp::Ordering::Equal => 0,
    }
}

pub trait BoardMethod {
    fn board(&self) -> &HashMap<u8, Box<dyn PieceMaker>>;
    fn board_mut(&mut self) -> &mut HashMap<u8, Box<dyn PieceMaker>>;

    fn insert(&mut self, key: u8, value: Box<dyn PieceMaker>) {
        self.board_mut().insert(key, value);
    }

    fn remove(&mut self, key: &u8) -> Option<Box<dyn PieceMaker>> {
        self.board_mut().remove(key)
    }

    fn get(&self, key: &u8) -> Option<&dyn PieceMaker> {
        self.board().get(key).map(|piece| &**piece)
    }
}

pub struct BoardState {
    board: HashMap<u8, Box<dyn PieceMaker>>,
}

impl BoardMethod for BoardState {
    fn board(&self) -> &HashMap<u8, Box<dyn PieceMaker>> {
        &self.board
    }

    fn board_mut(&mut self) -> &mut HashMap<u8, Box<dyn PieceMaker>> {
        &mut self.board
    }
}

impl BoardState {
    pub fn new() -> Self {
        BoardState {
            board: HashMap::new(),
        }
    }

    pub fn reset(&mut self) {
        self.board.clear();

        let back_rank: [fn(bool) -> Box<dyn PieceMaker>; 8] = [
            Rook::boxed,
            Knight::boxed,
            Bishop::boxed,
            King::boxed,
            Queen::boxed,
            Bishop::boxed,
            Knight::boxed,
            Rook::boxed,
        ];

        for (file, make) in back_rank.into_iter().enumerate() {
            self.insert(file as u8, make(false));
            self.insert(56 + file as u8, make(true));
        }
        for file in 0u8..8 {
            self.insert(8 + file, Pawn::boxed(false));
            self.insert(48 + file, Pawn::boxed(true));
        }
    }

    pub fn draw_board(&self) {
        println!("    0    1    2    3    4    5    6    7   ");
        println!("  -----------------------------------------");

        for y in 0u8..8 {
            print!("{} ", y);
            for x in 0u8..8 {
                match self.get(&index(x, y)) {
                    Some(piece) => print!("| {} ", piece),
                    None => print!("|    "),
                }
            }
            println!("|");
            println!("  -----------------------------------------");
        }
    }

    // returns true if no piece stands on (x, y)
    pub fn is_empty(&self, x: u8, y: u8) -> bool {
        self.get(&index(x, y)).is_none()
    }

    // returns the player (0 or 1) who owns the piece at (x, y) or -1 if no piece at (x, y)
    pub fn piece_player_at(&self, x: u8, y: u8) -> i8 {
        match self.get(&index(x, y)) {
            Some(piece) if piece.player() => 1,
            Some(_) => 0,
            None => -1,
        }
    }

    /// Every square strictly between origin and destination is empty. Walks one
    /// step at a time so it serves straight and diagonal lines alike, and so a
    /// descending move is not silently skipped by an empty range.
    pub fn path_is_clear(&self, x: u8, y: u8, x2: u8, y2: u8) -> bool {
        let (dx, dy) = (step(x, x2), step(y, y2));
        let (mut cx, mut cy) = (x as i8 + dx, y as i8 + dy);
        while (cx, cy) != (x2 as i8, y2 as i8) {
            if !self.is_empty(cx as u8, cy as u8) {
                return false;
            }
            cx += dx;
            cy += dy;
        }
        true
    }

    /// The destination holds nothing, or an enemy piece.
    pub fn can_land_on(&self, x: u8, y: u8, x2: u8, y2: u8) -> bool {
        self.piece_player_at(x2, y2) != self.piece_player_at(x, y)
    }

    // returns true if piece successfully moved from (x, y) to (x2, y2)
    pub fn move_piece(&mut self, x: u8, y: u8, x2: u8, y2: u8) -> bool {
        if !self.is_valid_move(x, y, x2, y2) {
            return false;
        }
        match self.remove(&index(x, y)) {
            Some(piece) => {
                self.remove(&index(x2, y2));
                self.insert(index(x2, y2), piece);
                true
            }
            None => false,
        }
    }

    pub fn is_valid_move(&self, x: u8, y: u8, x2: u8, y2: u8) -> bool {
        match self.get(&index(x, y)) {
            Some(piece) => piece.validate(self, x, y, x2, y2),
            None => false,
        }
    }
}

impl Default for BoardState {
    fn default() -> Self {
        Self::new()
    }
}

/// The board is passed in rather than held: a piece lives inside the board, so
/// owning a handle back to it would be a cycle, and a `&mut` one would alias
/// the map the piece was looked up from.
pub trait PieceMaker: fmt::Display {
    fn player(&self) -> bool;

    fn validate(&self, board: &BoardState, x: u8, y: u8, x2: u8, y2: u8) -> bool;
}

/// Same body for all six kinds: a piece differs only in `validate`.
macro_rules! piece {
    ($name:ident, $player_0:literal, $player_1:literal) => {
        pub struct $name {
            player: bool,
        }

        impl $name {
            fn new(player: bool) -> Self {
                $name { player }
            }

            fn boxed(player: bool) -> Box<dyn PieceMaker> {
                Box::new($name::new(player))
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                if self.player {
                    write!(f, $player_1)
                } else {
                    write!(f, $player_0)
                }
            }
        }
    };
}

piece!(King, "1K", "2K");
piece!(Queen, "1Q", "2Q");
piece!(Rook, "1R", "2R");
piece!(Knight, "1N", "2N");
piece!(Bishop, "1B", "2B");
piece!(Pawn, "1P", "2P");

impl PieceMaker for King {
    fn player(&self) -> bool {
        self.player
    }

    // can move 8 ways, one space, can only take enemy player piece
    fn validate(&self, board: &BoardState, x: u8, y: u8, x2: u8, y2: u8) -> bool {
        if !in_bounds(x2, y2) || (x == x2 && y == y2) {
            return false;
        }
        abs_dif(x, x2) <= 1 && abs_dif(y, y2) <= 1 && board.can_land_on(x, y, x2, y2)
    }
}

impl PieceMaker for Rook {
    fn player(&self) -> bool {
        self.player
    }

    // can move 4 ways, optionally move into piece owned by enemy player
    fn validate(&self, board: &BoardState, x: u8, y: u8, x2: u8, y2: u8) -> bool {
        if !in_bounds(x2, y2) || (x == x2 && y == y2) {
            return false;
        }
        if x != x2 && y != y2 {
            return false; // if not aligned, false
        }
        board.path_is_clear(x, y, x2, y2) && board.can_land_on(x, y, x2, y2)
    }
}

impl PieceMaker for Bishop {
    fn player(&self) -> bool {
        self.player
    }

    // can move 4 ways diagonally, until contact with piece is made, can only take enemy player piece
    fn validate(&self, board: &BoardState, x: u8, y: u8, x2: u8, y2: u8) -> bool {
        if !in_bounds(x2, y2) || (x == x2 && y == y2) {
            return false;
        }
        if abs_dif(x, x2) != abs_dif(y, y2) {
            return false;
        }
        board.path_is_clear(x, y, x2, y2) && board.can_land_on(x, y, x2, y2)
    }
}

impl PieceMaker for Knight {
    fn player(&self) -> bool {
        self.player
    }

    // can move in L pattern in any direction, jumping over pieces, can only take enemy player piece
    fn validate(&self, board: &BoardState, x: u8, y: u8, x2: u8, y2: u8) -> bool {
        if !in_bounds(x2, y2) {
            return false;
        }
        let l_shaped = (abs_dif(x, x2) == 1 && abs_dif(y, y2) == 2)
            || (abs_dif(x, x2) == 2 && abs_dif(y, y2) == 1);
        l_shaped && board.can_land_on(x, y, x2, y2)
    }
}

impl PieceMaker for Queen {
    fn player(&self) -> bool {
        self.player
    }

    fn validate(&self, board: &BoardState, x: u8, y: u8, x2: u8, y2: u8) -> bool {
        Rook::new(self.player).validate(board, x, y, x2, y2)
            || Bishop::new(self.player).validate(board, x, y, x2, y2)
    }
}

impl PieceMaker for Pawn {
    fn player(&self) -> bool {
        self.player
    }

    // can move forward 1 or 2 if at starting location and NOT blocked by piece
    // can move diagonally forward 1 ONLY IF blocked by piece of enemy player
    fn validate(&self, board: &BoardState, x: u8, y: u8, x2: u8, y2: u8) -> bool {
        if !in_bounds(x2, y2) {
            return false;
        }
        // player 0 starts on rank 1 and advances up the board; player 1 mirrors it.
        let (forward, start_rank, enemy) = if self.player { (-1, 6, 0) } else { (1, 1, 1) };
        let advance = y2 as i8 - y as i8;

        if x == x2 {
            if advance == forward {
                return board.is_empty(x2, y2);
            }
            return advance == 2 * forward
                && y == start_rank
                && board.path_is_clear(x, y, x2, y2)
                && board.is_empty(x2, y2);
        }
        // if attacking diagonally
        abs_dif(x, x2) == 1 && advance == forward && board.piece_player_at(x2, y2) == enemy
    }
}

fn main() {
    let mut board = BoardState::new();

    board.reset();
    board.draw_board();

    let help_message = "move <origin X> <origin Y> <destination X> <destination Y>\n\
        valid_move <origin X> <origin Y> <destination X> <destination Y>\n\
        draw\n\
        reset\n\
        exit\n\
        help";

    loop {
        println!(":");
        let mut input = String::new();
        if io::stdin().read_line(&mut input).unwrap_or(0) == 0 {
            return; // end of input
        }

        let vector: Vec<&str> = input.split_whitespace().collect();

        match vector.first() {
            Some(&"move") => {
                call_move(&mut board, &vector);
            }
            Some(&"valid_move") => {
                call_valid_move(&mut board, &vector);
            }
            Some(&"draw") => {
                board.draw_board();
            }
            Some(&"reset") => {
                board.reset();
                board.draw_board();
            }
            Some(&"help") => {
                println!("{}", help_message);
            }
            Some(&"exit") => {
                println!("EXITING");
                return;
            }
            _ => {
                println!("unknown command, try help");
                continue;
            }
        }
    }
}

/// The four coordinates following the command word, or `None` on bad input.
fn parse_coordinates(s: &[&str], usage: &str) -> Option<[u8; 4]> {
    let mut array: [u8; 4] = [0; 4];
    for i in 1..5 {
        match s.get(i).and_then(|slice| slice.parse::<u8>().ok()) {
            Some(value) => array[i - 1] = value,
            None => {
                println!("usage: {usage}");
                return None;
            }
        }
    }
    Some(array)
}

fn call_move(bs: &mut BoardState, s: &[&str]) -> bool {
    let Some(a) = parse_coordinates(
        s,
        "move <origin X> <origin Y> <destination X> <destination Y>",
    ) else {
        return false;
    };

    if bs.move_piece(a[0], a[1], a[2], a[3]) {
        println!("successfully moved");
        bs.draw_board();
        true
    } else {
        println!("invalid move");
        false
    }
}

fn call_valid_move(bs: &mut BoardState, s: &[&str]) -> bool {
    let Some(a) = parse_coordinates(
        s,
        "is_valid_move <origin X> <origin Y> <destination X> <destination Y>",
    ) else {
        return false;
    };

    if bs.is_valid_move(a[0], a[1], a[2], a[3]) {
        println!("valid move");
        true
    } else {
        println!("invalid move");
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh() -> BoardState {
        let mut board = BoardState::new();
        board.reset();
        board
    }

    #[test]
    fn test_reset_layout() {
        let board = fresh();
        assert_eq!(board.board().len(), 32);
        assert_eq!(board.piece_player_at(0, 0), 0);
        assert_eq!(board.piece_player_at(0, 7), 1);
        assert!(board.is_empty(0, 3));
    }

    #[test]
    fn test_pawn_opening() {
        let board = fresh();
        assert!(board.is_valid_move(0, 1, 0, 2));
        assert!(board.is_valid_move(0, 1, 0, 3), "two from the start rank");
        assert!(!board.is_valid_move(0, 1, 0, 4));
        assert!(!board.is_valid_move(0, 1, 1, 2), "no capture on empty");
    }

    #[test]
    fn test_pawn_blocked() {
        let mut board = fresh();
        board.insert(index(0, 2), Pawn::boxed(true));
        assert!(!board.is_valid_move(0, 1, 0, 2), "blocked head-on");
        assert!(!board.is_valid_move(0, 1, 0, 3), "cannot jump the blocker");
        assert!(board.is_valid_move(1, 1, 0, 2), "captures diagonally");
    }

    #[test]
    fn test_rook_blocked() {
        let mut board = fresh();
        assert!(!board.is_valid_move(0, 0, 0, 3), "own pawn blocks");
        board.remove(&index(0, 1));
        assert!(board.is_valid_move(0, 0, 0, 3));
        assert!(board.is_valid_move(0, 0, 0, 6), "captures enemy pawn");
        assert!(!board.is_valid_move(0, 0, 0, 7), "stops at the capture");
    }

    #[test]
    fn test_rook_descending() {
        let mut board = fresh();
        board.remove(&index(0, 6));
        assert!(board.is_valid_move(0, 7, 0, 4), "downward path is checked");
        assert!(
            !board.is_valid_move(0, 7, 0, 0),
            "blocked by the pawn on rank 1"
        );
    }

    #[test]
    fn test_knight_jumps() {
        let board = fresh();
        assert!(board.is_valid_move(1, 0, 0, 2), "jumps over its own rank");
        assert!(board.is_valid_move(1, 0, 2, 2));
        assert!(!board.is_valid_move(1, 0, 1, 2), "not an L");
    }

    #[test]
    fn test_bishop_diagonal() {
        let mut board = fresh();
        assert!(!board.is_valid_move(2, 0, 4, 2), "own pawn blocks");
        board.remove(&index(3, 1));
        assert!(board.is_valid_move(2, 0, 4, 2));
        assert!(board.is_valid_move(2, 0, 6, 4));
    }

    #[test]
    fn test_queen_combines() {
        let mut board = fresh();
        board.remove(&index(4, 1));
        assert!(board.is_valid_move(4, 0, 4, 4), "straight, like a rook");
        board.remove(&index(3, 1));
        assert!(board.is_valid_move(4, 0, 2, 2), "diagonal, like a bishop");
        assert!(!board.is_valid_move(4, 0, 5, 3), "neither");
    }

    #[test]
    fn test_king_one_square() {
        let mut board = fresh();
        board.remove(&index(3, 1));
        assert!(board.is_valid_move(3, 0, 3, 1));
        assert!(!board.is_valid_move(3, 0, 3, 2), "further than one square");
        assert!(!board.is_valid_move(3, 0, 4, 0), "own queen");
    }

    #[test]
    fn test_move_applies() {
        let mut board = fresh();
        assert!(board.move_piece(0, 1, 0, 3));
        assert!(board.is_empty(0, 1));
        assert_eq!(board.piece_player_at(0, 3), 0);
        assert_eq!(board.board().len(), 32);
    }

    #[test]
    fn test_capture_removes() {
        let mut board = fresh();
        board.remove(&index(0, 1)); // 31 left
        assert!(board.move_piece(0, 0, 0, 6), "rook takes the enemy pawn");
        assert_eq!(board.board().len(), 30);
        assert_eq!(board.piece_player_at(0, 6), 0);
    }

    #[test]
    fn test_rejected_move_keeps() {
        let mut board = fresh();
        assert!(!board.move_piece(0, 0, 0, 3), "blocked by its own pawn");
        assert_eq!(board.piece_player_at(0, 0), 0);
        assert_eq!(board.board().len(), 32);
    }
}
