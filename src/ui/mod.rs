// ui/mod.rs

use ratatui::{
    prelude::*,
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph, List, ListItem},
};
use shakmaty::{File, Piece, Position, Rank, Role, Square};

use crate::app::App;
use ratatui::widgets::{Gauge, Wrap};


#[cfg_attr(test, allow(dead_code))]
pub fn draw(frame: &mut Frame, app: &mut App) {
    let main_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),      // Status Bar
            Constraint::Min(0),         // Matches Container
            Constraint::Percentage(25), // Bottom Container
        ])
        .split(frame.size());

    draw_status_bar(frame, app, main_layout[0]);
    draw_matches_container(frame, app, main_layout[1]);
    draw_bottom_container(frame, app, main_layout[2]);
}

#[cfg_attr(test, allow(dead_code))]
fn draw_matches_container(frame: &mut Frame, app: &mut App, area: Rect) {
    let active_matches: Vec<_> = app.active_matches.iter().filter(|(_, m)| m.board.is_some()).collect();

    let num_matches = active_matches.len().max(1);
    let match_constraints = vec![Constraint::Percentage(100 / num_matches as u16); num_matches];
    let content_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(match_constraints)
        .split(area);

    let mut sorted_matches = active_matches;
    sorted_matches.sort_by_key(|(id, _)| *id);

    for (i, (match_id, match_state)) in sorted_matches.iter().enumerate() {
        let match_pane_area = content_layout[i];
        draw_match_pane(frame, match_pane_area, match_id, match_state);
    }
}

#[cfg_attr(test, allow(dead_code))]
fn draw_match_pane(frame: &mut Frame, area: Rect, match_id: &usize, match_state: &crate::app::ActiveMatch) {
    let match_pane = Block::default().borders(Borders::ALL).title(format!(
        "M {}: {} vs {}",
        match_id,
        match_state.white_player.split('.').next().unwrap_or(""),
        match_state.black_player.split('.').next().unwrap_or("")
    ));
    frame.render_widget(match_pane, area);

    let inner_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0), // Board
            Constraint::Length(5), // SAN
        ])
        .margin(1)
        .split(area);

    if let Some(board) = &match_state.board {
        draw_board(frame, inner_layout[0], board, ""); // Title is handled by the pane
    }

    let san_widget = Paragraph::new(match_state.san.as_str())
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!("SAN | Material: {}", match_state.material)),
        )
        .wrap(Wrap { trim: true });
    frame.render_widget(san_widget, inner_layout[1]);
}

#[cfg_attr(test, allow(dead_code))]
fn draw_bottom_container(frame: &mut Frame, app: &mut App, area: Rect) {
    let bottom_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(50), // Log
            Constraint::Percentage(25), // Workers
            Constraint::Percentage(25), // System Stats
        ])
        .split(area);

    // Log View
    let log_items: Vec<ListItem> = app.evolution_log.iter().map(|msg| ListItem::new(msg.as_str())).collect();
    let log_list = List::new(log_items)
        .block(Block::default().borders(Borders::ALL).title("Log"))
        .direction(ratatui::widgets::ListDirection::BottomToTop);
    frame.render_stateful_widget(log_list, bottom_layout[0], &mut app.evolution_log_state);

    // Worker List
    draw_worker_list(frame, app, bottom_layout[1]);

    // System Stats
    draw_system_stats_pane(frame, app, bottom_layout[2]);
}

#[cfg_attr(test, allow(dead_code))]
fn draw_status_bar(frame: &mut Frame, app: &App, area: Rect) {
    let status_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(33),
            Constraint::Percentage(33),
            Constraint::Percentage(34),
        ])
        .split(area);

    // Generation Status
    let progress = app.evolution_matches_completed as f64 / app.evolution_total_matches.max(1) as f64;
    let progress_title = format!(
        "G: {} | R: {} | M: {}/{}",
        app.evolution_current_generation,
        app.evolution_current_round,
        app.evolution_matches_completed,
        app.evolution_total_matches
    );
    let progress_bar = Gauge::default()
        .block(Block::default().borders(Borders::ALL).title(progress_title))
        .gauge_style(Style::default().fg(Color::Green))
        .percent((progress * 100.0).min(100.0) as u16);
    frame.render_widget(progress_bar, status_chunks[0]);

    // CPU Status
    let cpu_usage = app.system.global_cpu_usage();
    let cpu_gauge = Gauge::default()
        .block(Block::default().borders(Borders::ALL).title("CPU Usage"))
        .gauge_style(Style::default().fg(if cpu_usage > 80.0 { Color::Red } else { Color::Green }))
        .percent(cpu_usage as u16)
        .label(format!("{cpu_usage:.2}%"));
    frame.render_widget(cpu_gauge, status_chunks[1]);

    // Memory Status
    let total_mem = app.system.total_memory();
    let used_mem = app.system.used_memory();
    let mem_percent = (used_mem as f64 / total_mem as f64) * 100.0;
    let mem_label = format!(
        "{:.2} / {:.2} GB",
        used_mem as f64 / 1_073_741_824.0,
        total_mem as f64 / 1_073_741_824.0
    );
    let mem_gauge = Gauge::default()
        .block(Block::default().borders(Borders::ALL).title("Memory Usage"))
        .gauge_style(Style::default().fg(if mem_percent > 80.0 { Color::Red } else { Color::Green }))
        .percent(mem_percent as u16)
        .label(mem_label);
    frame.render_widget(mem_gauge, status_chunks[2]);
}

#[cfg_attr(test, allow(dead_code))]
fn draw_system_stats_pane(frame: &mut Frame, app: &App, area: Rect) {
    // Component Temperatures
    let temp_text: Vec<Line> = app
        .components
        .iter()
        .map(|c| {
            let temp = c.temperature().map(|t| format!("{t:.2}°C")).unwrap_or_else(|| "N/A".to_string());
            Line::from(format!("{}: {}", c.label(), temp))
        })
        .collect();
    let temp_paragraph = Paragraph::new(temp_text)
        .block(Block::default().borders(Borders::ALL).title("Temperatures"))
        .wrap(Wrap { trim: true });
    frame.render_widget(temp_paragraph, area);
}

#[cfg_attr(test, allow(dead_code))]
fn draw_worker_list(frame: &mut Frame, _app: &App, area: Rect) {
    let workers_block = Block::default().borders(Borders::ALL).title("Worker Status");
    let list = List::new([ListItem::new("Status now handled by Web UI.")]);
    frame.render_widget(list.block(workers_block), area);
}

#[cfg_attr(test, allow(dead_code))]
fn draw_board(frame: &mut Frame, area: Rect, chess: &shakmaty::Chess, title: &str) {
    let board = chess.board();
    let mut board_text = Text::default();

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

            let bg_color = if (file_idx + rank_idx) % 2 == 0 {
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
        Paragraph::new(board_text).block(Block::default().title(title).borders(Borders::ALL));
    frame.render_widget(board_widget, area);
}

#[cfg_attr(test, allow(dead_code))]
fn get_piece_symbol(piece: Option<Piece>) -> &'static str {
    match piece {
        Some(Piece {
            role: Role::King,
            color: shakmaty::Color::White,
        }) => "♔",
        Some(Piece {
            role: Role::Queen,
            color: shakmaty::Color::White,
        }) => "♕",
        Some(Piece {
            role: Role::Rook,
            color: shakmaty::Color::White,
        }) => "♖",
        Some(Piece {
            role: Role::Bishop,
            color: shakmaty::Color::White,
        }) => "♗",
        Some(Piece {
            role: Role::Knight,
            color: shakmaty::Color::White,
        }) => "♘",
        Some(Piece {
            role: Role::Pawn,
            color: shakmaty::Color::White,
        }) => "♙",
        Some(Piece {
            role: Role::King,
            color: shakmaty::Color::Black,
        }) => "♚",
        Some(Piece {
            role: Role::Queen,
            color: shakmaty::Color::Black,
        }) => "♛",
        Some(Piece {
            role: Role::Rook,
            color: shakmaty::Color::Black,
        }) => "♜",
        Some(Piece {
            role: Role::Bishop,
            color: shakmaty::Color::Black,
        }) => "♝",
        Some(Piece {
            role: Role::Knight,
            color: shakmaty::Color::Black,
        }) => "♞",
        Some(Piece {
            role: Role::Pawn,
            color: shakmaty::Color::Black,
        }) => "♟",
        None => " ",
    }
}
