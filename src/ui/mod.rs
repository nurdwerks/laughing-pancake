// ui/mod.rs

use ratatui::{
    prelude::*,
    style::{Color, Style, Modifier},
    widgets::{Block, Borders, Paragraph, List, ListItem},
};
use shakmaty::{File, Piece, Position, Rank, Role, Square};
use std::str::FromStr;

use crate::app::{App, AppMode};
use ratatui::widgets::{Gauge, Wrap};

pub fn draw(frame: &mut Frame, app: &mut App) {
    match app.mode {
        AppMode::Game => draw_game_screen(frame, app),
        AppMode::Config => draw_config_screen(frame, app),
        AppMode::Evolve => draw_evolve_screen(frame, app),
    }
}

fn draw_game_screen(frame: &mut Frame, app: &App) {
    let main_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(70), Constraint::Percentage(30)].as_ref())
        .split(frame.size());

    draw_board(frame, main_layout[0], app);
    draw_game_info(frame, main_layout[1], app);
}

fn draw_evolve_screen(frame: &mut Frame, app: &mut App) {
    let main_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Progress bar and generation info
            Constraint::Min(0),    // Main content
            Constraint::Percentage(25), // Log
        ])
        .split(frame.size());

    // --- Progress Bar and Generation Info ---
    let progress_block = Block::default().borders(Borders::ALL).title(format!(
        "Generation: {} | Matches: {}/{}",
        app.evolution_current_generation,
        app.evolution_matches_completed,
        app.evolution_total_matches
    ));
    let progress = app.evolution_matches_completed as f64 / app.evolution_total_matches.max(1) as f64;
    let progress_bar = Gauge::default()
        .block(progress_block)
        .gauge_style(Style::default().fg(Color::Green))
        .percent((progress * 100.0) as u16);
    frame.render_widget(progress_bar, main_layout[0]);

    // --- Main Content Area ---
    let content_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(40), // Board
            Constraint::Percentage(30), // Match Info
            Constraint::Percentage(30), // SAN Movelist
        ])
        .split(main_layout[1]);

    // Draw Board
    let board_block = Block::default().borders(Borders::ALL).title("Current Match");
    if let Some(board) = &app.evolution_current_match_board {
        // Re-use the existing board drawing logic, but with the evolution board state
        let mut temp_app = App::new(None, None);
        temp_app.game_state.chess = board.clone();
        draw_board(frame, content_layout[0], &temp_app);
    } else {
        frame.render_widget(board_block, content_layout[0]);
    }

    // Draw Match Info
    let info_text = vec![
        Line::from(vec![
            Span::styled("Evaluation: ", Style::default().bold()),
            Span::raw(format!("{}", app.evolution_current_match_eval)),
        ]),
        Line::from(vec![
            Span::styled("Material: ", Style::default().bold()),
            Span::raw(format!("{}", app.evolution_material_advantage)),
        ]),
        Line::from(vec![
            Span::styled("White: ", Style::default().bold()),
            Span::raw(&app.evolution_white_player),
        ]),
        Line::from(vec![
            Span::styled("Black: ", Style::default().bold()),
            Span::raw(&app.evolution_black_player),
        ]),
    ];
    let info_widget = Paragraph::new(info_text)
        .block(Block::default().borders(Borders::ALL).title("Match Info"))
        .wrap(Wrap { trim: true });
    frame.render_widget(info_widget, content_layout[1]);

    // Draw SAN Movelist
    let san_widget = Paragraph::new(app.evolution_current_match_san.as_str())
        .block(Block::default().borders(Borders::ALL).title("SAN"))
        .wrap(Wrap { trim: true });
    frame.render_widget(san_widget, content_layout[2]);


    // --- Log View ---
    let log_items: Vec<ListItem> = app
        .evolution_log
        .iter()
        .map(|msg| ListItem::new(msg.as_str()))
        .collect();
    let log_list = List::new(log_items)
        .block(Block::default().borders(Borders::ALL).title("Log"))
        .direction(ratatui::widgets::ListDirection::BottomToTop);
    frame.render_widget(log_list, main_layout[2]);
}

fn draw_config_screen(frame: &mut Frame, app: &App) {
    let main_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)].as_ref())
        .split(frame.size());

    // Draw profile list
    let profiles: Vec<ListItem> = app.profiles
        .iter()
        .enumerate()
        .map(|(i, name)| {
            let style = if i == app.selected_profile_index {
                Style::default().add_modifier(Modifier::BOLD).bg(Color::Gray)
            } else {
                Style::default()
            };
            ListItem::new(name.as_str()).style(style)
        })
        .collect();

    let profiles_list = List::new(profiles)
        .block(Block::default().borders(Borders::ALL).title("Profiles"))
        .highlight_style(Style::default().add_modifier(Modifier::BOLD))
        .highlight_symbol("> ");
    frame.render_widget(profiles_list, main_layout[0]);

    // Draw config details
    let config = &app.current_search_config;
    let config_items = [
        format!("Search Algorithm: {:?}", config.search_algorithm),
        format!("MCTS Simulations: {}", config.mcts_simulations),
        format!("Aspiration Windows: {}", config.use_aspiration_windows),
        format!("History Heuristic: {}", config.use_history_heuristic),
        format!("Killer Moves: {}", config.use_killer_moves),
        format!("Quiescence Search: {}", config.use_quiescence_search),
        format!("PVS: {}", config.use_pvs),
        format!("Null Move Pruning: {}", config.use_null_move_pruning),
        format!("LMR: {}", config.use_lmr),
        format!("Futility Pruning: {}", config.use_futility_pruning),
        format!("Delta Pruning: {}", config.use_delta_pruning),
        format!("Pawn Structure Weight: {}", config.pawn_structure_weight),
        format!("Piece Mobility Weight: {}", config.piece_mobility_weight),
        format!("King Safety Weight: {}", config.king_safety_weight),
        format!("Piece Development Weight: {}", config.piece_development_weight),
        format!("Rook Placement Weight: {}", config.rook_placement_weight),
        format!("Bishop Placement Weight: {}", config.bishop_placement_weight),
        format!("Knight Placement Weight: {}", config.knight_placement_weight),
        format!("Passed Pawn Weight: {}", config.passed_pawn_weight),
        format!("Isolated Pawn Weight: {}", config.isolated_pawn_weight),
        format!("Doubled Pawn Weight: {}", config.doubled_pawn_weight),
        format!("Bishop Pair Weight: {}", config.bishop_pair_weight),
        // New Advanced Pawn Structure Weights
        format!("Pawn Chain Weight: {}", config.pawn_chain_weight),
        format!("Ram Weight: {}", config.ram_weight),
        format!("Candidate Passed Pawn Weight: {}", config.candidate_passed_pawn_weight),
        // New Sophisticated King Safety Weights
        format!("King Pawn Shield Weight: {}", config.king_pawn_shield_weight),
        format!("King Open File Penalty: {}", config.king_open_file_penalty),
        format!("King Attackers Weight: {}", config.king_attackers_weight),
        // New Threat Analysis Weight
        format!("Threat Analysis Weight: {}", config.threat_analysis_weight),
        // New evaluation terms
        format!("Tempo Bonus Weight: {}", config.tempo_bonus_weight),
        format!("Space Evaluation Weight: {}", config.space_evaluation_weight),
        format!("Initiative Evaluation Weight: {}", config.initiative_evaluation_weight),
    ];

    let mut config_text = vec![Line::from(Span::styled(
        "Current Configuration",
        Style::default().bold(),
    ))];

    for (i, item) in config_items.iter().enumerate() {
        let style = if i == app.selected_config_line {
            Style::default().bg(Color::Blue)
        } else {
            Style::default()
        };
        config_text.push(Line::from(Span::styled(item, style)));
    }

    config_text.extend(vec![
        Line::from(""),
        Line::from(Span::styled("Controls:", Style::default().bold())),
        Line::from("Up/Down: Navigate profiles"),
        Line::from("'k'/'j': Navigate settings"),
        Line::from("'h'/'l': Adjust setting value"),
        Line::from("Enter: Load profile"),
        Line::from("'s': Save to selected profile"),
        Line::from("'c' or Esc: Close"),
    ]);

    if let Some(error) = &app.error_message {
        config_text.push(Line::from(Span::styled(
            error,
            Style::default().fg(Color::Red),
        )));
    }

    let config_widget =
        Paragraph::new(config_text).block(Block::default().borders(Borders::ALL).title("AI Settings"));
    frame.render_widget(config_widget, main_layout[1]);
}


fn draw_board(frame: &mut Frame, area: Rect, app: &App) {
    let board = app.game_state.chess.board();
    let mut board_text = Text::default();

    let from_square = app.user_input.get(0..2).and_then(|s| Square::from_str(s).ok());
    let to_square = app.user_input.get(2..4).and_then(|s| Square::from_str(s).ok());

    for rank_idx in (0..8).rev() {
        let mut line = Line::default();
        line.spans.push(Span::styled(
            format!("{} ", rank_idx + 1),
            Style::default().fg(Color::Gray),
        ));
        for file_idx in 0..8 {
            let square = Square::from_coords(File::new(file_idx), Rank::new(rank_idx));
            let piece = board.piece_at(square);
            let symbol = get_piece_symbol(piece);

            let is_selected = from_square.map_or(false, |s| s == square) || to_square.map_or(false, |s| s == square);

            let bg_color = if is_selected {
                Color::Yellow
            } else if (file_idx + rank_idx) % 2 == 0 {
                Color::Rgb(181, 136, 99) // Dark square
            } else {
                Color::Rgb(240, 217, 181) // Light square
            };

            let fg_color = if let Some(p) = piece {
                if p.color == shakmaty::Color::White {
                    Color::White
                } else {
                    Color::Black
                }
            } else {
                bg_color
            };

            line.spans.push(Span::styled(
                format!(" {} ", symbol),
                Style::default().bg(bg_color).fg(fg_color),
            ));
        }
        board_text.lines.push(line);
    }

    let mut file_labels = Line::default();
    file_labels.spans.push(Span::raw("  "));
    for file in 'a'..='h' {
        file_labels.spans.push(Span::styled(
            format!(" {} ", file),
            Style::default().fg(Color::Gray),
        ));
    }
    board_text.lines.push(file_labels);

    let board_widget =
        Paragraph::new(board_text).block(Block::default().title("Chess Board").borders(Borders::ALL));
    frame.render_widget(board_widget, area);
}

fn draw_game_info(frame: &mut Frame, area: Rect, app: &App) {
    let turn = if app.game_state.chess.turn() == shakmaty::Color::White {
        "White"
    } else {
        "Black"
    };

    let mut info_text = vec![
        Line::from(vec![
            Span::styled("Turn: ", Style::default().bold()),
            Span::raw(turn),
        ]),
        Line::from(vec![
            Span::styled("PGN: ", Style::default().bold()),
            Span::raw(app.game_state.get_pgn()),
        ]),
        Line::from(vec![
            Span::styled("Input: ", Style::default().bold()),
            Span::raw(&app.user_input),
        ]),
        Line::from(vec![
            Span::styled("Mode: ", Style::default().bold()),
            Span::raw(match app.game_mode {
                crate::app::GameMode::PlayerVsAi => "Player vs AI",
                crate::app::GameMode::AiVsAi => "AI vs AI",
            }),
        ]),
        Line::from(Span::raw("")),
        Line::from(Span::styled("Press 's' to switch mode", Style::default().italic())),
    ];

    if let Some(error) = &app.error_message {
        info_text.push(Line::from(vec![
            Span::styled("Error: ", Style::default().bold().fg(Color::Red)),
            Span::styled(error, Style::default().fg(Color::Red)),
        ]));
    }

    if let Some(result) = &app.game_result {
        info_text.push(Line::from(vec![
            Span::styled("Game Over: ", Style::default().bold().fg(Color::Green)),
            Span::styled(result, Style::default().fg(Color::Green)),
        ]));
    }

    let info_widget = Paragraph::new(info_text)
        .block(Block::default().title("Game Info").borders(Borders::ALL))
        .wrap(ratatui::widgets::Wrap { trim: true });
    frame.render_widget(info_widget, area);
}

fn get_piece_symbol(piece: Option<Piece>) -> &'static str {
    match piece {
        Some(Piece {
            role: Role::King, ..
        }) => "♚",
        Some(Piece {
            role: Role::Queen, ..
        }) => "♛",
        Some(Piece {
            role: Role::Rook, ..
        }) => "♜",
        Some(Piece {
            role: Role::Bishop, ..
        }) => "♝",
        Some(Piece {
            role: Role::Knight, ..
        }) => "♞",
        Some(Piece {
            role: Role::Pawn, ..
        }) => "♟",
        None => " ",
    }
}
