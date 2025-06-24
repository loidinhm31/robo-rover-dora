use dora_node_api::{arrow::array::{types::GenericBinaryType}, DoraNode, Event};
use dora_node_api::arrow::array::{Array, AsArray};
use arm_bot_lib::ArmStatus;
use eyre::Result;
use std::error::Error;
use std::collections::HashMap;

fn main() -> Result<(), Box<dyn Error>> {
    let _guard = init_tracing();

    println!("🔍 Starting enhanced monitor node with detailed debugging...");

    let (mut node, mut events) = DoraNode::init_from_env()?;

    println!("✅ Monitor node initialized successfully");
    println!("📡 Monitoring all dataflow activity with enhanced logging...");

    let mut event_count = 0;
    let mut input_stats: HashMap<String, usize> = HashMap::new();

    while let Some(event) = events.recv() {
        event_count += 1;
        println!("\n🔔 Event #{}: New event received", event_count);

        match event {
            Event::Input { id, metadata, data } => {
                let input_id = id.as_str();
                *input_stats.entry(input_id.to_string()).or_insert(0) += 1;

                let data_len = data.len();
                println!("📥 INPUT EVENT:");
                println!("   📋 ID: '{}'", input_id);
                println!("   📊 Data length: {} bytes", data_len);
                println!("   📈 Count for this input: {}", input_stats[input_id]);
                println!("   ⏰ Metadata: {:?}", metadata);

                // Try to get bytes array information
                if let Some(bytes_array) = data.as_bytes_opt::<GenericBinaryType<i32>>() {
                    println!("   🔍 Bytes array details:");
                    println!("      📦 Array length: {}", bytes_array.len());
                    println!("      📐 Array data type: GenericBinaryType<i32>");

                    if bytes_array.len() > 0 {
                        let bytes = bytes_array.value(0);
                        println!("      📏 First element size: {} bytes", bytes.len());
                        println!("      🔢 Raw bytes (first 50): {:?}",
                                 &bytes[..std::cmp::min(bytes.len(), 50)]);
                    }
                } else {
                    println!("   ❌ Failed to parse as bytes array");
                    println!("   🔍 Raw Arrow data type: {:?}", data.data_type());
                }

                // Process specific input types
                match input_id {
                    "arm_command" => {
                        println!("   🦾 ARM COMMAND DETAILS:");

                        if let Some(bytes_array) = data.as_bytes_opt::<GenericBinaryType<i32>>() {
                            if bytes_array.len() > 0 {
                                let bytes = bytes_array.value(0);

                                // Try to parse as JSON to see the command structure
                                match serde_json::from_slice::<serde_json::Value>(bytes) {
                                    Ok(json_value) => {
                                        println!("      ✅ Successfully parsed JSON:");
                                        println!("      📄 Content: {}",
                                                 serde_json::to_string_pretty(&json_value).unwrap_or_else(|_| "Failed to pretty print".to_string()));

                                        // Try to extract command type
                                        if let Some(command) = json_value.get("command") {
                                            println!("      🎯 Command field found: {}", command);
                                        }
                                        if let Some(metadata) = json_value.get("metadata") {
                                            println!("      📋 Metadata field found: {}", metadata);
                                        }
                                    }
                                    Err(e) => {
                                        println!("      ❌ Failed to parse as JSON: {}", e);
                                        println!("      📝 Raw string attempt: {}",
                                                 String::from_utf8_lossy(bytes));
                                    }
                                }
                            }
                        }
                    }

                    "joint_feedback" => {
                        println!("   🔄 JOINT FEEDBACK DETAILS:");

                        if let Some(bytes_array) = data.as_bytes_opt::<GenericBinaryType<i32>>() {
                            if bytes_array.len() > 0 {
                                let bytes = bytes_array.value(0);

                                match serde_json::from_slice::<ArmStatus>(bytes) {
                                    Ok(status) => {
                                        println!("      ✅ Successfully parsed ArmStatus:");
                                        println!("      🔗 Joint count: {}", status.joint_state.positions.len());
                                        println!("      🚀 Is moving: {}", status.is_moving);
                                        println!("      🏠 Is homed: {}", status.is_homed);
                                        println!("      📍 End effector pose: [{:.3}, {:.3}, {:.3}, {:.3}, {:.3}, {:.3}]",
                                                 status.end_effector_pose[0], status.end_effector_pose[1], status.end_effector_pose[2],
                                                 status.end_effector_pose[3], status.end_effector_pose[4], status.end_effector_pose[5]);
                                        println!("      ⚡ Joint positions: {:?}", status.joint_state.positions);
                                        println!("      💨 Joint velocities: {:?}", status.joint_state.velocities);
                                        println!("      ⏰ Timestamp: {}", status.joint_state.timestamp);

                                        if let Some(ref error) = status.error_state {
                                            println!("      ⚠️  Error state: {}", error);
                                        }
                                        if let Some(ref cmd) = status.current_command {
                                            println!("      🎮 Current command: {}", cmd);
                                        }
                                        println!("      🎯 Reachability: {:?}", status.reachability_status);
                                    }
                                    Err(e) => {
                                        println!("      ❌ Failed to parse as ArmStatus: {}", e);
                                        println!("      📝 Raw string: {}", String::from_utf8_lossy(bytes));
                                    }
                                }
                            }
                        }
                    }

                    "keyboard" => {
                        println!("   ⌨️  KEYBOARD INPUT DETAILS:");

                        if let Some(bytes_array) = data.as_bytes_opt::<GenericBinaryType<i32>>() {
                            if bytes_array.len() > 0 {
                                let bytes = bytes_array.value(0);

                                match std::str::from_utf8(bytes) {
                                    Ok(char_data) => {
                                        let trimmed = char_data.trim();
                                        println!("      ✅ Keyboard character: '{}'", trimmed);
                                        println!("      📏 Length: {} chars", trimmed.len());
                                        println!("      🔤 Raw string: {:?}", char_data);
                                        println!("      🔢 ASCII values: {:?}",
                                                 trimmed.chars().map(|c| c as u32).collect::<Vec<_>>());

                                        // Check for special characters
                                        match trimmed {
                                            " " => println!("      🎯 Detected: SPACE character"),
                                            "w" | "a" | "s" | "d" | "q" | "e" => {
                                                println!("      🎯 Detected: Movement command '{}'", trimmed);
                                            }
                                            "home" => println!("      🎯 Detected: HOME command"),
                                            _ => println!("      ❓ Unknown/unmapped character"),
                                        }
                                    }
                                    Err(e) => {
                                        println!("      ❌ Failed to parse as UTF-8: {}", e);
                                        println!("      🔢 Raw bytes: {:?}", bytes);
                                    }
                                }
                            } else {
                                println!("      ⚠️  Empty bytes array for keyboard input");
                            }
                        } else {
                            println!("      ❌ Failed to parse keyboard data as bytes array");
                        }
                    }

                    _ => {
                        println!("   ❓ UNKNOWN INPUT TYPE: '{}'", input_id);

                        // Still try to parse and show what we can
                        if let Some(bytes_array) = data.as_bytes_opt::<GenericBinaryType<i32>>() {
                            if bytes_array.len() > 0 {
                                let bytes = bytes_array.value(0);
                                println!("      📝 String representation: {}", String::from_utf8_lossy(bytes));

                                // Try JSON parse
                                if let Ok(json) = serde_json::from_slice::<serde_json::Value>(bytes) {
                                    println!("      📄 JSON content: {}", json);
                                }
                            }
                        }
                    }
                }

                println!("   ✅ Input processing complete");
            }

            Event::Stop => {
                println!("\n🛑 STOP EVENT RECEIVED");
                println!("📊 Final statistics:");
                println!("   🔢 Total events processed: {}", event_count);
                println!("   📈 Input breakdown:");
                for (input_id, count) in &input_stats {
                    println!("      {} {}: {} events",
                             match input_id.as_str() {
                                 "keyboard" => "⌨️ ",
                                 "arm_command" => "🦾",
                                 "joint_feedback" => "🔄",
                                 _ => "❓",
                             },
                             input_id, count);
                }
                println!("🏁 Monitor node stopping gracefully");
                break;
            }

            other_event => {
                println!("   🔄 OTHER EVENT TYPE: {:?}", other_event);
            }
        }

    }

    println!("\n🎯 Monitor node finished after processing {} events", event_count);
    Ok(())
}

fn init_tracing() -> tracing::subscriber::DefaultGuard {
    let subscriber = tracing_subscriber::fmt()
        .with_env_filter(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "debug".to_string())
        )
        .with_target(false)  // Remove target for cleaner output
        .with_file(false)    // Remove file info for cleaner output
        .with_line_number(false)  // Remove line numbers for cleaner output
        .finish();

    tracing::subscriber::set_default(subscriber)
}