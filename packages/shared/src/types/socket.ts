// Socket.IO event types — web_bridge/src/main.rs is source of truth

export type AuthErrorReason =
  | "invalid_credentials"
  | "token_expired"
  | "rate_limited"
  | "idle_timeout";

export interface AuthErrorEvent {
  reason: AuthErrorReason;
}

import type { VideoFrame } from "./telemetry";
import type { DetectionFrame, TrackingTelemetry } from "./tracking";
import type { WebArmCommand, WebRoverCommand, WebTrackingCommand } from "./commands";
import type { SpeechTranscription } from "./voice";
import type { SystemMetrics } from "./performance";
import type { FleetStatus, FleetSelectCommand, ActiveRoversStatus } from "./fleet";

export interface ServerToClientEvents {
  video_frame: (frame: VideoFrame) => void;
  audio_frame: (frame: { timestamp: number; frame_id: number; sample_rate: number; channels: number; format: string; data: number[] }) => void;
  detections: (frame: DetectionFrame) => void;
  tracked_detections: (frame: DetectionFrame) => void;
  tracking_telemetry: (telemetry: TrackingTelemetry) => void;
  servo_telemetry: (telemetry: TrackingTelemetry) => void;
  transcription: (data: SpeechTranscription) => void;
  performance_metrics: (metrics: SystemMetrics) => void;
  fleet_status: (status: FleetStatus) => void;
  active_rovers_status: (status: ActiveRoversStatus) => void;
}

export interface ClientToServerEvents {
  arm_command: (command: WebArmCommand) => void;
  rover_command: (command: WebRoverCommand) => void;
  tracking_command: (command: WebTrackingCommand) => void;
  camera_control: (control: { command: string }) => void;
  audio_control: (control: { command: string }) => void;
  tts_command: (command: { text: string }) => void;
  audio_stream: (data: { audio_data: number[] }) => void;
  performance_control: (control: { enabled: boolean }) => void;
  fleet_select: (command: FleetSelectCommand) => void;
}
