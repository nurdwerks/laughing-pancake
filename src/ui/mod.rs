// ui/mod.rs

use ratatui::{
    prelude::*,
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph, List, ListItem},
};
use shakmaty::{File, Piece, Position, Rank, Role, Square};
use std::str::FromStr;

use crate::app::App;
use ratatui::widgets::{Gauge, Wrap};


pub fn draw(frame: &mut Frame, app: &mut App) {
    draw_evolve_screen(frame, app)
}

fn draw_evolve_screen(frame: &mut Frame, app: &mut App) {
    let main_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Top status bar
            Constraint::Min(0),    // Main content
            Constraint::Percentage(25), // Log
        ])
        .split(frame.size());

    // --- Top Status Bar ---
    let top_bar_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(34), // Generation Progress
            Constraint::Percentage(33), // CPU Usage
            Constraint::Percentage(33), // Memory Usage
        ])
        .split(main_layout[0]);

    // Generation Progress
    let progress = app.evolution_matches_completed as f64 / app.evolution_total_matches.max(1) as f64;
    let progress_bar = Gauge::default()
        .block(Block::default().borders(Borders::ALL).title(format!(
            "Generation: {} | Matches: {}/{}",
            app.evolution_current_generation,
            app.evolution_matches_completed,
            app.evolution_total_matches
        )))
        .gauge_style(Style::default().fg(Color::Green))
        .percent((progress * 100.0) as u16);
    frame.render_widget(progress_bar, top_bar_layout[0]);

    // CPU Usage
    let cpu_gauge = Gauge::default()
        .block(Block::default().borders(Borders::ALL).title("CPU Usage"))
        .gauge_style(Style::default().fg(Color::Yellow))
        .percent(app.cpu_usage as u16);
    frame.render_widget(cpu_gauge, top_bar_layout[1]);

    // Memory Usage
    let mem_usage_gb = app.memory_usage as f64 / 1_073_741_824.0;
    let mem_total_gb = app.total_memory as f64 / 1_073_741_824.0;
    let mem_percentage = (app.memory_usage as f64 / app.total_memory.max(1) as f64) * 100.0;
    let mem_gauge = Gauge::default()
        .block(Block::default().borders(Borders::ALL).title("Memory Usage"))
        .gauge_style(Style::default().fg(Color::Red))
        .label(format!("{mem_usage_gb:.2}/{mem_total_gb:.2} GB"))
        .percent(mem_percentage as u16);
    frame.render_widget(mem_gauge, top_bar_layout[2]);

    // --- Main Content Area ---
    let content_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(60), // Top row: Board and Worker List
            Constraint::Percentage(40), // Bottom row: Match Info and SAN
        ])
        .split(main_layout[1]);

    let top_row_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(50), // Board
            Constraint::Percentage(50), // Worker List
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

    // Draw Worker List
    let workers_block = Block::default().borders(Borders::ALL).title("Running Threads");
    let mut worker_items: Vec<ListItem> = {
        let workers = app.evolution_workers.lock().unwrap();
        let mut worker_vec: Vec<_> = workers.iter().collect();
        // Sort by longest running first
        worker_vec.sort_by_key(|w| w.start_time);
        worker_vec.reverse();
        worker_vec.iter().map(|w| {
            let elapsed = w.start_time.elapsed();
            ListItem::new(format!("{:.2?}: {}", elapsed, w.name))
        }).collect()
    };
    if worker_items.is_empty() {
        worker_items.push(ListItem::new("Waiting for AI move..."));
    }
    let workers_list = List::new(worker_items).block(workers_block);
    frame.render_widget(workers_list, top_row_layout[1]);


    // --- Match Info Panes ---
    let match_info_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(50), // White Player
            Constraint::Percentage(50), // Black Player
        ])
        .split(bottom_row_layout[0]);

    let white_config = app.get_config_for_player(&app.evolution_white_player);
    let black_config = app.get_config_for_player(&app.evolution_black_player);

    let white_info_text = if let Some(config) = white_config {
        vec![
            Line::from(vec![Span::styled("White", Style::default().bold().fg(Color::Cyan))]),
            Line::from(vec![Span::raw(format!("Profile: {}", app.evolution_white_player))]),
            Line::from(vec![Span::raw(format!("Depth: {}", config.search_depth))]),
            Line::from(vec![Span::raw(format!("PVS: {}", config.use_pvs))]),
            Line::from(vec![Span::raw(format!("NMP: {}", config.use_null_move_pruning))]),
            Line::from(vec![Span::raw(format!("LMR: {}", config.use_lmr))]),

        ]
    } else {
        vec![Line::from("White: Waiting...")]
    };

    let black_info_text = if let Some(config) = black_config {
        vec![
            Line::from(vec![Span::styled("Black", Style::default().bold().fg(Color::Blue))]),
            Line::from(vec![Span::raw(format!("Profile: {}", app.evolution_black_player))]),
            Line::from(vec![Span::raw(format!("Depth: {}", config.search_depth))]),
            Line::from(vec![Span::raw(format!("PVS: {}", config.use_pvs))]),
            Line::from(vec![Span::raw(format!("NMP: {}", config.use_null_move_pruning))]),
            Line::from(vec![Span::raw(format!("LMR: {}", config.use_lmr))]),
        ]
    } else {
        vec![Line::from("Black: Waiting...")]
    };

    let white_info_widget = Paragraph::new(white_info_text)
        .block(Block::default().borders(Borders::ALL).title("White Player"))
        .wrap(Wrap { trim: true });
    frame.render_widget(white_info_widget, match_info_layout[0]);

    let black_info_widget = Paragraph::new(black_info_text)
        .block(Block::default().borders(Borders::ALL).title("Black Player"))
        .wrap(Wrap { trim: true });
    frame.render_widget(black_info_widget, match_info_layout[1]);

    // Draw SAN Movelist
    let san_widget = Paragraph::new(app.evolution_current_match_san.as_str())
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!("SAN | Eval: {} | Material: {}", app.evolution_current_match_eval, app.evolution_material_advantage))
        )
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

            let is_selected = from_square == Some(square) || to_square == Some(square);

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
                format!(" {symbol} "),
                Style::default().bg(bg_color).fg(fg_color),
            ));
        }
        board_text.lines.push(line);
    }

    let mut file_labels = Line::default();
    file_labels.spans.push(Span::raw("  "));
    for file in 'a'..='h' {
        file_labels.spans.push(Span::styled(
            format!(" {file} "),
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
