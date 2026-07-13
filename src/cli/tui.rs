use std::collections::{BTreeMap, HashSet};
use std::io::{self, IsTerminal as _, Write as _};

use anyhow::{Result, bail};
use clap::Args as ClapArgs;
use crossterm::cursor::{Hide, MoveTo, Show};
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::execute;
use crossterm::queue;
use crossterm::style::{
    Attribute, Color, Print, ResetColor, SetAttribute, SetBackgroundColor, SetForegroundColor,
};
use crossterm::terminal::{
    self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode,
    enable_raw_mode,
};
use devclean::{Category, ScanReport, human_bytes, load_config, scan};

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

struct TerminalGuard;

impl TerminalGuard {
    fn enter() -> Result<Self> {
        enable_raw_mode()?;
        if let Err(error) = execute!(io::stdout(), EnterAlternateScreen, Hide) {
            let _ = disable_raw_mode();
            return Err(error.into());
        }
        Ok(Self)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = execute!(io::stdout(), Show, LeaveAlternateScreen, ResetColor);
        let _ = disable_raw_mode();
    }
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
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        bail!("tui requires an interactive terminal; use `scan` for non-interactive output");
    }
    if report.candidates.is_empty() {
        println!("no cleanup candidates");
        return Ok(());
    }

    let mut state = State::default();
    {
        let _terminal = TerminalGuard::enter()?;
        event_loop(&report, &mut state)?;
    }
    print_selected_command(&report, &state.selected);
    Ok(())
}

fn event_loop(report: &ScanReport, state: &mut State) -> Result<()> {
    loop {
        draw(report, state)?;
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
            KeyCode::Char(' ') => toggle_selection(state),
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

fn draw(report: &ScanReport, state: &State) -> Result<()> {
    let (width, height) = terminal::size()?;
    let mut output = io::stdout();
    queue!(output, MoveTo(0, 0), Clear(ClearType::All))?;
    draw_line(
        &mut output,
        0,
        width,
        &format!(
            "devclean tui — read-only plan · {} candidates · {} reclaimable · {} selected",
            report.candidates.len(),
            human_bytes(report.total_bytes),
            human_bytes(selected_bytes(report, &state.selected))
        ),
    )?;
    draw_line(&mut output, 2, width, "candidates by project")?;

    let chart = category_chart(report);
    let chart_lines = chart.lines().count().min(5);
    let chart_height = u16::try_from(chart_lines).unwrap_or(5);
    let visible_rows = usize::from(height.saturating_sub(chart_height.saturating_add(5))).max(1);
    let start = state.cursor.saturating_sub(visible_rows.saturating_sub(1));
    for (screen_row, (index, candidate)) in report
        .candidates
        .iter()
        .enumerate()
        .skip(start)
        .take(visible_rows)
        .enumerate()
    {
        let mark = if state.selected.contains(&index) {
            "[x]"
        } else {
            "[ ]"
        };
        let project = candidate
            .path
            .parent()
            .map_or_else(|| "<root>".into(), |path| path.to_string_lossy());
        let row = u16::try_from(screen_row)
            .unwrap_or(u16::MAX)
            .saturating_add(3);
        if index == state.cursor {
            queue!(
                output,
                SetForegroundColor(Color::Black),
                SetBackgroundColor(Color::Cyan),
                SetAttribute(Attribute::Bold)
            )?;
        }
        draw_line(
            &mut output,
            row,
            width,
            &format!(
                "{mark} {} · {} · {project}",
                human_bytes(candidate.bytes),
                candidate.category
            ),
        )?;
        if index == state.cursor {
            queue!(output, ResetColor, SetAttribute(Attribute::Reset))?;
        }
    }

    let chart_start = u16::try_from(visible_rows)
        .unwrap_or(u16::MAX)
        .saturating_add(3);
    draw_line(&mut output, chart_start, width, "disk usage by category")?;
    for (offset, line) in chart.lines().take(5).enumerate() {
        draw_line(
            &mut output,
            chart_start
                .saturating_add(u16::try_from(offset).unwrap_or(0))
                .saturating_add(1),
            width,
            line,
        )?;
    }
    draw_line(
        &mut output,
        height.saturating_sub(1),
        width,
        "↑/↓ move · Space toggle · a all · n none · Enter print exact clean command · q quit",
    )?;
    output.flush()?;
    Ok(())
}

fn draw_line(output: &mut io::Stdout, row: u16, width: u16, value: &str) -> Result<()> {
    let clipped = value
        .chars()
        .take(usize::from(width.saturating_sub(1)))
        .collect::<String>();
    queue!(output, MoveTo(0, row), Print(clipped))?;
    Ok(())
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
