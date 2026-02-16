use anyhow::Result;
use clap::{Parser, Subcommand};
use myosotis::Memory;
use myosotis::node::Value;
use myosotis::storage;

#[derive(Parser)]
#[command(name = "myo")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Init {
        file: String,
    },
    History {
        file: String,
    },
    Create {
        file: String,
        ty: String,
    },
    Set {
        file: String,
        id: u64,
        key: String,
        value: String,
    },
    Commit {
        file: String,
        message: String,
    },
    Show {
        file: String,
        id: u64,
        #[arg(long)]
        at: Option<u64>,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Init { file } => {
            if storage::exists(&file) {
                println!("File already exists: {}", file);
                return Ok(());
            }

            let mem = Memory::new();
            storage::save(&file, &mem)?;
            println!("Initialized new memory at {}", file);
        }

        Commands::History { file } => {
            let mem = storage::load(&file)?;
            println!("Commit history:");
            for commit in &mem.commits {
                println!("Commit {} - {:?}", commit.id, commit.message);
            }
        }
        Commands::Create { file, ty } => {
            let mut mem = if storage::exists(&file) {
                storage::load(&file)?
            } else {
                Memory::new()
            };

            let id = mem.create(&ty);
            storage::save(&file, &mem)?;
            println!("Created node {} of type '{}' in {}", id, ty, file);
        }
        Commands::Set {
            file,
            id,
            key,
            value,
        } => {
            let mut mem = storage::load(&file)?;

            mem.set(id, &key, Value::Str(value.clone()));

            storage::save(&file, &mem)?;
            println!("Set node {} field '{}' = '{}'", id, key, value);
        }
        Commands::Commit { file, message } => {
            let mut mem = storage::load(&file)?;

            mem.commit(Some(message.clone()));

            storage::save(&file, &mem)?;
            println!(
                "Committed {} with message {:?}",
                mem.commits.last().map(|c| c.id).unwrap_or(0),
                message
            );
        }
        Commands::Show { file, id, at } => {
            let mem = storage::load(&file)?;

            if let Some(commit_id) = at {
                if let Some(commit) = mem.commits.iter().find(|c| c.id == commit_id) {
                    match commit.changes.get(&id) {
                        Some(node) => {
                            println!("Node {} @ commit {}:", id, commit_id);
                            println!("  type: {}", node.ty);
                            println!("  fields:");
                            for (k, v) in &node.fields {
                                println!("    {}: {:?}", k, v);
                            }
                        }
                        None => println!("Node {} not found in commit {}", id, commit_id),
                    }
                } else {
                    println!("Commit {} not found", commit_id);
                }
            } else {
                match mem.nodes.get(&id) {
                    Some(node) => {
                        println!("Node {} (current):", id);
                        println!("  type: {}", node.ty);
                        println!("  fields:");
                        for (k, v) in &node.fields {
                            println!("    {}: {:?}", k, v);
                        }
                    }
                    None => println!("Node {} not found in current state", id),
                }
            }
        }
    }

    Ok(())
}
