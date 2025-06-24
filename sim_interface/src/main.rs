use dora_node_api::{DoraNode, Event, dora_core::config::DataId, arrow::array::{BinaryArray, types::GenericBinaryType}};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use futures_util::{SinkExt, StreamExt, TryStreamExt};
use arm_bot_lib::{ArmCommand, ArmStatus, JointState, ReachabilityStatus, SimulationConfig, CommandMetadata};
use eyre::Result;
use tracing::{info, warn, error};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use dora_node_api::arrow::array::{Array, AsArray};

fn main() -> Result<()> {
    let _guard = init_tracing();

    info!("Starting sim_interface node");

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;

    rt.block_on(async {
        sim_interface_async().await
    })
}

fn init_tracing() -> tracing::subscriber::DefaultGuard {
    let subscriber = tracing_subscriber::fmt()
        .with_env_filter(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string())
        )
        .finish();

    tracing::subscriber::set_default(subscriber)
}

async fn sim_interface_async() -> Result<()> {
    let (mut node, mut events) = DoraNode::init_from_env()?;
    let output_id = DataId::from("joint_feedback".to_owned());

    // Load simulation configuration
    let sim_config = SimulationConfig::load_from_file("config/simulation.toml")
        .unwrap_or_else(|_| SimulationConfig {
            unity_websocket_port: 8080,
            update_rate_hz: 60.0,
            physics_timestep: 0.02,
        });

    let ws_url = format!("ws://127.0.0.1:{}/arm_sim", sim_config.unity_websocket_port);
    info!("Attempting to connect to Unity simulation at: {}", ws_url);

    // Try to connect to Unity simulation
    let mut unity_connected = false;
    let mut ws_sender = None;
    let mut ws_receiver = None;

    // Attempt initial connection
    match connect_to_unity(&ws_url).await {
        Ok((sender, receiver)) => {
            ws_sender = Some(sender);
            ws_receiver = Some(receiver);
            unity_connected = true;
            info!("Connected to Unity simulation");
        }
        Err(e) => {
            warn!("Failed to connect to Unity simulation: {}. Running in mock mode.", e);
        }
    }

    // Mock simulation state
    let mut mock_sim = MockSimulation::new();

    let update_interval = Duration::from_secs_f64(1.0 / sim_config.update_rate_hz);
    let mut last_update = std::time::Instant::now();

    loop {
        // Handle dora events with recv()
        if let Some(event) = events.recv() {
            match event {
                Event::Input { id, metadata: _, data } => {
                    let id_str = id.as_str();
                    if id_str == "arm_command" {
                        if let Some(bytes_array) = data.as_bytes_opt::<GenericBinaryType<i32>>() {
                            if bytes_array.len() > 0 {
                                let bytes = bytes_array.value(0);
                                if let Ok(cmd_data) = serde_json::from_slice::<serde_json::Value>(bytes) {
                                    if unity_connected {
                                        if let Some(ref mut sender) = ws_sender {
                                            let msg = Message::Text(cmd_data.to_string());
                                            if let Err(e) = sender.send(msg).await {
                                                error!("Failed to send to Unity: {}", e);
                                                unity_connected = false;
                                            }
                                        }
                                    } else {
                                        // Use mock simulation
                                        if let Ok(cmd_with_meta) = serde_json::from_value::<CommandWithMetadata>(cmd_data) {
                                            if let Some(ref command) = cmd_with_meta.command {
                                                mock_sim.process_command(command);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                Event::Stop => {
                    info!("Simulation interface stopping");
                    break;
                }
                _ => {}
            }
        }

        // Handle Unity messages (non-blocking)
        if let Some(ref mut receiver) = ws_receiver {
            match receiver.try_next().await {
                Ok(Some(msg_result)) => {
                    match msg_result {
                        Message::Text(text) => {
                            if let Ok(status) = serde_json::from_str::<ArmStatus>(&text) {
                                let serialized = serde_json::to_vec(&status)?;
                                let arrow_data = BinaryArray::from_vec(vec![serialized.as_slice()]);
                                node.send_output(
                                    output_id.clone(),
                                    Default::default(),
                                    arrow_data
                                )?;
                            }
                        }
                        Message::Close(_) => {
                            warn!("Unity disconnected");
                            unity_connected = false;
                            ws_sender = None;
                            ws_receiver = None;
                        }
                        _ => {}
                    }
                }
                Ok(None) => {
                    unity_connected = false;
                    ws_sender = None;
                    ws_receiver = None;
                }
                Err(_) => {
                    // No messages available
                }
            }
        }

        // Periodic updates for mock simulation
        if !unity_connected && last_update.elapsed() >= update_interval {
            let status = mock_sim.get_current_status();
            let serialized = serde_json::to_vec(&status)?;
            let arrow_data = BinaryArray::from_vec(vec![serialized.as_slice()]);
            node.send_output(
                output_id.clone(),
                Default::default(),
                arrow_data
            )?;
            last_update = std::time::Instant::now();
        }

        // Attempt reconnection to Unity periodically
        if !unity_connected && last_update.elapsed() >= Duration::from_secs(5) {
            match connect_to_unity(&ws_url).await {
                Ok((sender, receiver)) => {
                    ws_sender = Some(sender);
                    ws_receiver = Some(receiver);
                    unity_connected = true;
                    info!("Reconnected to Unity simulation");
                }
                Err(_) => {
                    // Silent retry
                }
            }
        }

        // Small delay to prevent busy waiting
        tokio::time::sleep(tokio::time::Duration::from_millis(1)).await;
    }

    Ok(())
}

async fn connect_to_unity(url: &str) -> Result<(
    futures_util::stream::SplitSink<tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>, Message>,
    futures_util::stream::SplitStream<tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>>
)> {
    let (ws_stream, _) = connect_async(url).await?;
    let (sender, receiver) = ws_stream.split();
    Ok((sender, receiver))
}

#[derive(serde::Deserialize)]
struct CommandWithMetadata {
    command: Option<ArmCommand>,
    metadata: CommandMetadata,
}

// Mock simulation implementation
struct MockSimulation {
    joint_positions: Vec<f64>,
    target_positions: Vec<f64>,
    joint_velocities: Vec<f64>,
    is_moving: bool,
    last_command: Option<String>,
}

impl MockSimulation {
    fn new() -> Self {
        Self {
            joint_positions: vec![0.0; 6],
            target_positions: vec![0.0; 6],
            joint_velocities: vec![0.0; 6],
            is_moving: false,
            last_command: None,
        }
    }

    fn process_command(&mut self, command: &ArmCommand) {
        match command {
            ArmCommand::JointPosition { joint_angles, .. } => {
                self.target_positions = joint_angles.clone();
                self.is_moving = true;
                self.last_command = Some("JointPosition".to_string());
            }
            ArmCommand::RelativeMove { delta_joints } => {
                for (i, &delta) in delta_joints.iter().enumerate() {
                    if i < self.target_positions.len() {
                        self.target_positions[i] += delta;
                    }
                }
                self.is_moving = true;
                self.last_command = Some("RelativeMove".to_string());
            }
            ArmCommand::Home => {
                self.target_positions = vec![0.0; self.target_positions.len()];
                self.is_moving = true;
                self.last_command = Some("Home".to_string());
            }
            ArmCommand::Stop | ArmCommand::EmergencyStop => {
                self.target_positions = self.joint_positions.clone();
                self.is_moving = false;
                self.last_command = Some("Stop".to_string());
            }
            _ => {
                self.last_command = Some("CartesianMove".to_string());
            }
        }
    }

    fn get_current_status(&mut self) -> ArmStatus {
        let mut any_moving = false;
        for i in 0..self.joint_positions.len() {
            let error = self.target_positions[i] - self.joint_positions[i];
            if error.abs() > 0.001 {
                let step = error * 0.1;
                self.joint_positions[i] += step;
                self.joint_velocities[i] = step * 10.0;
                any_moving = true;
            } else {
                self.joint_velocities[i] = 0.0;
            }
        }

        self.is_moving = any_moving;

        ArmStatus {
            joint_state: JointState {
                positions: self.joint_positions.clone(),
                velocities: self.joint_velocities.clone(),
                efforts: vec![0.0; self.joint_positions.len()],
                timestamp: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as u64,
            },
            end_effector_pose: [0.0, 0.0, 0.5, 0.0, 0.0, 0.0],
            is_moving: self.is_moving,
            is_homed: self.joint_positions.iter().all(|&x| x.abs() < 0.01),
            error_state: None,
            current_command: self.last_command.clone(),
            reachability_status: ReachabilityStatus::Reachable,
        }
    }
}