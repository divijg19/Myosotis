use anyhow::Result;
use clap::{Parser, Subcommand};
use myosotis::Memory;
use myosotis::MyosotisError;
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
    DeleteNode {
        file: String,
        id: u64,
    },
    DeleteField {
        file: String,
        id: u64,
        key: String,
    },
    Compact {
        file: String,
        #[arg(long)]
        at: Option<u64>,
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

            if !mem.head_state.contains_key(&id) {
                return Err(anyhow::anyhow!(MyosotisError::NodeNotFound(id)));
            }

            mem.set(id, &key, Value::Str(value.clone()))?;

            storage::save(&file, &mem)?;
            println!("Set node {} field '{}' = '{}'", id, key, value);
        }
        Commands::Commit { file, message } => {
            let mut mem = storage::load(&file)?;

            mem.commit(Some(message.clone()))?;

            storage::save(&file, &mem)?;
            println!(
                "Committed {} with message {:?}",
                mem.commits.last().map(|c| c.id).unwrap_or(0),
                message
            );
        }
        Commands::DeleteNode { file, id } => {
            let mut mem = storage::load(&file)?;
            mem.delete_node(id)?;
            storage::save(&file, &mem)?;
            println!("Staged delete-node for node {}", id);
        }
        Commands::DeleteField { file, id, key } => {
            let mut mem = storage::load(&file)?;
            mem.delete_field(id, &key)?;
            storage::save(&file, &mem)?;
            println!("Staged delete-field '{}' on node {}", key, id);
        }
        Commands::Compact { file, at } => {
            storage::compact(&file, at)?;
            println!("Compacted log in {}", file);
        }
        Commands::Show { file, id, at } => {
            let mem = storage::load(&file)?;

            if let Some(commit_id) = at {
                let state = mem
                    .state_at_commit(commit_id)
                    .map_err(|e| anyhow::anyhow!(e))?;

                let node = state
                    .get(&id)
                    .ok_or_else(|| anyhow::anyhow!(MyosotisError::NodeNotFound(id)))?;
                if node.deleted {
                    return Err(anyhow::anyhow!(MyosotisError::NodeDeleted(id)));
                }

                println!("Node {} @ commit {}:", id, commit_id);
                println!("  type: {}", node.ty);
                println!("  fields:");
                let mut keys: Vec<&String> = node.fields.keys().collect();
                keys.sort();
                for k in keys {
                    println!("    {}: {:?}", k, node.fields.get(k).unwrap());
                }
            } else {
                let node = mem
                    .head_state
                    .get(&id)
                    .ok_or_else(|| anyhow::anyhow!(MyosotisError::NodeNotFound(id)))?;
                if node.deleted {
                    return Err(anyhow::anyhow!(MyosotisError::NodeDeleted(id)));
                }

                println!("Node {} (current):", id);
                println!("  type: {}", node.ty);
                println!("  fields:");
                let mut keys: Vec<&String> = node.fields.keys().collect();
                keys.sort();
                for k in keys {
                    println!("    {}: {:?}", k, node.fields.get(k).unwrap());
                }
            }
        }
    }

    Ok(())
}
