//! Persistent Borsh-backed Todo queue CLI.
//!
//! Usage:
//!   todo add "Buy groceries"
//!   todo list
//!   todo done

mod queue;

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use borsh::{BorshDeserialize, BorshSerialize};
use clap::{Parser, Subcommand};

use queue::Queue;

#[derive(Debug, Clone, BorshSerialize, BorshDeserialize)]
struct Todo {
    id: u64,
    description: String,
    created_at: u64,
}

/// On-disk representation: next-id counter plus the queue contents as a Vec.
#[derive(Debug, Default, BorshSerialize, BorshDeserialize)]
struct StoreFile {
    next_id: u64,
    todos: Vec<Todo>,
}

#[derive(Parser)]
#[command(name = "todo", about = "Persistent FIFO todo queue (Borsh-backed)")]
struct Cli {
    /// Override the storage file path (defaults to ./todos.bin).
    #[arg(long, global = true)]
    file: Option<PathBuf>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Add a task to the queue.
    Add {
        /// Task description.
        description: String,
    },
    /// List all pending tasks in FIFO order.
    List,
    /// Complete (remove) the oldest task.
    Done,
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn default_path() -> PathBuf {
    PathBuf::from("todos.bin")
}

fn load(path: &Path) -> Result<(Queue<Todo>, u64)> {
    match fs::read(path) {
        Ok(bytes) => {
            let store = StoreFile::try_from_slice(&bytes)
                .with_context(|| format!("failed to decode {}", path.display()))?;
            let mut q = Queue::new();
            for t in store.todos {
                q.enqueue(t);
            }
            Ok((q, store.next_id))
        }
        Err(e) if e.kind() == ErrorKind::NotFound => Ok((Queue::new(), 0)),
        Err(e) => Err(e).with_context(|| format!("failed to read {}", path.display())),
    }
}

fn save(path: &Path, queue: &Queue<Todo>, next_id: u64) -> Result<()> {
    let store = StoreFile {
        next_id,
        todos: queue.iter().cloned().collect(),
    };
    let bytes = borsh::to_vec(&store).context("borsh encode failed")?;
    // Atomic-ish write: write to a sibling temp file then rename.
    let tmp = path.with_extension("bin.tmp");
    fs::write(&tmp, &bytes).with_context(|| format!("failed to write {}", tmp.display()))?;
    fs::rename(&tmp, path)
        .with_context(|| format!("failed to rename into {}", path.display()))?;
    Ok(())
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let path = cli.file.unwrap_or_else(default_path);
    let (mut queue, mut next_id) = load(&path)?;

    match cli.command {
        Command::Add { description } => {
            let todo = Todo {
                id: next_id,
                description,
                created_at: now_unix(),
            };
            next_id += 1;
            println!("added #{}: {}", todo.id, todo.description);
            queue.enqueue(todo);
            save(&path, &queue, next_id)?;
        }
        Command::List => {
            if queue.is_empty() {
                println!("(no tasks)");
            } else {
                for (i, t) in queue.iter().enumerate() {
                    println!("{}. [#{}] {}", i + 1, t.id, t.description);
                }
            }
        }
        Command::Done => match queue.dequeue() {
            Some(t) => {
                println!("completed #{}: {}", t.id, t.description);
                save(&path, &queue, next_id)?;
            }
            None => {
                println!("(nothing to complete)");
            }
        },
    }

    Ok(())
}
