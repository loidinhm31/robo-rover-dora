// Central export for all types

// Commands
export type {
  JointPositions,
  WebArmCommand,
  WebRoverCommand,
  WebTrackingCommand,
} from "./commands";

// Telemetry
export type {
  ArmTelemetry,
  RoverTelemetry,
  VideoFrame,
  OverlayData,
  VideoStats,
  CameraStatus,
} from "./telemetry";

// Tracking
export type {
  BoundingBox,
  DetectionResult,
  DetectionFrame,
  DetectionDisplaySettings,
  TrackingState,
  ControlMode,
  TrackingTarget,
  TrackingTelemetry,
  ControlOutput,
} from "./tracking";

// Voice
export type { SpeechTranscription, SpeechStats } from "./voice";

// Performance
export type { NodeMetrics, SystemMetrics } from "./performance";

// UI
export type { LogEntry, ConnectionState } from "./ui";

// Socket
export type { ServerToClientEvents, ClientToServerEvents } from "./socket";

// Fleet
export type {
  FleetStatus,
  FleetSelectCommand,
  RoverStatus,
  FleetRosterUpdate,
  ActiveRoversStatus,
} from "./fleet";
