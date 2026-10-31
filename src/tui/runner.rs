//! The two-thread runtime: the machine thread running the simulation, and
//! the UI thread event loop. See the crate-level docs in `tui.rs`.

use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use crossterm::event::{self, DisableMouseCapture, EnableMouseCapture, Event};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

use super::app::App;
use super::snapshot::Snapshot;
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

/// Run the interactive TUI.
///
/// Takes ownership of the simulator and returns it when the user quits,
/// so the caller can print the final state.
pub fn run(simulator: Simulator, max_cycles: u32) -> io::Result<Simulator> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.hide_cursor()?;

    let result = run_app(&mut terminal, simulator, max_cycles);

    // Always restore the terminal, even on error
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    result
}

fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    simulator: Simulator,
    max_cycles: u32,
) -> io::Result<Simulator> {
    // Shared state between the machine and UI threads
    let sim = Arc::new(Mutex::new(simulator));
    let quit = Arc::new(AtomicBool::new(false));

    // Start in the stopped state; the user presses P to run
    sim.lock().unwrap().stop_flag = true;

    // Spawn the machine thread
    let machine_sim = Arc::clone(&sim);
    let machine_quit = Arc::clone(&quit);
    let machine_handle = thread::spawn(move || {
        machine_thread(machine_sim, machine_quit, max_cycles as u64);
    });

    // UI thread (main thread)
    let mut app = App::new();

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

    // Signal the machine thread to exit and wait for it
    quit.store(true, Ordering::Relaxed);
    machine_handle.join().unwrap();

    // Recover the simulator from the Arc<Mutex<>>
    let sim = Arc::try_unwrap(sim)
        .ok()
        .expect("machine thread should have released its Arc reference")
        .into_inner()
        .unwrap_or_else(|e| e.into_inner());
    Ok(sim)
}
