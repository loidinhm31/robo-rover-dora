import { Table, tableFromArrays, tableFromIPC, tableToIPC } from "apache-arrow";
import {
  ArmCommand,
  ArmTelemetry,
  ArrowMessage,
  RoverCommand,
  RoverTelemetry,
} from "@repo/ui/types/robo-rover.js";

function generateId(): string {
  return (
    Math.random().toString(36).substring(2, 15) +
    Math.random().toString(36).substring(2, 15)
  );
}

function getCurrentTimestamp(): number {
  return Date.now();
}

// Convert Arrow table to base64 string
export function tableToBase64(table: Table): string {
  const buffer = tableToIPC(table, "stream");
  return btoa(String.fromCharCode(...new Uint8Array(buffer)));
}

// Convert base64 string to Arrow table
export function tableFromBase64(base64Data: string): Table {
  const binaryString = atob(base64Data);
  const buffer = new Uint8Array(binaryString.length);
  for (let i = 0; i < binaryString.length; i++) {
    buffer[i] = binaryString.charCodeAt(i);
  }
  return tableFromIPC(buffer);
}

// ARM TELEMETRY CONVERSION
export function armTelemetryFromArrow(base64Data: string): ArmTelemetry {
  const table = tableFromBase64(base64Data);
  const row = table.get(0)!;

  const jointAnglesJson = row.joint_angles;
  const jointVelocitiesJson = row.joint_velocities;

  return {
    end_effector_pose: [
      row.end_effector_x,
      row.end_effector_y,
      row.end_effector_z,
      row.end_effector_roll,
      row.end_effector_pitch,
      row.end_effector_yaw,
    ],
    joint_angles:
      jointAnglesJson && jointAnglesJson !== "null"
        ? JSON.parse(jointAnglesJson)
        : undefined,
    joint_velocities:
      jointVelocitiesJson && jointVelocitiesJson !== "null"
        ? JSON.parse(jointVelocitiesJson)
        : undefined,
    is_moving: row.is_moving,
    source: row.source,
    timestamp: Number(row.timestamp),
  };
}

// ROVER TELEMETRY CONVERSION
export function roverTelemetryFromArrow(base64Data: string): RoverTelemetry {
  const table = tableFromBase64(base64Data);
  const row = table.get(0)!;

  const navAnglesJson = row.nav_angles;
  const navDistsJson = row.nav_dists;

  return {
    position: [row.position_x, row.position_y],
    yaw: row.yaw,
    pitch: row.pitch,
    roll: row.roll,
    velocity: row.velocity,
    timestamp: Number(row.timestamp),
    near_sample: row.near_sample,
    picking_up: row.picking_up,
    nav_angles:
      navAnglesJson && navAnglesJson !== "null"
        ? JSON.parse(navAnglesJson)
        : undefined,
    nav_dists:
      navDistsJson && navDistsJson !== "null"
        ? JSON.parse(navDistsJson)
        : undefined,
  };
}

// ARM COMMAND CONVERSION
export function armCommandToArrow(command: ArmCommand): ArrowMessage {
  const commandId = generateId();
  const timestamp = getCurrentTimestamp();

  // Create data arrays with proper typing
  const data = {
    command_type: [command.type],
    x: new Float64Array([command.x ?? 0.0]),
    y: new Float64Array([command.y ?? 0.0]),
    z: new Float64Array([command.z ?? 0.0]),
    roll: new Float64Array([command.roll ?? 0.0]),
    pitch: new Float64Array([command.pitch ?? 0.0]),
    yaw: new Float64Array([command.yaw ?? 0.0]),
    max_velocity: new Float64Array([command.max_velocity ?? 1.0]),
    joint_angles: [
      command.joint_angles ? JSON.stringify(command.joint_angles) : null,
    ],
    delta_joints: [
      command.delta_joints ? JSON.stringify(command.delta_joints) : null,
    ],
    command_id: [commandId],
    timestamp: new BigUint64Array([BigInt(timestamp)]),
  };

  const table = tableFromArrays(data);
  const arrowData = tableToBase64(table);

  return {
    message_type: "command",
    schema_name: "arm_command",
    arrow_data: arrowData,
    timestamp,
  };
}

// ROVER COMMAND CONVERSION - Simplified working approach
export function roverCommandToArrow(command: RoverCommand): ArrowMessage {
  const commandId = generateId();
  const timestamp = getCurrentTimestamp();

  // Simple approach: Create many different strings to prevent dictionary encoding
  const uniqueId1 = `cmd_${commandId}_${timestamp}_${Math.random().toString(36).substring(2)}`;
  const uniqueId2 = `backup_${Date.now()}_${Math.random().toString(36).substring(2)}`;
  const uniqueId3 = `fallback_${performance.now()}_${Math.random().toString(36).substring(2)}`;

  // Create multi-row data to force Utf8 type instead of Dictionary
  const multiRowData = {
    throttle: new Float64Array([command.throttle, 0.0, 0.0]),
    brake: new Float64Array([command.brake, 0.0, 0.0]),
    steering_angle: new Float64Array([command.steering_angle, 0.0, 0.0]),
    timestamp: new BigUint64Array([BigInt(timestamp), BigInt(0), BigInt(0)]),
    command_id: [uniqueId1, uniqueId2, uniqueId3], // Three different strings
  };

  // Create full table then slice to get just the first row
  const fullTable = tableFromArrays(multiRowData);
  const singleRowTable = fullTable.slice(0, 1);

  // Debug: Check the resulting schema
  console.log("Final rover command schema:");
  singleRowTable.schema.fields.forEach((field) => {
    console.log(
      `  ${field.name}: ${field.type.toString()}, nullable: ${field.nullable}`,
    );
  });

  const arrowData = tableToBase64(singleRowTable);

  return {
    message_type: "command",
    schema_name: "rover_command",
    arrow_data: arrowData,
    timestamp,
  };
}
