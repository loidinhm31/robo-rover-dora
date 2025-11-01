use dora_node_api::arrow::array::{Array, BinaryArray};
use dora_node_api::{DoraNode, Event};
use eyre::{eyre, Result};
use kokoro_tiny::TtsEngine;
use robo_rover_lib::TtsCommand;
use std::env;
use std::fs;
use std::path::PathBuf;

fn init_tracing() -> tracing::subscriber::DefaultGuard {
    let subscriber = tracing_subscriber::fmt()
        .with_env_filter(std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string()))
        .with_target(false)
        .with_file(false)
        .with_line_number(false)
        .finish();

    tracing::subscriber::set_default(subscriber)
}

/// Setup models from local directory to cache
fn setup_local_models() -> Result<()> {
    let cache_dir = dirs::cache_dir()
        .ok_or_else(|| eyre!("Could not determine cache directory"))?
        .join("kokoros");

    let local_model_dir = PathBuf::from("../../models/.cache");

    let model_files = [
        ("kokoro-v1.0.onnx", "Kokoro ONNX model"),
        ("voices-v1.0.bin", "Voices data"),
    ];

    // Create cache directory if it doesn't exist
    fs::create_dir_all(&cache_dir)?;

    for (filename, description) in &model_files {
        let cache_path = cache_dir.join(filename);
        let local_path = local_model_dir.join(filename);

        // If file doesn't exist in cache but exists locally, copy it
        if !cache_path.exists() {
            if local_path.exists() {
                tracing::info!("Installing {} from local directory...", description);
                fs::copy(&local_path, &cache_path)?;
                tracing::info!("✓ Installed: {}", cache_path.display());
            } else {
                tracing::warn!("Local model not found: {}", local_path.display());
            }
        } else {
            tracing::debug!("{} already in cache: {}", description, cache_path.display());
        }
    }

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
                "tts_command" => {
                    if let Some(binary_array) = data.as_any().downcast_ref::<BinaryArray>() {
                        if binary_array.len() > 0 {
                            let command_bytes = binary_array.value(0);
                            if let Ok(tts_command) = serde_json::from_slice::<TtsCommand>(command_bytes) {
                                tracing::info!("TTS command received: '{}'", tts_command.text);

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
