// ui/mod.rs

use ratatui::{
    prelude::*,
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph, List, ListItem},
};
use shakmaty::{File, Piece, Position, Rank, Role, Square};

use crate::app::App;
use ratatui::widgets::{Gauge, Wrap};


pub fn draw(frame: &mut Frame, app: &mut App) {
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(9), // For status bar
            Constraint::Min(0),    // For matches
        ])
        .split(frame.size());

    draw_status_bar(frame, app, main_chunks[0]);
    draw_evolve_screen(frame, app, main_chunks[1]);
}

fn draw_evolve_screen(frame: &mut Frame, app: &mut App, area: Rect) {
    let main_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),      // Top status bar
            Constraint::Min(0),         // Main content area for matches
            Constraint::Percentage(25), // Bottom area for Log and Workers
        ])
        .split(area);

    // --- Top Status Bar ---
    draw_status_bar(frame, app, main_layout[0]);

    // --- Main Content Area (Matches) ---
    // Filter active matches to only include those that are currently running
    let active_matches: Vec<_> = app.active_matches.iter().filter(|(_, m)| m.board.is_some()).collect();

    let num_matches = active_matches.len().max(1); // Avoid division by zero
    let match_constraints = vec![Constraint::Percentage(100 / num_matches as u16); num_matches];
    let content_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(match_constraints)
        .split(main_layout[1]);

    // Sort matches by ID to ensure a consistent display order
    let mut sorted_matches = active_matches;
    sorted_matches.sort_by_key(|(id, _)| *id);

    for (i, (match_id, match_state)) in sorted_matches.iter().enumerate() {
        let match_pane = content_layout[i];
        let match_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(70), // Board
                Constraint::Percentage(30), // SAN
            ])
            .split(match_pane);

        // Draw Board
        let board_title = format!(
            "Match {} | W: {} vs B: {}",
            match_id,
            match_state.white_player.split('.').next().unwrap_or(""),
            match_state.black_player.split('.').next().unwrap_or("")
        );

        if let Some(board) = &match_state.board {
            draw_board(frame, match_layout[0], board, &board_title);
        } else {
            let board_block = Block::default().borders(Borders::ALL).title(board_title);
            frame.render_widget(board_block, match_layout[0]);
        }

        // Draw SAN Movelist
        let san_widget = Paragraph::new(match_state.san.as_str())
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(format!("SAN | Eval: {} | Material: {}", match_state.eval, match_state.material))
            )
            .wrap(Wrap { trim: true });
        frame.render_widget(san_widget, match_layout[1]);
    }

    // --- Bottom Pane (Log, Workers, and System Stats) ---
    let bottom_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(50), // Log
            Constraint::Percentage(25), // Worker List
            Constraint::Percentage(25), // System Stats
        ])
        .split(main_layout[2]);

    // --- Log View ---
    let log_items: Vec<ListItem> = app
        .evolution_log
        .iter()
        .map(|msg| ListItem::new(msg.as_str()))
        .collect();
    let log_list = List::new(log_items)
        .block(Block::default().borders(Borders::ALL).title("Log"))
        .direction(ratatui::widgets::ListDirection::BottomToTop);
    frame.render_stateful_widget(log_list, bottom_layout[0], &mut app.evolution_log_state);

    // --- Worker List ---
    draw_worker_list(frame, app, bottom_layout[1]);

    // --- System Stats ---
    draw_system_stats_pane(frame, app, bottom_layout[2]);
}

fn draw_status_bar(frame: &mut Frame, app: &App, area: Rect) {
    let progress = app.evolution_matches_completed as f64 / app.evolution_total_matches.max(1) as f64;
    let progress_title = if app.graceful_quit {
        "Graceful shutdown initiated...".to_string()
    } else {
        format!(
            "Generation: {} | Matches: {}/{}",
            app.evolution_current_generation,
            app.evolution_matches_completed,
            app.evolution_total_matches
        )
    };
    let progress_bar = Gauge::default()
        .block(Block::default().borders(Borders::ALL).title(progress_title))
        .gauge_style(Style::default().fg(Color::Green))
        .percent((progress * 100.0) as u16);
    frame.render_widget(progress_bar, area);
}

fn draw_system_stats_pane(frame: &mut Frame, app: &App, area: Rect) {
    let stats_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    // CPU Core Usage
    let cpu_text: Vec<Line> = app
        .system
        .cpus()
        .iter()
        .enumerate()
        .map(|(i, cpu)| Line::from(format!("Core {}: {:.2}%", i, cpu.cpu_usage())))
        .collect();
    let cpu_paragraph = Paragraph::new(cpu_text)
        .block(Block::default().borders(Borders::ALL).title("CPU Core Usage"))
        .wrap(Wrap { trim: true });
    frame.render_widget(cpu_paragraph, stats_layout[0]);

    // Component Temperatures
    let temp_text: Vec<Line> = app
        .components
        .iter()
        .map(|c| {
            let temp = c.temperature().map(|t| format!("{:.2}°C", t)).unwrap_or_else(|| "N/A".to_string());
            Line::from(format!("{}: {}", c.label(), temp))
        })
        .collect();
    let temp_paragraph = Paragraph::new(temp_text)
        .block(Block::default().borders(Borders::ALL).title("Temperatures"))
        .wrap(Wrap { trim: true });
    frame.render_widget(temp_paragraph, stats_layout[1]);
}

fn draw_worker_list(frame: &mut Frame, app: &App, area: Rect) {
    let workers_block = Block::default().borders(Borders::ALL).title("Running Threads");
    let worker_items: Vec<ListItem> = {
        let workers = app.evolution_workers.lock().unwrap();
        let mut worker_vec: Vec<_> = workers.iter().cloned().collect();
        // Sort by start_time ascending, so longest running are first
        worker_vec.sort_by_key(|w| w.start_time);
        worker_vec
            .iter()
            .map(|w| {
                let elapsed = w.start_time.elapsed();
                ListItem::new(format!("{:.2?}: {}", elapsed, w.name))
            })
            .collect()
    };

    let list = if worker_items.is_empty() {
        List::new([ListItem::new("Waiting for AI move...")])
    } else {
        List::new(worker_items)
    };
    frame.render_widget(list.block(workers_block), area);
}

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
