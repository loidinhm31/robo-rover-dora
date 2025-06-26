use robo_rover_lib::{ArmCommand, ArmStatus, JointState, ReachabilityStatus, RoverCommand, RoverTelemetry, SimulationConfig};
use dora_node_api::arrow::array::{Array, AsArray};
use dora_node_api::{arrow::array::{types::GenericBinaryType, BinaryArray}, dora_core::config::DataId, DoraNode, Event};
use eyre::Result;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use axum::http::Method;
use serde_json::Value;
use socketioxide::{extract::{Data, SocketRef}, SocketIo};
use tokio;
use tower::ServiceBuilder;
use tower_http::cors::{Any, CorsLayer};

// Shared state between SocketIO and dora
#[derive(Clone)]
struct SharedState {
    latest_arm_command: Arc<Mutex<Option<ArmCommand>>>,
    latest_rover_command: Arc<Mutex<Option<RoverCommand>>>,
    latest_rover_telemetry: Arc<Mutex<Option<RoverTelemetry>>>,
    unity_connected: Arc<Mutex<bool>>,
    operation_mode: Arc<Mutex<String>>, // "arm" or "rover"
}

impl SharedState {
    fn new() -> Self {
        Self {
            latest_arm_command: Arc::new(Mutex::new(None)),
            latest_rover_command: Arc::new(Mutex::new(None)),
            latest_rover_telemetry: Arc::new(Mutex::new(None)),
            unity_connected: Arc::new(Mutex::new(false)),
            operation_mode: Arc::new(Mutex::new("arm".to_string())),
        }
    }
}

fn main() -> Result<()> {
    let _guard = init_tracing();

    println!("Starting Sim Interface Node with SocketIO Server");

    // Use existing pattern from your sim_interface
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;

    rt.block_on(async {
        sim_interface_async().await
    })
}

async fn sim_interface_async() -> Result<()> {
    let (mut node, mut events) = DoraNode::init_from_env()?;
    let joint_feedback_output = DataId::from("joint_feedback".to_owned());
    let rover_telemetry_output = DataId::from("rover_telemetry".to_owned());

    // Load simulation configuration
    let sim_config = SimulationConfig::load_from_file("config/simulation.toml")
        .unwrap_or_else(|_| SimulationConfig {
            unity_websocket_port: 8080,
            update_rate_hz: 60.0,
            physics_timestep: 0.02,
        });

    let shared_state = SharedState::new();
    let shared_state_clone = shared_state.clone();

    // Start SocketIO server for Unity
    let socketio_handle = tokio::spawn(async move {
        if let Err(e) = start_socketio_server(shared_state_clone).await {
            println!("SocketIO server error: {}", e);
        }
    });

    // Mock simulation for arm (existing functionality)
    let mut mock_sim = MockSimulation::new();

    let update_interval = Duration::from_secs_f64(1.0 / sim_config.update_rate_hz);
    let mut last_update = std::time::Instant::now();

    println!("Sim interface initialized");
    println!("SocketIO server running on port 4567 for Unity");

    loop {
        // Handle dora events (existing pattern)
        if let Some(event) = events.recv() {
            match event {
                Event::Input { id, data, .. } => {
                    let id_str = id.as_str();

                    if id_str == "arm_command" {
                        // Handle ARM commands (existing functionality)
                        if let Some(bytes_array) = data.as_bytes_opt::<GenericBinaryType<i32>>() {
                            if bytes_array.len() > 0 {
                                let bytes = bytes_array.value(0);
                                if let Ok(cmd_data) = serde_json::from_slice::<serde_json::Value>(bytes) {
                                    println!("Received arm command for Unity");

                                    // Set mode to arm
                                    if let Ok(mut mode) = shared_state.operation_mode.lock() {
                                        *mode = "arm".to_string();
                                    }

                                    // Store for SocketIO to send to Unity
                                    if let Ok(arm_cmd) = serde_json::from_value::<ArmCommand>(cmd_data.clone()) {
                                        if let Ok(mut cmd) = shared_state.latest_arm_command.lock() {
                                            *cmd = Some(arm_cmd);
                                        }
                                    }

                                    // Apply to mock simulation
                                    mock_sim.apply_command(&cmd_data);
                                }
                            }
                        }
                    }
                    else if id_str == "rover_command" {
                        // Handle ROVER commands (new functionality)
                        if let Some(bytes_array) = data.as_bytes_opt::<GenericBinaryType<i32>>() {
                            if bytes_array.len() > 0 {
                                let bytes = bytes_array.value(0);
                                if let Ok(rover_cmd) = serde_json::from_slice::<RoverCommand>(bytes) {
                                    println!("Received rover command: throttle={:.2}, brake={:.2}, steer={:.2}",
                                           rover_cmd.throttle, rover_cmd.brake, rover_cmd.steering_angle);

                                    // Set mode to rover
                                    if let Ok(mut mode) = shared_state.operation_mode.lock() {
                                        *mode = "rover".to_string();
                                    }

                                    // Store for SocketIO to send to Unity
                                    if let Ok(mut cmd) = shared_state.latest_rover_command.lock() {
                                        *cmd = Some(rover_cmd);
                                    }
                                }
                            }
                        }
                    }
                }
                Event::Stop => {
                    println!("Stopping sim interface");
                    break;
                }
                _ => {}
            }
        }

        // Send feedback based on current mode
        let now = std::time::Instant::now();
        if now.duration_since(last_update) >= update_interval {
            let current_mode = shared_state.operation_mode.lock().unwrap().clone();

            match current_mode.as_str() {
                "arm" => {
                    // Send ARM feedback (existing functionality)
                    mock_sim.update(update_interval.as_secs_f64());
                    let status = mock_sim.get_current_status();

                    match serde_json::to_vec(&status) {
                        Ok(serialized) => {
                            if let Err(e) = node.send_output(
                                joint_feedback_output.clone(),
                                Default::default(),
                                BinaryArray::from_vec(vec![&*serialized]),
                            ) {
                                println!("Failed to send joint feedback: {}", e);
                            }
                        }
                        Err(e) => {
                            println!("Failed to serialize joint feedback: {}", e);
                        }
                    }
                }
                "rover" => {
                    // Send ROVER telemetry
                    if let Ok(mut telemetry_opt) = shared_state.latest_rover_telemetry.lock() {
                        if let Some(telemetry) = telemetry_opt.take() {
                            println!("Forwarding rover telemetry: pos=({:.2}, {:.2}), vel={:.2}",
                                   telemetry.position.0, telemetry.position.1, telemetry.velocity);

                            match serde_json::to_vec(&telemetry) {
                                Ok(serialized) => {
                                    if let Err(e) = node.send_output(
                                        rover_telemetry_output.clone(),
                                        Default::default(),
                                        BinaryArray::from_vec(vec![&*serialized]),
                                    ) {
                                        println!("Failed to send rover telemetry: {}", e);
                                    }
                                }
                                Err(e) => {
                                    println!("Failed to serialize rover telemetry: {}", e);
                                }
                            }
                        }
                    }
                }
                _ => {}
            }

            last_update = now;
        } else {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    socketio_handle.abort();
    Ok(())
}

async fn start_socketio_server(shared_state: SharedState) -> Result<()> {
    println!("🌐 Starting SocketIO server on port 4567 (like Python server)");

    // Create SocketIO instance
    let (layer, io) = SocketIo::new_layer();

    let shared_state_clone = shared_state.clone();

    // Handle Unity connections (matches Python @sio.on patterns)
    io.ns("/", move |socket: SocketRef| {
        println!("🔗 Unity connected: {}", socket.id);

        let state = shared_state_clone.clone();

        // Update connection status
        if let Ok(mut connected) = state.unity_connected.lock() {
            *connected = true;
        }

        // Handle telemetry from Unity (matches Python @sio.on('telemetry'))
        socket.on("telemetry", {
            let state = state.clone();
            move |socket: SocketRef, Data::<Value>(data)| {
                println!("Received telemetry from Unity");

                // Parse rover telemetry (like Python telemetry function)
                match parse_unity_telemetry(&data) {
                    Ok(telemetry) => {
                        if let Ok(mut tel) = state.latest_rover_telemetry.lock() {
                            *tel = Some(telemetry);
                        }
                    }
                    Err(e) => {
                        println!("Failed to parse Unity telemetry: {}", e);
                    }
                }
            }
        });

        // Handle Unity connection request (matches Python @sio.on('connect'))
        socket.on("connect", {
            let state = state.clone();
            move |socket: SocketRef| {
                println!("Unity SocketIO connected");
                // Send initial response if needed
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

        // Start command sending loop
        let socket_clone = socket.clone();
        let state_clone = state.clone();
        tokio::spawn(async move {
            command_sender_loop(socket_clone, state_clone).await;
        });
    });

    // Create HTTP app with CORS
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::POST])
        .allow_headers(Any);

    let app = axum::Router::new()
        .layer(ServiceBuilder::new().layer(cors).layer(layer));

    let listener = tokio::net::TcpListener::bind("127.0.0.1:4567").await?;
    println!("SocketIO server listening on 127.0.0.1:4567");

    axum::serve(listener, app).await?;
    Ok(())
}

async fn command_sender_loop(socket: SocketRef, state: SharedState) {
    let mut interval = tokio::time::interval(Duration::from_millis(50)); // 20 Hz

    loop {
        interval.tick().await;

        let current_mode = {
            let mode_guard = state.operation_mode.lock().unwrap();
            mode_guard.clone()
        };

        match current_mode.as_str() {
            "rover" => {
                // Send rover commands (matches Python send_control function)
                if let Ok(mut cmd_opt) = state.latest_rover_command.lock() {
                    if let Some(command) = cmd_opt.take() {
                        let command_data = serde_json::json!({
                            "throttle": command.throttle.to_string(),
                            "brake": command.brake.to_string(),
                            "steering_angle": command.steering_angle.to_string(),
                            "inset_image1": "",
                            "inset_image2": "",
                        });

                        println!("Sending rover command to Unity via SocketIO");

                        // This matches Python: sio.emit("data", data, skip_sid=True)
                        if let Err(e) = socket.emit("data", command_data) {
                            println!("Failed to send rover command to Unity: {}", e);
                        }
                    }
                }
            }
            "arm" => {
                // Send arm commands if needed
                if let Ok(mut cmd_opt) = state.latest_arm_command.lock() {
                    if let Some(_command) = cmd_opt.take() {
                        // Send arm-specific commands to Unity if needed
                        println!("Arm command handling (implement if needed)");
                    }
                }
            }
            _ => {}
        }

        // Check connection status
        if let Ok(connected) = state.unity_connected.lock() {
            if !*connected {
                println!("Unity disconnected, stopping command loop");
                break;
            }
        }
    }
}

fn parse_unity_telemetry(data: &Value) -> Result<RoverTelemetry> {
    // Parse exactly like Python telemetry function
    Ok(RoverTelemetry {
        position: (
            data["x"].as_f64().unwrap_or(0.0),
            data["y"].as_f64().unwrap_or(0.0),
        ),
        yaw: data["yaw"].as_f64().unwrap_or(0.0),
        pitch: data["pitch"].as_f64().unwrap_or(0.0),
        roll: data["roll"].as_f64().unwrap_or(0.0),
        velocity: data["vel"].as_f64().unwrap_or(0.0),
        nav_angles: data["nav_angles"].as_array()
            .map(|arr| arr.iter().filter_map(|v| v.as_f64()).collect()),
        nav_dists: data["nav_dists"].as_array()
            .map(|arr| arr.iter().filter_map(|v| v.as_f64()).collect()),
        near_sample: data["near_sample"].as_bool().unwrap_or(false),
        picking_up: data["picking_up"].as_bool().unwrap_or(false),
        timestamp: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64,
    })
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
        println!("Applying command to mock simulation");
        // Mock command application
        self.is_moving = true;
        self.last_command = Some("MockCommand".to_string());
    }

    fn update(&mut self, _dt: f64) {
        // Simple physics update
        for i in 0..self.joint_positions.len() {
            let error = self.target_positions[i] - self.joint_positions[i];
            if error.abs() > 0.001 {
                let step = error * 0.1;
                self.joint_positions[i] += step;
                self.joint_velocities[i] = step * 10.0;
            } else {
                self.joint_velocities[i] = 0.0;
            }
        }
    }

    fn get_current_status(&self) -> ArmStatus {
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

fn init_tracing() -> tracing::subscriber::DefaultGuard {
    let subscriber = tracing_subscriber::fmt()
        .with_env_filter(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "println".to_string())
        )
        .finish();

    tracing::subscriber::set_default(subscriber)
}