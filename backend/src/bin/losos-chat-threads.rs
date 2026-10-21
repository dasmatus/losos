//! `losos-chat-threads` — the thread list Dendrite lacks, read from
//! Dendrite's own relations table.
//!
//! One question per connection, under socket activation (`modules/chat.nix`):
//! stdin and stdout are the socket lososd connected to. The question is one
//! JSON line ([`losos_ctl::chat::ThreadsAsk`]), the answer `{"rows": [...]}`
//! or `{"error": "..."}`. It runs as `shared`, the user chat belongs to,
//! because the database is in the shared data domain lososd cannot read.
//! It only reads; what the caller may see of each thread lososd asks
//! Dendrite with the caller's own token ([`losos_ctl::chat::Chat::threads`]).

use clap::Parser;
use std::io::{BufRead, Write};
use std::path::PathBuf;

#[derive(Parser)]
#[command(version, about = "Answer one thread-list question about LosOS chat")]
struct Cli {
    /// Dendrite's sync database.
    #[arg(long)]
    db: PathBuf,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let mut line = String::new();
    std::io::stdin().lock().take_line(&mut line)?;
    let answer = match serde_json::from_str::<losos_ctl::chat::ThreadsAsk>(line.trim()) {
        Ok(ask) => match losos_ctl::chat::read_threads(&cli.db, &ask) {
            Ok(rows) => serde_json::json!({ "rows": rows }),
            Err(e) => {
                eprintln!("losos-chat-threads: {e:#}");
                serde_json::json!({ "error": "the relations table could not be read" })
            }
        },
        Err(_) => serde_json::json!({ "error": "the question is not a JSON line" }),
    };
    let mut out = std::io::stdout().lock();
    serde_json::to_writer(&mut out, &answer)?;
    out.write_all(b"\n")?;
    Ok(())
}

/// One line, read with a bound: the question is a few hundred bytes.
trait TakeLine {
    fn take_line(&mut self, buf: &mut String) -> std::io::Result<usize>;
}

impl<R: BufRead> TakeLine for R {
    fn take_line(&mut self, buf: &mut String) -> std::io::Result<usize> {
        use std::io::Read;
        self.take(64 * 1024).read_line(buf)
    }
}
