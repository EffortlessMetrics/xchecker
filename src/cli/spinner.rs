use std::io::Write;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

pub(super) struct SpinnerGuard {
    running: Arc<AtomicBool>,
    handle: Option<thread::JoinHandle<()>>,
}

impl SpinnerGuard {
    pub(super) fn new() -> Self {
        let running = Arc::new(AtomicBool::new(true));
        let running_clone = running.clone();

        // Hide cursor to prevent flickering
        let _ = crossterm::execute!(std::io::stdout(), crossterm::cursor::Hide);

        let handle = thread::spawn(move || {
            let frames = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
            let mut i = 0;
            while running_clone.load(Ordering::Relaxed) {
                print!("\r{} Running health checks...", frames[i]);
                let _ = std::io::stdout().flush();
                i = (i + 1) % frames.len();
                thread::sleep(Duration::from_millis(80));
            }
            // Clear the line when done (use crossterm for portability)
            let _ = crossterm::execute!(
                std::io::stdout(),
                crossterm::terminal::Clear(crossterm::terminal::ClearType::CurrentLine)
            );
            print!("\r");
            let _ = std::io::stdout().flush();
        });

        Self {
            running,
            handle: Some(handle),
        }
    }
}

impl Drop for SpinnerGuard {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
        // Restore cursor
        let _ = crossterm::execute!(std::io::stdout(), crossterm::cursor::Show);
    }
}
