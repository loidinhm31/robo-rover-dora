use dora_node_api::arrow::array::{Array, BinaryArray};
use dora_node_api::{DoraNode, Event};
use eyre::{eyre, Result};
use kokoro_tiny::TtsEngine;
use robo_rover_lib::{init_tracing, TtsCommand};
use std::env;
use std::fs;
use std::path::PathBuf;

/// Setup models from local directory to cache
fn setup_local_models() -> Result<()> {
    // Set KOKORO_CACHE_DIR to project's models/.cache directory
    // This tells kokoro-tiny where to find/store models
    let cache_dir = PathBuf::from("models/.cache/kokoro");

    // Ensure cache directory exists
    fs::create_dir_all(&cache_dir)?;

    // Set environment variable for kokoro-tiny to use this cache directory
    std::env::set_var("KOKORO_CACHE_DIR", cache_dir.to_string_lossy().to_string());

    tracing::info!("Kokoro cache directory set to: {}", cache_dir.display());

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let _guard = init_tracing();

    tracing::info!("Starting Kokoro TTS node...");
    tracing::info!("Using Kokoro-82M model for high-quality speech synthesis");

    // Setup local models to cache directory
    if let Err(e) = setup_local_models() {
        tracing::warn!("Could not setup local models: {}", e);
        tracing::info!("Will attempt to download models at runtime...");
    }

    // Get configuration from environment variables
    let default_voice = env::var("TTS_VOICE").unwrap_or_else(|_| "af_sky".to_string());
    let volume = env::var("TTS_VOLUME")
        .ok()
        .and_then(|v| v.parse::<f32>().ok())
        .unwrap_or(0.8);

    tracing::info!("TTS configuration: voice={}, volume={}", default_voice, volume);

    // Initialize Kokoro TTS engine
    // If models are in cache, this will be instant
    tracing::info!("Initializing Kokoro TTS engine...");
    let mut tts = match TtsEngine::new().await {
        Ok(engine) => {
            tracing::info!("Kokoro TTS engine initialized successfully");
            engine
        }
        Err(e) => {
            tracing::error!("Failed to initialize Kokoro TTS engine: {}", e);
            return Err(eyre!("Kokoro TTS initialization failed: {}", e));
        }
    };

    // Initialize Dora node
    let (_node, mut events) = DoraNode::init_from_env()?;

    tracing::info!("TTS node ready to process commands");

    // Main event loop
    loop {
        match events.recv() {
            Some(Event::Input { id, data, .. }) => match id.as_str() {
                "tts_command" | "tts_command_web" => {
                    if let Some(binary_array) = data.as_any().downcast_ref::<BinaryArray>() {
                        if binary_array.len() > 0 {
                            let command_bytes = binary_array.value(0);
                            if let Ok(tts_command) = serde_json::from_slice::<TtsCommand>(command_bytes) {
                                tracing::info!("TTS command received from {}: '{}'", id, tts_command.text);

                                // Synthesize and play the text
                                match tts.synthesize(&tts_command.text, Some(&default_voice)) {
                                    Ok(audio) => {
                                        tracing::debug!("Audio synthesized, {} samples", audio.len());

                                        // Play the audio with configured volume
                                        if let Err(e) = tts.play(&audio, volume) {
                                            tracing::error!("Failed to play audio: {}", e);
                                        } else {
                                            tracing::info!("TTS playback completed");
                                        }
                                    }
                                    Err(e) => {
                                        tracing::error!("TTS synthesis error: {}", e);
                                    }
                                }
                            } else {
                                tracing::error!("Failed to parse TTS command");
                            }
                        }
                    }
                }
                other => {
                    tracing::warn!("Ignoring unexpected input: {}", other);
                }
            },
            Some(Event::Stop(_)) => {
                tracing::info!("Stop event received");
                break;
            }
            Some(_) => {}
            None => {
                break;
            }
        }
    }

    tracing::info!("Kokoro TTS node stopped");
    Ok(())
}
