# Robo-Fleet Control

React 19 + TypeScript control UI for the Robo-Fleet distributed rover system. Real-time telemetry, 6-DOF arm control, camera feeds with object detection overlays, click-to-track, fleet management, location mapping, and voice control (TTS/STT).

## Quick Start

### Prerequisites
- Node.js 18+
- pnpm 9.1.0
- Rust 1.70+ (for Tauri desktop builds)

### Install & Run

```bash
# Install dependencies
pnpm install

# Development
pnpm dev              # All targets (web + Tauri)
pnpm dev:web          # Web only (http://localhost:25010)
pnpm dev:native       # Tauri desktop (port 1420)

# Build
pnpm build            # Production build
pnpm check-types      # Type check
pnpm lint             # Linting
```

## Environment Variables

Create a `.env` file in the root directory (or per-app in `apps/web/.env`, `apps/native/.env`):

```env
# Socket.IO backend connection (orchestra/web_bridge)
# Local:      http://localhost:3030
# Production: https://robo-fleet.qm-hub-v001.cloud  (use https:// not wss://)
VITE_SOCKET_IO_URL=http://localhost:3030

# Basic auth credentials (must match web_bridge config)
VITE_AUTH_USERNAME=user
VITE_AUTH_PASSWORD=pass
```

> **Production deployment**: browsers on HTTPS pages block `ws://` connections (mixed content). Use a `wss://` endpoint via Cloudflare Tunnel. See **[Cloudflare Tunnel Deployment](./docs/deployment-cloudflare-tunnel.md)** for setup steps.

## Monorepo Structure

```
apps/
  web/       (@robo-fleet/web)     — Vite browser app (port 25010)
  native/    (@robo-fleet/native)  — Tauri v2 desktop (port 1420)
packages/
  ui/        (@robo-fleet/ui)      — All React components, hooks, services
  shared/    (@robo-fleet/shared)  — Pure TypeScript types & constants
  tsconfig/  (@robo-fleet/tsconfig)
  eslint-config/                   — Shared ESLint rules
```

**Legacy directories:** `src/` (legacy pre-monorepo app) and `old.app/` (archived). New work goes in `packages/` and `apps/`.

## Key Features

- **Real-time Rover Control**: Omnidirectional wheel commands via Socket.IO, 100ms command throttle
- **6-DOF Arm Control**: Joint slider panel with position validation and home preset
- **JPEG Video Streaming**: Live camera feed with Canvas-based detection overlays
- **Click-to-Track**: Map pixel coordinates to normalized detection bboxes, emit tracking commands
- **Fleet Management**: Multi-rover selector with health metrics, fleet status monitoring
- **Location Map**: Canvas 2D path visualization with zoom/pan, wheel kinematics integration
- **Voice Control**: TTS message playback, microphone STT integration, transcription display
- **Performance Monitoring**: Floating metrics panel (latency, FPS, memory, CPU)
- **Terminal/IDE Theme**: Dark mode with syntax-highlighting colors, custom glass-morphic UI

## Tech Stack

- **React 19**, **TypeScript 5.8**, **Vite**
- **Tailwind CSS v4** with `@tailwindcss/vite` plugin
- **Socket.IO Client 4.8** (real-time bidirectional comms)
- **Tauri v2** (desktop app framework)
- **Lucide React** (icon library)
- **react-joystick-component** (joystick input)

## Architecture & Socket.IO Patterns

See `/docs/architecture.md` for complete system design, component hierarchy (Atomic Design), Socket.IO event maps, type system, and state management patterns.

Two coexisting Socket.IO patterns:
- **Pattern A** (production): Direct `useRef<Socket>` + `io()` in components
- **Pattern B** (extensible): Service abstraction via `ServiceFactory` DI

## Styling

**Tailwind CSS v4** with custom CSS classes in `src/styles/globals.css`:
- `.glass-card`, `.glass-card-blur` — glassmorphic panels
- `.btn-primary/secondary/success/warning/destructive/info` — button variants
- `.status-glow-*` — status indicator glows
- Fonts: **IBM Plex Sans** (sans), **JetBrains Mono** / **Fira Code** (mono)

## Socket.IO Events

**Received:** `video_frame`, `tracked_detections`, `servo_telemetry`, `rover_core_telemetry`, `arm_telemetry`, `transcription`, `performance_metrics`, `fleet_status`, `command_ack`

**Emitted:** `rover_command`, `arm_command`, `tracking_command`, `fleet_select`, `audio_control`, `tts_command`, `audio_stream`

See `/docs/architecture.md` for full event type definitions.

## Project Layout

| File/Dir | Purpose |
|----------|---------|
| `src/components/` | React components (atomic design structure) |
| `src/styles/globals.css` | Tailwind theme + custom utilities |
| `src/types/` | TypeScript type definitions |
| `src/constants/` | Joint limits, color maps, helpers |
| `src/hooks/` | Custom React hooks |
| `apps/web/`, `apps/native/` | Thin deployment shells |
| `docs/` | Comprehensive documentation |

## Notes

- **No external state management** — all state in `RoboRoverControl` component, props-down architecture
- **Service Factory pattern** — optional DI layer for testability (not yet active)
- **Tauri IPC** — minimal Rust backend; all logic in JavaScript
- **Responsive design** — optimized for control room displays (16:9)

## Documentation

- **[Architecture](./docs/architecture.md)** — System design, component hierarchy, Socket.IO patterns, data flow
- **[Project Overview & PDR](./docs/project-overview-pdr.md)** — Product vision, requirements, capabilities
- **[Code Standards](./docs/code-standards.md)** — Conventions, naming, styling, patterns
- **[Codebase Summary](./docs/codebase-summary.md)** — File inventory, LOC breakdown, key files
- **[Cloudflare Tunnel Deployment](./docs/deployment-cloudflare-tunnel.md)** — Expose `web_bridge` via `wss://` for HTTPS pages (mixed content fix)

## Further Reading

See `CLAUDE.md` for component-specific guidance and implementation details.
