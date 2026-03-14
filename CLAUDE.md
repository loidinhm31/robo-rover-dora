# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

React + TypeScript control UI for the Robo-Fleet distributed rover system. This is a **Turborepo pnpm monorepo** with two deployment targets (web browser and Tauri desktop) sharing a common UI library. Connects to the `orchestra/web_bridge` Rust backend via Socket.IO on port 3030.

## Build Commands

```bash
pnpm install              # Install all dependencies
pnpm dev                  # Dev servers for all apps
pnpm dev:web              # Web app only (port 25010)
pnpm dev:native           # Tauri desktop app only (port 1420)
pnpm build                # Production build (all apps + packages)
pnpm check-types          # TypeScript type checking across workspace
pnpm lint                 # ESLint across workspace

# Filter to a specific app/package
pnpm turbo build --filter=@robo-fleet/web
pnpm turbo build --filter=@robo-fleet/ui
```

## Monorepo Structure

```
apps/
  web/       (@robo-fleet/web)    — Vite browser app, port 25010
  native/    (@robo-fleet/native) — Tauri v2 desktop app, port 1420
packages/
  ui/        (@robo-fleet/ui)     — All React components, hooks, services, adapters
  shared/    (@robo-fleet/shared) — Pure TypeScript types and constants (zero deps)
  tsconfig/  (@robo-fleet/tsconfig) — Shared TS configs
  eslint-config/                  — Shared ESLint rules
```

Both apps are thin shells — they import `RoboRoverControl` from `@robo-fleet/ui` and pass env-based config (`VITE_SOCKET_IO_URL`, `VITE_AUTH_USERNAME`, `VITE_AUTH_PASSWORD`).

**Legacy directories:** `src/` (pre-monorepo root app, still functional) and `old.app/` (archived previous version). New work should go in `packages/` and `apps/`.

## Architecture

### Component Hierarchy (Atomic Design in `packages/ui/src/components/`)

- **Atoms**: `BatteryIndicator`, `IconBadge`, `LoadingSpinner`, `StatCard`, `StatusBadge`, `ToggleButton`, `ValueDisplay`
- **Molecules**: `CollapsibleSection`, `InputWithAction`, `SliderControl`, `StatPanel`, `ToggleControl`
- **Organisms**: `DraggablePanel`, `FleetSelector`, `JointControlPanel`
- **Features**: `CameraViewer`, `FloatingMetrics`, `LocationMap`, `TranscriptionDisplay`, `VoiceControls`
- **Templates**: `AppShell`
- **Pages**: `RoboRoverControl` (main entry — self-contained, manages its own Socket.IO connection)

### Socket.IO Patterns (Two Coexisting Approaches)

**Pattern A — Direct socket (active, used by `RoboRoverControl` and `CameraViewer`):**
Components hold a `socketRef = useRef<Socket>()` and call `io()` directly. This is the pattern used in production.

**Pattern B — Service abstraction (used by `AppShell`, hooks like `useConnection`/`useTelemetry`):**
DI via `ServiceFactory.ts` — module-level singletons set with `setXxxService()` / `getXxxService()`. Services: `ISocketService`, `IRoverCommandService`, `ITrackingService`, `IFleetService`, `ITelemetryService`, `IMediaService`, `IVoiceService`. Initialized by the `RoboControlApp` embed component.

### State Management

No external state library. All state lives as local `useState` in `RoboRoverControl` and flows down as props. The service layer uses callback-based pub/sub (each `on*()` returns an unsubscribe function).

### Styling

**Tailwind CSS v4** with the `@tailwindcss/vite` plugin (CSS-first config, no JS config needed at app level). Design tokens and custom classes defined in `packages/ui/src/styles/globals.css`:
- Terminal/IDE dark theme with syntax-highlighting-inspired color palette
- Fonts: IBM Plex Sans (sans), JetBrains Mono / Fira Code (mono)
- Custom classes: `.glass-card`, `.glass-card-blur`, `.gradient-bg`, `.scanline-effect`, `.btn-primary/secondary/success/warning/destructive/info`, `.glass-slider`, `.glass-input`, `.status-glow-*`

### Key Socket.IO Events

**Received from backend:** `video_frame`, `tracked_detections`, `servo_telemetry`, `rover_core_telemetry`, `arm_telemetry`, `transcription`, `performance_metrics`, `fleet_status`, `command_ack`

**Emitted to backend:** `rover_command`, `arm_command`, `tracking_command`, `fleet_select`, `audio_control`, `tts_command`, `audio_stream`

### Shared Types (`packages/shared/src/types/`)

All data types are organized by domain: `commands.ts`, `telemetry.ts`, `tracking.ts`, `voice.ts`, `performance.ts`, `fleet.ts`, `socket.ts` (typed Socket.IO event maps), `ui.ts`. Constants in `src/constants/` include `JOINT_LIMITS`, `DEFAULT_CLASS_COLORS`, and helper functions.

### Turbo Pipeline

`build` has `dependsOn: ["^build"]` — packages build before apps. `check-types` and `lint` also depend on package builds. `dev` is persistent and non-cached.

## qm-hub-app Embed Interface

The app is embedded in `qm-hub-app` via `packages/ui/src/embed/RoboControlApp.tsx`, exported from `@robo-fleet/ui/embed`.

```typescript
<RoboControlApp
  socketUrl="ws://..."         // falls back to VITE_SOCKET_IO_URL
  auth={{ username, password }}
  embedded={true}
  useRouter={false}            // share qm-hub-app's BrowserRouter
  basePath="/robo-control"
  authTokens={{ accessToken, refreshToken, userId }} // optional — not used for socket auth
  onLogoutRequest={() => {}}
/>
```

Unlike the other embed apps, socket authentication uses `VITE_AUTH_USERNAME`/`VITE_AUTH_PASSWORD` credentials, not the `authTokens` JWT. The `authTokens` prop is accepted for interface compatibility but the socket connection uses its own auth flow.

**No sync schema**: This app has no `*-app-schema.json`, no local database, and no IndexedDB tables. All data flows through the live Socket.IO connection.

## Key Implementation Details

- **Command throttle**: `RoboRoverControl` enforces 100ms minimum between emitted commands via `useRef` timestamp
- **CameraViewer**: Renders JPEG frames on `<canvas>` via `Blob` + `createObjectURL`, draws detection overlays with Canvas 2D API, supports click-to-track (maps pixel → normalized bbox → tracking command)
- **Audio playback**: S16LE PCM → Web Audio API `AudioBuffer` queue with scheduling, 8kHz low-pass filter, max queue size 20
- **Wheel kinematics**: 50ms `setInterval` integrating omnidirectional wheel angular positions for location map visualization
- **Vite aliases** (in `apps/web/vite.config.ts`): `@robo-fleet/ui` → `../../packages/ui/src`, `@robo-fleet/ui/styles` → `../../packages/ui/src/styles/globals.css`
- **Legacy directories**: `src/` (pre-monorepo root app) and `old.app/` (archived version) are non-functional. All new work goes in `packages/` and `apps/`.
