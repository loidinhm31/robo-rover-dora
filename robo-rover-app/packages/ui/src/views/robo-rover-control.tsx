import React, { useCallback, useEffect, useRef, useState } from "react";
import { io, Socket } from "socket.io-client";
import { Joystick } from "react-joystick-component";
import {
  ArmTelemetry,
  ConnectionState,
  createHomePosition,
  JOINT_LIMITS,
  JointPositions,
  LogEntry,
  RoverTelemetry,
  validateJointPositions,
  WebArmCommand,
  WebRoverCommand,
} from "@repo/ui/types/robo-rover";
import { IJoystickUpdateEvent } from "react-joystick-component/build/lib/Joystick.js";

const SOCKET_URL = "http://localhost:8080";
const THROTTLE_DELAY = 100; // ms between updates

const RoboRoverController: React.FC = () => {
  // Connection state
  const [connection, setConnection] = useState<ConnectionState>({
    isConnected: false,
    clientId: null,
    commandsSent: 0,
    commandsReceived: 0,
  });

  // Control mode
  const [controlMode, setControlMode] = useState<"arm" | "rover">("rover");

  // Telemetry state
  const [armTelemetry, setArmTelemetry] = useState<ArmTelemetry | null>(null);
  const [roverTelemetry, setRoverTelemetry] = useState<RoverTelemetry | null>(
    null,
  );
  const [logs, setLogs] = useState<LogEntry[]>([]);

  // LeKiwi joint position controls
  const [jointPositions, setJointPositions] =
    useState<JointPositions>(createHomePosition());

  // Rover velocity controls
  const [roverVelocity, setRoverVelocity] = useState({
    v_x: 0.0,
    v_y: 0.0,
    omega_z: 0.0,
  });

  const socketRef = useRef<Socket | null>(null);
  const lastCommandTime = useRef<number>(0);
  const MAX_LOGS = 50;

  // Add log entry
  const addLog = useCallback(
    (message: string, type: LogEntry["type"] = "info") => {
      const entry: LogEntry = {
        timestamp: new Date().toISOString(),
        message,
        type,
      };
      setLogs((prev) => [entry, ...prev].slice(0, MAX_LOGS));
    },
    [],
  );

  // Throttled command sender
  const sendThrottled = useCallback((callback: () => void) => {
    const now = Date.now();
    if (now - lastCommandTime.current >= THROTTLE_DELAY) {
      callback();
      lastCommandTime.current = now;
    }
  }, []);

  // Connect to Socket.IO server
  const connect = useCallback(() => {
    if (socketRef.current?.connected) {
      addLog("Already connected", "warning");
      return;
    }

    const socket = io(SOCKET_URL, {
      transports: ["websocket", "polling"],
      reconnection: true,
      reconnectionDelay: 1000,
      reconnectionAttempts: 5,
    });

    socket.on("connect", () => {
      addLog(`Connected (ID: ${socket.id})`, "success");
      setConnection((prev) => ({
        ...prev,
        isConnected: true,
        clientId: socket.id as string,
      }));
    });

    socket.on("disconnect", (reason) => {
      addLog(`Disconnected: ${reason}`, "warning");
      setConnection((prev) => ({
        ...prev,
        isConnected: false,
        clientId: null,
      }));
    });

    socket.on("command_ack", (data) => {
      setConnection((prev) => ({
        ...prev,
        commandsReceived: prev.commandsReceived + 1,
      }));
    });

    socket.on("rover_command_ack", (data) => {
      setConnection((prev) => ({
        ...prev,
        commandsReceived: prev.commandsReceived + 1,
      }));
    });

    socket.on("error", (data) => {
      addLog(`Error: ${data.message}`, "error");
    });

    socket.on("arm_telemetry", (data: ArmTelemetry) => {
      setArmTelemetry(data);
    });

    socket.on("rover_telemetry", (data: RoverTelemetry) => {
      setRoverTelemetry(data);
    });

    socketRef.current = socket;
  }, [addLog]);

  const disconnect = useCallback(() => {
    if (socketRef.current) {
      socketRef.current.disconnect();
      socketRef.current = null;
    }
  }, []);

  // Send arm command
  const sendArmCommand = useCallback(
    (command: WebArmCommand) => {
      if (!socketRef.current?.connected) return;

      try {
        if (command.joint_positions) {
          const error = validateJointPositions(command.joint_positions);
          if (error) {
            addLog(`Validation error: ${error}`, "error");
            return;
          }
        }

        socketRef.current.emit("joint_command", command);
        setConnection((prev) => ({
          ...prev,
          commandsSent: prev.commandsSent + 1,
        }));
      } catch (error) {
        addLog(`Failed to send ARM command: ${error}`, "error");
      }
    },
    [addLog],
  );

  // Send rover command
  const sendRoverCommand = useCallback(
    (command: WebRoverCommand) => {
      if (!socketRef.current?.connected) return;

      try {
        socketRef.current.emit("rover_command", command);
        setConnection((prev) => ({
          ...prev,
          commandsSent: prev.commandsSent + 1,
        }));
      } catch (error) {
        addLog(`Failed to send ROVER command: ${error}`, "error");
      }
    },
    [addLog],
  );

  // Real-time ARM joint control
  useEffect(() => {
    if (!connection.isConnected || controlMode !== "arm") return;

    const sendJointUpdate = () => {
      const command: WebArmCommand = {
        command_type: "joint_position",
        joint_positions: jointPositions,
      };
      sendArmCommand(command);
    };

    sendThrottled(sendJointUpdate);
  }, [
    jointPositions,
    connection.isConnected,
    controlMode,
    sendArmCommand,
    sendThrottled,
  ]);

  // Real-time ROVER velocity control
  useEffect(() => {
    if (!connection.isConnected || controlMode !== "rover") return;

    const sendVelocityUpdate = () => {
      const command: WebRoverCommand = {
        command_type: "velocity",
        v_x: roverVelocity.v_x,
        v_y: roverVelocity.v_y,
        omega_z: roverVelocity.omega_z,
      };
      sendRoverCommand(command);
    };

    sendThrottled(sendVelocityUpdate);
  }, [
    roverVelocity,
    connection.isConnected,
    controlMode,
    sendRoverCommand,
    sendThrottled,
  ]);

  // Joystick move handler
  const handleJoystickMove = useCallback((event: IJoystickUpdateEvent) => {
    if (!event.x || !event.y) return;

    // Normalize joystick values (-100 to 100) to velocity range (-1 to 1)
    const v_y = event.x / 100; // Left/Right (x maps to v_y)
    const v_x = -event.y / 100; // Forward/Back (y maps to v_x, inverted)

    setRoverVelocity((prev) => ({
      ...prev,
      v_x: v_x,
      v_y: v_y,
    }));
  }, []);

  // Joystick stop handler
  const handleJoystickStop = useCallback(() => {
    setRoverVelocity((prev) => ({
      ...prev,
      v_x: 0,
      v_y: 0,
    }));
  }, []);

  // Home position
  const sendHome = useCallback(() => {
    const command: WebArmCommand = { command_type: "home" };
    sendArmCommand(command);
    setJointPositions(createHomePosition());
  }, [sendArmCommand]);

  // Emergency stop
  const emergencyStop = useCallback(() => {
    if (controlMode === "arm") {
      sendArmCommand({ command_type: "stop" });
    } else {
      sendRoverCommand({ command_type: "stop" });
      setRoverVelocity({ v_x: 0, v_y: 0, omega_z: 0 });
    }
    addLog("EMERGENCY STOP ACTIVATED", "warning");
  }, [controlMode, sendArmCommand, sendRoverCommand, addLog]);

  // Auto-connect on mount
  useEffect(() => {
    connect();
    return () => disconnect();
  }, [connect, disconnect]);

  // Cleanup on unmount
  useEffect(() => {
    return () => {
      if (socketRef.current?.connected) {
        emergencyStop();
      }
    };
  }, [emergencyStop]);

  const radToDeg = (rad: number) => ((rad * 180) / Math.PI).toFixed(1);

  return (
    <div className="min-h-screen bg-gradient-to-br from-blue-900 via-purple-900 to-pink-800 relative overflow-hidden">
      {/* Animated background elements */}
      <div className="absolute inset-0 overflow-hidden pointer-events-none">
        <div className="absolute top-20 left-10 w-32 h-32 bg-yellow-400 opacity-20 rounded-3xl rotate-45 animate-pulse"></div>
        <div className="absolute top-40 right-20 w-24 h-24 bg-cyan-400 opacity-20 rounded-2xl rotate-12 animate-bounce"></div>
        <div className="absolute bottom-20 left-1/4 w-40 h-40 bg-orange-400 opacity-20 rounded-full animate-pulse"></div>
        <div className="absolute bottom-40 right-1/3 w-28 h-28 bg-blue-400 opacity-20 rounded-3xl -rotate-12 animate-bounce"></div>
      </div>

      <div className="relative z-10 p-4 max-w-7xl mx-auto space-y-4">
        {/* Header */}
        <div className="backdrop-blur-xl bg-white/10 rounded-3xl shadow-2xl p-6 border border-white/20">
          <div className="flex items-center justify-between">
            <div>
              <h1 className="text-3xl font-bold text-white tracking-tight">
                ROBO ROVER
              </h1>
              <p className="text-sm text-white/70 mt-1">
                Real-time Control System
              </p>
            </div>
            <div className="flex items-center gap-4">
              <div className="text-right">
                <div
                  className={`text-sm font-semibold ${connection.isConnected ? "text-green-300" : "text-red-300"}`}
                >
                  {connection.isConnected ? "● CONNECTED" : "○ DISCONNECTED"}
                </div>
                <div className="text-xs text-white/60">
                  {connection.commandsSent} sent
                </div>
              </div>
              {!connection.isConnected && (
                <button
                  onClick={connect}
                  className="px-6 py-3 bg-gradient-to-r from-cyan-500 to-blue-500 text-white rounded-2xl font-semibold shadow-lg hover:shadow-xl transition-all duration-300 hover:scale-105"
                >
                  CONNECT
                </button>
              )}
            </div>
          </div>
        </div>

        {/* Control Mode Selector */}
        <div className="backdrop-blur-xl bg-white/10 rounded-3xl shadow-2xl p-4 border border-white/20">
          <div className="flex gap-4">
            <button
              onClick={() => setControlMode("rover")}
              className={`flex-1 py-4 px-6 rounded-2xl font-semibold transition-all duration-300 ${
                controlMode === "rover"
                  ? "bg-gradient-to-r from-cyan-500 to-blue-500 text-white shadow-lg scale-105"
                  : "bg-white/5 text-white/60 hover:bg-white/10"
              }`}
            >
              🚗 ROVER CONTROL
            </button>
            <button
              onClick={() => setControlMode("arm")}
              className={`flex-1 py-4 px-6 rounded-2xl font-semibold transition-all duration-300 ${
                controlMode === "arm"
                  ? "bg-gradient-to-r from-purple-500 to-pink-500 text-white shadow-lg scale-105"
                  : "bg-white/5 text-white/60 hover:bg-white/10"
              }`}
            >
              🦾 ARM CONTROL
            </button>
          </div>
        </div>

        <div className="grid grid-cols-1 lg:grid-cols-2 gap-4">
          {/* Main Control Panel */}
          <div className="backdrop-blur-xl bg-white/10 rounded-3xl shadow-2xl p-6 border border-white/20">
            {controlMode === "rover" ? (
              <div className="space-y-6">
                <h2 className="text-2xl font-bold text-white mb-4">
                  Rover Navigation
                </h2>

                {/* React Joystick Component */}
                <div className="flex flex-col items-center space-y-4">
                  <div className="relative">
                    <Joystick
                      size={240}
                      baseColor="rgba(255, 255, 255, 0.1)"
                      stickColor="linear-gradient(135deg, #06b6d4 0%, #3b82f6 100%)"
                      move={handleJoystickMove}
                      stop={handleJoystickStop}
                      throttle={50}
                    />
                    <div className="absolute inset-0 rounded-full border-4 border-white/20 pointer-events-none"></div>
                  </div>

                  <div className="text-white/80 text-center text-sm">
                    Drag joystick to move rover
                  </div>
                </div>

                {/* Rotation Control */}
                <div className="space-y-2">
                  <div className="flex justify-between text-sm text-white/90">
                    <span className="font-medium">Rotation</span>
                    <span className="text-cyan-300 font-mono">
                      {roverVelocity.omega_z.toFixed(2)} rad/s
                    </span>
                  </div>
                  <input
                    type="range"
                    min="-1.0"
                    max="1.0"
                    step="0.05"
                    value={roverVelocity.omega_z}
                    onChange={(e) =>
                      setRoverVelocity((prev) => ({
                        ...prev,
                        omega_z: parseFloat(e.target.value),
                      }))
                    }
                    className="w-full h-3 bg-white/10 rounded-full appearance-none cursor-pointer
                             [&::-webkit-slider-thumb]:appearance-none [&::-webkit-slider-thumb]:w-6 [&::-webkit-slider-thumb]:h-6
                             [&::-webkit-slider-thumb]:rounded-full [&::-webkit-slider-thumb]:bg-gradient-to-r
                             [&::-webkit-slider-thumb]:from-cyan-400 [&::-webkit-slider-thumb]:to-blue-500
                             [&::-webkit-slider-thumb]:shadow-lg [&::-webkit-slider-thumb]:cursor-pointer"
                  />
                  <div className="flex justify-between text-xs text-white/60">
                    <span>← CCW</span>
                    <span>CW →</span>
                  </div>
                </div>

                {/* Velocity Display */}
                <div className="grid grid-cols-2 gap-4 mt-6">
                  <div className="backdrop-blur-md bg-white/5 rounded-2xl p-4 border border-white/10">
                    <div className="text-xs text-white/60 mb-1">
                      FORWARD/BACK
                    </div>
                    <div className="text-2xl font-bold text-cyan-300 font-mono">
                      {roverVelocity.v_x.toFixed(2)}
                    </div>
                    <div className="text-xs text-white/60">m/s</div>
                  </div>
                  <div className="backdrop-blur-md bg-white/5 rounded-2xl p-4 border border-white/10">
                    <div className="text-xs text-white/60 mb-1">LEFT/RIGHT</div>
                    <div className="text-2xl font-bold text-cyan-300 font-mono">
                      {roverVelocity.v_y.toFixed(2)}
                    </div>
                    <div className="text-xs text-white/60">m/s</div>
                  </div>
                </div>
              </div>
            ) : (
              <div className="space-y-4">
                <h2 className="text-2xl font-bold text-white mb-4">
                  6-DOF Arm Control
                </h2>

                {/* Joint Controls */}
                {Object.entries(jointPositions).map(([joint, value]) => (
                  <div key={joint} className="space-y-2">
                    <div className="flex justify-between text-sm text-white/90">
                      <span className="font-medium capitalize">
                        {joint.replace("_", " ")}
                      </span>
                      <span className="text-purple-300 font-mono">
                        {value.toFixed(3)} rad ({radToDeg(value)}°)
                      </span>
                    </div>
                    <input
                      type="range"
                      min={JOINT_LIMITS[joint as keyof JointPositions].min}
                      max={JOINT_LIMITS[joint as keyof JointPositions].max}
                      step="0.01"
                      value={value}
                      onChange={(e) =>
                        setJointPositions((prev) => ({
                          ...prev,
                          [joint]: parseFloat(e.target.value),
                        }))
                      }
                      className="w-full h-3 bg-white/10 rounded-full appearance-none cursor-pointer
                               [&::-webkit-slider-thumb]:appearance-none [&::-webkit-slider-thumb]:w-6 [&::-webkit-slider-thumb]:h-6
                               [&::-webkit-slider-thumb]:rounded-full [&::-webkit-slider-thumb]:bg-gradient-to-r
                               [&::-webkit-slider-thumb]:from-purple-400 [&::-webkit-slider-thumb]:to-pink-500
                               [&::-webkit-slider-thumb]:shadow-lg [&::-webkit-slider-thumb]:cursor-pointer"
                    />
                  </div>
                ))}

                <button
                  onClick={sendHome}
                  disabled={!connection.isConnected}
                  className="w-full mt-6 py-4 bg-gradient-to-r from-green-500 to-emerald-500 text-white rounded-2xl font-semibold shadow-lg hover:shadow-xl transition-all duration-300 hover:scale-105 disabled:opacity-50 disabled:cursor-not-allowed disabled:hover:scale-100"
                >
                  🏠 HOME POSITION
                </button>
              </div>
            )}
          </div>

          {/* Telemetry & Status */}
          <div className="space-y-4">
            {/* Emergency Stop */}
            <button
              onClick={emergencyStop}
              disabled={!connection.isConnected}
              className="w-full py-6 bg-gradient-to-r from-red-600 to-red-500 text-white rounded-3xl font-bold text-lg shadow-2xl hover:shadow-xl transition-all duration-300 hover:scale-105 disabled:opacity-50 disabled:cursor-not-allowed disabled:hover:scale-100 border-2 border-white/20"
            >
              ⚠️ EMERGENCY STOP
            </button>

            {/* Telemetry Display */}
            <div className="backdrop-blur-xl bg-white/10 rounded-3xl shadow-2xl p-6 border border-white/20">
              <h3 className="text-xl font-bold text-white mb-4">
                {controlMode === "rover" ? "🚗 Rover Status" : "🦾 Arm Status"}
              </h3>

              {controlMode === "rover" && roverTelemetry ? (
                <div className="space-y-3">
                  <div className="backdrop-blur-md bg-white/5 rounded-2xl p-4 border border-white/10">
                    <div className="text-xs text-white/60 mb-1">POSITION</div>
                    <div className="text-lg font-mono text-cyan-300">
                      [{roverTelemetry.position[0].toFixed(2)},{" "}
                      {roverTelemetry.position[1].toFixed(2)}]
                    </div>
                  </div>
                  <div className="grid grid-cols-3 gap-2">
                    <div className="backdrop-blur-md bg-white/5 rounded-2xl p-3 border border-white/10">
                      <div className="text-xs text-white/60 mb-1">YAW</div>
                      <div className="text-sm font-mono text-cyan-300">
                        {roverTelemetry.yaw.toFixed(1)}°
                      </div>
                    </div>
                    <div className="backdrop-blur-md bg-white/5 rounded-2xl p-3 border border-white/10">
                      <div className="text-xs text-white/60 mb-1">PITCH</div>
                      <div className="text-sm font-mono text-cyan-300">
                        {roverTelemetry.pitch.toFixed(1)}°
                      </div>
                    </div>
                    <div className="backdrop-blur-md bg-white/5 rounded-2xl p-3 border border-white/10">
                      <div className="text-xs text-white/60 mb-1">ROLL</div>
                      <div className="text-sm font-mono text-cyan-300">
                        {roverTelemetry.roll.toFixed(1)}°
                      </div>
                    </div>
                  </div>
                  <div className="backdrop-blur-md bg-white/5 rounded-2xl p-4 border border-white/10">
                    <div className="text-xs text-white/60 mb-1">VELOCITY</div>
                    <div className="text-lg font-mono text-cyan-300">
                      {roverTelemetry.velocity.toFixed(2)} m/s
                    </div>
                  </div>
                </div>
              ) : controlMode === "arm" && armTelemetry ? (
                <div className="space-y-3">
                  <div className="backdrop-blur-md bg-white/5 rounded-2xl p-4 border border-white/10">
                    <div className="text-xs text-white/60 mb-1">STATUS</div>
                    <div
                      className={`text-lg font-semibold ${armTelemetry.is_moving ? "text-yellow-300" : "text-green-300"}`}
                    >
                      {armTelemetry.is_moving ? "🔄 MOVING" : "✓ READY"}
                    </div>
                  </div>
                  {armTelemetry.joint_angles && (
                    <div className="backdrop-blur-md bg-white/5 rounded-2xl p-4 border border-white/10">
                      <div className="text-xs text-white/60 mb-2">
                        JOINT ANGLES
                      </div>
                      <div className="text-xs font-mono text-purple-300 space-y-1">
                        {armTelemetry.joint_angles.map((angle, idx) => (
                          <div key={idx}>
                            J{idx + 1}: {angle.toFixed(3)} rad
                          </div>
                        ))}
                      </div>
                    </div>
                  )}
                </div>
              ) : (
                <div className="text-center text-white/40 py-8">
                  No telemetry data available
                </div>
              )}
            </div>

            {/* Activity Log */}
            <div className="backdrop-blur-xl bg-white/10 rounded-3xl shadow-2xl p-6 border border-white/20">
              <div className="flex justify-between items-center mb-4">
                <h3 className="text-xl font-bold text-white">Activity Log</h3>
                <button
                  onClick={() => setLogs([])}
                  className="text-xs px-3 py-1 bg-white/10 text-white/70 rounded-lg hover:bg-white/20 transition-colors"
                >
                  Clear
                </button>
              </div>
              <div className="backdrop-blur-md bg-black/30 rounded-2xl p-3 h-48 overflow-y-auto font-mono text-xs space-y-1">
                {logs.length === 0 ? (
                  <div className="text-white/30 text-center py-8">
                    No activity yet...
                  </div>
                ) : (
                  logs.slice(0, 10).map((log, idx) => (
                    <div
                      key={idx}
                      className={`${
                        log.type === "error"
                          ? "text-red-300"
                          : log.type === "success"
                            ? "text-green-300"
                            : log.type === "warning"
                              ? "text-yellow-300"
                              : "text-cyan-200"
                      }`}
                    >
                      <span className="text-white/40">
                        [{new Date(log.timestamp).toLocaleTimeString()}]
                      </span>{" "}
                      {log.message}
                    </div>
                  ))
                )}
              </div>
            </div>
          </div>
        </div>

        {/* Quick Info */}
        <div className="backdrop-blur-xl bg-white/10 rounded-3xl shadow-2xl p-4 border border-white/20">
          <div className="flex items-center justify-center gap-8 text-sm text-white/70">
            <div>
              <span className="font-semibold text-white">
                Real-time Control:
              </span>{" "}
              Move controls to send commands instantly
            </div>
            <div className="w-px h-6 bg-white/20"></div>
            <div>
              <span className="font-semibold text-white">Throttle:</span>{" "}
              {THROTTLE_DELAY}ms between updates
            </div>
          </div>
        </div>
      </div>
    </div>
  );
};

export default RoboRoverController;