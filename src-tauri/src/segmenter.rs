use std::sync::mpsc::{Receiver, Sender};

const MARKER: &str = "[UnityCrossThreadLogger]";

pub struct Chunk {
    pub content: String,
}

pub fn start(rx: Receiver<String>, tx: Sender<Chunk>) {
    std::thread::spawn(move || {
        run(rx, tx);
    });
}

fn run(rx: Receiver<String>, tx: Sender<Chunk>) {
    let mut buffer: Vec<String> = Vec::new();

    for line in rx {
        if line.starts_with(MARKER) {
            // Flush the accumulated buffer as a complete chunk
            flush(&mut buffer, &tx);

            // Strip the marker prefix and keep whatever follows on this line
            let remainder = line[MARKER.len()..].trim().to_string();
            if !remainder.is_empty() {
                buffer.push(remainder);
            }
        } else {
            buffer.push(line);
        }
    }

    // Flush any remaining content when the channel closes
    flush(&mut buffer, &tx);
}

fn flush(buffer: &mut Vec<String>, tx: &Sender<Chunk>) {
    if buffer.is_empty() {
        return;
    }

    let content = buffer.join("\n");
    buffer.clear();

    // Drop the chunk silently if the receiver is gone
    let _ = tx.send(Chunk { content });
}
