//! The two-thread runtime: the machine thread running the simulation, and
//! the UI thread event loop. See the crate-level docs in `tui.rs`.

use std::io;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use crossterm::cursor::Show;
use crossterm::event::{self, DisableMouseCapture, EnableMouseCapture, Event};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use rust_i18n::t;

use super::app::App;
use super::config::{Config, ConfigStore};
use super::palette::Theme;
use super::snapshot::Snapshot;
use crate::i18n::Language;
use crate::ssem::simulator::Simulator;

/// Frame rate target for the UI thread.
const FRAME_INTERVAL: Duration = Duration::from_millis(16);

/// Target wall-clock duration for each batch of cycles in the machine thread.
/// Short enough (~1 ms) that the UI thread can acquire the lock between batches.
const BATCH_DURATION: f64 = 0.001;

/// Maximum number of cycles per batch, capping how long the machine thread
/// can hold the lock even at very high target speeds.
const MAX_BATCH_SIZE: u64 = 10_000;

/// How long the machine thread sleeps when the simulation is stopped,
/// to avoid busy-waiting.
const IDLE_SLEEP: Duration = Duration::from_millis(10);

/// The machine thread: runs the simulation at the target speed.
///
/// Executes cycles in time-limited batches (~1 ms), releasing the lock between
/// batches so the UI thread can acquire it. Sleeps until the batch deadline to
/// maintain the target speed. Uses absolute deadlines so that `sleep()` overshoot
/// does not accumulate over time.
fn machine_thread(sim: Arc<Mutex<Simulator>>, quit: Arc<AtomicBool>, max_cycles: u64) {
    let mut next_deadline = Instant::now();
    let mut last_speed: u32 = 0;

    loop {
        if quit.load(Ordering::Relaxed) {
            break;
        }

        // Read the control state under a brief lock
        let (should_run, speed) = {
            let s = sim.lock().unwrap();
            (!s.stop_flag, s.speed)
        };

        if !should_run {
            next_deadline = Instant::now();
            last_speed = 0;
            thread::sleep(IDLE_SLEEP);
            continue;
        }

        // Resynchronize when the speed changes
        if speed != last_speed {
            next_deadline = Instant::now();
            last_speed = speed;
        }

        // Calculate batch size: enough cycles to fill ~1 ms at the target speed
        let speed = speed.max(1);
        let period = 1.0 / speed as f64;
        let batch_size = ((BATCH_DURATION / period) as u64).clamp(1, MAX_BATCH_SIZE);
        let deadline = next_deadline + Duration::from_secs_f64(batch_size as f64 * period);

        // Execute the batch (lock held for the duration of the batch)
        {
            let mut s = sim.lock().unwrap();
            for _ in 0..batch_size {
                if s.stop_flag {
                    break;
                }
                if s.cycle_count >= max_cycles {
                    s.stop_flag = true;
                    break;
                }
                s.instruction_cycle();
                s.check_breakpoints();
            }
        }

        // Sleep until the absolute deadline, so sleep() overshoot does not
        // accumulate. If execution fell behind schedule (e.g. speed set beyond
        // what the machine can deliver), resynchronize instead of catching up
        // in a burst.
        let now = Instant::now();
        if deadline > now {
            thread::sleep(deadline - now);
            next_deadline = deadline;
        } else {
            next_deadline = now + Duration::from_secs_f64(batch_size as f64 * period);
        }
    }
}

/// Best-effort terminal restore: disable raw mode, leave the alternate
/// screen, release mouse capture, and show the cursor.
fn restore_terminal() -> io::Result<()> {
    disable_raw_mode()?;
    execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture)?;
    execute!(io::stdout(), Show)?;
    Ok(())
}

/// Restores the terminal on drop unless disarmed with [`TerminalGuard::restore`],
/// so a panic or an early `?` cannot leave the terminal in raw mode on the
/// alternate screen with the cursor hidden.
struct TerminalGuard {
    active: bool,
    restore: fn() -> io::Result<()>,
}

impl TerminalGuard {
    fn new() -> Self {
        Self {
            active: true,
            restore: restore_terminal,
        }
    }

    /// Test-only constructor with an injectable restore, so the drop/disarm
    /// behavior can be exercised without a real TTY.
    #[cfg(test)]
    fn with_restore(restore: fn() -> io::Result<()>) -> Self {
        Self {
            active: true,
            restore,
        }
    }

    /// Restore the terminal now and disarm the guard (Drop becomes a no-op).
    fn restore(mut self) -> io::Result<()> {
        self.active = false;
        (self.restore)()
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        if self.active {
            let _ = (self.restore)();
        }
    }
}

/// Run the interactive TUI.
///
/// Takes ownership of the simulator and returns it when the user quits,
/// so the caller can print the final state. The interface starts from the
/// view modes saved in the config file; `theme` (from `--theme`), when
/// given, overrides the saved theme for this session. The view modes are
/// saved back to the config file when the user quits, with `language` as
/// the saved language: the language the app started with (config file or
/// environment), not a one-off `--lang` override.
pub fn run(
    simulator: Simulator,
    max_cycles: u32,
    theme: Option<Theme>,
    language: Language,
) -> io::Result<Simulator> {
    // Load the persisted view modes before touching the terminal, so a
    // warning about an invalid config file prints cleanly
    let (store, load_warning) = ConfigStore::load();
    if let Some(warning) = load_warning {
        eprintln!("{}", t!("main.warning", message = warning));
    }

    // Setup terminal
    //
    // The guard restores the terminal (raw mode, alternate screen, mouse
    // capture, cursor) on drop, so a panic in `run_app` or a partway setup
    // failure cannot leave the user's terminal broken. It is disarmed by the
    // explicit `restore()` on the normal path.
    let guard = TerminalGuard::new();
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.hide_cursor()?;

    let result = run_app(&mut terminal, simulator, max_cycles, theme, store, language);

    // Always restore the terminal, even on error, before printing anything
    // (a config-save failure cannot be reported while the TUI owns the terminal)
    guard.restore()?;

    if let Ok((_, Some(error))) = &result {
        eprintln!("{}", t!("main.warning", message = error.as_str()));
    }

    result.map(|(sim, _)| sim)
}

/// The config to start from: the saved view modes, with the language
/// replaced by the one the app actually started with (config file or
/// environment), so closing the app persists that choice and it survives
/// restarts — even when it originally came from the environment.
pub(super) fn startup_config(store: ConfigStore, language: Language) -> (Config, Option<PathBuf>) {
    let (mut config, path) = store.into_parts();
    config.language = language;
    (config, path)
}

fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    simulator: Simulator,
    max_cycles: u32,
    theme: Option<Theme>,
    store: ConfigStore,
    language: Language,
) -> io::Result<(Simulator, Option<String>)> {
    // Shared state between the machine and UI threads
    let sim = Arc::new(Mutex::new(simulator));
    let quit = Arc::new(AtomicBool::new(false));

    // Start in the stopped state; the user presses Space to run
    sim.lock().unwrap().stop_flag = true;

    // Spawn the machine thread
    let machine_sim = Arc::clone(&sim);
    let machine_quit = Arc::clone(&quit);
    let machine_handle = thread::spawn(move || {
        machine_thread(machine_sim, machine_quit, max_cycles as u64);
    });

    // UI thread (main thread)
    let (config, config_path) = startup_config(store, language);
    let mut app = App::from_parts(config, theme, config_path);

    loop {
        // Handle input (non-blocking with a short timeout)
        if event::poll(FRAME_INTERVAL)? {
            match event::read()? {
                Event::Key(key) => {
                    let mut s = sim.lock().unwrap();
                    app.handle_key(key, &mut s);
                }
                Event::Mouse(mouse) => {
                    let mut s = sim.lock().unwrap();
                    app.handle_mouse(mouse, &mut s);
                }
                _ => {}
            }
        }

        // Take a snapshot under a brief lock, then draw without holding it.
        // This keeps the lock available for the machine thread during terminal I/O.
        let snapshot = {
            let s = sim.lock().unwrap();
            app.update_speed_display(!s.stop_flag, s.cycle_count);
            Snapshot::from_sim(&s)
        };

        terminal.draw(|f| app.draw(f, &snapshot))?;

        if app.should_quit {
            quit.store(true, Ordering::Relaxed);
            break;
        }
    }

    // Save the view modes once, on close
    app.save_config();

    // Signal the machine thread to exit and wait for it
    quit.store(true, Ordering::Relaxed);
    machine_handle.join().unwrap();

    // Recover the simulator from the Arc<Mutex<>>
    let sim = Arc::try_unwrap(sim)
        .ok()
        .expect("machine thread should have released its Arc reference")
        .into_inner()
        .unwrap_or_else(|e| e.into_inner());
    Ok((sim, app.config_error))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::panic;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn guard_restores_terminal_on_panic() {
        static CALLS: AtomicUsize = AtomicUsize::new(0);
        fn mock() -> io::Result<()> {
            CALLS.fetch_add(1, Ordering::Relaxed);
            Ok(())
        }
        let result = panic::catch_unwind(|| {
            let _guard = TerminalGuard::with_restore(mock);
            panic!("simulated app panic");
        });
        assert!(result.is_err());
        assert_eq!(
            1,
            CALLS.load(Ordering::Relaxed),
            "Drop must run the terminal restore exactly once when unwinding"
        );
    }

    #[test]
    fn guard_restore_disarms_drop() {
        static CALLS: AtomicUsize = AtomicUsize::new(0);
        fn mock() -> io::Result<()> {
            CALLS.fetch_add(1, Ordering::Relaxed);
            Ok(())
        }
        let guard = TerminalGuard::with_restore(mock);
        guard.restore().unwrap();
        assert_eq!(
            1,
            CALLS.load(Ordering::Relaxed),
            "explicit restore runs once and disarms Drop (no second call)"
        );
    }

    #[test]
    fn guard_drop_is_safe_when_restore_fails() {
        fn failing() -> io::Result<()> {
            Err(io::Error::other("no tty"))
        }
        // Without a TTY the real restore errors; Drop must swallow it, not panic
        drop(TerminalGuard::with_restore(failing));
    }
}
