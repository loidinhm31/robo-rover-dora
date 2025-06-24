use dora_node_api::{DoraNode, Event, dora_core::config::DataId, arrow::array::BinaryArray};
use eyre::Result;
use std::error::Error;
use std::time::{Duration, Instant};
use tracing::info;

fn main() -> Result<(), Box<dyn Error>> {
    let _guard = init_tracing();

    println!("Starting test_keyboard node");

    let (mut node, mut events) = DoraNode::init_from_env()?;
    let output_id = DataId::from("char".to_owned());

    let test_commands = vec!["w", "s", "a", "d", "q", "e", " ", "home"];
    let mut command_index = 0;
    let mut last_send = Instant::now();
    let send_interval = Duration::from_secs(2);

    println!("🤖 Custom Rust keyboard node started");
    println!("Available commands: {:?}", test_commands);

    while let Some(event) = events.recv() {
        match event {
            Event::Input { id, .. } => {
                if id.as_str() == "tick" && last_send.elapsed() >= send_interval {
                    if command_index < test_commands.len() {
                        let cmd = test_commands[command_index];
                        println!("📤 Sending command: '{}'", cmd);

                        let cmd_bytes = cmd.as_bytes();
                        let arrow_data = BinaryArray::from_vec(vec![cmd_bytes]);
                        node.send_output(output_id.clone(), Default::default(), arrow_data)?;

                        command_index += 1;
                        last_send = Instant::now();
                    } else {
                        println!("✅ All test commands sent, cycling...");
                        command_index = 0;
                    }
                }
            }
            Event::Stop => {
                println!("🛑 Custom keyboard node stopping");
                break;
            }
            _ => {}
        }
    }

    Ok(())
}

fn init_tracing() -> tracing::subscriber::DefaultGuard {
    let subscriber = tracing_subscriber::fmt()
        .with_env_filter(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string())
        )
        .finish();

    tracing::subscriber::set_default(subscriber)
}