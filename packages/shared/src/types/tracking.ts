// Object detection and tracking types

export interface BoundingBox {
  x1: number;
  y1: number;
  x2: number;
  y2: number;
}

export interface DetectionResult {
  bbox: BoundingBox;
  class_id: number;
  class_name: string;
  confidence: number;
  tracking_id?: number;
}

export interface DetectionFrame {
  frame_id: number;
  timestamp: number;
  width: number;
  height: number;
  detections: DetectionResult[];
}

export interface DetectionDisplaySettings {
  enabled: boolean;
  showLabels: boolean;
  showConfidence: boolean;
  showBoundingBoxes: boolean;
  minConfidence: number;
  classColors: Record<string, string>;
}

export type TrackingState = "Disabled" | "DetectionOnly" | "Enabled" | "Tracking" | "TargetLost";
export type ControlMode = "Manual" | "Autonomous";

export interface TrackingTarget {
  tracking_id: number;
  class_name: string;
  bbox: BoundingBox;
  last_seen: number;
  confidence: number;
  lost_frames: number;
}

export interface TrackingTelemetry {
  state: TrackingState;
  target: TrackingTarget | null;
  distance_estimate: number | null;
  control_output: ControlOutput | null;
  control_mode: ControlMode;
  timestamp: number;
}

export interface ControlOutput {
  omega_z: number;
  v_x: number;
  error_x: number;
  error_size: number;
}
