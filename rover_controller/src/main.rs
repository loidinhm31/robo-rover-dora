use dora_node_api::{DoraNode, Event, dora_core::config::DataId, arrow::array::{BinaryArray, types::GenericBinaryType}};
use dora_node_api::arrow::array::{Array, AsArray};
use robo_rover_lib::RoverCommand;
use eyre::Result;
use std::error::Error;
use tracing::{info, debug};

fn main() -> Result<(), Box<dyn Error>> {
    let _guard = init_tracing();

    info!("Starting Rover Controller Node");

    let (mut node, mut events) = DoraNode::init_from_env()?;
    let output_id = DataId::from("rover_command".to_owned());

    info!("Rover controller initialized");
    info!("Keyboard controls: W/S=Throttle/Brake, A/D=Steer, X=Stop");

    while let Some(event) = events.recv() {
        match event {
            Event::Input { id, data, .. } => {
                if id.as_str() == "keyboard" {
                    if let Some(bytes_array) = data.as_bytes_opt::<GenericBinaryType<i32>>() {
                        if bytes_array.len() > 0 {
                            let bytes = bytes_array.value(0);
                            if let Ok(char_data) = std::str::from_utf8(bytes) {
                                let key = char_data.trim().to_lowercase();
                                println!("🎮 Key pressed: '{}'", key);

                                let command = match key.as_str() {
                                    "w" => RoverCommand::new(0.4, 0.0, 0.0),
                                    "s" => RoverCommand::new(0.0, 1.0, 0.0),
                                    "a" => RoverCommand::new(0.0, 0.0, -10.0),
                                    "d" => RoverCommand::new(0.0, 0.0, 10.0),
                                    "x" => RoverCommand::new(0.0, 1.0, 0.0),
                                    _ => continue,
                                };

                                print!("📤 Sending rover command: throttle={:.2}, brake={:.2}, steer={:.2}",
                                       command.throttle, command.brake, command.steering_angle);

                                let serialized = serde_json::to_vec(&command)?;
                                node.send_output(output_id.clone(), Default::default(), BinaryArray::from_vec(vec![&*serialized]))?;
                            }
                        }
                    }
                }
            }
            Event::Stop => {
                println!("Stopping rover controller");
                break;
            }
            _ => {}
        }
    }

    Ok(())
}

fn init_tracing() -> tracing::subscriber::DefaultGuard {
    let subscriber = tracing_subscriber::fmt()
        .with_env_filter(std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string()))
        .finish();
    tracing::subscriber::set_default(subscriber)
}