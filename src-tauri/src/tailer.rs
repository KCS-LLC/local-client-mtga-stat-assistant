use std::fs::File;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc::Sender,
    Arc,
};
use std::thread;
use std::time::Duration;

const POLL_MS: u64 = 250;

pub enum StartPosition {
    Beginning,
    End,
}

pub fn start(
    path: PathBuf,
    start_pos: StartPosition,
    sender: Sender<String>,
    running: Arc<AtomicBool>,
) {
    thread::spawn(move || {
        run(path, start_pos, sender, running);
    });
}

fn open_at(path: &PathBuf, pos: u64) -> Option<BufReader<File>> {
    let mut file = File::open(path).ok()?;
    file.seek(SeekFrom::Start(pos)).ok()?;
    Some(BufReader::new(file))
}

fn run(
    path: PathBuf,
    start_pos: StartPosition,
    sender: Sender<String>,
    running: Arc<AtomicBool>,
) {
    // Wait for the log file to exist before doing anything
    while running.load(Ordering::Relaxed) && !path.exists() {
        thread::sleep(Duration::from_millis(POLL_MS));
    }
    if !running.load(Ordering::Relaxed) {
        return;
    }

    let initial_pos = match start_pos {
        StartPosition::Beginning => 0,
        StartPosition::End => std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0),
    };

    let mut reader = match open_at(&path, initial_pos) {
        Some(r) => r,
        None => {
            eprintln!("[tailer] failed to open {:?}", path);
            return;
        }
    };
    let mut read_pos = initial_pos;

    while running.load(Ordering::Relaxed) {
        // Detect MTGA restart: log is cleared on each MTGA launch, so file shrinks
        let file_len = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
        if file_len < read_pos {
            eprintln!("[tailer] log reset detected, restarting from beginning");
            match open_at(&path, 0) {
                Some(r) => {
                    reader = r;
                    read_pos = 0;
                }
                None => {
                    thread::sleep(Duration::from_millis(POLL_MS));
                    continue;
                }
            }
        }

        // Drain all complete lines available since last poll
        loop {
            let mut line = String::new();
            match reader.read_line(&mut line) {
                Ok(0) => break,
                Ok(_) if line.ends_with('\n') => {
                    read_pos += line.len() as u64;
                    let trimmed = line.trim_end().to_string();
                    if !trimmed.is_empty() {
                        // Stop sending if the receiver has been dropped
                        if sender.send(trimmed).is_err() {
                            return;
                        }
                    }
                }
                Ok(_) => {
                    // Partial line at EOF — MTGA mid-write. Seek back and wait.
                    let _ = reader.seek_relative(-(line.len() as i64));
                    break;
                }
                Err(e) => {
                    eprintln!("[tailer] read error: {}", e);
                    break;
                }
            }
        }

        thread::sleep(Duration::from_millis(POLL_MS));
    }
}
