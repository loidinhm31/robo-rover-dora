/**
 * RoboRoverControl - Main rover control page component
 *
 * Terminal/IDE-style UI for rover control with joystick, arm joints,
 * camera viewer, location map, and voice controls.
 *
 * This is the single source of truth for the control page UI.
 * Both web and native apps should import this component.
 */

import React, {
  Suspense,
  useCallback,
  useEffect,
  useRef,
  useState,
} from "react";
import { io, Socket } from "socket.io-client";
import { Joystick } from "react-joystick-component";
import type { IJoystickUpdateEvent } from "react-joystick-component/build/lib/Joystick.js";
import {
  Activity,
  AlertTriangle,
  Camera,
  Eye,
  EyeOff,
  Gauge,
  Home,
} from "lucide-react";

// Import types from shared package
import type {
  ConnectionState,
  FleetStatus,
  JointPositions,
  LogEntry,
  SpeechTranscription,
  SystemMetrics,
  TrackingTelemetry,
  WebArmCommand,
  WebRoverCommand,
} from "@robo-fleet/shared/types";
import {
  createHomePosition,
  createFleetSelectCommand,
  validateJointPositions,
} from "@robo-fleet/shared/constants";

// Import UI components from the same package
import { CameraViewer } from "../features/CameraViewer";
import { RobotLocationMap } from "../features/LocationMap";
import { TranscriptionDisplay } from "../features/TranscriptionDisplay";
import { VoiceControls } from "../features/VoiceControls";
import { FloatingMetrics } from "../features/FloatingMetrics";
import { IconBadge } from "../atoms";
import { CollapsibleSection } from "../molecules";
import { FleetSelector, JointControlPanel, ServerSettings, type SocketAuth } from "../organisms";
import { detectMixedContent } from "../../utils/url-validation";

const THROTTLE_DELAY = 100; // ms between updates

// Extended JointPositions with wheel visualization
interface ExtendedJointPositions extends JointPositions {
  wheel1: number;
  wheel2: number;
  wheel3: number;
}

export interface RoboRoverControlProps {
  /** Socket.IO server URL */
  socketUrl?: string;
  /** Authentication credentials */
  auth?: {
    username: string;
    password: string;
  };
}

const STORAGE_KEY = "robo-fleet-server-url";
const AUTH_STORAGE_KEY = "robo-fleet-auth";

const getStoredAuth = (): SocketAuth | undefined => {
  try {
    const raw = localStorage.getItem(AUTH_STORAGE_KEY);
    if (raw) return JSON.parse(raw) as SocketAuth;
  } catch { /* ignore */ }
  return undefined;
};

export const RoboRoverControl: React.FC<RoboRoverControlProps> = ({
  socketUrl = "http://localhost:3030",
  auth: authProp,
}) => {
  const [serverUrl, setServerUrl] = useState<string>(
    () => localStorage.getItem(STORAGE_KEY) ?? socketUrl
  );
  const [socketAuth, setSocketAuth] = useState<SocketAuth | undefined>(
    () => getStoredAuth() ?? authProp
  );

  // Connection state
  const [connection, setConnection] = useState<ConnectionState>({
    isConnected: false,
    clientId: null,
    commandsSent: 0,
    commandsReceived: 0,
  });

  // Telemetry state
  const [servoTelemetry, setServoTelemetry] = useState<TrackingTelemetry | null>(null);

  // Speech recognition state
  const [transcription, setTranscription] = useState<SpeechTranscription | null>(null);
  const [isAudioActive, setIsAudioActive] = useState(false);

  // Performance metrics state - per robot (entity_id -> metrics)
  const [performanceMetrics, setPerformanceMetrics] = useState<Map<string, SystemMetrics>>(
    new Map()
  );

  // Fleet status state
  const [fleetStatus, setFleetStatus] = useState<FleetStatus | null>(null);

  const [logs, setLogs] = useState<LogEntry[]>([]);
  const [showCamera, setShowCamera] = useState(false);
  const [showLocationMap, setShowLocationMap] = useState(false);

  // LeKiwi joint position controls (now includes wheels)
  const [jointPositions, setJointPositions] = useState<ExtendedJointPositions>({
    ...createHomePosition(),
    wheel1: 0.0,
    wheel2: 0.0,
    wheel3: 0.0,
  });

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
  const lastUpdateTime = useRef<number>(Date.now());
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

    const mixedContentError = detectMixedContent(serverUrl);
    if (mixedContentError) {
      addLog(mixedContentError, "error");
      addLog("Update server URL in settings to use wss:// or https://", "info");
      return;
    }

    const socket = io(serverUrl, {
      transports: ["websocket", "polling"],
      reconnection: true,
      reconnectionDelay: 1000,
      reconnectionAttempts: 5,
      auth: socketAuth,
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

    socket.on("connect_error", (error) => {
      addLog(`Connection error: ${error.message}`, "error");
      if (error.message.includes("authentication") || error.message.includes("auth")) {
        addLog("Authentication failed - check credentials", "error");
      }
    });

    socket.on("command_ack", () => {
      setConnection((prev) => ({
        ...prev,
        commandsReceived: prev.commandsReceived + 1,
      }));
    });

    socket.on("servo_telemetry", (data: TrackingTelemetry) => {
      setServoTelemetry(data);
    });

    socket.on("transcription", (data: SpeechTranscription) => {
      setTranscription(data);
      addLog(`Transcription: "${data.text}" (${(data.confidence * 100).toFixed(0)}%)`, "info");
    });

    socket.on("performance_metrics", (data: SystemMetrics) => {
      if (data.entity_id) {
        setPerformanceMetrics((prev) => {
          const newMap = new Map(prev);
          newMap.set(data.entity_id!, data);
          return newMap;
        });
      } else if (fleetStatus?.selected_entity) {
        setPerformanceMetrics((prev) => {
          const newMap = new Map(prev);
          newMap.set(fleetStatus.selected_entity, data);
          return newMap;
        });
      }
    });

    socket.on("fleet_status", (data: FleetStatus) => {
      setFleetStatus(data);
      addLog(`Fleet status: Selected rover is ${data.selected_entity}`, "info");
    });

    socketRef.current = socket;
  }, [serverUrl, socketAuth, addLog, fleetStatus?.selected_entity]);

  // Disconnect from Socket.IO server
  const disconnect = useCallback(() => {
    if (socketRef.current) {
      socketRef.current.disconnect();
      socketRef.current = null;
      addLog("Manually disconnected", "info");
    }
  }, [addLog]);

  // Select rover from fleet
  const selectRover = useCallback(
    (entityId: string) => {
      if (!connection.isConnected || !socketRef.current) {
        addLog("Cannot select rover - not connected", "error");
        return;
      }

      const selectCommand = createFleetSelectCommand(entityId);
      socketRef.current.emit("fleet_select", selectCommand);
      addLog(`Switching to rover: ${entityId}`, "info");
    },
    [connection.isConnected, addLog],
  );

  // Send ARM command
  const sendArmCommand = useCallback(
    (command: WebArmCommand) => {
      if (!connection.isConnected || !socketRef.current) {
        addLog("Cannot send command - not connected", "error");
        return;
      }

      socketRef.current.emit("arm_command", command);
      setConnection((prev) => ({
        ...prev,
        commandsSent: prev.commandsSent + 1,
      }));
    },
    [connection.isConnected, addLog],
  );

  // Send ROVER command
  const sendRoverCommand = useCallback(
    (command: WebRoverCommand) => {
      if (!connection.isConnected || !socketRef.current) {
        addLog("Cannot send rover command - not connected", "error");
        return;
      }

      socketRef.current.emit("rover_command", command);
      setConnection((prev) => ({
        ...prev,
        commandsSent: prev.commandsSent + 1,
      }));
    },
    [connection.isConnected, addLog],
  );

  // Audio control functions
  const startAudio = useCallback(() => {
    if (!connection.isConnected || !socketRef.current) {
      addLog("Cannot start audio - not connected", "error");
      return;
    }

    socketRef.current.emit("audio_control", { command: "start" });
    setIsAudioActive(true);
    addLog("Audio capture started", "success");
  }, [connection.isConnected, addLog]);

  const stopAudio = useCallback(() => {
    if (!connection.isConnected || !socketRef.current) {
      addLog("Cannot stop audio - not connected", "error");
      return;
    }

    socketRef.current.emit("audio_control", { command: "stop" });
    setIsAudioActive(false);
    addLog("Audio capture stopped", "info");
  }, [connection.isConnected, addLog]);

  // Update joint position
  const updateJoint = useCallback((joint: keyof JointPositions, value: number) => {
    setJointPositions((prev) => {
      const newPositions = { ...prev, [joint]: value };
      const error = validateJointPositions(newPositions);
      if (error) {
        console.warn(error);
      }
      return newPositions as ExtendedJointPositions;
    });
  }, []);

  // Real-time ARM joint control
  useEffect(() => {
    if (!connection.isConnected) return;

    const sendJointUpdate = () => {
      const command: WebArmCommand = {
        command_type: "joint_position",
        joint_positions: {
          shoulder_pan: jointPositions.shoulder_pan,
          shoulder_lift: jointPositions.shoulder_lift,
          elbow_flex: jointPositions.elbow_flex,
          wrist_flex: jointPositions.wrist_flex,
          wrist_roll: jointPositions.wrist_roll,
          gripper: jointPositions.gripper,
        },
      };
      sendArmCommand(command);
    };

    sendThrottled(sendJointUpdate);
  }, [
    jointPositions.shoulder_pan,
    jointPositions.shoulder_lift,
    jointPositions.elbow_flex,
    jointPositions.wrist_flex,
    jointPositions.wrist_roll,
    jointPositions.gripper,
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

  // Integrate wheel velocities into wheel positions for visualization
  useEffect(() => {
    const intervalId = setInterval(() => {
      const now = Date.now();
      const dt = (now - lastUpdateTime.current) / 1000;
      lastUpdateTime.current = now;

      const WHEEL_RADIUS = 0.05;
      const ROBOT_RADIUS = 0.15;
      const { v_x, v_y, omega_z } = roverVelocity;

      const omega1 = (v_y / WHEEL_RADIUS) + (omega_z * ROBOT_RADIUS / WHEEL_RADIUS);
      const omega2 = ((-0.5 * v_y + 0.866 * v_x) / WHEEL_RADIUS) + (omega_z * ROBOT_RADIUS / WHEEL_RADIUS);
      const omega3 = ((-0.5 * v_y - 0.866 * v_x) / WHEEL_RADIUS) + (omega_z * ROBOT_RADIUS / WHEEL_RADIUS);

      setJointPositions((prev) => ({
        ...prev,
        wheel1: prev.wheel1 + omega1 * dt,
        wheel2: prev.wheel2 + omega2 * dt,
        wheel3: prev.wheel3 + omega3 * dt,
      }));
    }, 50);

    return () => clearInterval(intervalId);
  }, [roverVelocity]);

  // Joystick move handler
  const handleJoystickMove = useCallback((event: IJoystickUpdateEvent) => {
    if (!event.x || !event.y) return;

    const v_y = event.x / 100;
    const v_x = -event.y / 100;

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
    setJointPositions({
      ...createHomePosition(),
      wheel1: 0.0,
      wheel2: 0.0,
      wheel3: 0.0,
    });
  }, [sendArmCommand]);

  // Emergency stop
  const emergencyStop = useCallback(() => {
    sendArmCommand({ command_type: "stop" });
    sendRoverCommand({ command_type: "stop" });
    setRoverVelocity({ v_x: 0, v_y: 0, omega_z: 0 });
    addLog("EMERGENCY STOP ACTIVATED", "warning");
  }, [sendArmCommand, sendRoverCommand, addLog]);

  // Handle connect from settings dialog — saves url + auth to localStorage, reconnects
  const handleConnectSettings = useCallback((url: string, auth: SocketAuth | undefined) => {
    localStorage.setItem(STORAGE_KEY, url);
    if (auth) {
      localStorage.setItem(AUTH_STORAGE_KEY, JSON.stringify(auth));
    } else {
      localStorage.removeItem(AUTH_STORAGE_KEY);
    }
    setSocketAuth(auth);
    setServerUrl(url);
    disconnect();
    addLog(`Connecting to: ${url}`, "info");
  }, [disconnect, addLog]);

  // Auto-connect on mount and reconnect when serverUrl or auth changes
  useEffect(() => {
    connect();
    return () => disconnect();
  }, [serverUrl, socketAuth]); // eslint-disable-line react-hooks/exhaustive-deps

  // Cleanup on unmount
  useEffect(() => {
    return () => {
      if (socketRef.current) {
        socketRef.current.disconnect();
      }
    };
  }, []);

  return (
    <div className="min-h-screen gradient-bg relative scanline-effect">
      <div className="relative z-10 max-w-7xl mx-auto">
        {/* Header - Terminal-style status bar */}
        <div className="sticky top-0 z-50 glass-card shadow-xl p-2 md:p-3 border-b-2 border-syntax-blue/30">
          <div className="flex flex-col md:flex-row items-start md:items-center justify-between gap-2">
            {/* Left: Title and Status Indicators */}
            <div className="flex items-center gap-2 md:gap-3 flex-wrap w-full md:w-auto">
              <div className="flex items-center gap-2">
                <div className="w-2 h-2 bg-syntax-blue rounded-full animate-pulse"></div>
                <h1 className="text-base md:text-lg font-mono font-bold text-syntax-cyan tracking-tight">
                  robot@fleet-control:~$
                </h1>
              </div>

              {/* Connection Status - Terminal style */}
              <div className="bg-slate-900/80 border border-slate-700 rounded px-2 py-1 flex items-center gap-1.5">
                {connection.isConnected ? (
                  <>
                    <div className="w-2 h-2 bg-syntax-green rounded-full status-glow-green"></div>
                    <span className="text-xs font-mono font-semibold text-syntax-green">
                      [ONLINE]
                    </span>
                  </>
                ) : (
                  <>
                    <div className="w-2 h-2 bg-syntax-red rounded-full status-glow-red"></div>
                    <span className="text-xs font-mono font-semibold text-syntax-red">
                      [OFFLINE]
                    </span>
                  </>
                )}
              </div>

              {/* Control Mode - Syntax colored */}
              {servoTelemetry && (
                <div className="bg-slate-900/80 border border-slate-700 rounded px-2 py-1 flex items-center gap-1.5">
                  {servoTelemetry.control_mode === "Autonomous" ? (
                    <>
                      <div className="w-2 h-2 bg-syntax-blue rounded-full status-glow-blue"></div>
                      <span className="text-xs font-mono font-semibold text-syntax-blue">
                        [AUTO]
                      </span>
                    </>
                  ) : (
                    <>
                      <div className="w-2 h-2 bg-syntax-purple rounded-full"></div>
                      <span className="text-xs font-mono font-semibold text-syntax-purple">
                        [MANUAL]
                      </span>
                    </>
                  )}
                  {servoTelemetry.distance_estimate !== null && (
                    <span className="text-xs text-syntax-cyan font-mono ml-1">
                      {servoTelemetry.distance_estimate?.toFixed(1)}m
                    </span>
                  )}
                </div>
              )}

              {/* Commands Count */}
              <div className="text-xs text-slate-500 font-mono hidden md:block">
                tx: <span className="text-syntax-orange">{connection.commandsSent}</span>
              </div>
            </div>

            {/* Right: Settings, Emergency Stop */}
            <div className="flex items-center gap-2 w-full md:w-auto">
              <ServerSettings
                currentUrl={serverUrl}
                currentAuth={socketAuth}
                isConnected={connection.isConnected}
                onConnect={handleConnectSettings}
                onDisconnect={disconnect}
              />

              {/* Emergency Stop Button - Terminal style */}
              <button
                onClick={emergencyStop}
                disabled={!connection.isConnected}
                className="group relative px-4 md:px-6 py-2 bg-red-600 hover:bg-red-500 text-white rounded font-black text-sm md:text-base shadow-lg shadow-red-500/30 hover:shadow-red-500/50 transition-all duration-200 disabled:opacity-40 disabled:hover:bg-red-600 disabled:shadow-none border-2 border-red-400/50 active:scale-95 flex-1 md:flex-none font-mono cursor-pointer"
                style={{
                  animation: connection.isConnected ? 'pulse-slow 3s infinite' : 'none'
                }}
              >
                <div className="flex items-center gap-2 justify-center">
                  <AlertTriangle className="w-4 h-4" />
                  <span className="tracking-wider">[ STOP ]</span>
                </div>
              </button>
            </div>
          </div>
        </div>

        <div className="p-3 md:p-4 space-y-3 md:space-y-4 pt-3 md:pt-4">
          {/* Fleet Selector */}
          <FleetSelector
            fleetStatus={fleetStatus}
            metricsMap={performanceMetrics}
            onSelectRover={selectRover}
            className="max-w-md"
          />

          <div className="grid grid-cols-1 lg:grid-cols-2 gap-4">
            {/* Location Map Viewer */}
            {showLocationMap && (
              <div className="glass-card rounded-lg shadow-2xl p-4 md:p-6 border-l-4 border-syntax-purple">
                <div className="flex items-center justify-between mb-4">
                  <div className="flex items-center gap-2">
                    <Eye className="w-5 h-5 text-syntax-purple" />
                    <h2 className="text-xl md:text-2xl font-mono font-bold text-syntax-purple">
                      {"<"} LOCATION_MAP {"/>"}
                    </h2>
                  </div>
                  <button
                    onClick={() => setShowLocationMap(false)}
                    className="btn-warning px-3 py-2 rounded text-xs font-mono flex items-center gap-2 cursor-pointer"
                  >
                    <EyeOff className="w-3 h-3" />
                    close()
                  </button>
                </div>
                <div className="bg-slate-900/70 border border-slate-700 rounded-lg p-2 md:p-4">
                  <Suspense
                    fallback={
                      <div className="h-96 flex items-center justify-center text-slate-500 font-mono text-sm">
                        // loading map...
                      </div>
                    }
                  >
                    <RobotLocationMap telemetry={null} />
                  </Suspense>
                </div>
                <div className="mt-3 text-xs text-slate-500 text-center font-mono">
                  // mouse: rotate | scroll: zoom | drag: pan
                </div>
                <div className="mt-3 grid grid-cols-3 gap-2 text-xs font-mono">
                  <div className="bg-slate-900/70 border border-slate-700 p-2 rounded">
                    <div className="text-slate-500">wheel[0]</div>
                    <div className="text-syntax-cyan">
                      {(jointPositions.wheel1 % (2 * Math.PI)).toFixed(2)} <span className="text-slate-600">rad</span>
                    </div>
                  </div>
                  <div className="bg-slate-900/70 border border-slate-700 p-2 rounded">
                    <div className="text-slate-500">wheel[1]</div>
                    <div className="text-syntax-cyan">
                      {(jointPositions.wheel2 % (2 * Math.PI)).toFixed(2)} <span className="text-slate-600">rad</span>
                    </div>
                  </div>
                  <div className="bg-slate-900/70 border border-slate-700 p-2 rounded">
                    <div className="text-slate-500">wheel[2]</div>
                    <div className="text-syntax-cyan">
                      {(jointPositions.wheel3 % (2 * Math.PI)).toFixed(2)} <span className="text-slate-600">rad</span>
                    </div>
                  </div>
                </div>
              </div>
            )}

            {/* Camera Viewer */}
            {showCamera && (
              <CameraViewer
                isConnected={connection.isConnected}
                socket={socketRef.current}
                onClose={() => setShowCamera(false)}
              />
            )}
          </div>

          <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
            {!showLocationMap && (
              <button
                onClick={() => setShowLocationMap(true)}
                className="w-full py-3 bg-slate-900/70 border border-slate-700 rounded-lg text-slate-300 hover:text-syntax-purple hover:border-syntax-purple/50 transition-all font-mono text-sm flex items-center justify-center gap-2 cursor-pointer"
              >
                <Eye className="w-4 h-4" />
                <span className="text-syntax-purple">show</span>
                <span className="text-slate-500">(</span>
                <span className="text-syntax-orange">"location_map"</span>
                <span className="text-slate-500">)</span>
              </button>
            )}
            {!showCamera && (
              <button
                onClick={() => setShowCamera(true)}
                className="w-full py-3 bg-slate-900/70 border border-slate-700 rounded-lg text-slate-300 hover:text-syntax-cyan hover:border-syntax-cyan/50 transition-all font-mono text-sm flex items-center justify-center gap-2 cursor-pointer"
              >
                <Camera className="w-4 h-4" />
                <span className="text-syntax-cyan">show</span>
                <span className="text-slate-500">(</span>
                <span className="text-syntax-orange">"camera_feed"</span>
                <span className="text-slate-500">)</span>
              </button>
            )}
          </div>

          {/* Speech Transcription Display */}
          <div className="mt-3">
            <TranscriptionDisplay
              transcription={transcription}
              isAudioActive={isAudioActive}
              maxHistory={5}
              onStartAudio={startAudio}
              onStopAudio={stopAudio}
            />
          </div>

          {/* Voice Communication Controls */}
          <VoiceControls
            socket={socketRef.current}
            isConnected={connection.isConnected}
            onLog={addLog}
          />

          {/* Main Control Grid */}
          <div className="grid grid-cols-1 lg:grid-cols-2 gap-3 md:gap-4">
            {/* LEFT COLUMN: ROVER CONTROL */}
            <div className="space-y-3 md:space-y-4">
              <div className="glass-card rounded-lg shadow-2xl p-4 md:p-6 border-l-4 border-syntax-cyan">
                <div className="flex items-center gap-2 mb-4 md:mb-6">
                  <Activity className="w-5 h-5 md:w-6 md:h-6 text-syntax-cyan" />
                  <h2 className="text-xl md:text-2xl font-mono font-bold text-syntax-cyan">
                    {"<"} ROVER_CONTROL {"/>"}
                  </h2>
                </div>

                {/* Joystick Control */}
                <div className="flex flex-col items-center space-y-4">
                  <div className="bg-slate-900/50 border-2 border-syntax-cyan/30 rounded-full p-4 md:p-6 relative">
                    <Joystick
                      size={typeof window !== "undefined" && window.innerWidth < 768 ? 180 : 240}
                      baseColor="rgba(30, 41, 59, 0.8)"
                      stickColor="linear-gradient(135deg, #06b6d4 0%, #3b82f6 100%)"
                      move={handleJoystickMove}
                      stop={handleJoystickStop}
                      throttle={50}
                    />
                    <div className="absolute inset-0 rounded-full border-2 border-syntax-cyan/40 pointer-events-none shadow-inner"></div>
                  </div>

                  <div className="text-slate-400 text-center font-mono text-xs">
                    // drag joystick to move
                  </div>
                </div>

                {/* Rotation Control */}
                <div className="mt-4 bg-slate-900/70 border border-slate-700 rounded-lg p-4 md:p-5 space-y-3">
                  <div className="flex justify-between text-xs md:text-sm font-mono text-slate-300">
                    <span className="text-syntax-orange">omega_z:</span>
                    <span className="text-syntax-cyan">
                      {roverVelocity.omega_z.toFixed(2)} <span className="text-slate-500">rad/s</span>
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
                  <div className="flex justify-between text-xs text-slate-600 font-mono">
                    <span>-1.0</span>
                    <span className="text-slate-500">0.0</span>
                    <span>+1.0</span>
                  </div>
                </div>
              </div>
            </div>

            {/* RIGHT COLUMN: ARM CONTROL */}
            <div className="space-y-3 md:space-y-4">
              <CollapsibleSection
                title="ARM JOINTS"
                isExpanded={expandedSections.armJoints}
                onToggle={() =>
                  setExpandedSections((prev) => ({
                    ...prev,
                    armJoints: !prev.armJoints,
                  }))
                }
                headerRight={
                  <IconBadge icon={Gauge} color="text-syntax-purple" size="md" />
                }
              >
                <JointControlPanel
                  jointPositions={jointPositions}
                  onJointChange={updateJoint}
                  disabled={!connection.isConnected}
                />
                <button
                  onClick={sendHome}
                  disabled={!connection.isConnected}
                  className="w-full py-3 md:py-4 btn-warning rounded-lg font-mono font-bold text-sm md:text-base flex items-center justify-center gap-2 disabled:opacity-50 disabled:cursor-not-allowed mt-4 cursor-pointer"
                >
                  <Home className="w-4 h-4" />
                  <span>{">"} HOME_POSITION()</span>
                </button>
              </CollapsibleSection>
            </div>
          </div>

          {/* Activity Logs - Terminal style */}
          <CollapsibleSection
            title={`SYSTEM_LOG [${logs.length}]`}
            isExpanded={expandedSections.logs}
            onToggle={() =>
              setExpandedSections((prev) => ({
                ...prev,
                logs: !prev.logs,
              }))
            }
            headerRight={
              <IconBadge icon={Activity} color="text-syntax-cyan" size="md" />
            }
            contentClassName="bg-slate-950 border-2 border-slate-800 rounded-lg p-3 md:p-4 max-h-48 overflow-y-auto font-mono text-xs space-y-1"
          >
            {logs.length === 0 ? (
              <div className="text-slate-600 text-center py-8">
                // no logs yet
              </div>
            ) : (
              logs.slice(0, 10).map((log, idx) => (
                <div
                  key={idx}
                  className={`${
                    log.type === "error"
                      ? "text-syntax-red"
                      : log.type === "success"
                        ? "text-syntax-green"
                        : log.type === "warning"
                          ? "text-syntax-yellow"
                          : "text-syntax-cyan"
                  }`}
                >
                  <span className="text-slate-600">
                    [{new Date(log.timestamp).toLocaleTimeString()}]
                  </span>{" "}
                  <span className={log.type === "error" ? "font-bold" : ""}>
                    {log.message}
                  </span>
                </div>
              ))
            )}
          </CollapsibleSection>

          {/* System Info Bar - Terminal style */}
          <div className="glass-card rounded-lg shadow-2xl p-3 md:p-4 border-t-2 border-syntax-blue/30">
            <div className="flex flex-col md:flex-row items-center justify-center gap-3 md:gap-6 text-xs font-mono text-slate-400">
              <div className="flex items-center gap-2">
                <div className="w-2 h-2 bg-syntax-green rounded-full status-glow-green"></div>
                <span className="text-syntax-cyan">realtime_control</span>
                <span className="text-slate-600">=</span>
                <span className="text-syntax-green">true</span>
              </div>
              <div className="hidden md:block w-px h-6 bg-slate-700"></div>
              <div>
                <span className="text-syntax-orange">throttle</span>
                <span className="text-slate-600">:</span>{" "}
                <span className="text-syntax-yellow">{THROTTLE_DELAY}</span>
                <span className="text-slate-500">ms</span>
              </div>
              <div className="hidden md:block w-px h-6 bg-slate-700"></div>
              <div className="flex items-center gap-2">
                <span className="text-syntax-purple">map_visible</span>
                <span className="text-slate-600">:</span>{" "}
                <span className={showLocationMap ? "text-syntax-green" : "text-syntax-red"}>
                  {showLocationMap ? "true" : "false"}
                </span>
              </div>
            </div>
          </div>
        </div>
      </div>

      {/* Floating Performance Metrics */}
      <FloatingMetrics metrics={performanceMetrics} socket={socketRef.current} />
    </div>
  );
};

export default RoboRoverControl;
