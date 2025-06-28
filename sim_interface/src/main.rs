use dora_node_api::arrow::array::{Array, AsArray};
use dora_node_api::{arrow::array::{types::GenericBinaryType, BinaryArray}, dora_core::config::DataId, DoraNode, Event};
use eyre::Result;
use robo_rover_lib::{ArmCommand, ArmStatus, CommandMetadata, JointState, ReachabilityStatus, RoverCommand, RoverTelemetry, SimulationConfig};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use axum::http::Method;
use serde_json::Value;
use socketioxide::{extract::{Data, SocketRef}, SocketIo};
use tokio;
use tower::ServiceBuilder;
use tower_http::cors::{Any, CorsLayer};

#[derive(Clone)]
struct SharedState {
    latest_arm_command: Arc<Mutex<Option<ArmCommand>>>,
    latest_rover_command: Arc<Mutex<Option<RoverCommand>>>,
    latest_rover_telemetry: Arc<Mutex<Option<RoverTelemetry>>>,
    unity_connected: Arc<Mutex<bool>>,
    operation_mode: Arc<Mutex<String>>, // "arm" or "rover"

    // Debug counters
    commands_sent: Arc<AtomicU64>,
    telemetry_received: Arc<AtomicU64>,
    connection_count: Arc<AtomicU64>,
}

impl SharedState {
    fn new() -> Self {
        Self {
            latest_arm_command: Arc::new(Mutex::new(None)),
            latest_rover_command: Arc::new(Mutex::new(None)),
            latest_rover_telemetry: Arc::new(Mutex::new(None)),
            unity_connected: Arc::new(Mutex::new(false)),
            operation_mode: Arc::new(Mutex::new("rover".to_string())), // Default to rover

            // Initialize debug counters
            commands_sent: Arc::new(AtomicU64::new(0)),
            telemetry_received: Arc::new(AtomicU64::new(0)),
            connection_count: Arc::new(AtomicU64::new(0)),
        }
    }
}

fn main() -> Result<()> {
    let _guard = init_tracing();

    println!("Starting Sim Interface Node with SocketIO Server");
    println!("Debug mode enabled for Unity integration");

    // Use multithreaded runtime for proper SocketIO server support
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;

    rt.block_on(async {
        sim_interface_async().await
    })
}

async fn sim_interface_async() -> Result<()> {
    // Pre-check: Ensure port 4567 is available
    println!("Checking if port 4567 is available...");
    match tokio::net::TcpListener::bind("127.0.0.1:4567").await {
        Ok(test_listener) => {
            drop(test_listener); // Release the port
            println!("Port 4567 is available");
        }
        Err(e) => {
            println!("Port 4567 is not available: {}", e);
            println!("Check if another process is using port 4567:");
            println!("   netstat -an | grep 4567");
            println!("   lsof -i :4567");
            return Err(e.into());
        }
    }

    let (mut node, mut events) = DoraNode::init_from_env()?;
    let joint_feedback_output = DataId::from("joint_feedback".to_owned());
    let rover_telemetry_output = DataId::from("rover_telemetry".to_owned());

    // Load simulation configuration
    let sim_config = SimulationConfig::load_from_file("config/simulation.toml")
        .unwrap_or_else(|_| {
            println!("Using default simulation config");
            SimulationConfig {
                unity_websocket_port: 4567,
                update_rate_hz: 60.0,
                physics_timestep: 0.02,
            }
        });

    let shared_state = SharedState::new();

    // Start SocketIO server in a separate task
    let shared_state_clone = shared_state.clone();
    let socketio_handle = tokio::spawn(async move {
        println!("Starting SocketIO server task...");
        match start_socketio_server_properly(shared_state_clone).await {
            Ok(_) => println!("SocketIO server completed"),
            Err(e) => println!("SocketIO server error: {}", e),
        }
    });

    // Give the SocketIO server time to start
    tokio::time::sleep(Duration::from_millis(1000)).await;

    // Verify the server is running
    println!("Verifying SocketIO server is running...");
    match tokio::time::timeout(
        Duration::from_secs(3),
        tokio::net::TcpStream::connect("127.0.0.1:4567")
    ).await {
        Ok(Ok(_)) => {
            println!("SocketIO server is responding on port 4567");
        }
        Ok(Err(e)) => {
            println!("Cannot connect to SocketIO server: {}", e);
        }
        Err(_) => {
            println!("Timeout connecting to SocketIO server");
        }
    }

    // Mock simulation for arm
    let mut mock_sim = MockSimulation::new();

    let update_interval = Duration::from_secs_f64(1.0 / sim_config.update_rate_hz);
    let mut last_update = std::time::Instant::now();
    let mut debug_counter = 0u64;

    println!("Sim interface initialized");
    println!("SocketIO server should be running on port 4567 for Unity");
    println!("Waiting for events from dora nodes...");
    println!("Test connection: curl -X GET http://127.0.0.1:4567/socket.io/");

    loop {
        // Non-blocking event processing with timeout
        let event_future = tokio::time::timeout(Duration::from_millis(10), async {
            events.recv()
        });

        if let Ok(Some(event)) = event_future.await {
            debug_counter += 1;

            match event {
                Event::Input { id, data, .. } => {
                    let id_str = id.as_str();
                    println!("Received input #{}: '{}'", debug_counter, id_str);

                    match id_str {
                        "arm_command" => {
                            println!("Processing ARM command...");
                            if let Some(bytes_array) = data.as_bytes_opt::<GenericBinaryType<i32>>() {
                                if bytes_array.len() > 0 {
                                    let bytes = bytes_array.value(0);
                                    if let Ok(cmd_data) = serde_json::from_slice::<serde_json::Value>(bytes) {
                                        println!("   Raw arm command: {}", cmd_data);
                                        mock_sim.apply_command(&cmd_data);

                                        // Store for potential Unity forwarding if needed
                                        if let Ok(mut mode) = shared_state.operation_mode.lock() {
                                            *mode = "arm".to_string();
                                        }
                                    }
                                }
                            }
                        }

                        "rover_command" => {
                            println!("Processing ROVER command...");
                            if let Some(bytes_array) = data.as_bytes_opt::<GenericBinaryType<i32>>() {
                                if bytes_array.len() > 0 {
                                    let bytes = bytes_array.value(0);
                                    if let Ok(cmd_data) = serde_json::from_slice::<serde_json::Value>(bytes) {
                                        println!("   Raw rover command: {}", cmd_data);

                                        // Debug rover command processing
                                        if let Err(e) = debug_rover_command_processing(&shared_state, &cmd_data).await {
                                            println!("   Error processing rover command: {}", e);
                                        }
                                    } else {
                                        println!("   Failed to parse rover command JSON");
                                    }
                                } else {
                                    println!("   Empty rover command data");
                                }
                            } else {
                                println!("   Failed to read rover command bytes");
                            }
                        }

                        _ => {
                            println!("Unknown input: '{}'", id_str);
                        }
                    }
                }

                Event::Stop => {
                    println!("Stop event received");
                    break;
                }

                _ => {
                    println!("Other event received");
                }
            }
        }

        // Periodic updates
        let now = std::time::Instant::now();
        if now.duration_since(last_update) >= update_interval {
            // Update mock simulation
            mock_sim.update();

            // Send arm feedback
            let arm_status = mock_sim.get_arm_status();
            let arm_serialized = serde_json::to_vec(&arm_status)?;
            let arm_arrow = BinaryArray::from_vec(vec![arm_serialized.as_slice()]);
            node.send_output(joint_feedback_output.clone(), Default::default(), arm_arrow)?;

            // Send rover telemetry if available
            if let Ok(rover_tel_opt) = shared_state.latest_rover_telemetry.lock() {
                if let Some(rover_tel) = rover_tel_opt.as_ref() {
                    let rover_serialized = serde_json::to_vec(rover_tel)?;
                    let rover_arrow = BinaryArray::from_vec(vec![rover_serialized.as_slice()]);
                    node.send_output(rover_telemetry_output.clone(), Default::default(), rover_arrow)?;
                }
            }

            last_update = now;
        }

        // Print debug stats every 10 seconds (600 * 10ms loops)
        if debug_counter % 1000 == 0 && debug_counter > 0 {
            print_debug_stats(&shared_state);
        }

        // Small delay to prevent busy waiting
        tokio::time::sleep(Duration::from_millis(1)).await;
    }

    socketio_handle.abort();
    println!("Sim interface shutdown complete");
    Ok(())
}

async fn start_socketio_server_properly(shared_state: SharedState) -> Result<()> {
    println!("Starting SocketIO server on port 4567 for Unity");
    println!("Server will be available at: http://127.0.0.1:4567");

    // Create SocketIO instance
    let (layer, io) = SocketIo::new_layer();

    let shared_state_clone = shared_state.clone();

    // Handle Unity connections
    io.ns("/", move |socket: SocketRef| {
        let connection_id = shared_state_clone.connection_count.fetch_add(1, Ordering::SeqCst) + 1;
        println!("Unity connected: {} (Connection #{})", socket.id, connection_id);

        let state = shared_state_clone.clone();

        // Update connection status
        if let Ok(mut connected) = state.unity_connected.lock() {
            *connected = true;
        }

        // Handle telemetry from Unity
        socket.on("telemetry", {
            let state = state.clone();
            move |_socket: SocketRef, Data::<Value>(data)| {
                let count = state.telemetry_received.fetch_add(1, Ordering::SeqCst) + 1;

                if count <= 10 || count % 20 == 0 {
                    println!("Received telemetry #{} from Unity", count);
                    println!("   Raw data: {}", data);
                }

                match parse_unity_telemetry(&data) {
                    Ok(telemetry) => {
                        if count <= 5 || count % 20 == 0 {
                            println!("   Parsed telemetry successfully:");
                            println!("      Position: ({:.2}, {:.2})", telemetry.position.0, telemetry.position.1);
                            println!("      Velocity: {:.2} m/s", telemetry.velocity);
                            println!("      Yaw: {:.1}°", telemetry.yaw.to_degrees());
                        }

                        if let Ok(mut tel) = state.latest_rover_telemetry.lock() {
                            *tel = Some(telemetry);
                        }
                    }
                    Err(e) => {
                        println!("   Failed to parse Unity telemetry: {}", e);
                    }
                }
            }
        });

        // Handle connection acknowledgment
        socket.on("connect", {
            move |socket: SocketRef| {
                println!("Unity SocketIO connected and acknowledged");
                // Send test command to verify connection
                let test_data = serde_json::json!({
                    "throttle": "0.0",
                    "brake": "0.0", 
                    "steering_angle": "0.0",
                    "inset_image1": "",
                    "inset_image2": "",
                    "_test": "connection_test",
                    "_server": "dora_sim_interface"
                });

                if let Err(e) = socket.emit("data", test_data) {
                    println!("Failed to send connection test: {}", e);
                } else {
                    println!("Sent connection test command to Unity");
                }
            }
        });

        // Handle disconnect
        socket.on_disconnect({
            let state = state.clone();
            move |socket: SocketRef| {
                println!("Unity disconnected: {}", socket.id);
                if let Ok(mut connected) = state.unity_connected.lock() {
                    *connected = false;
                }
            }
        });

        // Start command sending loop for this connection
        let socket_clone = socket.clone();
        let state_clone = state.clone();
        tokio::spawn(async move {
            command_sender_loop(socket_clone, state_clone).await;
        });
    });

    // Create HTTP app with CORS
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers(Any);

    let app = axum::Router::new()
        .layer(ServiceBuilder::new().layer(cors).layer(layer));

    // Bind and start server
    println!("Attempting to bind to 127.0.0.1:4567...");
    let listener = match tokio::net::TcpListener::bind("127.0.0.1:4567").await {
        Ok(listener) => {
            println!("Successfully bound to 127.0.0.1:4567");
            listener
        }
        Err(e) => {
            println!("❌ Failed to bind to 127.0.0.1:4567: {}", e);
            return Err(e.into());
        }
    };

    println!("SocketIO server listening on 127.0.0.1:4567");
    println!("Test with: curl -X GET http://127.0.0.1:4567/socket.io/");
    println!("Unity should connect to: http://127.0.0.1:4567");

    // Start serving
    match axum::serve(listener, app).await {
        Ok(_) => {
            println!("SocketIO server completed successfully");
            Ok(())
        }
        Err(e) => {
            println!("SocketIO server error: {}", e);
            Err(e.into())
        }
    }
}

async fn command_sender_loop(socket: SocketRef, state: SharedState) {
    let mut interval = tokio::time::interval(Duration::from_millis(100)); // 10 Hz
    let mut last_command_time = std::time::Instant::now();
    let mut loop_count = 0u64;

    println!("Starting command sender loop for connection {}", socket.id);

    loop {
        interval.tick().await;
        loop_count += 1;

        // Check if Unity is still connected
        let connected = {
            if let Ok(conn) = state.unity_connected.lock() {
                *conn
            } else {
                false
            }
        };

        if !connected {
            println!("Unity disconnected, stopping command loop for {}", socket.id);
            break;
        }

        let current_mode = {
            let mode_guard = state.operation_mode.lock().unwrap();
            mode_guard.clone()
        };

        match current_mode.as_str() {
            "rover" => {
                // Send rover commands
                if let Ok(mut cmd_opt) = state.latest_rover_command.lock() {
                    if let Some(command) = cmd_opt.take() {
                        let count = state.commands_sent.fetch_add(1, Ordering::SeqCst) + 1;

                        let command_data = serde_json::json!({
                            "throttle": command.throttle.to_string(),
                            "brake": command.brake.to_string(),
                            "steering_angle": command.steering_angle.to_string(),
                            "inset_image1": "",
                            "inset_image2": "",
                            "_debug_count": count.to_string(),
                            "_timestamp": std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap()
                                .as_millis().to_string(),
                            "_source": "dora_sim_interface"
                        });

                        // Log first few commands and every 10th command
                        if count <= 10 || count % 10 == 0 {
                            println!("📤 Sending rover command #{} to Unity via SocketIO:", count);
                            println!("   Throttle: {:.3}", command.throttle);
                            println!("   Brake: {:.3}", command.brake);
                            println!("   Steering: {:.3}°", command.steering_angle);
                        }

                        match socket.emit("data", command_data) {
                            Ok(_) => {
                                if count <= 10 || count % 10 == 0 {
                                    println!("   ✅ Command sent successfully to Unity");
                                }
                                last_command_time = std::time::Instant::now();
                            }
                            Err(e) => {
                                println!("   Failed to send rover command: {}", e);
                            }
                        }
                    }
                }
            }
            _ => {
                // Send periodic heartbeat when no commands
                if last_command_time.elapsed() > Duration::from_secs(5) {
                    let heartbeat = serde_json::json!({
                        "throttle": "0.0",
                        "brake": "0.0",
                        "steering_angle": "0.0",
                        "inset_image1": "",
                        "inset_image2": "",
                        "_heartbeat": "true",
                        "_source": "dora_sim_interface"
                    });

                    if let Err(e) = socket.emit("data", heartbeat) {
                        println!("Failed to send heartbeat: {}", e);
                    } else {
                        println!("Sent heartbeat to Unity");
                    }

                    last_command_time = std::time::Instant::now();
                }
            }
        }

        // Print debug stats every 50 loops (5 seconds)
        if loop_count % 50 == 0 {
            let stats_line = format!(
                "📊 Stats: Commands: {}, Telemetry: {}, Mode: {}, Connected: {}",
                state.commands_sent.load(Ordering::SeqCst),
                state.telemetry_received.load(Ordering::SeqCst),
                current_mode,
                connected
            );
            println!("{}", stats_line);
        }
    }
}

// Enhanced telemetry parsing with debugging
fn parse_unity_telemetry(data: &Value) -> Result<RoverTelemetry> {
    Ok(RoverTelemetry {
        position: (
            data["x"].as_f64().unwrap_or(0.0),
            data["y"].as_f64().unwrap_or(0.0),
        ),
        yaw: data["yaw"].as_f64().unwrap_or(0.0),
        pitch: data["pitch"].as_f64().unwrap_or(0.0),
        roll: data["roll"].as_f64().unwrap_or(0.0),
        velocity: data["vel"].as_f64()
            .or_else(|| data["speed"].as_f64())
            .unwrap_or(0.0),
        nav_angles: data["nav_angles"].as_array()
            .map(|arr| arr.iter().filter_map(|v| v.as_f64()).collect()),
        nav_dists: data["nav_dists"].as_array()
            .map(|arr| arr.iter().filter_map(|v| v.as_f64()).collect()),
        near_sample: data["near_sample"].as_str()
            .map(|s| s == "1" || s.to_lowercase() == "true")
            .or_else(|| data["near_sample"].as_bool())
            .unwrap_or(false),
        picking_up: data["picking_up"].as_str()
            .map(|s| s == "1" || s.to_lowercase() == "true")
            .or_else(|| data["picking_up"].as_bool())
            .unwrap_or(false),
        timestamp: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64,
    })
}

// Debug rover command processing
async fn debug_rover_command_processing(
    shared_state: &SharedState,
    cmd_data: &serde_json::Value
) -> Result<()> {
    println!("Processing rover command from dora:");
    println!("   Raw command data: {}", cmd_data);

    // Try to parse as RoverCommandWithMetadata
    if let Ok(cmd_with_metadata) = serde_json::from_value::<RoverCommandWithMetadata>(cmd_data.clone()) {
        println!("   Successfully parsed as RoverCommandWithMetadata:");
        println!("      Throttle: {:.3}", cmd_with_metadata.command.throttle);
        println!("      Brake: {:.3}", cmd_with_metadata.command.brake);
        println!("      Steering: {:.3}°", cmd_with_metadata.command.steering_angle);
        println!("      Command ID: {}", cmd_with_metadata.metadata.command_id);

        // Store the command
        if let Ok(mut latest_cmd) = shared_state.latest_rover_command.lock() {
            *latest_cmd = Some(cmd_with_metadata.command);
            println!("   Command stored for SocketIO transmission");
        }

        // Set operation mode to rover
        if let Ok(mut mode) = shared_state.operation_mode.lock() {
            *mode = "rover".to_string();
            println!("   Operation mode set to 'rover'");
        }
    } else {
        println!("   Failed to parse rover command");
    }

    Ok(())
}

fn print_debug_stats(state: &SharedState) {
    let connected = state.unity_connected.lock().map(|c| *c).unwrap_or(false);
    let mode = state.operation_mode.lock().map(|m| m.clone()).unwrap_or_else(|_| "unknown".to_string());

    println!("📊 Debug Stats:");
    println!("   Commands sent: {}", state.commands_sent.load(Ordering::SeqCst));
    println!("   Telemetry received: {}", state.telemetry_received.load(Ordering::SeqCst));
    println!("   Unity connected: {}", connected);
    println!("   Current mode: {}", mode);
}

// Mock simulation (existing functionality for arm)
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

    fn apply_command(&mut self, cmd_data: &serde_json::Value) {
        self.last_command = Some(cmd_data.to_string());

        if let Some(_command) = cmd_data.get("command") {
            // Simulate arm movement
            self.is_moving = true;
        }
    }

    fn update(&mut self) {
        // Simple simulation update
        for i in 0..self.joint_positions.len() {
            let error = self.target_positions[i] - self.joint_positions[i];
            self.joint_positions[i] += error * 0.1; // Simple proportional control
            self.joint_velocities[i] = error * 0.1;
        }

        // Check if still moving
        let max_error = self.joint_positions.iter()
            .zip(self.target_positions.iter())
            .map(|(actual, target)| (target - actual).abs())
            .fold(0.0, f64::max);

        self.is_moving = max_error > 0.001;
    }

    fn get_arm_status(&self) -> ArmStatus {
        ArmStatus {
            joint_state: JointState {
                positions: self.joint_positions.clone(),
                velocities: self.joint_velocities.clone(),
                efforts: vec![0.0; 6],
                timestamp: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as u64,
            },
            end_effector_pose: [0.0; 6],
            is_moving: self.is_moving,
            is_homed: true,
            error_state: None,
            current_command: self.last_command.clone(),
            reachability_status: ReachabilityStatus::Reachable,
        }
    }
}

fn init_tracing() -> tracing::subscriber::DefaultGuard {
    let subscriber = tracing_subscriber::fmt()
        .with_env_filter(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string())
        )
        .with_target(false)
        .with_file(false)
        .with_line_number(false)
        .finish();

    tracing::subscriber::set_default(subscriber)
}

#[derive(serde::Serialize, serde::Deserialize)]
struct RoverCommandWithMetadata {
    command: RoverCommand,
    metadata: CommandMetadata,
}