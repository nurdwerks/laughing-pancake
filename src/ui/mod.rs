// ui/mod.rs

use ratatui::{
    prelude::*,
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph, List, ListItem},
};
use shakmaty::{File, Piece, Position, Rank, Role, Square};
use std::str::FromStr;

use crate::app::App;
use crate::game::search::MoveTreeNode;
use ratatui::widgets::{Gauge, Wrap};

fn format_move_tree(node: &MoveTreeNode, depth: usize) -> String {
    let mut s = String::new();
    let indent = if depth > 0 {
        format!("{}-", "  ".repeat(depth - 1))
    } else {
        String::new()
    };
    s.push_str(&format!("{}{} (Score: {})\n", indent, node.move_san, node.score));

    // To prevent the output from becoming too large, limit the depth and number of children shown
    if depth < 5 {
        let mut sorted_children = node.children.clone();
        sorted_children.sort_by_key(|c| -c.score); // Show best moves first

        for child in sorted_children.iter().take(5) { // Limit to top 5 children
            s.push_str(&format_move_tree(child, depth + 1));
        }
    }
    s
}


pub fn draw(frame: &mut Frame, app: &mut App) {
    draw_evolve_screen(frame, app)
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
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(60), // Top row: Board and Move Tree
            Constraint::Percentage(40), // Bottom row: Match Info and SAN
        ])
        .split(main_layout[1]);

    let top_row_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(50), // Board
            Constraint::Percentage(50), // Move Tree
        ])
        .split(content_layout[0]);

    let bottom_row_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(50), // Match Info
            Constraint::Percentage(50), // SAN Movelist
        ])
        .split(content_layout[1]);

    // Draw Board
    let board_block = Block::default().borders(Borders::ALL).title("Current Match");
    if let Some(board) = &app.evolution_current_match_board {
        draw_board(frame, top_row_layout[0], board, "");
    } else {
        frame.render_widget(board_block, top_row_layout[0]);
    }

    // Draw Move Tree
    let move_tree_block = Block::default().borders(Borders::ALL).title("Move Tree");
    let move_tree_text = if let Some(tree) = &app.evolution_move_tree {
        format_move_tree(tree, 0)
    } else {
        "Waiting for AI move...".to_string()
    };
    let move_tree_widget = Paragraph::new(move_tree_text)
        .block(move_tree_block)
        .wrap(Wrap { trim: true });
    frame.render_widget(move_tree_widget, top_row_layout[1]);

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
    frame.render_widget(info_widget, bottom_row_layout[0]);

    // Draw SAN Movelist
    let san_widget = Paragraph::new(app.evolution_current_match_san.as_str())
        .block(Block::default().borders(Borders::ALL).title("SAN"))
        .wrap(Wrap { trim: true });
    frame.render_widget(san_widget, bottom_row_layout[1]);


    // --- Log View ---
    let log_items: Vec<ListItem> = app
        .evolution_log
        .iter()
        .map(|msg| ListItem::new(msg.as_str()))
        .collect();
    let log_list = List::new(log_items)
        .block(Block::default().borders(Borders::ALL).title("Log"))
        .direction(ratatui::widgets::ListDirection::BottomToTop);
    frame.render_stateful_widget(log_list, main_layout[2], &mut app.evolution_log_state);
}

fn draw_board(frame: &mut Frame, area: Rect, chess: &shakmaty::Chess, user_input: &str) {
    let board = chess.board();
    let mut board_text = Text::default();

    let from_square = user_input.get(0..2).and_then(|s| Square::from_str(s).ok());
    let to_square = user_input.get(2..4).and_then(|s| Square::from_str(s).ok());

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
                    Color::Cyan
                } else {
                    Color::Blue
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
