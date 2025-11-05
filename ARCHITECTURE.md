# Robo-Rover Distributed Architecture

## Overview

The robo-rover system uses a **distributed architecture** with two deployment targets:

- **Orchestra (Workstation)**: Heavy AI/ML processing, web interface, fleet control
- **Rover-Kiwi (Raspberry Pi 5)**: Hardware I/O, motor control, low-latency control loops

Communication between machines uses **Zenoh** (pub/sub protocol) for efficient real-time data exchange.

## Architecture Diagram

```
┌─────────────────────────────────┐          ┌─────────────────────────────────┐
│   ORCHESTRA (Workstation)       │          │   ROVER-KIWI (Raspberry Pi 5)   │
│                                 │          │                                 │
│  ┌──────────────────┐           │          │  ┌──────────────────┐           │
│  │  Web UI (3000)   │           │          │  │  Hardware I/O    │           │
│  │  Socket.IO (3030)│           │          │  │  - Camera        │           │
│  └────────┬─────────┘           │          │  │  - Microphone    │           │
│           │                     │          │  │  - Motors        │           │
│  ┌────────▼─────────┐           │          │  │  - Servos        │           │
│  │   web-bridge     │           │          │  └────────┬─────────┘           │
│  └────────┬─────────┘           │          │           │                     │
│           │                     │          │  ┌────────▼─────────┐           │
│  ┌────────▼─────────┐           │          │  │ Controllers      │           │
│  │  Heavy Compute   │           │   Zenoh  │  │ - rover          │           │
│  │  - YOLO Detect   │◄──────────┼──────────┤►│ - arm            │           │
│  │  - SORT Track    │   P2P     │          │  │ - visual servo   │           │
│  │  - Whisper STT   │           │          │  └────────┬─────────┘           │
│  │  - Video Encode  │           │          │           │                     │
│  │  - Audio Convert │           │          │  ┌────────▼─────────┐           │
│  └────────┬─────────┘           │          │  │  zenoh-bridge    │           │
│           │                     │          │  │  (rover mode)    │           │
│  ┌────────▼─────────┐           │          │  └──────────────────┘           │
│  │  orchestra-      │           │          │                                 │
│  │  bridge          │           │          │  Publishes:                     │
│  │  (orchestra mode)│           │          │  - Raw video (RGB8)             │
│  └──────────────────┘           │          │  - Raw audio (Float32)          │
│                                 │          │  - Telemetry                    │
│  Subscribes:                    │          │                                 │
│  - Raw data from rover          │          │  Subscribes:                    │
│                                 │          │  - Commands from orchestra      │
│  Publishes:                     │          │                                 │
│  - Commands to rover            │          │                                 │
│  - Processed detections         │          │                                 │
└─────────────────────────────────┘          └─────────────────────────────────┘
```

## Directory Structure

```
robo-rover-dora/
├── orchestra/                      # Workstation nodes (heavy compute)
│   ├── object_detector/            # YOLOv12n inference
│   ├── object_tracker/             # SORT tracking
│   ├── speech_recognizer/          # Whisper.cpp STT
│   ├── command_parser/             # NLU pattern matching
│   ├── audio_converter/            # Float32 → Int16LE
│   ├── video_encoder/              # RGB8 → JPEG
│   ├── web_bridge/                 # Socket.IO server
│   ├── sim_interface/              # Unity simulation (can run on either side)
│   ├── zenoh_bridge/               # Orchestra Zenoh bridge (orchestra-only)
│   └── orchestra-dataflow.yml      # Orchestra Dora dataflow
│
├── rover-kiwi/                     # Raspberry Pi nodes (hardware I/O)
│   ├── audio_capture/              # Microphone (cpal)
│   ├── audio_playback/             # Speaker output
│   ├── kornia_capture/             # Camera (GStreamer)
│   ├── arm_controller/             # Arm servo control
│   ├── rover_controller/           # Motor control
│   ├── visual_servo_controller/    # PID autonomous following
│   ├── kokoro_tts/                 # Local TTS feedback
│   ├── performance_monitor/        # System metrics
│   ├── dispatcher_keyboard/        # Keyboard control (dev)
│   ├── zenoh_bridge/               # Rover Zenoh bridge (rover-only)
│   └── rover-kiwi-dataflow.yml     # Rover Dora dataflow
│
├── robo_rover_lib/                 # Shared types and utilities
│
└──
```

## Zenoh Bridge - Split Implementation

The system uses **two separate zenoh_bridge implementations** for clean separation:

### Rover Zenoh Bridge (`rover_zenoh_bridge`)
**Location**: `rover-kiwi/zenoh_bridge/`
**Package**: `rover_zenoh_bridge`
**Binary**: `target/release/rover_zenoh_bridge`
**Runs on**: Raspberry Pi

**Behavior**:
- **Publishes TO Zenoh**: Raw sensor data for orchestra processing
  - `rover/{entity_id}/video/raw` - RGB8 frames (640×480×3)
  - `rover/{entity_id}/audio/raw` - Float32 audio (16kHz, mono)
  - `rover/{entity_id}/telemetry/rover` - Position/velocity
  - `rover/{entity_id}/telemetry/arm` - Joint angles
  - `rover/{entity_id}/telemetry/servo` - Visual servo state
  - `rover/{entity_id}/metrics` - System performance

- **Subscribes FROM Zenoh**: Commands from orchestra
  - `rover/{entity_id}/cmd/movement` - Velocity commands
  - `rover/{entity_id}/cmd/arm` - Joint commands
  - `rover/{entity_id}/cmd/camera` - Camera on/off
  - `rover/{entity_id}/cmd/audio` - Microphone on/off
  - `rover/{entity_id}/cmd/tracking_telemetry` - Tracking results
  - `rover/{entity_id}/cmd/tts` - TTS commands
  - `rover/{entity_id}/cmd/audio_stream` - Web UI audio stream

### Orchestra Zenoh Bridge (`orchestra_zenoh_bridge`)
**Location**: `orchestra/zenoh_bridge/`
**Package**: `orchestra_zenoh_bridge`
**Binary**: `target/release/orchestra_zenoh_bridge`
**Runs on**: Workstation

**Behavior**:
- **Subscribes FROM Zenoh**: Raw data from selected rover
  - `rover/{selected_entity}/video/raw` - RGB8 for ML inference
  - `rover/{selected_entity}/audio/raw` - Float32 for STT
  - `rover/{selected_entity}/telemetry/*` - All telemetry
  - `rover/{selected_entity}/metrics` - Performance data

- **Publishes TO Zenoh**: Commands and processed results to rover
  - `rover/{selected_entity}/cmd/*` - All command types
  - `rover/{selected_entity}/video/detections` - YOLO detections

### Environment Variables

```bash
# Rover configuration (rover_zenoh_bridge)
ENTITY_ID=rover-kiwi        # Unique rover identifier
ZENOH_MODE=peer             # Peer-to-peer discovery

# Orchestra configuration (orchestra_zenoh_bridge)
ENTITY_ID=orchestra         # Orchestra identifier
SELECTED_ENTITY=rover-kiwi  # Which rover to control
ZENOH_MODE=peer
```

## Data Flow

### Rover → Orchestra (Sensor Data)

1. **Hardware capture** (gst-camera, audio-capture)
2. **Raw data** → `rover/{entity_id}/*` topics via Zenoh
3. **Orchestra receives** and forwards to compute nodes:
   - RGB8 → object-detector → object-tracker
   - Float32 audio → speech-recognizer → command-parser
4. **Processing results** published back to Zenoh

### Orchestra → Rover (Commands)

1. **Web UI** → web-bridge (Socket.IO)
2. **web-bridge** → orchestra-bridge (Dora)
3. **orchestra-bridge** → `rover/{entity_id}/cmd/*` via Zenoh
4. **Rover zenoh-bridge** → controllers (Dora)
5. **Controllers execute** on hardware

## Performance Characteristics

### Rover (Raspberry Pi 5)
- **CPU**: ~35% (down from 110% on single machine)
  - Hardware I/O: 10%
  - Control loops: 15%
  - Video encoding: 0% (moved to orchestra)
  - No ML inference
- **Memory**: ~350MB
- **Network**: ~27 MB/s upload (RGB8 @ 30fps)
- **Latency**: <20ms command response

### Orchestra (Workstation)
- **CPU**: ~80% (with GPU acceleration)
  - YOLO: 25% CPU (or GPU)
  - Whisper: 20% CPU (or GPU)
  - Video encoding: 30%
  - Audio conversion: 5%
- **Memory**: ~1.5GB (ML models)
- **Network**: ~27 MB/s download + 1 MB/s upload
- **Latency**: 1-5ms Zenoh overhead

### Network Requirements
- **Bandwidth**: 30 Mbps (gigabit LAN recommended)
- **Latency**: <10ms on LAN
- **Topology**: Direct P2P via Zenoh multicast discovery
- **Protocol**: Zenoh over TCP/UDP (automatic selection)

## Deployment

### Prerequisites

**On both machines**:
```bash
# Install Dora
cargo install dora-cli
```

**On Orchestra**:
- ONNX Runtime for YOLO
- Whisper model for STT
- Kokoro TTS models (optional)

**On Rover-Kiwi**:
- GStreamer for camera
- cpal for audio
- Kokoro TTS models for local feedback

### Build and Deploy

#### 1. Orchestra (Workstation)

```bash
cd /home/loidinh/ws/robo-rover-dora

# Build all orchestra nodes
./deployments/orchestra/deploy.sh

# Start orchestra dataflow
dora up
dora start deployments/orchestra/orchestra-dataflow.yml --name orchestra --attach
```

#### 2. Rover-Kiwi (Raspberry Pi)

```bash
cd /home/loidinh/ws/robo-rover-dora

# Build all rover nodes
./deployments/rover-kiwi/deploy.sh

# Start rover dataflow
dora up
dora start deployments/rover-kiwi/rover-kiwi-dataflow.yml --name rover-kiwi --attach
```

#### 3. Access Web UI

Open browser: `http://<workstation-ip>:3000`

Socket.IO connects to `<workstation-ip>:3030`

### Startup Sequence

**Important**: Start in this order for proper Zenoh discovery:

1. **Start orchestra first** (waits for rover data)
2. **Start rover second** (publishes data immediately)
3. Zenoh peers discover each other via multicast (takes 1-2 seconds)
4. Data flows automatically once both are running

## Extending the System

### Adding a New Rover

1. Copy rover-kiwi directory: `cp -r rover-kiwi rover-b`
2. Update `ENTITY_ID=rover-b` in rover-b-dataflow.yml
3. Build and deploy rover-b on second Raspberry Pi
4. Orchestra can switch between rovers using `SELECTED_ENTITY` variable

### Adding Heavy Compute Node

1. Create node in `orchestra/` directory
2. Add to `orchestra-dataflow.yml`
3. Connect inputs from orchestra-bridge outputs
4. Publish results back to orchestra-bridge for Zenoh transmission

### Fleet Management (Future)

Current: Orchestra processes ONE selected rover at a time
Future: Orchestra processes MULTIPLE rovers in parallel with:
- Per-rover processing threads
- Shared ML model instances
- Web UI multi-rover dashboard

## Key Design Decisions

### Why Raw RGB8 Video from Rover?

**Decision**: Rover sends raw RGB8 (27 MB/s), not JPEG

**Rationale**:
- ML inference needs raw pixels (YOLO input)
- Decoding JPEG on orchestra adds latency
- Gigabit LAN handles 27 MB/s easily
- Saves rover CPU (30% encoding overhead)

**Tradeoff**: Not suitable for WAN deployment (would need H.264 encoding)

### Why Visual Servoing on Rover?

**Decision**: PID control runs on rover, not orchestra

**Rationale**:
- Low-latency control loop (<5ms required)
- Network latency too high for PID (10-20ms)
- Tracking telemetry sent from orchestra is sufficient

**Implementation**: Tracking runs on orchestra, servo control on rover

## References

- **Zenoh Protocol**: https://zenoh.io
- **Dora Framework**: https://github.com/dora-rs/dora
- **Cargo.toml**: Workspace configuration
