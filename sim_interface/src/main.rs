use dora_node_api::arrow::array::{Array, AsArray};
use dora_node_api::{
    arrow::array::{types::GenericBinaryType, BinaryArray},
    dora_core::config::DataId,
    DoraNode, Event,
};
use eyre::Result;
use robo_rover_lib::{
    ArmCommand, ArmTelemetry, CommandMetadata, RoverCommand, RoverTelemetry, SimulationConfig,
};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use uuid;

use axum::http::Method;
use serde_json::Value;
use socketioxide::{
    extract::{Data, SocketRef},
    SocketIo,
};
use tokio;
use tower::ServiceBuilder;
use tower_http::cors::{Any, CorsLayer};

#[derive(Clone)]
struct SharedState {
    latest_arm_command: Arc<Mutex<Option<ArmCommand>>>,
    latest_rover_command: Arc<Mutex<Option<RoverCommand>>>,
    latest_rover_telemetry: Arc<Mutex<Option<RoverTelemetry>>>,
    latest_arm_telemetry: Arc<Mutex<Option<ArmTelemetry>>>,
    unity_connected: Arc<Mutex<bool>>,
    // Debug counters
    commands_sent: Arc<AtomicU64>,
    rover_telemetry_received: Arc<AtomicU64>,
    arm_telemetry_received: Arc<AtomicU64>,
    connection_count: Arc<AtomicU64>,
}

impl SharedState {
    fn new() -> Self {
        Self {
            latest_arm_command: Arc::new(Mutex::new(None)),
            latest_rover_command: Arc::new(Mutex::new(None)),
            latest_rover_telemetry: Arc::new(Mutex::new(None)),
            latest_arm_telemetry: Arc::new(Mutex::new(None)),
            unity_connected: Arc::new(Mutex::new(false)),
            // Initialize debug counters
            commands_sent: Arc::new(AtomicU64::new(0)),
            rover_telemetry_received: Arc::new(AtomicU64::new(0)),
            arm_telemetry_received: Arc::new(Default::default()),
            connection_count: Arc::new(AtomicU64::new(0)),
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
struct RoverCommandWithMetadata {
    command: RoverCommand,
    metadata: CommandMetadata,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct ArmCommandWithMetadata {
    command: Option<ArmCommand>,
    metadata: CommandMetadata,
}

fn main() -> Result<()> {
    let _guard = init_tracing();

    println!("Starting Sim Interface Node with SocketIO Server");
    println!("Debug mode enabled for Unity integration");

    // Use multithreaded runtime for proper SocketIO server support
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;

    rt.block_on(async { sim_interface_async().await })
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
    let rover_telemetry_output = DataId::from("rover_telemetry".to_owned());
    let arm_telemetry_output = DataId::from("arm_telemetry".to_owned());

    // Load simulation configuration
    let sim_config =
        SimulationConfig::load_from_file("config/simulation.toml").unwrap_or_else(|_| {
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
        tokio::net::TcpStream::connect("127.0.0.1:4567"),
    )
    .await
    {
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

    let update_interval = Duration::from_secs_f64(1.0 / sim_config.update_rate_hz);
    let mut last_update = std::time::Instant::now();
    let mut debug_counter = 0u64;

    println!("Sim interface initialized");
    println!("SocketIO server should be running on port 4567 for Unity");
    println!("Waiting for events from dora nodes...");
    println!("Test connection: curl -X GET http://127.0.0.1:4567/socket.io/");

    loop {
        // Non-blocking event processing with timeout
        let event_future = tokio::time::timeout(Duration::from_millis(10), async { events.recv() });

        if let Ok(Some(event)) = event_future.await {
            debug_counter += 1;

            match event {
                Event::Input { id, data, .. } => {
                    let id_str = id.as_str();
                    println!("Received input #{}: '{}'", debug_counter, id_str);

                    match id_str {
                        "arm_command" => {
                            println!("Processing ARM command...");
                            if let Some(bytes_array) = data.as_bytes_opt::<GenericBinaryType<i32>>()
                            {
                                if bytes_array.len() > 0 {
                                    let bytes = bytes_array.value(0);
                                    if let Ok(cmd_data) =
                                        serde_json::from_slice::<serde_json::Value>(bytes)
                                    {
                                        if let Err(e) =
                                            debug_arm_command_processing(&shared_state, &cmd_data)
                                                .await
                                        {
                                            println!("Error processing arm command: {}", e);
                                        }
                                    }
                                }
                            }
                        }

                        "rover_command" => {
                            println!("Processing ROVER command...");
                            if let Some(bytes_array) = data.as_bytes_opt::<GenericBinaryType<i32>>()
                            {
                                if bytes_array.len() > 0 {
                                    let bytes = bytes_array.value(0);
                                    if let Ok(cmd_data) =
                                        serde_json::from_slice::<serde_json::Value>(bytes)
                                    {
                                        if let Err(e) =
                                            debug_rover_command_processing(&shared_state, &cmd_data)
                                                .await
                                        {
                                            println!("Error processing rover command: {}", e);
                                        }
                                    } else {
                                        println!("Failed to parse rover command JSON");
                                    }
                                } else {
                                    println!("Empty rover command data");
                                }
                            } else {
                                println!("Failed to read rover command bytes");
                            }
                        }

                        _ => {
                            println!("Unknown input: '{}'", id_str);
                        }
                    }
                }

                Event::Stop(_) => {
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
            // Send rover telemetry if available
            if let Ok(rover_tel_opt) = shared_state.latest_rover_telemetry.lock() {
                if let Some(rover_tel) = rover_tel_opt.as_ref() {
                    let rover_serialized = serde_json::to_vec(rover_tel)?;
                    let rover_arrow = BinaryArray::from_vec(vec![rover_serialized.as_slice()]);
                    node.send_output(
                        rover_telemetry_output.clone(),
                        Default::default(),
                        rover_arrow,
                    )?;
                }
            }

            // Send arm telemetry if available
            if let Ok(arm_tel_opt) = shared_state.latest_arm_telemetry.lock() {
                if let Some(arm_tel) = arm_tel_opt.as_ref() {
                    let arm_tel_serialized = serde_json::to_vec(arm_tel)?;
                    let arm_tel_arrow = BinaryArray::from_vec(vec![arm_tel_serialized.as_slice()]);
                    node.send_output(
                        arm_telemetry_output.clone(),
                        Default::default(),
                        arm_tel_arrow,
                    )?;
                }
            }

            last_update = now;
        }

        // Print debug stats every 10 seconds (1000 * 10ms loops)
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

async fn debug_arm_command_processing(
    shared_state: &SharedState,
    cmd_data: &serde_json::Value,
) -> Result<()> {
    println!("Processing arm command from dora:");
    println!("   Raw command data: {}", cmd_data);

    // Try to parse as ArmCommandWithMetadata first
    if let Ok(cmd_with_metadata) =
        serde_json::from_slice::<ArmCommandWithMetadata>(cmd_data.to_string().as_bytes())
    {
        if let Some(arm_command) = cmd_with_metadata.command {
            println!("   Successfully parsed ArmCommandWithMetadata:");
            println!("      Command: {:?}", arm_command);
            println!(
                "      Command ID: {}",
                cmd_with_metadata.metadata.command_id
            );

            // Store the command for Unity transmission
            if let Ok(mut latest_cmd) = shared_state.latest_arm_command.lock() {
                *latest_cmd = Some(arm_command);
                println!("   ARM command stored for SocketIO transmission");
            }
        } else {
            println!("   ArmCommandWithMetadata has no command");
        }
    } else {
        println!("   Failed to parse arm command");
    }

    Ok(())
}

async fn debug_rover_command_processing(
    shared_state: &SharedState,
    cmd_data: &serde_json::Value,
) -> Result<()> {
    println!("Processing rover command from dora:");
    println!("   Raw command data: {}", cmd_data);

    // Try to parse as RoverCommandWithMetadata
    if let Ok(cmd_with_metadata) =
        serde_json::from_slice::<RoverCommandWithMetadata>(cmd_data.to_string().as_bytes())
    {
        println!("   Successfully parsed RoverCommandWithMetadata:");
        println!("      Throttle: {:.3}", cmd_with_metadata.command.throttle);
        println!("      Brake: {:.3}", cmd_with_metadata.command.brake);
        println!(
            "      Steering: {:.3}°",
            cmd_with_metadata.command.steering_angle
        );
        println!(
            "      Command ID: {}",
            cmd_with_metadata.metadata.command_id
        );

        // Store the command
        if let Ok(mut latest_cmd) = shared_state.latest_rover_command.lock() {
            *latest_cmd = Some(cmd_with_metadata.command);
            println!("   ROVER command stored for SocketIO transmission");
        }

        // REMOVED: No more mode switching!
    } else {
        println!("   Failed to parse rover command");
    }

    Ok(())
}

fn print_debug_stats(state: &SharedState) {
    let connected = state.unity_connected.lock().map(|c| *c).unwrap_or(false);

    println!("Debug Stats:");
    println!(
        "   Commands sent: {}",
        state.commands_sent.load(Ordering::SeqCst)
    );
    println!(
        "   Telemetry received: {}",
        state.rover_telemetry_received.load(Ordering::SeqCst)
    );
    println!("   Unity connected: {}", connected);
    println!("   Mode: ALWAYS_FORWARD_BOTH (Fixed!)");
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

        // Handle rover telemetry from Unity
        socket.on("telemetry", {
            let socket_state = shared_state.clone();
            move |_socket: SocketRef, Data::<Value>(data)| {
                let count = socket_state.rover_telemetry_received.fetch_add(1, Ordering::SeqCst) + 1;

                if count <= 10 || count % 20 == 0 {
                    println!("Received rover telemetry #{} from Unity", count);
                    println!("   Raw data: {}", data);
                }

                match parse_unity_rover_telemetry(&data) {
                    Ok(rover_telemetry) => {
                        if count <= 5 || count % 20 == 0 {
                            println!("   Parsed telemetry successfully:");
                            println!("      Position: ({:.2}, {:.2})", rover_telemetry.position.0, rover_telemetry.position.1);
                            println!("      Velocity: {:.2} m/s", rover_telemetry.velocity);
                            println!("      Yaw: {:.1}°", rover_telemetry.yaw.to_degrees());
                        }

                        // Store the rover telemetry for dora output
                        if let Ok(mut latest_rover_tel) = socket_state.latest_rover_telemetry.lock() {
                            *latest_rover_tel = Some(rover_telemetry);
                        }
                    }
                    Err(e) => {
                        println!("   Failed to parse Unity telemetry: {}", e);
                    }
                }
            }
        });

        // Handle arm telemetry from Unity
        socket.on("arm_telemetry", {
            let socket_state = shared_state.clone();
            move |_socket: SocketRef, Data::<Value>(data)| {
                let count = socket_state.arm_telemetry_received.fetch_add(1, Ordering::SeqCst) + 1;

                if count <= 10 || count % 20 == 0 {
                    println!("Received arm telemetry #{} from Unity", count);
                    println!("   Raw data: {}", data);
                }

                match parse_unity_arm_telemetry(&data) {
                    Ok(arm_telemetry) => {
                        if count <= 5 || count % 20 == 0 {
                            println!("   Parsed telemetry successfully:");
                            println!("      End effector pose: [{:.3}, {:.3}, {:.3}, {:.3}, {:.3}, {:.3}]",
                                     arm_telemetry.end_effector_pose[0], arm_telemetry.end_effector_pose[1],
                                     arm_telemetry.end_effector_pose[2], arm_telemetry.end_effector_pose[3],
                                     arm_telemetry.end_effector_pose[4], arm_telemetry.end_effector_pose[5]);
                            println!("      Is moving: {}", arm_telemetry.is_moving);
                            println!("      Timestamp: {}", arm_telemetry.timestamp);
                        }

                        if let Ok(mut latest_arm_tel) = socket_state.latest_arm_telemetry.lock() {
                            *latest_arm_tel = Some(arm_telemetry);
                            println!("   ARM telemetry stored for dora dataflow");
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
            let socket_state = state.clone();
            move |socket: SocketRef| {
                println!("Unity disconnected: {}", socket.id);
                if let Ok(mut connected) = socket_state.unity_connected.lock() {
                    *connected = false;
                }
            }
        });

        // Start command sending loop for this connection
        let socket_clone = socket.clone();
        let state_clone = shared_state.clone();
        tokio::spawn(async move {
            command_sender_loop(socket_clone, state_clone).await;
        });
    });

    // Create HTTP app with CORS
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers(Any);

    let app = axum::Router::new().layer(ServiceBuilder::new().layer(cors).layer(layer));

    // Bind and start server
    println!("Attempting to bind to 127.0.0.1:4567...");
    let listener = match tokio::net::TcpListener::bind("127.0.0.1:4567").await {
        Ok(listener) => {
            println!("Successfully bound to 127.0.0.1:4567");
            listener
        }
        Err(e) => {
            println!("Failed to bind to 127.0.0.1:4567: {}", e);
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
    let mut loop_count = 0u64;

    println!(
        "Starting FIXED command sender loop for connection {}",
        socket.id
    );

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
            println!(
                "Unity disconnected, stopping command loop for {}",
                socket.id
            );
            break;
        }

        // FIXED: Always try to send both types - no mode switching!
        let mut commands_sent = 0;

        // Try to send rover commands
        if let Ok(mut cmd_opt) = state.latest_rover_command.lock() {
            if let Some(command) = cmd_opt.take() {
                let count = state.commands_sent.fetch_add(1, Ordering::SeqCst) + 1;

                let command_data = serde_json::json!({
                    "throttle": command.throttle.to_string(),
                    "brake": command.brake.to_string(),
                    "steering_angle": command.steering_angle.to_string(),
                    "inset_image1": "",
                    "inset_image2": "",
                    "_command_id": command.command_id,
                    "_timestamp": command.timestamp,
                    "_source": "dora_sim_interface"
                });

                if count <= 10 || count % 20 == 0 {
                    println!("Sending rover command #{} to Unity", count);
                    println!(
                        "   Data: throttle={:.3}, steering={:.1}°, brake={:.3}",
                        command.throttle, command.steering_angle, command.brake
                    );
                }

                match socket.emit("data", command_data) {
                    Ok(_) => {
                        commands_sent += 1;
                    }
                    Err(e) => {
                        println!("Failed to send rover command: {}", e);
                    }
                }
            }
        }

        // Try to send arm commands
        if let Ok(mut cmd_opt) = state.latest_arm_command.lock() {
            if let Some(command) = cmd_opt.take() {
                let count = state.commands_sent.fetch_add(1, Ordering::SeqCst) + 1;

                // Convert Rust ArmCommand to Unity format
                let unity_command = convert_arm_command_to_unity(&command);

                let command_data = serde_json::json!({
                    "command": unity_command,
                    "_command_id": uuid::Uuid::new_v4().to_string(),
                    "_timestamp": SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_millis() as u64,
                    "_source": "dora_sim_interface"
                });

                if count <= 10 || count % 20 == 0 {
                    println!("Sending arm command #{} to Unity", count);
                    println!("   Command: {:?}", command);
                }

                // Send as "arm_command" event to Unity
                match socket.emit("arm_command", command_data) {
                    Ok(_) => {
                        commands_sent += 1;
                    }
                    Err(e) => {
                        println!("Failed to send arm command: {}", e);
                    }
                }
            }
        }

        // Debug logging
        if commands_sent > 0 && loop_count % 10 == 0 {
            println!("Sent {} commands in loop #{}", commands_sent, loop_count);
        }

        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

// Convert Rust ArmCommand to Unity-compatible format
fn convert_arm_command_to_unity(arm_cmd: &ArmCommand) -> serde_json::Value {
    match arm_cmd {
        ArmCommand::CartesianMove {
            x,
            y,
            z,
            roll,
            pitch,
            yaw,
            max_velocity,
        } => {
            serde_json::json!({
                "type": "CartesianMove",
                "x": x,
                "y": y,
                "z": z,
                "roll": roll,
                "pitch": pitch,
                "yaw": yaw,
                "max_velocity": max_velocity.unwrap_or(1.5)
            })
        }
        ArmCommand::JointPosition {
            joint_angles,
            max_velocity,
        } => {
            serde_json::json!({
                "type": "JointPosition",
                "joint_angles": joint_angles,
                "max_velocity": max_velocity.unwrap_or(2.0)
            })
        }
        ArmCommand::RelativeMove { delta_joints } => {
            serde_json::json!({
                "type": "RelativeMove",
                "delta_joints": delta_joints
            })
        }
        ArmCommand::Stop => {
            serde_json::json!({
                "type": "Stop"
            })
        }
        ArmCommand::Home => {
            serde_json::json!({
                "type": "Home"
            })
        }
        ArmCommand::EmergencyStop => {
            serde_json::json!({
                "type": "EmergencyStop"
            })
        }
    }
}

fn parse_unity_rover_telemetry(data: &Value) -> Result<RoverTelemetry> {
    let position = data
        .get("position")
        .and_then(|v| v.as_str())
        .map(|s| {
            let parts: Vec<&str> = s.split(';').collect();
            if parts.len() >= 2 {
                (
                    parts[0].parse().unwrap_or(0.0),
                    parts[1].parse().unwrap_or(0.0),
                )
            } else {
                (0.0, 0.0)
            }
        })
        .unwrap_or((0.0, 0.0));

    let yaw = data
        .get("yaw")
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(0.0);

    let velocity = data
        .get("speed")
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(0.0);

    Ok(RoverTelemetry {
        position,
        yaw: yaw.to_radians(),
        pitch: 0.0,
        roll: 0.0,
        velocity,
        nav_angles: None,
        nav_dists: None,
        near_sample: false,
        picking_up: false,
        timestamp: SystemTime::now()
            .duration_since(UNIX_EPOCH)?
            .as_millis() as u64,
    })
}

fn parse_unity_arm_telemetry(data: &Value) -> Result<ArmTelemetry> {
    let end_effector_pose =
        if let Some(pose_array) = data.get("end_effector_pose").and_then(|v| v.as_array()) {
            [
                pose_array.get(0).and_then(|v| v.as_f64()).unwrap_or(0.0),
                pose_array.get(1).and_then(|v| v.as_f64()).unwrap_or(0.0),
                pose_array.get(2).and_then(|v| v.as_f64()).unwrap_or(0.0),
                pose_array.get(3).and_then(|v| v.as_f64()).unwrap_or(0.0),
                pose_array.get(4).and_then(|v| v.as_f64()).unwrap_or(0.0),
                pose_array.get(5).and_then(|v| v.as_f64()).unwrap_or(0.0),
            ]
        } else {
            [0.0; 6]
        };

    let is_moving = data
        .get("is_moving")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let timestamp = data
        .get("timestamp")
        .and_then(|v| v.as_u64())
        .unwrap_or_else(|| {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64
        });

    let joint_angles = data
        .get("joint_angles")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_f64()).collect());

    let joint_velocities = data
        .get("joint_velocities")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_f64()).collect());

    Ok(ArmTelemetry {
        end_effector_pose,
        is_moving,
        timestamp,
        joint_angles,
        joint_velocities,
        source: "unity_simulation".to_string(),
    })
}

fn init_tracing() -> tracing::subscriber::DefaultGuard {
    let subscriber = tracing_subscriber::fmt()
        .with_env_filter(std::env::var("RUST_LOG").unwrap_or_else(|_| "debug".to_string()))
        .with_target(false)
        .with_file(false)
        .with_line_number(false)
        .finish();

    tracing::subscriber::set_default(subscriber)
}
