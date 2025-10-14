// ui/mod.rs

use crate::app::App;
use ratatui::{
    prelude::*,
    style::{Color, Style, Modifier},
    widgets::{Block, Borders, Paragraph, List, ListItem},
};
use shakmaty::{File, Piece, Position, Rank, Role, Square};
use std::str::FromStr;

pub fn draw(frame: &mut Frame, app: &App) {
    if app.show_ai_config {
        draw_config_screen(frame, app);
    } else {
        let main_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(70), Constraint::Percentage(30)].as_ref())
            .split(frame.size());

        draw_board(frame, main_layout[0], app);
        draw_game_info(frame, main_layout[1], app);
    }
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
