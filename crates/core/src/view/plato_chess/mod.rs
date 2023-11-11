use crate::font::Fonts;
use crate::framebuffer::{Framebuffer, UpdateMode};
use crate::settings::ChessEngineSettings;

use crate::view::common::locate_by_id;
use crate::view::common::{toggle_main_menu, toggle_battery_menu, toggle_clock_menu};
use crate::view::filler::Filler;
use crate::view::key::KeyKind;
use crate::view::menu::{Menu, MenuKind};
use crate::view::top_bar::TopBar;
use crate::view::ID_FEEDER;
use crate::view::{
    Bus, Context, EntryId, EntryKind, Event, Hub, Id, Rectangle, RenderData, RenderQueue, SliderId, View, ViewId,
    GestureEvent,
};
use crate::view::{SMALL_BAR_HEIGHT, THICKNESS_MEDIUM};

use crate::color::BLACK;
use crate::device::CURRENT_DEVICE;
use crate::geom::halves;
use crate::unit::scale_by_dpi;

mod bottom_bar;
use self::bottom_bar::BottomBar;

mod sliders_dialog;
use sliders_dialog::{SlidersConstructors, SlidersDialog};

mod chess_board;
mod chess_cell;
use self::chess_board::ChessBoard;

mod chess_moves;
use self::chess_moves::{DetailedChessMove, MovesArea};

use chess::{ChessMove, Color, Piece, Square, ALL_COLORS, ALL_SQUARES};
use chess_uci::uci_command::{parse, EngineCommand, GuiCommand, Opt};
use chess_uci::{GameOption, Player, UciEngine};

use log::{debug, info, warn};

use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{ChildStdin, ChildStdout};
use std::process::{Command, Stdio};
use std::str::FromStr;
use std::thread;
use std::time::Duration;
use std::time::Instant;

pub struct PlatoChess {
    id: Id,
    chess: UciEngine<ChildStdout, ChildStdin>,
    chess_engine_settings: ChessEngineSettings,
    rect: Rectangle,
    children: Vec<Box<dyn View>>,
    child_chess_index: usize,
    active_square: Option<Square>,
    en_passant: Option<Square>,
    move_start: Instant,
}

fn launch_engine(path: &PathBuf, hub: &Hub) -> Option<ChildStdin> {
    let (stdin, stdout) = if let Ok(mut process) = Command::new(path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        info!("Process {:?} launched as chess engine", path.as_path().to_str());
        (process.stdin.take(), process.stdout.take())
    } else {
        warn!("No {:?} chess engine", path.to_str());
        (None, None)
    };

    if let Some(source) = stdout {
        let mut source = BufReader::new(source);
        let hub = hub.clone();
        thread::spawn(move || {
            info!("In chess engine reading thread");
            let buf = &mut String::new();
            while let Ok(n) = source.read_line(buf) {
                if n == 0 {
                    break;
                }
                debug!("received line : {}", buf);
                match parse(&mut buf.clone()) {
                    Ok(EngineCommand::Info { .. }) => true,
                    Ok(command) => {
                        info!("received engine command: {:?}", command);
                        hub.send(Event::ChessCommand(command)).is_ok()
                    }
                    _ => false,
                };
                buf.clear();
            }
            info!("End of chess engine reading thread");
        });
    }

    stdin
}

impl PlatoChess {
    pub fn new(rect: Rectangle, rq: &mut RenderQueue, context: &mut Context, hub: &Hub) -> PlatoChess {
        let stdin = launch_engine(&context.settings.chess_engine_settings.path, hub);
        let mut new = PlatoChess {
            id: ID_FEEDER.next(),
            chess: UciEngine::new(None, stdin),
            chess_engine_settings: context.settings.chess_engine_settings.clone(),
            rect,
            children: Vec::new(),
            child_chess_index: 0,
            active_square: None,
            en_passant: None,
            move_start: Instant::now(),
        };

        let sizes = Self::compute_sizes(rect);
        info!("Constructing chess ui with sizes : {:?}", sizes);

        // Children 0: TopBar
        new.children.push(Box::new(TopBar::new(
            sizes[0],
            Event::Back,
            "♔♛ Chess".to_string(),
            context,
        )));

        let separator = Filler::new(sizes[1], BLACK);
        new.children.push(Box::new(separator) as Box<dyn View>);

        let board = ChessBoard::new(sizes[2]);
        // Children 2: Chess board
        new.children.push(Box::new(board) as Box<dyn View>);
        new.child_chess_index = new.children.len() - 1;

        let separator = Filler::new(sizes[3], BLACK);
        new.children.push(Box::new(separator) as Box<dyn View>);

        // Children 4: move bar
        let font_size = context.settings.chess_engine_settings.font_size;
        let margin_width = context.settings.chess_engine_settings.margin_width;
        let move_bar = MovesArea::new(sizes[4], font_size, margin_width, context);
        new.children.push(Box::new(move_bar) as Box<dyn View>);

        let separator = Filler::new(sizes[5], BLACK);
        new.children.push(Box::new(separator) as Box<dyn View>);

        let bottom_bar = BottomBar::new(sizes[6], "Status", Color::White, true, context);
        // Children 6: Bottom Bar
        new.children.push(Box::new(bottom_bar) as Box<dyn View>);

        new.chess.init();
        new.chess.game_option(GameOption::Player {
            color: Color::White,
            value: Player::Human { elo: None },
        });
        new.chess.game_option(GameOption::Player {
            color: Color::Black,
            value: Player::Engine {
                elo: Some(new.chess_engine_settings.elo),
            },
        });
        new.chess.push(GuiCommand::SetOption {
            option: Opt::SlowMover {
                value: new.chess_engine_settings.slow_motion,
            },
        });
        new.new_game(rq);

        rq.add(RenderData::new(new.id(), *new.rect(), UpdateMode::Full));
        new
    }

    fn compute_sizes(rect: Rectangle) -> [Rectangle; 7] {
        let dpi = CURRENT_DEVICE.dpi;
        let thickness = scale_by_dpi(THICKNESS_MEDIUM, dpi) as i32;
        let small_height = scale_by_dpi(SMALL_BAR_HEIGHT, dpi) as i32;
        let (small_thickness, big_thickness) = halves(thickness);
        [
            // TopBar
            rect![rect.min.x, rect.min.y,
                  rect.max.x, rect.min.y + small_height - small_thickness],
            // Separator
            rect![rect.min.x, rect.min.y + small_height - small_thickness,
                  rect.max.x, rect.min.y + small_height + big_thickness],
            // board
            rect![rect.min.x, rect.min.y + small_height + big_thickness,
                  rect.max.x, rect.max.y - 2 * small_height - small_thickness],
            //separator
            rect![rect.min.x, rect.max.y - 2 * small_height - small_thickness,
                  rect.max.x, rect.max.y - 2 * small_height + big_thickness],
            // moves area
            rect![rect.min.x, rect.max.y - 2 * small_height + big_thickness,
                  rect.max.x, rect.max.y - small_height,],
            //separator
            rect![rect.min.x, rect.max.y - small_height - small_thickness,
                  rect.max.x, rect.max.y - small_height + big_thickness],
            // bottom bar
            rect![rect.min.x, rect.max.y - small_height + big_thickness,
                rect.max.x, rect.max.y],]
    }

}

impl PlatoChess {
    fn new_game(&mut self, rq: &mut RenderQueue) {
        self.chess.new_game();
        if let Some(chess_moves) = self.children[4].downcast_mut::<MovesArea>() {
            chess_moves.clear(rq);
        }
        self.redraw_all_squares(rq);
        if let Some(bottom_bar) = self.children[6].downcast_mut::<BottomBar>() {
            let new_text = "New game".to_string();
            bottom_bar.update_name(&new_text, rq);
            bottom_bar.update_color(self.chess.side_to_move(), rq);
            let human_to_play = matches!(
                self.chess.get_player(self.chess.side_to_move()),
                Player::Human { elo: _ }
            );
            bottom_bar.update_player(human_to_play, rq);
            bottom_bar.update_clocks(
                self.chess.clock(Color::White).unwrap_or(Duration::new(0, 0)),
                self.chess.clock(Color::Black).unwrap_or(Duration::new(0, 0)),
                rq,
            );
        }
        self.move_start = Instant::now();
        self.start_move(rq);
    }

    fn start_move(&mut self, rq: &mut RenderQueue) {
        self.chess.stop();

        let human_player = matches!(
            self.chess.get_player(self.chess.side_to_move()),
            Player::Human { elo: _ }
        );

        // tell player on bottom bar
        if let Some(bottom_bar) = self.children[6].downcast_mut::<BottomBar>() {
            let new_text = format!(
                "{} to play",
                match self.chess.side_to_move() {
                    Color::White => "White",
                    Color::Black => "Black",
                }
            );
            bottom_bar.update_name(&new_text, rq);
            bottom_bar.update_color(self.chess.side_to_move(), rq);
            bottom_bar.update_player(human_player, rq);
            bottom_bar.update_clocks(
                self.chess.clock(Color::White).unwrap_or(Duration::new(0, 0)),
                self.chess.clock(Color::Black).unwrap_or(Duration::new(0, 0)),
                rq,
            );
        }

        self.move_start = Instant::now();
        if ! human_player {
            self.chess.go();
        }
    }

    fn make_move(
        &mut self, rq: &mut RenderQueue, chess_move: &ChessMove, context: &mut Context,
    ) -> Result<ChessMove, &str> {
        let position = self.chess.current_position();
        let is_valid = position.legal(*chess_move);
        if is_valid {
            let elapsed = self.move_start.elapsed();
            self.chess.update_clock(elapsed);

            info!("New move {:?} in {}s", chess_move, elapsed.as_secs());

            // append in moves listing
            if let Some(move_area) = self.children[4].downcast_mut::<MovesArea>() {
                move_area.append(&DetailedChessMove::new(position, chess_move), rq, context);
            }
            self.chess.make_move(chess_move);
            self.draw_move(chess_move, rq);

            Ok(*chess_move)
        } else {
            // detect promotion move
            if let Player::Human { elo: _ } = self.chess.get_player(self.chess.side_to_move()) {
                let (source, dest) = (chess_move.get_source(), chess_move.get_dest());
                if position.legal(ChessMove::new(source, dest, Some(Piece::Queen))) {
                    self.toggle_promotion_menu(chess_move, Some(true), rq, context);
                    return Err("Need promotion piece");
                }
            }
            Err("Invalid movement in current position")
        }
    }

    fn manage_chess_command(&mut self, command: &EngineCommand, rq: &mut RenderQueue, context: &mut Context) -> bool {
        match command {
            EngineCommand::UciOk => {
                if let Some(bottom_bar) = self.children[6].downcast_mut::<BottomBar>() {
                    if let Some(mut name) = self.chess.name() {
                        if let Some(author) = self.chess.author() {
                            name.push_str(" (by ");
                            name = name + &author + ")";
                        }
                        bottom_bar.update_name(&name, rq);
                    }
                }
                true
            }
            EngineCommand::BestMove { best_move: Some(best_move), ponder: _ } => {
                if let Player::Engine { elo: _ } = self.chess.get_player(self.chess.side_to_move()) {
                    if self.make_move(rq, best_move, context).is_ok() {
                        self.start_move(rq);
                        true
                    } else {
                        false
                    }
                } else {
                    false
                }
            },
            EngineCommand::BestMove { best_move: None, ponder: _ } => {
                if let Some(bottom_bar) = self.children[6].downcast_mut::<BottomBar>() {
                    let new_text = "# Mate".to_string();
                    bottom_bar.update_name(&new_text, rq);
                }
                true
            }
            _ => true,
        }
    }
}

impl PlatoChess {
    fn redraw_all_squares(&mut self, rq: &mut RenderQueue) {
        let position = self.chess.current_position();
        if let Some(child) = self.child_mut(self.child_chess_index).downcast_mut::<ChessBoard>() {
            for square in ALL_SQUARES {
                child.update_square(rq, square, position.piece_on(square), position.color_on(square));
            }
        }
    }

    pub fn draw_move(&mut self, chess_move: &ChessMove, rq: &mut RenderQueue) {
        let (source, dest) = (chess_move.get_source(), chess_move.get_dest());
        let position = self.chess.current_position();

        // detect that it was a castle
        let castle = if position.piece_on(dest) == Some(Piece::King) {
            match (source.to_index(), dest.to_index()) {
                (4, 6) => (Some(Square::from_str("f1")), Some(Square::from_str("h1"))),
                (4, 2) => (Some(Square::from_str("d1")), Some(Square::from_str("a1"))),
                (60, 62) => (Some(Square::from_str("f8")), Some(Square::from_str("h8"))),
                (60, 58) => (Some(Square::from_str("d8")), Some(Square::from_str("a8"))),
                _ => (None, None),
            }
        } else {
            (None, None)
        };

        // update board
        let old_en_passant = self.en_passant;
        if let Some(child) = self.child_mut(self.child_chess_index).downcast_mut::<ChessBoard>() {
            child.update_square(rq, source, position.piece_on(source), position.color_on(source));
            child.update_square(rq, dest, position.piece_on(dest), position.color_on(dest));
            if let (Some(Ok(a)), Some(Ok(b))) = castle {
                child.update_square(rq, a, position.piece_on(a), position.color_on(a));
                child.update_square(rq, b, position.piece_on(b), position.color_on(b));
            }
            if let Some(square) = old_en_passant {
                child.update_square(rq, square, position.piece_on(square), position.color_on(square));
            }
            self.en_passant = position.en_passant();
        }
    }

    fn toggle_chess_menu(
        &mut self, rect: Rectangle, enable: Option<bool>, rq: &mut RenderQueue, context: &mut Context,
    ) {
        if let Some(index) = locate_by_id(self, ViewId::ChessMenu) {
            // menu already exist
            if let Some(true) = enable {
                // toggle open on already open menu. Nothing
            } else {
                // toggle close
                rq.add(RenderData::new(self.id(), *self.child(index).rect(), UpdateMode::Gui));
                self.children.remove(index);
            }
        } else {
            // Menu does not exist
            if let Some(false) = enable {
                // toggle close on non-existing menu. Nothing.
            } else {
                // create menu

                let white_human = matches!(self.chess.get_player(Color::White), Player::Human { elo: _ });
                let black_human = matches!(self.chess.get_player(Color::Black), Player::Human { elo: _ });
                let entries = vec![
                    EntryKind::Command("New".to_string(), EntryId::New),
                    EntryKind::Command("Save".to_string(), EntryId::Save),
                    EntryKind::Separator,
                    EntryKind::Message("Chess Settings".to_string(), None),
                    EntryKind::SubMenu(
                        "Players".to_string(),
                        vec![
                            EntryKind::Message("Whites".to_string(), None),
                            EntryKind::RadioButton(
                                "Human".to_string(),
                                EntryId::ChessPlayer(Color::White, Player::Human { elo: None }),
                                white_human,
                            ),
                            EntryKind::RadioButton(
                                "Engine".to_string(),
                                EntryId::ChessPlayer(Color::White, Player::Engine { elo: None }),
                                !white_human,
                            ),
                            EntryKind::Separator,
                            EntryKind::Message("Blacks".to_string(), None),
                            EntryKind::RadioButton(
                                "Human".to_string(),
                                EntryId::ChessPlayer(Color::Black, Player::Human { elo: None }),
                                black_human,
                            ),
                            EntryKind::RadioButton(
                                "Engine".to_string(),
                                EntryId::ChessPlayer(Color::Black, Player::Engine { elo: None }),
                                !black_human,
                            ),
                        ],
                    ),
                    EntryKind::Separator,
                    EntryKind::Command("Engine Behavior…".to_string(), EntryId::ChessEngineSettings),
                    EntryKind::SubMenu(
                        "Time Control".to_string(),
                        vec![
                            EntryKind::Message("Set to (total min, incr sec)".to_string(), None),
                            EntryKind::Command("1 +0".to_string(), EntryId::ChessTimeControl(1, 0)),
                            EntryKind::Command("2 +1".to_string(), EntryId::ChessTimeControl(2, 1)),
                            EntryKind::Command("3 +0".to_string(), EntryId::ChessTimeControl(3, 0)),
                            EntryKind::Command("3 +2".to_string(), EntryId::ChessTimeControl(3, 2)),
                            EntryKind::Command("5 +0".to_string(), EntryId::ChessTimeControl(5, 0)),
                            EntryKind::Command("5 +3".to_string(), EntryId::ChessTimeControl(5, 3)),
                            EntryKind::Command("10 +0".to_string(), EntryId::ChessTimeControl(10, 0)),
                            EntryKind::Command("10 +5".to_string(), EntryId::ChessTimeControl(10, 5)),
                            EntryKind::Command("15 +10".to_string(), EntryId::ChessTimeControl(15, 10)),
                            EntryKind::Command("30 +0".to_string(), EntryId::ChessTimeControl(30, 0)),
                            EntryKind::Command("30 +20".to_string(), EntryId::ChessTimeControl(30, 20)),
                            EntryKind::Command("90 +0".to_string(), EntryId::ChessTimeControl(90, 0)),
                            EntryKind::Command("90 +30".to_string(), EntryId::ChessTimeControl(90, 30)),
                        ],
                    ),
                ];

                let sketch_menu = Menu::new(rect, ViewId::SketchMenu, MenuKind::Contextual, entries, context);
                rq.add(RenderData::new(sketch_menu.id(), *sketch_menu.rect(), UpdateMode::Gui));
                self.children.push(Box::new(sketch_menu) as Box<dyn View>);
            }
        }
    }

    fn toggle_engine_page(&mut self, enable: Option<bool>, rq: &mut RenderQueue, context: &mut Context) {
        if let Some(index) = locate_by_id(self, ViewId::ChessSettings) {
            if let Some(true) = enable {
            } else {
                rq.add(RenderData::new(self.id(), *self.child(index).rect(), UpdateMode::Gui));
                self.children.remove(index);
            }
        } else if let Some(false) = enable {
        } else {
            let sliders_dialog = SlidersDialog::new(
                context,
                ViewId::ChessSettings,
                "Chess Engine".to_string(),
                vec![
                    SlidersConstructors::new(
                        SliderId::ChessElo,
                        context.settings.chess_engine_settings.elo as f32,
                        1350.,
                        2850.,
                    ),
                    SlidersConstructors::new(
                        SliderId::ChessSlow,
                        context.settings.chess_engine_settings.slow_motion as f32,
                        10.,
                        1000.,
                    ),
                ],
            );
            rq.add(RenderData::new(sliders_dialog.id(), *sliders_dialog.rect(), UpdateMode::Gui));
            self.children.push(Box::new(sliders_dialog) as Box<dyn View>);
        }
    }

    fn toggle_promotion_menu(
        &mut self, chess_move: &ChessMove, enable: Option<bool>, rq: &mut RenderQueue, context: &mut Context,
    ) {
        if let Some(index) = locate_by_id(self, ViewId::ChessPromotion) {
            if let Some(true) = enable {
            } else {
                rq.add(RenderData::new(self.id(), *self.child(index).rect(), UpdateMode::Gui));
                self.children.remove(index);
            }
        } else if let Some(false) = enable {
        } else {
            let (source, dest) = (chess_move.get_source(), chess_move.get_dest());
            let entries = vec![
                EntryKind::Message("Promote to".to_string(), None),
                EntryKind::Command(
                    "♕ Queen".to_string(),
                    EntryId::ChessPiece(ChessMove::new(source, dest, Some(Piece::Queen))),
                ),
                EntryKind::Command(
                    "♖ Rook".to_string(),
                    EntryId::ChessPiece(ChessMove::new(source, dest, Some(Piece::Rook))),
                ),
                EntryKind::Command(
                    "♗ Bishop".to_string(),
                    EntryId::ChessPiece(ChessMove::new(source, dest, Some(Piece::Bishop))),
                ),
                EntryKind::Command(
                    "♘ Knight".to_string(),
                    EntryId::ChessPiece(ChessMove::new(source, dest, Some(Piece::Knight))),
                ),
                EntryKind::Command(
                    "♙ Pawn".to_string(),
                    EntryId::ChessPiece(ChessMove::new(source, dest, Some(Piece::Pawn))),
                ),
            ];
            let menu = Menu::new(
                rect!(self.rect.center(), self.rect.center()),
                ViewId::ChessPromotion,
                MenuKind::Contextual,
                entries,
                context,
            );
            rq.add(RenderData::new(menu.id(), *menu.rect(), UpdateMode::Gui));
            self.children.push(Box::new(menu) as Box<dyn View>);
        }
    }
}

impl View for PlatoChess {
    fn handle_event(
        &mut self, evt: &Event, hub: &Hub, _bus: &mut Bus, rq: &mut RenderQueue, context: &mut Context,
    ) -> bool {
        match *evt {
            Event::Gesture(GestureEvent::HoldFingerShort(center, ..)) if self.rect.includes(center) => {
                rq.add(RenderData::expose(self.rect, UpdateMode::Full));
                true
            },
            Event::Select(EntryId::New) => {
                self.new_game(rq);
                true
            }
            Event::Select(EntryId::ChessPlayer(color, player)) => {
                let current_player = self.chess.get_player(color);
                let player = if let Player::Engine { elo: _ } = player {
                    Player::Engine {
                        elo: Some(self.chess_engine_settings.elo),
                    }
                } else {
                    player
                };
                self.chess.game_option(GameOption::Player { color, value: player });

                if color == self.chess.side_to_move() {
                    match (current_player, player) {
                        (Player::Human { elo: _ }, Player::Engine { elo: _ }) => {
                            self.chess.go();
                            if let Some(bottom_bar) = self.children[6].downcast_mut::<BottomBar>() {
                                bottom_bar.update_player(false, rq);
                            }
                        }
                        (Player::Engine { elo: _ }, Player::Human { elo: _ }) => {
                            self.chess.stop();
                            if let Some(bottom_bar) = self.children[6].downcast_mut::<BottomBar>() {
                                bottom_bar.update_player(true, rq);
                            }
                        }
                        _ => {}
                    }
                }
                true
            }
            Event::Select(EntryId::ChessTimeControl(base, incr)) => {
                self.chess.game_option(GameOption::TotalTime {
                    color: Color::White,
                    value: Some(Duration::from_secs(base * 60)),
                });
                self.chess.game_option(GameOption::TotalTime {
                    color: Color::Black,
                    value: Some(Duration::from_secs(base * 60)),
                });
                self.chess.game_option(GameOption::Increment {
                    color: Color::White,
                    value: Some(Duration::from_secs(incr)),
                });
                self.chess.game_option(GameOption::Increment {
                    color: Color::Black,
                    value: Some(Duration::from_secs(incr)),
                });
                if let Some(bottom_bar) = self.children[6].downcast_mut::<BottomBar>() {
                    bottom_bar.update_clocks(
                        self.chess.clock(Color::White).unwrap_or(Duration::new(0, 0)),
                        self.chess.clock(Color::Black).unwrap_or(Duration::new(0, 0)),
                        rq,
                    );
                }
                true
            }
            Event::Select(EntryId::ChessPiece(chess_move)) => {
                if self.make_move(rq, &chess_move, context).is_ok() {
                    self.start_move(rq);
                }
                true
            }
            Event::Key(KeyKind::Alternate) => {
                if let Some(child) = self.child_mut(self.child_chess_index).downcast_mut::<ChessBoard>() {
                    child.reverse(hub, rq, context);
                }
                true
            }
            Event::Cancel => {
                self.chess.stop();
                if let Some(last) = self.chess.back_move() {
                    if let Some(move_area) = self.children[4].downcast_mut::<MovesArea>() {
                        move_area.pop(rq, context);
                    }
                    self.draw_move(&last, rq);
                }
                self.start_move(rq);
                true
            }
            Event::ChessCommand(ref command) => {
                self.chess.exec_command(command);
                self.manage_chess_command(command, rq, context);
                true
            }
            Event::ChessGo(human) => {
                if !human {
                    self.chess.stop();
                } else {
                    //
                }
                true
            }
            Event::ToggleNear(ViewId::TitleMenu, rect) => {
                self.toggle_chess_menu(rect, None, rq, context);
                true
            }
            Event::ToggleNear(ViewId::MainMenu, rect) => {
                toggle_main_menu(self, rect, None, rq, context);
                true
            },
            Event::ToggleNear(ViewId::BatteryMenu, rect) => {
                toggle_battery_menu(self, rect, None, rq, context);
                true
            },
            Event::ToggleNear(ViewId::ClockMenu, rect) => {
                toggle_clock_menu(self, rect, None, rq, context);
                true
            },
            Event::ToggleNear(ViewId::ChessSettings, _rect) => {
                self.toggle_engine_page(None, rq, context);
                true
            }
            Event::Select(EntryId::ChessEngineSettings) => {
                self.toggle_engine_page(Some(true), rq, context);
                true
            }
            Event::Close(ViewId::ChessSettings) => {
                self.toggle_engine_page(Some(false), rq, context);
                true
            }
            Event::SaveAll(ViewId::ChessSettings, ref values) => {
                if let (Some(elo), Some(slow)) = (values.first(), values.get(1)) {
                    let (elo, slow) = (*elo as u32, *slow as u32);
                    context.settings.chess_engine_settings.elo = elo;
                    context.settings.chess_engine_settings.slow_motion = slow;

                    for color in ALL_COLORS {
                        if let Player::Engine { .. } = self.chess.get_player(color) {
                            let opt = GameOption::Player {
                                color,
                                value: Player::Engine { elo: Some(elo) },
                            };
                            self.chess.game_option(opt);
                            self.chess.push(GuiCommand::SetOption {
                                option: Opt::SlowMover { value: slow },
                            });
                        }
                    }
                    self.toggle_engine_page(Some(false), rq, context);
                    true
                } else {
                    false
                }
            }
            Event::ChessCell(square, _active) => {
                match self.active_square {
                    None if self.chess.color_on(square) == Some(self.chess.side_to_move()) => {
                        if let Some(child) = self.child_mut(self.child_chess_index).downcast_mut::<ChessBoard>() {
                            child.set_active_square(rq, square, true);
                        }

                        self.active_square = Some(square);
                    }
                    None => {} // no piece of side to move below active cell
                    Some(already_active) if already_active == square => {
                        if let Some(child) = self.child_mut(self.child_chess_index).downcast_mut::<ChessBoard>() {
                            child.set_active_square(rq, already_active, false);
                        }
                        self.active_square = None
                    }
                    Some(already_active) => {
                        let chess_move = ChessMove::new(already_active, square, None);
                        self.active_square = None;
                        if let Some(child) = self.child_mut(self.child_chess_index).downcast_mut::<ChessBoard>() {
                            child.set_active_square(rq, already_active, false);
                            child.set_active_square(rq, square, false);
                        }

                        if self.make_move(rq, &chess_move, context).is_ok() {
                            self.start_move(rq);
                        }
                    }
                }
                true
            }
            _ => false,
        }
    }

    fn render(&self, _fb: &mut dyn Framebuffer, _rect: Rectangle, _fonts: &mut Fonts) {
    }

    fn resize(&mut self, rect: Rectangle, hub: &Hub, rq: &mut RenderQueue, context: &mut Context) {
        debug!("Resizing chess app from {} to {}", self.rect, rect);
        self.rect = rect;

        let sizes = Self::compute_sizes(rect);
        for (index, size) in sizes.iter().enumerate() {
            self.children[index].resize(*size, hub, rq, context);
        }
    }

    fn rect(&self) -> &Rectangle {
        &self.rect
    }
    fn rect_mut(&mut self) -> &mut Rectangle {
        &mut self.rect
    }
    fn children(&self) -> &Vec<Box<dyn View>> {
        &self.children
    }
    fn children_mut(&mut self) -> &mut Vec<Box<dyn View>> {
        &mut self.children
    }
    fn id(&self) -> Id {
        self.id
    }
}
