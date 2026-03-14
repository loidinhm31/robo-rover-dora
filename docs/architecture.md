# Robo-Fleet Control — Architecture

React 19 + TypeScript control UI for the Robo-Fleet distributed rover system. Turborepo pnpm monorepo with web browser and Tauri desktop targets sharing a common UI library. Connects to `orchestra/web_bridge` Rust backend via Socket.IO on port 3030.

```mermaid
flowchart TB
    subgraph Clients["Deployment Targets"]
        WEB["Web App<br/>(Vite, port 25010)"]
        NATIVE["Tauri Desktop<br/>(port 1420)"]
    end

    subgraph UI["@robo-fleet/ui"]
        RRC["RoboRoverControl<br/>(main page)"]
        EMBED["RoboControlApp<br/>(embed entry)"]
    end

    subgraph Backend["Orchestra Backend"]
        WB["web_bridge<br/>(Socket.IO, port 3030)"]
        ROVER["Rover Fleet<br/>(hardware)"]
    end

    WEB --> RRC
    NATIVE --> RRC
    EMBED --> RRC
    RRC <-->|Socket.IO| WB
    WB <--> ROVER

    classDef client fill:#1e293b,stroke:#3b82f6,color:#f1f5f9
    classDef ui fill:#1e293b,stroke:#10b981,color:#f1f5f9
    classDef backend fill:#1e293b,stroke:#f97316,color:#f1f5f9
    class WEB,NATIVE client
    class RRC,EMBED ui
    class WB,ROVER backend
```

## Monorepo Structure

```
robo-control-app/
├── apps/
│   ├── web/           @robo-fleet/web     — Vite browser app
│   └── native/        @robo-fleet/native  — Tauri v2 desktop app
├── packages/
│   ├── ui/            @robo-fleet/ui      — Components, hooks, services, adapters
│   ├── shared/        @robo-fleet/shared  — Pure TS types & constants (zero deps)
│   ├── tsconfig/      Shared TS configs
│   └── eslint-config/ Shared ESLint rules
├── src/               LEGACY — pre-monorepo root app
└── old.app/           LEGACY — archived previous version
```

Both apps are thin shells — import `RoboRoverControl` from `@robo-fleet/ui` and pass env-based config (`VITE_SOCKET_IO_URL`, `VITE_AUTH_USERNAME`, `VITE_AUTH_PASSWORD`).

### Build Pipeline

```mermaid
flowchart LR
    SHARED["@robo-fleet/shared<br/>(types, constants)"]
    UI["@robo-fleet/ui<br/>(components, hooks)"]
    WEB["@robo-fleet/web"]
    NATIVE["@robo-fleet/native"]

    SHARED --> UI --> WEB
    UI --> NATIVE

    classDef pkg fill:#1e293b,stroke:#a855f7,color:#f1f5f9
    classDef app fill:#1e293b,stroke:#3b82f6,color:#f1f5f9
    class SHARED,UI pkg
    class WEB,NATIVE app
```

Turbo tasks: `build` (`dependsOn: ["^build"]`), `dev` (persistent, no cache), `check-types`, `lint`.

## Component Hierarchy

Atomic Design in `packages/ui/src/components/`:

```mermaid
flowchart TB
    subgraph Pages
        RRC["RoboRoverControl"]
    end

    subgraph Templates
        AS["AppShell"]
    end

    subgraph Features
        CAM["CameraViewer"]
        LOC["LocationMap"]
        FM["FloatingMetrics"]
        TD["TranscriptionDisplay"]
        VC["VoiceControls"]
    end

    subgraph Organisms
        FS["FleetSelector"]
        JCP["JointControlPanel"]
        SS["ServerSettings"]
    end

    subgraph Molecules
        CS["CollapsibleSection"]
        SC["SliderControl"]
        SP["StatPanel"]
        TC["ToggleControl"]
    end

    subgraph Atoms
        BI["BatteryIndicator"]
        SB["StatusBadge"]
        SCd["StatCard"]
        TB["ToggleButton"]
        VD["ValueDisplay"]
        LS["LoadingSpinner"]
    end

    RRC --> AS
    RRC --> CAM
    RRC --> LOC
    RRC --> FM
    RRC --> FS
    RRC --> JCP
    RRC --> VC
    RRC --> TD
    RRC --> CS
    AS --> SB
    FS --> BI
    FS --> SCd
    JCP --> SC
    FM --> SP
    SP --> SCd
    CS --> TB

    classDef page fill:#3b82f6,stroke:#60a5fa,color:#f1f5f9
    classDef template fill:#8b5cf6,stroke:#a78bfa,color:#f1f5f9
    classDef feature fill:#10b981,stroke:#34d399,color:#f1f5f9
    classDef organism fill:#f97316,stroke:#fb923c,color:#f1f5f9
    classDef molecule fill:#06b6d4,stroke:#22d3ee,color:#f1f5f9
    classDef atom fill:#64748b,stroke:#94a3b8,color:#f1f5f9
    class RRC page
    class AS template
    class CAM,LOC,FM,TD,VC feature
    class FS,JCP,SS organism
    class CS,SC,SP,TC molecule
    class BI,SB,SCd,TB,VD,LS atom
```

## Socket.IO Patterns

Two coexisting approaches:

| Pattern | Where Used | Approach | Status |
|---------|-----------|----------|--------|
| **A — Direct socket** | `RoboRoverControl`, `CameraViewer` | `useRef<Socket>()`, direct `io()` call | Active, production |
| **B — Service abstraction** | `AppShell`, hooks (`useConnection`, `useTelemetry`) | DI via `ServiceFactory`, callback pub/sub | Available, extensible |

### Pattern A — Direct Socket (Primary)

```mermaid
sequenceDiagram
    autonumber
    participant User
    participant RRC as RoboRoverControl
    participant Socket as Socket.IO Client
    participant Server as web_bridge:3030

    User->>RRC: Connect (serverUrl, auth)
    RRC->>Socket: io(url, { transports, auth })
    Socket->>Server: WebSocket handshake

    Server-->>Socket: connect
    Socket-->>RRC: setConnection(isConnected: true)

    loop Telemetry Stream
        Server-->>Socket: rover_core_telemetry
        Socket-->>RRC: setRoverTelemetry(data)
        Server-->>Socket: arm_telemetry
        Socket-->>RRC: setArmTelemetry(data)
        Server-->>Socket: performance_metrics
        Socket-->>RRC: setPerformanceMetrics(map)
    end

    User->>RRC: Joystick input
    RRC->>RRC: sendThrottled (100ms min)
    RRC->>Socket: emit("rover_command", cmd)
    Socket->>Server: rover_command
    Server-->>Socket: command_ack
    Socket-->>RRC: commandsReceived++
```

### Pattern B — Service Abstraction

```mermaid
flowchart TB
    subgraph Entry["RoboControlApp (embed)"]
        INIT["useMemo: initialize all 7 services"]
    end

    subgraph Factory["ServiceFactory (singletons)"]
        SF_SOCK["setSocketService"]
        SF_CMD["setRoverCommandService"]
        SF_TRACK["setTrackingService"]
        SF_FLEET["setFleetService"]
        SF_TELE["setTelemetryService"]
        SF_MEDIA["setMediaService"]
        SF_VOICE["setVoiceService"]
    end

    subgraph Hooks["React Hooks"]
        UC["useConnection"]
        UT["useTelemetry"]
        UF["useFleet"]
    end

    INIT --> SF_SOCK & SF_CMD & SF_TRACK & SF_FLEET & SF_TELE & SF_MEDIA & SF_VOICE
    UC --> SF_SOCK
    UT --> SF_TELE
    UF --> SF_FLEET

    classDef entry fill:#1e293b,stroke:#10b981,color:#f1f5f9
    classDef factory fill:#1e293b,stroke:#f97316,color:#f1f5f9
    classDef hook fill:#1e293b,stroke:#3b82f6,color:#f1f5f9
    class INIT entry
    class SF_SOCK,SF_CMD,SF_TRACK,SF_FLEET,SF_TELE,SF_MEDIA,SF_VOICE factory
    class UC,UT,UF hook
```

### Service Interfaces

7 interfaces in `packages/ui/src/adapters/factory/interfaces/`:

| Interface | Purpose | Key Methods |
|-----------|---------|-------------|
| `ISocketService` | WebSocket lifecycle + event pub/sub | `connect`, `disconnect`, `emit`, `on`, `onStatusChange` |
| `IRoverCommandService` | Movement/arm control | `sendRoverCommand`, `sendArmCommand`, `emergencyStop`, `sendHome` |
| `ITrackingService` | Autonomous object tracking | `enableTracking`, `selectTarget`, `clearTarget` |
| `IFleetService` | Multi-rover selection | `selectRover`, `onFleetStatus` |
| `ITelemetryService` | All telemetry subscriptions | `onRoverTelemetry`, `onArmTelemetry`, `onServoTelemetry`, `onPerformanceMetrics` |
| `IMediaService` | Video/audio streams + detections | `startCamera`, `onVideoFrame`, `onDetections`, `onTrackedDetections` |
| `IVoiceService` | Text-to-speech + audio streaming | `sendTTS`, `streamAudio` |

All subscriptions return unsubscribe functions (`() => void`).

## Data Flow

### Command Path (User → Rover)

```mermaid
flowchart LR
    INPUT["User Input<br/>(joystick/slider/click)"]
    HANDLER["RoboRoverControl<br/>handleChange"]
    THROTTLE["sendThrottled<br/>(100ms min)"]
    EMIT["socket.emit<br/>(rover_command)"]
    SERVER["web_bridge"]
    ROVER["Rover<br/>Actuators"]

    INPUT --> HANDLER --> THROTTLE --> EMIT --> SERVER --> ROVER

    classDef ui fill:#1e293b,stroke:#3b82f6,color:#f1f5f9
    classDef net fill:#1e293b,stroke:#f97316,color:#f1f5f9
    classDef hw fill:#1e293b,stroke:#10b981,color:#f1f5f9
    class INPUT,HANDLER,THROTTLE ui
    class EMIT,SERVER net
    class ROVER hw
```

### Telemetry Path (Rover → UI)

```mermaid
flowchart LR
    SENSORS["Rover<br/>Sensors"]
    SERVER["web_bridge"]
    SOCKET["socket.on<br/>(telemetry events)"]
    STATE["useState<br/>(setRoverTelemetry)"]
    PROPS["Props → Child<br/>Components"]
    RENDER["UI Render"]

    SENSORS --> SERVER --> SOCKET --> STATE --> PROPS --> RENDER

    classDef hw fill:#1e293b,stroke:#10b981,color:#f1f5f9
    classDef net fill:#1e293b,stroke:#f97316,color:#f1f5f9
    classDef ui fill:#1e293b,stroke:#3b82f6,color:#f1f5f9
    class SENSORS hw
    class SERVER,SOCKET net
    class STATE,PROPS,RENDER ui
```

## Socket.IO Events

### Server → Client

```mermaid
flowchart LR
    subgraph Telemetry
        RCT["rover_core_telemetry"]
        AT["arm_telemetry"]
        ST["servo_telemetry"]
        PM["performance_metrics"]
    end

    subgraph Media
        VF["video_frame"]
        TD["tracked_detections"]
        TR["transcription"]
    end

    subgraph Control
        CA["command_ack"]
        FS["fleet_status"]
    end

    classDef tele fill:#1e293b,stroke:#10b981,color:#f1f5f9
    classDef media fill:#1e293b,stroke:#a855f7,color:#f1f5f9
    classDef ctrl fill:#1e293b,stroke:#3b82f6,color:#f1f5f9
    class RCT,AT,ST,PM tele
    class VF,TD,TR media
    class CA,FS ctrl
```

### Client → Server

| Event | Payload | Source |
|-------|---------|--------|
| `rover_command` | `WebRoverCommand` (velocity or wheel positions) | Joystick input |
| `arm_command` | `WebArmCommand` (joint positions / home / stop) | JointControlPanel |
| `tracking_command` | enable / disable / select_target | CameraViewer click |
| `fleet_select` | `FleetSelectCommand` (entity_id) | FleetSelector |
| `audio_control` | `{ command: "start" \| "stop" }` | VoiceControls |
| `tts_command` | `{ text: string }` | VoiceControls |
| `audio_stream` | Raw audio chunks | Microphone capture |
| `video_control` | start / stop / quality / FPS | CameraViewer |

## Type System

All types in `packages/shared/src/types/`, organized by domain:

```mermaid
classDiagram
    class WebRoverCommand {
        command_type: "velocity" | "wheel_position"
        v_x: number
        v_y: number
        omega_z: number
    }

    class WebArmCommand {
        command_type: "joint_position" | "home" | "stop"
        joint_positions: JointPositions
        max_velocity: number
    }

    class RoverTelemetry {
        position: [x, y, z]
        yaw: number
        pitch: number
        roll: number
        linear_velocity: [x, y, z]
    }

    class ArmTelemetry {
        end_effector: EndEffectorPose
        joint_positions: JointPositions
    }

    class DetectionResult {
        bbox: BoundingBox
        class_id: number
        class_name: string
        confidence: number
        tracking_id: string
    }

    class FleetStatus {
        selected_entity: string
        fleet_roster: string[]
    }

    class SystemMetrics {
        total_cpu_percent: number
        total_memory_mb: number
        battery_level: number
        dataflow_fps: number
    }

    class SpeechTranscription {
        text: string
        confidence: number
        duration: number
    }

    WebRoverCommand ..> RoverTelemetry : controls
    WebArmCommand ..> ArmTelemetry : controls
    FleetStatus --> SystemMetrics : per rover
    DetectionResult --> WebRoverCommand : click-to-track
```

### Constants

| Constant | Purpose |
|----------|---------|
| `JOINT_LIMITS` | Min/max radians for 6-DOF arm joints |
| `DEFAULT_CLASS_COLORS` | Detection class → color map |
| `createHomePosition()` | Zero-position arm command |
| `validateJointPositions()` | Clamp values to limits |
| `getClassColor()` | Lookup color for detection class |
| `createFleetSelectCommand()` | Build fleet select payload |

## Key Features

### CameraViewer — Video + Detection Overlays

```mermaid
stateDiagram-v2
    [*] --> Idle
    Idle --> Streaming: video_control start
    Streaming --> Rendering: video_frame received

    state Rendering {
        [*] --> DecodeJPEG
        DecodeJPEG --> DrawCanvas: Blob → ObjectURL → img.onload
        DrawCanvas --> DrawOverlays: if detections enabled
        DrawOverlays --> [*]
    }

    Rendering --> Streaming: next frame
    Streaming --> Idle: video_control stop

    state DrawOverlays {
        [*] --> BBoxes: strokeRect per detection
        BBoxes --> Labels: class_name + confidence
        Labels --> TrackingHighlight: if tracking_id matches
    }
```

- JPEG frames rendered on `<canvas>` via `Blob` + `createObjectURL`
- Detection overlays with Canvas 2D API (bounding boxes, labels, confidence %)
- Click-to-track: pixel coords → normalized bbox → find matching detection → emit `tracking_command`
- Audio: S16LE PCM → Web Audio API `AudioBuffer` queue, 8kHz low-pass filter, max queue 20

### LocationMap — 2D Path Visualization

Canvas-based grid with 1m spacing, robot position circle + heading arrow, zoom/pan via mouse wheel + drag. Integrates wheel kinematics at 50ms intervals for position estimation.

### JointControlPanel — 6-DOF Arm Control

6 `SliderControl` components, one per joint, bounded by `JOINT_LIMITS`. Each change emits a throttled `arm_command` with updated `joint_positions`.

### FleetSelector — Multi-Rover Management

Displays `fleet_roster` with per-rover metrics (battery, CPU, memory, FPS). Online detection via metrics timestamp freshness (< 10s). Click to emit `fleet_select`.

## State Management

No external state library. All state lives as local `useState` in `RoboRoverControl` and flows down as props.

```mermaid
flowchart TB
    RRC["RoboRoverControl<br/>(all state)"]

    RRC -->|connection| AS["AppShell"]
    RRC -->|roverTelemetry, armTelemetry| CAM["CameraViewer"]
    RRC -->|roverTelemetry| LOC["LocationMap"]
    RRC -->|performanceMetrics| FM["FloatingMetrics"]
    RRC -->|fleetStatus, performanceMetrics| FS["FleetSelector"]
    RRC -->|jointPositions| JCP["JointControlPanel"]
    RRC -->|transcription| TD["TranscriptionDisplay"]

    classDef root fill:#3b82f6,stroke:#60a5fa,color:#f1f5f9
    classDef child fill:#1e293b,stroke:#94a3b8,color:#f1f5f9
    class RRC root
    class AS,CAM,LOC,FM,FS,JCP,TD child
```

Key state:
- `connection: ConnectionState` — isConnected, clientId, commandsSent/Received
- `roverTelemetry: RoverTelemetry | null` — position, velocity, orientation
- `armTelemetry: ArmTelemetry | null` — end-effector, joint angles
- `servoTelemetry: TrackingTelemetry | null` — tracking state, control output
- `performanceMetrics: Map<string, SystemMetrics>` — per-entity metrics
- `fleetStatus: FleetStatus | null` — selected rover, roster
- `transcription: SpeechTranscription | null` — latest transcription
- `jointPositions: ExtendedJointPositions` — current slider values
- `roverVelocity: { v_x, v_y, omega_z }` — joystick output
- `logs: LogEntry[]` — event log (max 50 entries)

## Styling

**Tailwind CSS v4** with `@tailwindcss/vite` plugin. Terminal/IDE dark theme with syntax-highlighting colors.

| Token | Value | Purpose |
|-------|-------|---------|
| `--color-background` | `#0F172A` | Deep slate page bg |
| `--color-foreground` | `#F1F5F9` | Light text |
| `--color-card` | `#1E293B` | Dark panel bg |
| `--color-primary` | `#3B82F6` | VS Code blue |
| `--color-syntax-*` | 8 colors | IDE-style code highlighting |

Custom CSS classes: `.glass-card`, `.glass-card-blur`, `.gradient-bg`, `.scanline-effect`, `.btn-primary/secondary/success/warning/destructive/info`, `.glass-slider`, `.glass-input`, `.status-glow-*`.

Fonts: IBM Plex Sans (sans), JetBrains Mono / Fira Code (mono).

## Deployment

```mermaid
flowchart LR
    subgraph Web["Web Deployment"]
        VITE["Vite Build"] --> DIST["dist/"]
        ENV_W["VITE_SOCKET_IO_URL<br/>VITE_AUTH_USERNAME<br/>VITE_AUTH_PASSWORD"]
    end

    subgraph Desktop["Tauri Desktop"]
        TAURI["Tauri v2 Build"] --> BIN["Native Binary"]
        RUST["Minimal Rust Backend<br/>(plugins only)"]
        ENV_D["Same VITE_* env vars"]
    end

    subgraph Embed["qm-hub-app Embed"]
        SHADOW["ShadowWrapper"]
        RCA["RoboControlApp"]
        SHADOW --> RCA
    end

    classDef web fill:#1e293b,stroke:#3b82f6,color:#f1f5f9
    classDef desktop fill:#1e293b,stroke:#10b981,color:#f1f5f9
    classDef embed fill:#1e293b,stroke:#a855f7,color:#f1f5f9
    class VITE,DIST,ENV_W web
    class TAURI,BIN,RUST,ENV_D desktop
    class SHADOW,RCA embed
```

### Environment Variables

| Variable | Default | Purpose |
|----------|---------|---------|
| `VITE_SOCKET_IO_URL` | `http://localhost:3030` | Socket.IO server URL |
| `VITE_AUTH_USERNAME` | `""` | Socket.IO auth username |
| `VITE_AUTH_PASSWORD` | `""` | Socket.IO auth password |

### Tauri Backend

Minimal Rust — only `tauri_plugin_opener` initialized + placeholder `greet` command. All rover control is JavaScript/Socket.IO. No native data layer.

## qm-hub-app Embed

Robo Control is registered in `qm-hub-app` at `/robo-control/*` behind `AuthGuard + AppAccessGuard`. Entry via `RoboControlEmbed` → `ShadowWrapper` → `RoboControlApp`.

**Auth model**: `authTokens` (qm-hub JWT) accepted for interface consistency but unused. Socket.IO credentials (`auth.username`/`auth.password`) are independent — configure via `VITE_AUTH_USERNAME`/`VITE_AUTH_PASSWORD` in robo-control-app's own env.

**Fallback**: When embedded without `adapters`, `RoboControlApp` renders `RoboRoverControl` directly (Pattern A). Pattern B (service abstraction) available for future use by passing `adapters` prop.
