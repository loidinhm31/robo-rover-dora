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
import { Activity, Home, Radio, Zap, ChevronDown, ChevronUp, Gauge } from "lucide-react";

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

  // UI state for collapsible sections
  const [expandedSections, setExpandedSections] = useState({
    armJoints: true,
    logs: false,
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
    if (!connection.isConnected) return;

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
    sendArmCommand,
    sendThrottled,
  ]);

  // Real-time ROVER velocity control
  useEffect(() => {
    if (!connection.isConnected) return;

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
    sendArmCommand({ command_type: "stop" });
    sendRoverCommand({ command_type: "stop" });
    setRoverVelocity({ v_x: 0, v_y: 0, omega_z: 0 });
    addLog("EMERGENCY STOP ACTIVATED", "warning");
  }, [sendArmCommand, sendRoverCommand, addLog]);

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

  const toggleSection = (section: keyof typeof expandedSections) => {
    setExpandedSections(prev => ({ ...prev, [section]: !prev[section] }));
  };

  return (
    <div className="min-h-screen gradient-bg relative overflow-hidden">
      {/* Animated background elements */}
      <div
        className="cube-decoration top-20 left-10 bg-cyan-400"
        style={{ animationDelay: "0s" }}
      ></div>
      <div
        className="cube-decoration top-40 right-20 bg-blue-500"
        style={{ animationDelay: "2s" }}
      ></div>
      <div
        className="cube-decoration bottom-20 left-1/4 bg-orange-400"
        style={{ animationDelay: "4s" }}
      ></div>
      <div
        className="cube-decoration bottom-40 right-1/3 bg-yellow-400"
        style={{ animationDelay: "1s" }}
      ></div>
      <div
        className="cube-decoration top-1/2 left-1/2 bg-pink-400"
        style={{ animationDelay: "3s" }}
      ></div>

      <div className="relative z-10 p-3 md:p-4 max-w-7xl mx-auto space-y-3 md:space-y-4">
        {/* Header */}
        <div className="glass-card rounded-3xl shadow-2xl p-4 md:p-6">
          <div className="flex flex-col md:flex-row items-start md:items-center justify-between gap-4">
            <div>
              <h1 className="text-3xl md:text-4xl font-bold text-white tracking-tight mb-2">
                UNIFIED ROBOT CONTROL
              </h1>
              <p className="text-xs md:text-sm text-white/80">
                Simultaneous ARM & ROVER Control System
              </p>
            </div>
            <div className="flex items-center gap-3 md:gap-4 w-full md:w-auto">
              <div className="glass-card-light rounded-2xl px-4 md:px-6 py-3 flex-1 md:flex-none">
                <div className="flex items-center gap-2">
                  {connection.isConnected ? (
                    <>
                      <Radio className="w-4 h-4 text-green-400 animate-pulse" />
                      <span className="text-xs md:text-sm font-semibold text-green-300">
                        CONNECTED
                      </span>
                    </>
                  ) : (
                    <>
                      <Radio className="w-4 h-4 text-red-400" />
                      <span className="text-xs md:text-sm font-semibold text-red-300">
                        OFFLINE
                      </span>
                    </>
                  )}
                </div>
                <div className="text-xs text-white/60 mt-1">
                  {connection.commandsSent} commands
                </div>
              </div>
              {!connection.isConnected && (
                <button
                  onClick={connect}
                  className="btn-gradient-cyan px-6 md:px-8 py-3 md:py-4 rounded-2xl text-sm md:text-lg whitespace-nowrap"
                >
                  CONNECT
                </button>
              )}
            </div>
          </div>
        </div>

        {/* Emergency Stop */}
        <button
          onClick={emergencyStop}
          disabled={!connection.isConnected}
          className="w-full py-4 md:py-6 bg-gradient-to-r from-red-600 to-red-500 text-white rounded-3xl font-bold text-lg md:text-xl shadow-2xl hover:shadow-xl transition-all duration-300 hover:scale-105 disabled:opacity-50 disabled:cursor-not-allowed disabled:hover:scale-100 glass-card"
        >
          ⚠️ EMERGENCY STOP
        </button>

        {/* Main Control Grid */}
        <div className="grid grid-cols-1 lg:grid-cols-2 gap-3 md:gap-4">
          {/* LEFT COLUMN: ROVER CONTROL */}
          <div className="space-y-3 md:space-y-4">
            <div className="glass-card rounded-3xl shadow-2xl p-4 md:p-6">
              <div className="flex items-center gap-2 mb-4 md:mb-6">
                <Activity className="w-6 h-6 md:w-8 md:h-8 text-cyan-400" />
                <h2 className="text-2xl md:text-3xl font-bold text-white">
                  ROVER
                </h2>
              </div>

              {/* Joystick Control */}
              <div className="flex flex-col items-center space-y-4">
                <div className="glass-card-light rounded-full p-4 md:p-6 relative">
                  <Joystick
                    size={window.innerWidth < 768 ? 180 : 240}
                    baseColor="rgba(255, 255, 255, 0.1)"
                    stickColor="linear-gradient(135deg, #06b6d4 0%, #3b82f6 100%)"
                    move={handleJoystickMove}
                    stop={handleJoystickStop}
                    throttle={50}
                  />
                  <div className="absolute inset-0 rounded-full border-4 border-cyan-400/30 pointer-events-none"></div>
                </div>

                <div className="text-white/90 text-center font-medium text-sm">
                  Drag to control rover movement
                </div>
              </div>

              {/* Rotation Control */}
              <div className="mt-4 glass-card-light rounded-2xl p-4 md:p-5 space-y-3">
                <div className="flex justify-between text-xs md:text-sm font-semibold text-white">
                  <span>Rotation (ω)</span>
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
                  className="glass-slider w-full"
                />
                <div className="flex justify-between text-xs text-white/60">
                  <span>← CCW</span>
                  <span>CW →</span>
                </div>
              </div>

              {/* Velocity Display */}
              <div className="mt-4 grid grid-cols-2 gap-3 md:gap-4">
                <div className="glass-card-light rounded-2xl p-3 md:p-5">
                  <div className="text-xs text-white/70 mb-2 font-semibold">
                    FORWARD
                  </div>
                  <div className="text-2xl md:text-3xl font-bold text-cyan-300 font-mono">
                    {roverVelocity.v_x.toFixed(2)}
                  </div>
                  <div className="text-xs text-white/60 mt-1">m/s</div>
                </div>
                <div className="glass-card-light rounded-2xl p-3 md:p-5">
                  <div className="text-xs text-white/70 mb-2 font-semibold">
                    STRAFE
                  </div>
                  <div className="text-2xl md:text-3xl font-bold text-cyan-300 font-mono">
                    {roverVelocity.v_y.toFixed(2)}
                  </div>
                  <div className="text-xs text-white/60 mt-1">m/s</div>
                </div>
              </div>
            </div>

            {/* ROVER Telemetry */}
            <div className="glass-card rounded-3xl shadow-2xl p-4 md:p-6">
              <h3 className="text-lg md:text-xl font-bold text-white mb-3 md:mb-4 flex items-center gap-2">
                <Gauge className="w-5 h-5 md:w-6 md:h-6" />
                Rover Status
              </h3>

              {roverTelemetry ? (
                <div className="space-y-3">
                  <div className="glass-card-light rounded-2xl p-3 md:p-4">
                    <div className="text-xs text-white/70 mb-2 font-semibold">
                      POSITION
                    </div>
                    <div className="text-lg md:text-xl font-mono text-cyan-300">
                      [{roverTelemetry.position[0].toFixed(2)},{" "}
                      {roverTelemetry.position[1].toFixed(2)}]
                    </div>
                  </div>
                  <div className="grid grid-cols-3 gap-2">
                    <div className="glass-card-light rounded-2xl p-2 md:p-3">
                      <div className="text-xs text-white/70 mb-1">YAW</div>
                      <div className="text-base md:text-lg font-mono text-cyan-300">
                        {roverTelemetry.yaw.toFixed(1)}°
                      </div>
                    </div>
                    <div className="glass-card-light rounded-2xl p-2 md:p-3">
                      <div className="text-xs text-white/70 mb-1">PITCH</div>
                      <div className="text-base md:text-lg font-mono text-cyan-300">
                        {roverTelemetry.pitch.toFixed(1)}°
                      </div>
                    </div>
                    <div className="glass-card-light rounded-2xl p-2 md:p-3">
                      <div className="text-xs text-white/70 mb-1">ROLL</div>
                      <div className="text-base md:text-lg font-mono text-cyan-300">
                        {roverTelemetry.roll.toFixed(1)}°
                      </div>
                    </div>
                  </div>
                  <div className="glass-card-light rounded-2xl p-3 md:p-4">
                    <div className="text-xs text-white/70 mb-2 font-semibold">
                      VELOCITY
                    </div>
                    <div className="text-xl md:text-2xl font-mono text-cyan-300">
                      {roverTelemetry.velocity.toFixed(2)} m/s
                    </div>
                  </div>
                </div>
              ) : (
                <div className="text-center text-white/40 py-8 md:py-12">
                  <Activity className="w-10 h-10 md:w-12 md:h-12 mx-auto mb-3 opacity-50" />
                  <p className="text-sm">No telemetry data</p>
                </div>
              )}
            </div>
          </div>

          {/* RIGHT COLUMN: ARM CONTROL */}
          <div className="space-y-3 md:space-y-4">
            <div className="glass-card rounded-3xl shadow-2xl p-4 md:p-6">
              <div className="flex items-center justify-between mb-4 md:mb-6">
                <div className="flex items-center gap-2">
                  <Zap className="w-6 h-6 md:w-8 md:h-8 text-purple-400" />
                  <h2 className="text-2xl md:text-3xl font-bold text-white">
                    ARM
                  </h2>
                </div>
                <button
                  onClick={sendHome}
                  disabled={!connection.isConnected}
                  className="btn-gradient px-4 md:px-6 py-2 md:py-3 rounded-xl text-sm md:text-base flex items-center gap-2 disabled:opacity-50 disabled:cursor-not-allowed disabled:hover:scale-100"
                >
                  <Home className="w-4 h-4 md:w-5 md:h-5" />
                  HOME
                </button>
              </div>

              {/* Collapsible Joint Controls */}
              <div className="glass-card-light rounded-2xl overflow-hidden">
                <button
                  onClick={() => toggleSection('armJoints')}
                  className="w-full flex items-center justify-between p-3 md:p-4 hover:bg-white/5 transition-colors"
                >
                  <span className="text-sm md:text-base font-semibold text-white">
                    Joint Controls (6-DOF)
                  </span>
                  {expandedSections.armJoints ? (
                    <ChevronUp className="w-5 h-5 text-white/70" />
                  ) : (
                    <ChevronDown className="w-5 h-5 text-white/70" />
                  )}
                </button>

                {expandedSections.armJoints && (
                  <div className="p-3 md:p-4 space-y-3 md:space-y-4 border-t border-white/10">
                    {Object.entries(jointPositions).map(([joint, value]) => (
                      <div key={joint} className="space-y-2">
                        <div className="flex justify-between text-xs md:text-sm font-semibold text-white">
                          <span className="capitalize">
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
                          className="glass-slider w-full"
                          style={{
                            background: `linear-gradient(to right, rgb(192 132 252) 0%, rgb(236 72 153) 100%)`,
                          }}
                        />
                      </div>
                    ))}
                  </div>
                )}
              </div>
            </div>

            {/* ARM Telemetry */}
            <div className="glass-card rounded-3xl shadow-2xl p-4 md:p-6">
              <h3 className="text-lg md:text-xl font-bold text-white mb-3 md:mb-4 flex items-center gap-2">
                <Gauge className="w-5 h-5 md:w-6 md:h-6" />
                Arm Status
              </h3>

              {armTelemetry ? (
                <div className="space-y-3">
                  <div className="glass-card-light rounded-2xl p-3 md:p-4">
                    <div className="text-xs text-white/70 mb-2 font-semibold">
                      STATUS
                    </div>
                    <div
                      className={`text-lg md:text-xl font-bold ${armTelemetry.is_moving ? "text-yellow-300" : "text-green-300"}`}
                    >
                      {armTelemetry.is_moving ? "🔄 MOVING" : "✓ READY"}
                    </div>
                  </div>
                  {armTelemetry.joint_angles && (
                    <div className="glass-card-light rounded-2xl p-3 md:p-4">
                      <div className="text-xs text-white/70 mb-2 font-semibold">
                        JOINT ANGLES
                      </div>
                      <div className="text-xs font-mono text-purple-300 space-y-1">
                        {armTelemetry.joint_angles.map((angle, idx) => (
                          <div key={idx} className="flex justify-between">
                            <span>J{idx + 1}:</span>
                            <span>{angle.toFixed(3)} rad</span>
                          </div>
                        ))}
                      </div>
                    </div>
                  )}
                </div>
              ) : (
                <div className="text-center text-white/40 py-8 md:py-12">
                  <Zap className="w-10 h-10 md:w-12 md:h-12 mx-auto mb-3 opacity-50" />
                  <p className="text-sm">No telemetry data</p>
                </div>
              )}
            </div>
          </div>
        </div>

        {/* Activity Log */}
        <div className="glass-card rounded-3xl shadow-2xl p-4 md:p-6">
          <button
            onClick={() => toggleSection('logs')}
            className="w-full flex items-center justify-between mb-4"
          >
            <h3 className="text-lg md:text-xl font-bold text-white">Activity Log</h3>
            <div className="flex items-center gap-3">
              <button
                onClick={(e) => {
                  e.stopPropagation();
                  setLogs([]);
                }}
                className="text-xs px-3 md:px-4 py-2 glass-card-light text-white/80 rounded-xl hover:text-white transition-all hover:scale-105"
              >
                Clear
              </button>
              {expandedSections.logs ? (
                <ChevronUp className="w-5 h-5 text-white/70" />
              ) : (
                <ChevronDown className="w-5 h-5 text-white/70" />
              )}
            </div>
          </button>

          {expandedSections.logs && (
            <div className="backdrop-blur-md bg-black/40 rounded-2xl p-3 md:p-4 max-h-48 overflow-y-auto font-mono text-xs space-y-1 border border-white/10">
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
          )}
        </div>

        {/* Quick Info */}
        <div className="glass-card rounded-3xl shadow-2xl p-3 md:p-4">
          <div className="flex flex-col md:flex-row items-center justify-center gap-3 md:gap-6 text-xs md:text-sm text-white/80">
            <div className="flex items-center gap-2">
              <div className="w-2 h-2 bg-cyan-400 rounded-full animate-pulse"></div>
              <span>
                <span className="font-bold text-white">Real-time Control</span>{" "}
                - Both systems active
              </span>
            </div>
            <div className="hidden md:block w-px h-6 bg-white/20"></div>
            <div>
              <span className="font-bold text-white">Throttle:</span>{" "}
              {THROTTLE_DELAY}ms
            </div>
          </div>
        </div>
      </div>
    </div>
  );
};

export default RoboRoverController;