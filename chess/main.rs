//! Console chess. One `Piece` type carries a `PieceKind` tag; movement and
//! symbol are properties of the kind, not of six separate structs.

use std::fmt;

const BOARD_SIZE: i32 = 8;

type Offset = (i32, i32);

const DIAGONALS: [Offset; 4] = [(1, 1), (1, -1), (-1, 1), (-1, -1)];
const ORTHOGONALS: [Offset; 4] = [(0, 1), (0, -1), (1, 0), (-1, 0)];
const ALL_DIRECTIONS: [Offset; 8] = [
    (1, 1),
    (1, -1),
    (-1, 1),
    (-1, -1),
    (0, 1),
    (0, -1),
    (1, 0),
    (-1, 0),
];
const KNIGHT_JUMPS: [Offset; 8] = [
    (2, 1),
    (2, -1),
    (-2, 1),
    (-2, -1),
    (1, 2),
    (1, -2),
    (-1, 2),
    (-1, -2),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Color {
    Black,
    White,
}

impl Color {
    fn opponent(self) -> Self {
        match self {
            Color::Black => Color::White,
            Color::White => Color::Black,
        }
    }

    fn ansi_prefix(self) -> &'static str {
        match self {
            Color::Black => "\x1B[31;1m",
            Color::White => "\x1B[34;1m",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PieceKind {
    King,
    Queen,
    Rook,
    Bishop,
    Knight,
    Pawn,
}

impl PieceKind {
    const fn symbol(self) -> &'static str {
        match self {
            PieceKind::King => "KI",
            PieceKind::Queen => "QU",
            PieceKind::Rook => "RO",
            PieceKind::Bishop => "BI",
            PieceKind::Knight => "KN",
            PieceKind::Pawn => "PA",
        }
    }
}

const INITIAL_SETUP: [(PieceKind, i32, i32); 16] = [
    (PieceKind::Rook, 0, 0),
    (PieceKind::Knight, 1, 0),
    (PieceKind::Bishop, 2, 0),
    (PieceKind::Queen, 3, 0),
    (PieceKind::King, 4, 0),
    (PieceKind::Bishop, 5, 0),
    (PieceKind::Knight, 6, 0),
    (PieceKind::Rook, 7, 0),
    (PieceKind::Pawn, 0, 1),
    (PieceKind::Pawn, 1, 1),
    (PieceKind::Pawn, 2, 1),
    (PieceKind::Pawn, 3, 1),
    (PieceKind::Pawn, 4, 1),
    (PieceKind::Pawn, 5, 1),
    (PieceKind::Pawn, 6, 1),
    (PieceKind::Pawn, 7, 1),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ChessPosition {
    x_coord: i32,
    y_coord: i32,
}

impl ChessPosition {
    fn new(x_coord: i32, y_coord: i32) -> Self {
        ChessPosition { x_coord, y_coord }
    }

    fn from_string(string: &str) -> Option<Self> {
        let chars: Vec<char> = string.chars().collect();
        if chars.len() != 2 {
            return None;
        }
        let x_coord = chars[0] as i32 - 'a' as i32;
        let y_coord = chars[1] as i32 - '1' as i32;
        Some(ChessPosition::new(x_coord, y_coord))
    }

    fn offset_by(self, (dx, dy): Offset) -> Self {
        ChessPosition::new(self.x_coord + dx, self.y_coord + dy)
    }
}

impl fmt::Display for ChessPosition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let file = (b'a' + self.x_coord as u8) as char;
        write!(f, "{}{}", file, self.y_coord + 1)
    }
}

#[derive(Debug, Clone, Copy)]
struct Piece {
    kind: PieceKind,
    position: ChessPosition,
    color: Color,
    moved: bool,
}

impl Piece {
    fn new(kind: PieceKind, position: ChessPosition, color: Color) -> Self {
        Piece {
            kind,
            position,
            color,
            moved: false,
        }
    }

    fn symbol(&self) -> String {
        format!("{}{}\x1B[0m", self.color.ansi_prefix(), self.kind.symbol())
    }

    fn move_to(&mut self, target: ChessPosition) {
        self.position = target;
        self.moved = true;
    }

    /// Which way this piece's pawns advance.
    fn forward(&self) -> i32 {
        match self.color {
            Color::White => 1,
            Color::Black => -1,
        }
    }

    /// Where this piece may move.
    fn movable_positions(&self, board: &ChessBoard) -> Vec<ChessPosition> {
        match self.kind {
            PieceKind::King => self.steps(board, &ALL_DIRECTIONS),
            PieceKind::Knight => self.steps(board, &KNIGHT_JUMPS),
            PieceKind::Queen => self.rays(board, &ALL_DIRECTIONS),
            PieceKind::Rook => self.rays(board, &ORTHOGONALS),
            PieceKind::Bishop => self.rays(board, &DIAGONALS),
            PieceKind::Pawn => self.pawn_moves(board),
        }
    }

    /// What this piece attacks — the same squares, except a pawn guards its
    /// diagonals whether or not anything is standing on them.
    fn threatened_positions(&self, board: &ChessBoard) -> Vec<ChessPosition> {
        match self.kind {
            PieceKind::Pawn => self
                .pawn_diagonals()
                .into_iter()
                .filter(|target| board.contains(*target))
                .collect(),
            _ => self.movable_positions(board),
        }
    }

    fn steps(&self, board: &ChessBoard, offsets: &[Offset]) -> Vec<ChessPosition> {
        offsets
            .iter()
            .map(|offset| self.position.offset_by(*offset))
            .filter(|target| board.is_empty(*target) || board.holds_enemy_of(*target, self.color))
            .collect()
    }

    fn rays(&self, board: &ChessBoard, directions: &[Offset]) -> Vec<ChessPosition> {
        directions
            .iter()
            .flat_map(|direction| board.ray_targets(self.position, self.color, *direction))
            .collect()
    }

    fn pawn_moves(&self, board: &ChessBoard) -> Vec<ChessPosition> {
        let mut moves = Vec::new();
        let one_step = self.position.offset_by((0, self.forward()));
        if board.is_empty(one_step) {
            moves.push(one_step);
            let two_steps = self.position.offset_by((0, 2 * self.forward()));
            if !self.moved && board.is_empty(two_steps) {
                moves.push(two_steps);
            }
        }
        moves.extend(
            self.pawn_diagonals()
                .into_iter()
                .filter(|target| board.holds_enemy_of(*target, self.color)),
        );
        moves
    }

    fn pawn_diagonals(&self) -> [ChessPosition; 2] {
        let forward = self.forward();
        [
            self.position.offset_by((-1, forward)),
            self.position.offset_by((1, forward)),
        ]
    }
}

#[derive(Debug, Clone)]
struct ChessBoard {
    size: i32,
    pieces: Vec<Piece>,
}

impl ChessBoard {
    fn new(size: i32) -> Self {
        let mut board = ChessBoard {
            size,
            pieces: Vec::new(),
        };
        for &(kind, x, y) in &INITIAL_SETUP {
            board
                .pieces
                .push(Piece::new(kind, ChessPosition::new(x, y), Color::White));
            // Mirror the rank only: mirroring the file too would land the
            // black king on d8 opposite the white king on e1.
            board.pieces.push(Piece::new(
                kind,
                ChessPosition::new(x, size - y - 1),
                Color::Black,
            ));
        }
        board
    }

    fn contains(&self, position: ChessPosition) -> bool {
        (0..self.size).contains(&position.x_coord) && (0..self.size).contains(&position.y_coord)
    }

    fn piece_at(&self, position: ChessPosition) -> Option<&Piece> {
        self.pieces.iter().find(|piece| piece.position == position)
    }

    /// Derived rather than cached: a stored king position is one more thing to
    /// keep in step with the piece list.
    fn king_position(&self, color: Color) -> Option<ChessPosition> {
        self.pieces
            .iter()
            .find(|piece| piece.kind == PieceKind::King && piece.color == color)
            .map(|king| king.position)
    }

    /// Squares reachable along one direction, stopping at the first piece and
    /// including it only when it is an enemy.
    fn ray_targets(
        &self,
        from: ChessPosition,
        own_color: Color,
        direction: Offset,
    ) -> Vec<ChessPosition> {
        let mut targets = Vec::new();
        let mut current = from.offset_by(direction);
        while self.contains(current) {
            if let Some(piece) = self.piece_at(current) {
                if piece.color != own_color {
                    targets.push(current);
                }
                break;
            }
            targets.push(current);
            current = current.offset_by(direction);
        }
        targets
    }

    fn is_empty(&self, position: ChessPosition) -> bool {
        self.contains(position) && self.piece_at(position).is_none()
    }

    fn holds_enemy_of(&self, position: ChessPosition, color: Color) -> bool {
        self.piece_at(position)
            .is_some_and(|piece| piece.color != color)
    }

    fn execute_move(&mut self, command: MoveCommand) {
        self.pieces.retain(|piece| piece.position != command.dst);
        if let Some(source) = self
            .pieces
            .iter_mut()
            .find(|piece| piece.position == command.src)
        {
            source.move_to(command.dst);
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct MoveCommand {
    src: ChessPosition,
    dst: ChessPosition,
}

impl MoveCommand {
    fn from_string(string: &str) -> Option<Self> {
        let tokens: Vec<&str> = string.split_whitespace().collect();
        if tokens.len() != 2 {
            return None;
        }
        Some(MoveCommand {
            src: ChessPosition::from_string(tokens[0])?,
            dst: ChessPosition::from_string(tokens[1])?,
        })
    }
}

trait Renderer {
    fn render(&self, board: &ChessBoard);
    fn print_line(&self, line: &str);
}

struct ConsoleRenderer;

impl Renderer for ConsoleRenderer {
    fn render(&self, board: &ChessBoard) {
        for row in (0..board.size).rev() {
            self.draw_row(board, row);
        }
        let mut legend = " ".repeat(3);
        for column in 0..board.size {
            legend.push((b'a' + column as u8) as char);
        }
        println!("{legend}");
    }

    fn print_line(&self, line: &str) {
        println!("{line}");
    }
}

impl ConsoleRenderer {
    fn draw_row(&self, board: &ChessBoard, row: i32) {
        let white_square = "\u{001b}[47m";
        let black_square = "\u{001b}[40m";
        let reset = "\u{001b}[0m";

        print!("{:<2} ", row + 1);
        for column in 0..board.size {
            let shade = if (column + row % 2) % 2 == 1 {
                black_square
            } else {
                white_square
            };
            let contents = board
                .piece_at(ChessPosition::new(column, row))
                .map_or_else(|| " ".to_owned(), |piece| piece.symbol());
            print!("{shade}{contents}{reset}");
        }
        println!();
    }
}

struct ChessGame<'a> {
    board: ChessBoard,
    renderer: &'a dyn Renderer,
    turn: Color,
}

impl<'a> ChessGame<'a> {
    fn new(renderer: &'a dyn Renderer) -> Self {
        ChessGame {
            board: ChessBoard::new(BOARD_SIZE),
            renderer,
            turn: Color::White,
        }
    }

    fn run(&mut self) {
        self.renderer.render(&self.board);
        loop {
            let Some(line) = read_line() else {
                return; // end of input
            };
            let Some(command) = MoveCommand::from_string(&line) else {
                self.renderer
                    .print_line("Invalid command, please re-enter.");
                continue;
            };
            if !self.is_legal(command) {
                self.renderer.print_line(&format!(
                    "Illegal move {} {}, please re-enter.",
                    command.src, command.dst
                ));
                continue;
            }
            self.board.execute_move(command);
            self.turn = self.turn.opponent();
            self.renderer.render(&self.board);
        }
    }

    /// A move is legal when the mover owns the piece, the destination is one
    /// the piece can reach, and the resulting position leaves its own king
    /// unattacked.
    fn is_legal(&self, command: MoveCommand) -> bool {
        let Some(source) = self.board.piece_at(command.src) else {
            return false;
        };
        if source.color != self.turn
            || !source.movable_positions(&self.board).contains(&command.dst)
        {
            return false;
        }
        let mut projected = self.board.clone();
        projected.execute_move(command);
        !projected.is_king_attacked(self.turn)
    }
}

impl ChessBoard {
    fn is_king_attacked(&self, color: Color) -> bool {
        let Some(king) = self.king_position(color) else {
            return false;
        };
        self.pieces
            .iter()
            .filter(|piece| piece.color == color.opponent())
            .any(|piece| piece.threatened_positions(self).contains(&king))
    }
}

fn read_line() -> Option<String> {
    let mut input = String::new();
    match std::io::stdin().read_line(&mut input) {
        Ok(0) | Err(_) => None,
        Ok(_) => Some(input),
    }
}

fn main() {
    ChessGame::new(&ConsoleRenderer).run();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(square: &str) -> ChessPosition {
        ChessPosition::from_string(square).expect("valid square")
    }

    fn board_with(pieces: &[(PieceKind, Color, &str)]) -> ChessBoard {
        ChessBoard {
            size: BOARD_SIZE,
            pieces: pieces
                .iter()
                .map(|&(kind, color, square)| Piece::new(kind, at(square), color))
                .collect(),
        }
    }

    #[test]
    fn test_initial_setup() {
        let board = ChessBoard::new(BOARD_SIZE);
        assert_eq!(board.pieces.len(), 32);
        assert_eq!(board.king_position(Color::White), Some(at("e1")));
        assert_eq!(board.king_position(Color::Black), Some(at("e8")));
    }

    #[test]
    fn test_rook_blocked() {
        let board = board_with(&[
            (PieceKind::Rook, Color::White, "a1"),
            (PieceKind::Pawn, Color::White, "a3"),
            (PieceKind::Pawn, Color::Black, "d1"),
        ]);
        let moves = board.piece_at(at("a1")).unwrap().movable_positions(&board);
        assert!(moves.contains(&at("a2")));
        assert!(!moves.contains(&at("a3")), "own piece blocks");
        assert!(!moves.contains(&at("a4")), "cannot slide past a blocker");
        assert!(moves.contains(&at("d1")), "captures the enemy it reaches");
        assert!(!moves.contains(&at("e1")), "stops at the captured square");
    }

    #[test]
    fn test_pawn_push() {
        let board = board_with(&[(PieceKind::Pawn, Color::White, "b2")]);
        let moves = board.piece_at(at("b2")).unwrap().movable_positions(&board);
        assert!(moves.contains(&at("b3")));
        assert!(moves.contains(&at("b4")), "two squares on the first move");
        assert!(
            !moves.contains(&at("c3")),
            "no capture on an empty diagonal"
        );
    }

    #[test]
    fn test_pawn_capture() {
        let board = board_with(&[
            (PieceKind::Pawn, Color::White, "b2"),
            (PieceKind::Knight, Color::Black, "b3"),
            (PieceKind::Knight, Color::Black, "c3"),
        ]);
        let moves = board.piece_at(at("b2")).unwrap().movable_positions(&board);
        assert!(!moves.contains(&at("b3")), "blocked head-on");
        assert!(!moves.contains(&at("b4")), "cannot jump the blocker");
        assert!(moves.contains(&at("c3")), "captures diagonally");
    }

    #[test]
    fn test_pawn_direction() {
        let board = board_with(&[(PieceKind::Pawn, Color::Black, "b7")]);
        let moves = board.piece_at(at("b7")).unwrap().movable_positions(&board);
        assert!(moves.contains(&at("b6")), "black advances downward");
        assert!(!moves.contains(&at("b8")));
    }

    #[test]
    fn test_check_detection() {
        let board = board_with(&[
            (PieceKind::King, Color::White, "e1"),
            (PieceKind::Rook, Color::Black, "e8"),
        ]);
        assert!(board.is_king_attacked(Color::White));
        assert!(!board.is_king_attacked(Color::Black));
    }

    #[test]
    fn test_pinned_piece() {
        // The bishop on e2 is pinned by the rook on e8; moving it exposes e1.
        let mut game = ChessGame::new(&ConsoleRenderer);
        game.board = board_with(&[
            (PieceKind::King, Color::White, "e1"),
            (PieceKind::Bishop, Color::White, "e2"),
            (PieceKind::Rook, Color::Black, "e8"),
        ]);
        let command = MoveCommand {
            src: at("e2"),
            dst: at("d3"),
        };
        assert!(!game.is_legal(command));
    }

    #[test]
    fn test_capture_removes() {
        let mut board = board_with(&[
            (PieceKind::Rook, Color::White, "a1"),
            (PieceKind::Pawn, Color::Black, "a7"),
        ]);
        board.execute_move(MoveCommand {
            src: at("a1"),
            dst: at("a7"),
        });
        assert_eq!(board.pieces.len(), 1);
        assert_eq!(board.pieces[0].kind, PieceKind::Rook);
        assert_eq!(board.pieces[0].position, at("a7"));
    }
}
