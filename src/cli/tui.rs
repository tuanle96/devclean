use std::collections::{BTreeMap, HashSet};
use std::io::IsTerminal as _;

use anyhow::{Result, bail};
use clap::Args as ClapArgs;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use devclean::{Category, ScanReport, human_bytes, load_config, scan};
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};

use super::{SharedScanArgs, scan_options, select_categories};

#[derive(Debug, ClapArgs)]
pub(super) struct Args {
    #[command(flatten)]
    shared: SharedScanArgs,
    /// Print a deterministic non-interactive preview and exit.
    #[arg(long, hide = true)]
    snapshot: bool,
}

#[derive(Debug, Default)]
struct State {
    cursor: usize,
    selected: HashSet<usize>,
}

pub(super) fn run(arguments: &Args) -> Result<()> {
    let config = load_config(arguments.shared.config.as_deref())?;
    let categories = select_categories(&arguments.shared, true, false);
    let report = scan(&scan_options(
        &arguments.shared,
        &config,
        categories,
        false,
    )?)?;
    if arguments.snapshot {
        print_snapshot(&report);
        return Ok(());
    }
    if !std::io::stdin().is_terminal() || !std::io::stdout().is_terminal() {
        bail!("tui requires an interactive terminal; use `scan` for non-interactive output");
    }
    if report.candidates.is_empty() {
        println!("no cleanup candidates");
        return Ok(());
    }

    let mut state = State::default();
    let mut terminal = ratatui::init();
    let result = event_loop(&mut terminal, &report, &mut state);
    ratatui::restore();
    result?;
    print_selected_command(&report, &state.selected);
    Ok(())
}

fn event_loop(
    terminal: &mut ratatui::DefaultTerminal,
    report: &ScanReport,
    state: &mut State,
) -> Result<()> {
    loop {
        terminal.draw(|frame| draw(frame, report, state))?;
        let Event::Key(key) = event::read()? else {
            continue;
        };
        if key.kind != KeyEventKind::Press {
            continue;
        }
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => {
                state.selected.clear();
                return Ok(());
            }
            KeyCode::Enter => return Ok(()),
            KeyCode::Up | KeyCode::Char('k') => {
                state.cursor = state.cursor.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                state.cursor = (state.cursor + 1).min(report.candidates.len() - 1);
            }
            KeyCode::Char(' ') => {
                toggle_selection(state);
            }
            KeyCode::Char('a') => state.selected.extend(0..report.candidates.len()),
            KeyCode::Char('n') => state.selected.clear(),
            _ => {}
        }
    }
}

fn toggle_selection(state: &mut State) {
    if !state.selected.insert(state.cursor) {
        state.selected.remove(&state.cursor);
    }
}

fn draw(frame: &mut ratatui::Frame<'_>, report: &ScanReport, state: &State) {
    let [header, body, chart, footer] = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(8),
        Constraint::Length(7),
        Constraint::Length(2),
    ])
    .areas(frame.area());
    frame.render_widget(
        Paragraph::new(format!(
            "{} candidates · {} reclaimable · {} selected",
            report.candidates.len(),
            human_bytes(report.total_bytes),
            human_bytes(selected_bytes(report, &state.selected))
        ))
        .block(
            Block::default()
                .title(" devclean tui — read-only plan ")
                .borders(Borders::ALL),
        ),
        header,
    );

    let items = report
        .candidates
        .iter()
        .enumerate()
        .map(|(index, candidate)| {
            let mark = if state.selected.contains(&index) {
                "[x]"
            } else {
                "[ ]"
            };
            let project = candidate
                .path
                .parent()
                .map_or_else(|| "<root>".into(), |path| path.to_string_lossy());
            ListItem::new(Line::from(format!(
                "{mark} {} · {} · {}",
                human_bytes(candidate.bytes),
                candidate.category,
                project
            )))
        });
    let mut list_state = ListState::default().with_selected(Some(state.cursor));
    frame.render_stateful_widget(
        List::new(items)
            .block(
                Block::default()
                    .title(" candidates by project ")
                    .borders(Borders::ALL),
            )
            .highlight_style(
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
        body,
        &mut list_state,
    );
    frame.render_widget(
        Paragraph::new(category_chart(report)).block(
            Block::default()
                .title(" disk usage by category ")
                .borders(Borders::ALL),
        ),
        chart,
    );
    frame.render_widget(
        Paragraph::new(
            "↑/↓ move · Space toggle · a all · n none · Enter print exact clean command · q quit",
        ),
        footer,
    );
}

fn category_chart(report: &ScanReport) -> String {
    let mut totals = BTreeMap::<Category, u64>::new();
    for candidate in &report.candidates {
        *totals.entry(candidate.category).or_default() += candidate.bytes;
    }
    let maximum = totals.values().copied().max().unwrap_or(1);
    totals
        .into_iter()
        .take(5)
        .map(|(category, bytes)| {
            let width = usize::try_from(bytes.saturating_mul(18) / maximum).unwrap_or(18);
            format!(
                "{category:<23} {:<18} {}",
                "█".repeat(width.max(1)),
                human_bytes(bytes)
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn selected_bytes(report: &ScanReport, selected: &HashSet<usize>) -> u64 {
    selected
        .iter()
        .filter_map(|index| report.candidates.get(*index))
        .map(|candidate| candidate.bytes)
        .sum()
}

fn print_snapshot(report: &ScanReport) {
    println!(
        "devclean tui snapshot: {} candidates",
        report.candidates.len()
    );
    for candidate in &report.candidates {
        println!(
            "[ ]\t{}\t{}\t{}",
            candidate.category,
            human_bytes(candidate.bytes),
            candidate.path.display()
        );
    }
    println!("disk usage\n{}", category_chart(report));
}

fn print_selected_command(report: &ScanReport, selected: &HashSet<usize>) {
    if selected.is_empty() {
        println!("no candidates selected; no cleanup command generated");
        return;
    }
    print!("devclean clean");
    let mut indexes = selected.iter().copied().collect::<Vec<_>>();
    indexes.sort_unstable();
    for index in indexes {
        if let Some(candidate) = report.candidates.get(index) {
            print!(
                " --only-path {}",
                shell_quote(&candidate.path.to_string_lossy())
            );
        }
    }
    println!();
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}
