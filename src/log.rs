use std::fs::OpenOptions;
use std::io::Write;
use std::sync::Mutex;
use std::time::Instant;

static LOG: Mutex<Logger> = Mutex::new(Logger {
    lf: None,
    start: None,
});

struct Logger {
    lf: Option<std::fs::File>,
    start: Option<Instant>,
}

pub fn init(path: &std::path::Path) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .expect("Failed to open log file");
    let mut guard = LOG.lock().unwrap();
    guard.lf = Some(file);
    guard.start = Some(Instant::now());
    drop(guard);
    info("LOG", "Logger initialized");
}

pub fn info(kind: &str, msg: &str) {
    let mut guard = LOG.lock().unwrap();

    let start = match guard.start {
        Some(s) => s,
        None => return,
    };
    let elapsed = start.elapsed();
    let secs = elapsed.as_secs();
    let millis = elapsed.subsec_millis();

    let file = match guard.lf.as_mut() {
        Some(f) => f,
        None => return,
    };
    let _ = writeln!(
        file,
        "[{:02}:{:02}:{:02}.{:03}][{kind}] {msg}",
        secs / 3600,
        (secs % 3600) / 60,
        secs % 60,
        millis,
    );
}
