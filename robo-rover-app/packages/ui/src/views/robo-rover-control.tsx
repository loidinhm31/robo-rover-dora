import React, { useCallback, useEffect, useRef, useState } from "react";
import { io, Socket } from "socket.io-client";
import {
  armCommandToArrow,
  armTelemetryFromArrow,
  roverCommandToArrow,
  roverTelemetryFromArrow,
} from "@repo/ui/lib/arrow-utils";
import {
  ArmCommand,
  ArmTelemetry,
  ArrowMessage,
  ConnectionState,
  KeyboardState,
  LogEntry,
  RoverCommand,
  RoverTelemetry,
} from "@repo/ui/types/robo-rover.js";

const RoboRoverController: React.FC = () => {
  // Connection state
  const [connection, setConnection] = useState<ConnectionState>({
    isConnected: false,
    clientId: null,
    commandsSent: 0,
    commandsReceived: 0,
    arrowEnabled: false,
    schemasLoaded: false,
  });

  // Telemetry state
  const [armTelemetry, setArmTelemetry] = useState<ArmTelemetry | null>(null);
  const [roverTelemetry, setRoverTelemetry] = useState<RoverTelemetry | null>(
    null,
  );
  const [logs, setLogs] = useState<LogEntry[]>([]);

  // Control inputs state
  const [armControls, setArmControls] = useState({
    x: 0.01,
    y: 0.0,
    z: 0.0,
    roll: 0.0,
    pitch: 0.0,
    yaw: 0.0,
  });

  const [roverControls, setRoverControls] = useState({
    throttle: 0.0,
    brake: 0.0,
    steering: 0.0,
  });

  // UI state
  const [showLogs, setShowLogs] = useState(false);
  const [isCompact, setIsCompact] = useState(false);
  const [showKeyboardHelp, setShowKeyboardHelp] = useState(false);
  const [keyboardEnabled, setKeyboardEnabled] = useState(true);
  const [activeKeys, setActiveKeys] = useState<KeyboardState>({});
  const [showArrowStats, setShowArrowStats] = useState(true);

  // Refs
  const socketRef = useRef<Socket | null>(null);
  const logEndRef = useRef<HTMLDivElement>(null);

  // Arrow-specific state
  const [arrowStats, setArrowStats] = useState({
    messagesSent: 0,
    messagesReceived: 0,
    bytesTransferred: 0,
    compressionRatio: 0,
  });

  // Keyboard mappings
  const keyboardMappings = {
    // ARM Controls
    KeyW: { type: "arm", action: "x+", label: "W: X+" },
    KeyS: { type: "arm", action: "x-", label: "S: X-" },
    KeyA: { type: "arm", action: "y-", label: "A: Y-" },
    KeyD: { type: "arm", action: "y+", label: "D: Y+" },
    KeyQ: { type: "arm", action: "z+", label: "Q: Z+" },
    KeyE: { type: "arm", action: "z-", label: "E: Z-" },
    KeyR: { type: "arm", action: "home", label: "R: Home" },
    KeyT: { type: "arm", action: "stop", label: "T: Stop" },

    // ROVER Controls
    ArrowUp: { type: "rover", action: "forward", label: "↑: Forward" },
    ArrowDown: { type: "rover", action: "reverse", label: "↓: Reverse" },
    ArrowLeft: { type: "rover", action: "left", label: "←: Left" },
    ArrowRight: { type: "rover", action: "right", label: "→: Right" },
    KeyJ: { type: "rover", action: "steer_left", label: "J: Steer Left" },
    KeyL: { type: "rover", action: "steer_right", label: "L: Steer Right" },
    Space: { type: "rover", action: "brake", label: "Space: Brake" },
    KeyX: { type: "rover", action: "stop", label: "X: Stop" },

    // Emergency
    Escape: { type: "emergency", action: "stop", label: "ESC: Emergency Stop" },
  };

  // Utility functions
  const addLog = useCallback(
    (message: string, type: LogEntry["type"] = "info") => {
      const timestamp = new Date().toLocaleTimeString();
      setLogs((prev) => [...prev.slice(-49), { timestamp, message, type }]);
    },
    [],
  );

  const isDataFresh = (timestamp: number) => {
    return Date.now() - timestamp < 2000;
  };

  // Load schemas from server
  const loadSchemas = useCallback(() => {
    if (!socketRef.current?.connected) return;

    const schemas = [
      "arm_telemetry",
      "rover_telemetry",
      "arm_command",
      "rover_command",
    ];

    schemas.forEach((schemaName) => {
      console.log("getting schema", schemaName);
      socketRef.current?.emit("get_schema", { schema: schemaName });
    });

    addLog("Requesting Arrow schemas from server...", "info");
  }, [addLog]);

  // Socket connection management
  const connect = useCallback(() => {
    if (socketRef.current) {
      socketRef.current.disconnect();
    }

    addLog("Connecting to Web Bridge with Apache Arrow support...", "info");
    const socket = io("http://127.0.0.1:8080", {
      forceNew: true,
      transports: ["websocket", "polling"],
    });

    socket.on("connect", () => {
      setConnection((prev) => ({
        ...prev,
        isConnected: true,
        clientId: socket.id || null,
      }));
      addLog("Connected successfully - Loading Arrow schemas...", "success");
      loadSchemas();
    });

    socket.on("disconnect", () => {
      setConnection((prev) => ({
        ...prev,
        isConnected: false,
        clientId: null,
        arrowEnabled: false,
        schemasLoaded: false,
      }));
      addLog("Disconnected from Web Bridge", "error");
    });

    socket.on("status", (data) => {
      setConnection((prev) => ({
        ...prev,
        commandsReceived: prev.commandsReceived + 1,
      }));

      if (data.arrow_enabled) {
        setConnection((prev) => ({ ...prev, arrowEnabled: true }));
        addLog("✓ Apache Arrow enabled on server", "success");
      }

      addLog(`Status: ${data.message || JSON.stringify(data)}`, "info");
    });

    socket.on("error", (data) => {
      setConnection((prev) => ({
        ...prev,
        commandsReceived: prev.commandsReceived + 1,
      }));
      addLog(`Error: ${data.message || JSON.stringify(data)}`, "error");
    });

    // Handle Arrow telemetry
    socket.on("arrow_telemetry", (arrowMessage: ArrowMessage) => {
      try {
        setConnection((prev) => ({
          ...prev,
          commandsReceived: prev.commandsReceived + 1,
        }));

        setArrowStats((prev) => ({
          ...prev,
          messagesReceived: prev.messagesReceived + 1,
          bytesTransferred:
            prev.bytesTransferred + arrowMessage.arrow_data.length,
        }));

        if (arrowMessage.schema_name === "arm_telemetry") {
          const armData = armTelemetryFromArrow(arrowMessage.arrow_data);
          setArmTelemetry(armData);
          addLog("📡 ARM telemetry (Arrow)", "info");
        } else if (arrowMessage.schema_name === "rover_telemetry") {
          const roverData = roverTelemetryFromArrow(arrowMessage.arrow_data);
          setRoverTelemetry(roverData);
          addLog("📡 ROVER telemetry (Arrow)", "info");
        }
      } catch (error) {
        addLog(`Failed to parse Arrow telemetry: ${error}`, "error");
      }
    });

    // Handle schema responses
    socket.on("schema_response", (schemaData) => {
      addLog(`📋 Schema loaded: ${schemaData.schema_name}`, "success");

      // Check if all schemas are loaded
      setConnection((prev) => ({ ...prev, schemasLoaded: true }));
    });

    socket.on("pong", (data) => {
      addLog(
        `Ping: ${data.timestamp ? `${Date.now() - data.timestamp}ms` : "OK"}`,
        "success",
      );
    });

    socket.on("connect_error", (error) => {
      addLog(`Connection error: ${error.message}`, "error");
    });

    socketRef.current = socket;
  }, [addLog, loadSchemas]);

  const disconnect = useCallback(() => {
    if (socketRef.current) {
      socketRef.current.disconnect();
      socketRef.current = null;
    }
  }, []);

  // Command functions using Arrow format
  const sendArmCommand = useCallback(
    (type: ArmCommand["type"], params = {}) => {
      if (!socketRef.current?.connected) {
        addLog("Cannot send ARM command - not connected", "error");
        return;
      }

      if (!connection.arrowEnabled) {
        addLog("Cannot send ARM command - Arrow not enabled", "error");
        return;
      }

      try {
        const command: ArmCommand = { type, ...params };
        const arrowMessage = armCommandToArrow(command);

        socketRef.current.emit("arrow_arm_command", arrowMessage);

        setConnection((prev) => ({
          ...prev,
          commandsSent: prev.commandsSent + 1,
        }));

        setArrowStats((prev) => ({
          ...prev,
          messagesSent: prev.messagesSent + 1,
          bytesTransferred:
            prev.bytesTransferred + arrowMessage.arrow_data.length,
        }));

        addLog(`🦾 ARM: ${type} (Arrow)`, "info");
      } catch (error) {
        addLog(`Failed to send ARM command: ${error}`, "error");
      }
    },
    [addLog, connection.arrowEnabled],
  );

  const sendRoverCommand = useCallback(
    (throttle: number, brake: number, steering_angle: number) => {
      if (!socketRef.current?.connected) {
        addLog("Cannot send ROVER command - not connected", "error");
        return;
      }

      if (!connection.arrowEnabled) {
        addLog("Cannot send ROVER command - Arrow not enabled", "error");
        return;
      }

      try {
        const command: RoverCommand = { throttle, brake, steering_angle };
        const arrowMessage = roverCommandToArrow(command);

        socketRef.current.emit("arrow_rover_command", arrowMessage);

        setConnection((prev) => ({
          ...prev,
          commandsSent: prev.commandsSent + 1,
        }));

        setArrowStats((prev) => ({
          ...prev,
          messagesSent: prev.messagesSent + 1,
          bytesTransferred:
            prev.bytesTransferred + arrowMessage.arrow_data.length,
        }));

        addLog(
          `🚗 ROVER: T${throttle.toFixed(1)} B${brake.toFixed(1)} S${steering_angle.toFixed(1)}° (Arrow)`,
          "info",
        );
      } catch (error) {
        addLog(`Failed to send ROVER command: ${error}`, "error");
      }
    },
    [addLog, connection.arrowEnabled],
  );

  // Emergency stop for both systems
  const emergencyStopAll = useCallback(() => {
    sendArmCommand("emergency_stop");
    sendRoverCommand(0.0, 1.0, 0.0);
    addLog("🚨 EMERGENCY STOP - ALL SYSTEMS (Arrow)", "error");
  }, [sendArmCommand, sendRoverCommand, addLog]);

  // Keyboard control functions
  const executeKeyboardAction = useCallback(
    (code: string) => {
      if (
        !keyboardEnabled ||
        !connection.isConnected ||
        !connection.arrowEnabled
      )
        return;

      const mapping = keyboardMappings[code as keyof typeof keyboardMappings];
      if (!mapping) return;

      if (mapping.type === "arm") {
        switch (mapping.action) {
          case "x+":
            sendArmCommand("cartesian_move", {
              x: 0.01,
              y: 0,
              z: 0,
              roll: 0,
              pitch: 0,
              yaw: 0,
            });
            break;
          case "x-":
            sendArmCommand("cartesian_move", {
              x: -0.01,
              y: 0,
              z: 0,
              roll: 0,
              pitch: 0,
              yaw: 0,
            });
            break;
          case "y+":
            sendArmCommand("cartesian_move", {
              x: 0,
              y: 0.01,
              z: 0,
              roll: 0,
              pitch: 0,
              yaw: 0,
            });
            break;
          case "y-":
            sendArmCommand("cartesian_move", {
              x: 0,
              y: -0.01,
              z: 0,
              roll: 0,
              pitch: 0,
              yaw: 0,
            });
            break;
          case "z+":
            sendArmCommand("cartesian_move", {
              x: 0,
              y: 0,
              z: 0.01,
              roll: 0,
              pitch: 0,
              yaw: 0,
            });
            break;
          case "z-":
            sendArmCommand("cartesian_move", {
              x: 0,
              y: 0,
              z: -0.01,
              roll: 0,
              pitch: 0,
              yaw: 0,
            });
            break;
          case "home":
            sendArmCommand("home");
            break;
          case "stop":
            sendArmCommand("stop");
            break;
        }
      } else if (mapping.type === "rover") {
        switch (mapping.action) {
          case "forward":
            sendRoverCommand(0.3, 0.0, 0.0);
            break;
          case "reverse":
            sendRoverCommand(-0.2, 0.0, 0.0);
            break;
          case "left":
            sendRoverCommand(0.2, 0.0, 5.0);
            break;
          case "right":
            sendRoverCommand(0.2, 0.0, -5.0);
            break;
          case "steer_left":
            sendRoverCommand(0.0, 0.0, 5.0);
            break;
          case "steer_right":
            sendRoverCommand(0.0, 0.0, -5.0);
            break;
          case "brake":
            sendRoverCommand(0.0, 1.0, 0.0);
            break;
          case "stop":
            sendRoverCommand(0.0, 0.0, 0.0);
            break;
        }
      } else if (mapping.type === "emergency") {
        emergencyStopAll();
      }
    },
    [
      keyboardEnabled,
      connection.isConnected,
      connection.arrowEnabled,
      sendArmCommand,
      sendRoverCommand,
      emergencyStopAll,
    ],
  );

  // Keyboard event handlers
  const handleKeyDown = useCallback(
    (event: KeyboardEvent) => {
      // Don't trigger if user is typing in an input field
      if (
        event.target instanceof HTMLInputElement ||
        event.target instanceof HTMLTextAreaElement
      ) {
        return;
      }

      const code = event.code;

      // Prevent default for our mapped keys
      if (keyboardMappings[code as keyof typeof keyboardMappings]) {
        event.preventDefault();

        // Track active keys for visual feedback
        setActiveKeys((prev) => ({ ...prev, [code]: true }));

        // Execute action only once per key press (not on repeat)
        if (!event.repeat) {
          executeKeyboardAction(code);
        }
      }
    },
    [executeKeyboardAction],
  );

  const handleKeyUp = useCallback((event: KeyboardEvent) => {
    const code = event.code;

    // Remove from active keys
    setActiveKeys((prev) => {
      const newKeys = { ...prev };
      delete newKeys[code];
      return newKeys;
    });
  }, []);

  // Keyboard setup and cleanup
  useEffect(() => {
    if (keyboardEnabled) {
      document.addEventListener("keydown", handleKeyDown);
      document.addEventListener("keyup", handleKeyUp);

      return () => {
        document.removeEventListener("keydown", handleKeyDown);
        document.removeEventListener("keyup", handleKeyUp);
      };
    }
  }, [keyboardEnabled, handleKeyDown, handleKeyUp]);

  useEffect(() => {
    addLog("Robo Rover Controller initialized");

    return () => {
      disconnect();
    };
  }, [addLog, disconnect]);

  // Auto-scroll logs
  useEffect(() => {
    if (showLogs) {
      logEndRef.current?.scrollIntoView({ behavior: "smooth" });
    }
  }, [logs, showLogs]);

  // Responsive breakpoint detection
  useEffect(() => {
    const handleResize = () => {
      setIsCompact(window.innerWidth < 768);
    };

    handleResize();
    window.addEventListener("resize", handleResize);
    return () => window.removeEventListener("resize", handleResize);
  }, []);

  // Calculate compression ratio (approximate)
  useEffect(() => {
    if (arrowStats.messagesReceived > 0) {
      // Estimate JSON size vs Arrow size
      const estimatedJsonSize = arrowStats.messagesReceived * 200; // Rough estimate
      const compressionRatio =
        estimatedJsonSize / Math.max(arrowStats.bytesTransferred, 1);
      setArrowStats((prev) => ({ ...prev, compressionRatio }));
    }
  }, [arrowStats.messagesReceived, arrowStats.bytesTransferred]);

  // Format helpers
  const formatPoseValues = (pose: number[]) => {
    if (!pose || !Array.isArray(pose) || pose.length < 6) return null;
    const labels = ["X", "Y", "Z", "R", "P", "Y"];
    return (
      <div className="flex flex-wrap gap-1">
        {pose.slice(0, 6).map((val, idx) => (
          <span key={idx} className="text-xs bg-white/20 px-1.5 py-0.5 rounded">
            {labels[idx]}: {val.toFixed(3)}
          </span>
        ))}
      </div>
    );
  };

  const formatJointValues = (values: number[]) => {
    if (!values || !Array.isArray(values)) return null;
    return (
      <div className="flex flex-wrap gap-1 mt-1">
        {values.slice(0, 6).map((val, idx) => (
          <span key={idx} className="text-xs bg-white/20 px-1.5 py-0.5 rounded">
            J{idx + 1}: {val.toFixed(2)}
          </span>
        ))}
      </div>
    );
  };

  const formatBytes = (bytes: number) => {
    if (bytes === 0) return "0 B";
    const k = 1024;
    const sizes = ["B", "KB", "MB", "GB"];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + " " + sizes[i];
  };

  return (
    <div className="min-h-screen bg-gradient-to-br from-slate-900 via-slate-800 to-slate-900 text-white">
      {/* Header */}
      <div className="bg-black/20 backdrop-blur-sm border-b border-white/10">
        <div className="max-w-7xl mx-auto px-4 py-3">
          <div className="flex items-center justify-between">
            <div className="flex items-center space-x-4">
              <h1 className="text-xl font-bold bg-gradient-to-r from-blue-400 to-cyan-400 bg-clip-text text-transparent">
                🤖 Robo Rover Controller
              </h1>
              <div
                className={`flex items-center space-x-2 px-3 py-1 rounded-full text-sm ${
                  connection.isConnected
                    ? "bg-green-500/20 text-green-400 border border-green-500/30"
                    : "bg-red-500/20 text-red-400 border border-red-500/30"
                }`}
              >
                <div
                  className={`w-2 h-2 rounded-full ${connection.isConnected ? "bg-green-400" : "bg-red-400"}`}
                />
                <span>
                  {connection.isConnected ? "Connected" : "Disconnected"}
                </span>
              </div>

              {/* Arrow Status */}
              <div
                className={`flex items-center space-x-2 px-3 py-1 rounded-full text-sm ${
                  connection.arrowEnabled
                    ? "bg-purple-500/20 text-purple-400 border border-purple-500/30"
                    : "bg-gray-500/20 text-gray-400 border border-gray-500/30"
                }`}
              >
                <span>🏹</span>
                <span>
                  {connection.arrowEnabled ? "Arrow ON" : "Arrow OFF"}
                </span>
              </div>

              {/* Keyboard Status */}
              <div
                className={`flex items-center space-x-2 px-3 py-1 rounded-full text-sm ${
                  keyboardEnabled && connection.arrowEnabled
                    ? "bg-indigo-500/20 text-indigo-400 border border-indigo-500/30"
                    : "bg-gray-500/20 text-gray-400 border border-gray-500/30"
                }`}
              >
                <span>⌨️</span>
                <span>
                  {keyboardEnabled && connection.arrowEnabled
                    ? "Keyboard ON"
                    : "Keyboard OFF"}
                </span>
              </div>
            </div>

            <div className="flex items-center space-x-2">
              <div className="text-xs text-gray-400 hidden sm:block">
                ↑{connection.commandsSent} ↓{connection.commandsReceived}
              </div>

              <button
                onClick={() => setShowArrowStats(!showArrowStats)}
                className="px-3 py-1 bg-purple-600 hover:bg-purple-700 rounded-lg text-sm transition-colors"
              >
                📊 Arrow
              </button>

              <button
                onClick={() => setKeyboardEnabled(!keyboardEnabled)}
                className={`px-3 py-1 rounded-lg text-sm transition-colors ${
                  keyboardEnabled
                    ? "bg-indigo-600 hover:bg-indigo-700"
                    : "bg-gray-600 hover:bg-gray-700"
                }`}
              >
                ⌨️
              </button>

              <button
                onClick={() => setShowKeyboardHelp(!showKeyboardHelp)}
                className="px-3 py-1 bg-indigo-600 hover:bg-indigo-700 rounded-lg text-sm transition-colors"
              >
                Keys
              </button>

              <button
                onClick={() => setShowLogs(!showLogs)}
                className="px-3 py-1 bg-gray-700 hover:bg-gray-600 rounded-lg text-sm transition-colors"
              >
                {showLogs ? "Hide" : "Logs"}
              </button>

              <button
                onClick={connection.isConnected ? disconnect : connect}
                className={`px-4 py-1 rounded-lg text-sm font-medium transition-all ${
                  connection.isConnected
                    ? "bg-red-600 hover:bg-red-700 hover:scale-105"
                    : "bg-blue-600 hover:bg-blue-700 hover:scale-105"
                }`}
              >
                {connection.isConnected ? "Disconnect" : "Connect"}
              </button>

              <button
                onClick={emergencyStopAll}
                disabled={!connection.isConnected || !connection.arrowEnabled}
                className="px-4 py-1 bg-red-600 hover:bg-red-700 rounded-lg text-sm font-bold disabled:opacity-50 disabled:cursor-not-allowed transition-all hover:scale-105 border-2 border-red-400"
              >
                🛑 E-STOP
              </button>
            </div>
          </div>
        </div>
      </div>

      {/* Arrow Statistics Panel */}
      {showArrowStats && connection.arrowEnabled && (
        <div className="bg-purple-900/40 backdrop-blur-sm border-b border-purple-500/20">
          <div className="max-w-7xl mx-auto px-4 py-3">
            <div className="flex justify-between items-center mb-3">
              <h3 className="font-semibold text-purple-200">
                🏹 Apache Arrow Statistics
              </h3>
              <button
                onClick={() =>
                  setArrowStats({
                    messagesSent: 0,
                    messagesReceived: 0,
                    bytesTransferred: 0,
                    compressionRatio: 0,
                  })
                }
                className="text-xs bg-purple-600 hover:bg-purple-700 px-2 py-1 rounded transition-colors"
              >
                Reset
              </button>
            </div>
            <div className="grid grid-cols-2 md:grid-cols-4 gap-4 text-sm">
              <div className="bg-black/20 p-3 rounded-lg">
                <div className="text-purple-300 font-medium">Messages Sent</div>
                <div className="text-white text-xl">
                  {arrowStats.messagesSent}
                </div>
              </div>
              <div className="bg-black/20 p-3 rounded-lg">
                <div className="text-purple-300 font-medium">
                  Messages Received
                </div>
                <div className="text-white text-xl">
                  {arrowStats.messagesReceived}
                </div>
              </div>
              <div className="bg-black/20 p-3 rounded-lg">
                <div className="text-purple-300 font-medium">
                  Data Transferred
                </div>
                <div className="text-white text-xl">
                  {formatBytes(arrowStats.bytesTransferred)}
                </div>
              </div>
              <div className="bg-black/20 p-3 rounded-lg">
                <div className="text-purple-300 font-medium">Compression</div>
                <div className="text-white text-xl">
                  {arrowStats.compressionRatio > 0
                    ? `${arrowStats.compressionRatio.toFixed(1)}x`
                    : "N/A"}
                </div>
              </div>
            </div>
          </div>
        </div>
      )}

      {/* Keyboard Help Panel */}
      {showKeyboardHelp && (
        <div className="bg-black/40 backdrop-blur-sm border-b border-white/10">
          <div className="max-w-7xl mx-auto px-4 py-3">
            <div className="flex justify-between items-center mb-3">
              <h3 className="font-semibold text-indigo-200">
                ⌨️ Keyboard Controls
              </h3>
              <span className="text-xs text-gray-400">
                {keyboardEnabled ? "Active" : "Disabled"}
              </span>
            </div>
            <div className="grid grid-cols-2 md:grid-cols-3 gap-4 text-sm">
              <div>
                <h4 className="font-medium text-blue-300 mb-2">
                  🦾 ARM Control
                </h4>
                <div className="space-y-1">
                  {Object.entries(keyboardMappings)
                    .filter(([, mapping]) => mapping.type === "arm")
                    .map(([key, mapping]) => (
                      <div
                        key={key}
                        className={`flex items-center space-x-2 ${
                          activeKeys[key]
                            ? "text-yellow-400 font-bold"
                            : "text-gray-300"
                        }`}
                      >
                        <span className="w-12 text-right font-mono text-xs">
                          {mapping.label.split(":")[0]}
                        </span>
                        <span>{mapping.label.split(":")[1]}</span>
                      </div>
                    ))}
                </div>
              </div>
              <div>
                <h4 className="font-medium text-green-300 mb-2">
                  🚗 ROVER Control
                </h4>
                <div className="space-y-1">
                  {Object.entries(keyboardMappings)
                    .filter(([, mapping]) => mapping.type === "rover")
                    .map(([key, mapping]) => (
                      <div
                        key={key}
                        className={`flex items-center space-x-2 ${
                          activeKeys[key]
                            ? "text-yellow-400 font-bold"
                            : "text-gray-300"
                        }`}
                      >
                        <span className="w-12 text-right font-mono text-xs">
                          {mapping.label.split(":")[0]}
                        </span>
                        <span>{mapping.label.split(":")[1]}</span>
                      </div>
                    ))}
                </div>
              </div>
              <div>
                <h4 className="font-medium text-red-300 mb-2">🚨 Emergency</h4>
                <div className="space-y-1">
                  {Object.entries(keyboardMappings)
                    .filter(([, mapping]) => mapping.type === "emergency")
                    .map(([key, mapping]) => (
                      <div
                        key={key}
                        className={`flex items-center space-x-2 ${
                          activeKeys[key]
                            ? "text-yellow-400 font-bold"
                            : "text-gray-300"
                        }`}
                      >
                        <span className="w-12 text-right font-mono text-xs">
                          {mapping.label.split(":")[0]}
                        </span>
                        <span>{mapping.label.split(":")[1]}</span>
                      </div>
                    ))}
                </div>
              </div>
            </div>
            <div className="mt-3 text-xs text-gray-400">
              💡 Tip: Keyboard controls work when not typing in input fields.
              Active keys are highlighted in yellow.
            </div>
          </div>
        </div>
      )}

      {/* Logs Panel */}
      {showLogs && (
        <div className="bg-black/40 backdrop-blur-sm border-b border-white/10">
          <div className="max-w-7xl mx-auto px-4 py-3">
            <div className="flex justify-between items-center mb-2">
              <h3 className="font-semibold text-gray-200">Event Log</h3>
              <button
                onClick={() => setLogs([])}
                className="text-xs bg-orange-600 hover:bg-orange-700 px-2 py-1 rounded transition-colors"
              >
                Clear
              </button>
            </div>
            <div className="h-24 overflow-y-auto bg-black/50 p-2 rounded text-xs font-mono border border-white/10">
              {logs.slice(-10).map((log, idx) => (
                <div
                  key={idx}
                  className={`mb-1 ${
                    log.type === "error"
                      ? "text-red-400"
                      : log.type === "success"
                        ? "text-green-400"
                        : log.type === "warning"
                          ? "text-orange-400"
                          : "text-gray-300"
                  }`}
                >
                  [{log.timestamp}] {log.message}
                </div>
              ))}
              <div ref={logEndRef} />
            </div>
          </div>
        </div>
      )}

      {/* Main Control Panel */}
      <div className="max-w-7xl mx-auto px-4 py-6">
        {!connection.arrowEnabled && connection.isConnected && (
          <div className="mb-6 bg-yellow-900/40 backdrop-blur-sm rounded-2xl border border-yellow-500/20 p-4">
            <div className="flex items-center">
              <span className="text-yellow-400 text-xl mr-3">⚠️</span>
              <div>
                <div className="font-medium text-yellow-200">
                  Apache Arrow Not Available
                </div>
                <div className="text-sm text-yellow-300">
                  Connected to server but Arrow support is not enabled. Please
                  ensure you're connected to the web_bridge node with Arrow
                  support.
                </div>
              </div>
            </div>
          </div>
        )}

        <div
          className={`grid gap-6 ${isCompact ? "grid-cols-1" : "grid-cols-1 lg:grid-cols-2"}`}
        >
          {/* ARM Control Section */}
          <div className="bg-gradient-to-br from-blue-900/40 to-blue-800/40 backdrop-blur-sm rounded-2xl border border-blue-500/20 p-6">
            <div className="flex items-center justify-between mb-4">
              <h2 className="text-xl font-bold text-blue-300 flex items-center">
                🦾 ARM Control
                {armTelemetry?.is_moving && (
                  <span className="ml-2 flex items-center text-orange-400 text-sm">
                    <div className="w-2 h-2 bg-orange-400 rounded-full animate-pulse mr-1" />
                    Moving
                  </span>
                )}
                {connection.arrowEnabled && (
                  <span className="ml-2 text-xs bg-purple-500/20 text-purple-300 px-2 py-1 rounded">
                    🏹 Arrow
                  </span>
                )}
              </h2>
              <div className="flex space-x-2">
                <button
                  onClick={() => sendArmCommand("home")}
                  disabled={!connection.isConnected || !connection.arrowEnabled}
                  className="px-3 py-1 bg-green-600 hover:bg-green-700 rounded-lg text-sm disabled:opacity-50 transition-all hover:scale-105"
                >
                  🏠 Home
                </button>
                <button
                  onClick={() => sendArmCommand("stop")}
                  disabled={!connection.isConnected || !connection.arrowEnabled}
                  className="px-3 py-1 bg-orange-600 hover:bg-orange-700 rounded-lg text-sm disabled:opacity-50 transition-all hover:scale-105"
                >
                  ⏹️ Stop
                </button>
              </div>
            </div>

            {/* ARM Telemetry */}
            {armTelemetry && (
              <div
                className={`p-3 rounded-xl mb-4 border transition-all ${
                  isDataFresh(armTelemetry.timestamp)
                    ? "bg-green-500/10 border-green-500/30"
                    : "bg-gray-500/10 border-gray-500/30"
                }`}
              >
                <div className="text-sm font-medium mb-2 text-blue-200 flex items-center justify-between">
                  <span>End Effector Pose</span>
                  <span className="text-xs bg-purple-500/20 text-purple-300 px-2 py-1 rounded">
                    🏹 Arrow Data
                  </span>
                </div>
                {formatPoseValues(armTelemetry.end_effector_pose)}
                {armTelemetry.joint_angles && (
                  <>
                    <div className="text-sm font-medium mt-2 mb-1 text-blue-200">
                      Joint Angles
                    </div>
                    {formatJointValues(armTelemetry.joint_angles)}
                  </>
                )}
                <div className="text-xs text-gray-400 mt-2">
                  Last update:{" "}
                  {new Date(armTelemetry.timestamp).toLocaleTimeString()}
                  {armTelemetry.source && ` • Source: ${armTelemetry.source}`}
                </div>
              </div>
            )}

            {/* ARM Controls */}
            <div className="grid grid-cols-2 gap-4 mb-4">
              <div>
                <label className="text-sm text-blue-200 block mb-1">
                  Position (m)
                </label>
                <div className="grid grid-cols-3 gap-2">
                  <input
                    type="number"
                    placeholder="X"
                    value={armControls.x}
                    onChange={(e) =>
                      setArmControls((prev) => ({
                        ...prev,
                        x: parseFloat(e.target.value) || 0,
                      }))
                    }
                    step="0.001"
                    className="w-full p-2 bg-black/30 border border-blue-500/30 rounded-lg text-sm text-white placeholder-gray-400 focus:border-blue-400 focus:outline-none"
                  />
                  <input
                    type="number"
                    placeholder="Y"
                    value={armControls.y}
                    onChange={(e) =>
                      setArmControls((prev) => ({
                        ...prev,
                        y: parseFloat(e.target.value) || 0,
                      }))
                    }
                    step="0.001"
                    className="w-full p-2 bg-black/30 border border-blue-500/30 rounded-lg text-sm text-white placeholder-gray-400 focus:border-blue-400 focus:outline-none"
                  />
                  <input
                    type="number"
                    placeholder="Z"
                    value={armControls.z}
                    onChange={(e) =>
                      setArmControls((prev) => ({
                        ...prev,
                        z: parseFloat(e.target.value) || 0,
                      }))
                    }
                    step="0.001"
                    className="w-full p-2 bg-black/30 border border-blue-500/30 rounded-lg text-sm text-white placeholder-gray-400 focus:border-blue-400 focus:outline-none"
                  />
                </div>
              </div>
              <div>
                <label className="text-sm text-blue-200 block mb-1">
                  Rotation (rad)
                </label>
                <div className="grid grid-cols-3 gap-2">
                  <input
                    type="number"
                    placeholder="Roll"
                    value={armControls.roll}
                    onChange={(e) =>
                      setArmControls((prev) => ({
                        ...prev,
                        roll: parseFloat(e.target.value) || 0,
                      }))
                    }
                    step="0.1"
                    className="w-full p-2 bg-black/30 border border-blue-500/30 rounded-lg text-sm text-white placeholder-gray-400 focus:border-blue-400 focus:outline-none"
                  />
                  <input
                    type="number"
                    placeholder="Pitch"
                    value={armControls.pitch}
                    onChange={(e) =>
                      setArmControls((prev) => ({
                        ...prev,
                        pitch: parseFloat(e.target.value) || 0,
                      }))
                    }
                    step="0.1"
                    className="w-full p-2 bg-black/30 border border-blue-500/30 rounded-lg text-sm text-white placeholder-gray-400 focus:border-blue-400 focus:outline-none"
                  />
                  <input
                    type="number"
                    placeholder="Yaw"
                    value={armControls.yaw}
                    onChange={(e) =>
                      setArmControls((prev) => ({
                        ...prev,
                        yaw: parseFloat(e.target.value) || 0,
                      }))
                    }
                    step="0.1"
                    className="w-full p-2 bg-black/30 border border-blue-500/30 rounded-lg text-sm text-white placeholder-gray-400 focus:border-blue-400 focus:outline-none"
                  />
                </div>
              </div>
            </div>

            <button
              onClick={() => sendArmCommand("cartesian_move", armControls)}
              disabled={!connection.isConnected || !connection.arrowEnabled}
              className="w-full py-2 bg-blue-600 hover:bg-blue-700 rounded-lg font-medium mb-4 disabled:opacity-50 transition-all hover:scale-[1.02]"
            >
              Send Custom Move (Arrow)
            </button>

            {/* ARM Quick Controls with Keyboard Indicators */}
            <div className="text-sm text-blue-200 mb-2 flex items-center justify-between">
              <span>Quick Movements (1cm)</span>
              {keyboardEnabled && connection.arrowEnabled && (
                <span className="text-xs text-purple-300">
                  ⌨️ WASD, Q/E, R, T
                </span>
              )}
            </div>
            <div className="grid grid-cols-3 gap-2">
              <div></div>
              <button
                onClick={() =>
                  sendArmCommand("cartesian_move", {
                    x: 0.01,
                    y: 0,
                    z: 0,
                    roll: 0,
                    pitch: 0,
                    yaw: 0,
                  })
                }
                disabled={!connection.isConnected}
                className={`py-2 bg-blue-600 hover:bg-blue-700 rounded-lg text-sm disabled:opacity-50 transition-all hover:scale-105 ${
                  activeKeys["KeyW"] ? "ring-2 ring-yellow-400" : ""
                }`}
              >
                ↑ X+ {keyboardEnabled && "(W)"}
              </button>
              <div></div>

              <button
                onClick={() =>
                  sendArmCommand("cartesian_move", {
                    x: 0,
                    y: -0.01,
                    z: 0,
                    roll: 0,
                    pitch: 0,
                    yaw: 0,
                  })
                }
                disabled={!connection.isConnected}
                className={`py-2 bg-blue-600 hover:bg-blue-700 rounded-lg text-sm disabled:opacity-50 transition-all hover:scale-105 ${
                  activeKeys["KeyA"] ? "ring-2 ring-yellow-400" : ""
                }`}
              >
                ← Y- {keyboardEnabled && "(A)"}
              </button>
              <button
                onClick={() =>
                  sendArmCommand("cartesian_move", {
                    x: 0,
                    y: 0,
                    z: 0.01,
                    roll: 0,
                    pitch: 0,
                    yaw: 0,
                  })
                }
                disabled={!connection.isConnected}
                className={`py-2 bg-green-600 hover:bg-green-700 rounded-lg text-sm disabled:opacity-50 transition-all hover:scale-105 ${
                  activeKeys["KeyQ"] ? "ring-2 ring-yellow-400" : ""
                }`}
              >
                ↑ Z+ {keyboardEnabled && "(Q)"}
              </button>
              <button
                onClick={() =>
                  sendArmCommand("cartesian_move", {
                    x: 0,
                    y: 0.01,
                    z: 0,
                    roll: 0,
                    pitch: 0,
                    yaw: 0,
                  })
                }
                disabled={!connection.isConnected}
                className={`py-2 bg-blue-600 hover:bg-blue-700 rounded-lg text-sm disabled:opacity-50 transition-all hover:scale-105 ${
                  activeKeys["KeyD"] ? "ring-2 ring-yellow-400" : ""
                }`}
              >
                → Y+ {keyboardEnabled && "(D)"}
              </button>

              <div></div>
              <button
                onClick={() =>
                  sendArmCommand("cartesian_move", {
                    x: -0.01,
                    y: 0,
                    z: 0,
                    roll: 0,
                    pitch: 0,
                    yaw: 0,
                  })
                }
                disabled={!connection.isConnected}
                className={`py-2 bg-blue-600 hover:bg-blue-700 rounded-lg text-sm disabled:opacity-50 transition-all hover:scale-105 ${
                  activeKeys["KeyS"] ? "ring-2 ring-yellow-400" : ""
                }`}
              >
                ↓ X- {keyboardEnabled && "(S)"}
              </button>
              <button
                onClick={() =>
                  sendArmCommand("cartesian_move", {
                    x: 0,
                    y: 0,
                    z: -0.01,
                    roll: 0,
                    pitch: 0,
                    yaw: 0,
                  })
                }
                disabled={!connection.isConnected}
                className={`py-2 bg-orange-600 hover:bg-orange-700 rounded-lg text-sm disabled:opacity-50 transition-all hover:scale-105 ${
                  activeKeys["KeyE"] ? "ring-2 ring-yellow-400" : ""
                }`}
              >
                ↓ Z- {keyboardEnabled && "(E)"}
              </button>
            </div>
          </div>

          {/* ROVER Control Section */}
          <div className="bg-gradient-to-br from-green-900/40 to-green-800/40 backdrop-blur-sm rounded-2xl border border-green-500/20 p-6">
            <div className="flex items-center justify-between mb-4">
              <h2 className="text-xl font-bold text-green-300 flex items-center">
                🚗 ROVER Control
                {roverTelemetry && roverTelemetry.velocity > 0.1 && (
                  <span className="ml-2 flex items-center text-orange-400 text-sm">
                    <div className="w-2 h-2 bg-orange-400 rounded-full animate-pulse mr-1" />
                    Moving
                  </span>
                )}
                {connection.arrowEnabled && (
                  <span className="ml-2 text-xs bg-purple-500/20 text-purple-300 px-2 py-1 rounded">
                    🏹 Arrow
                  </span>
                )}
              </h2>
              <button
                onClick={() => sendRoverCommand(0.0, 0.0, 0.0)}
                disabled={!connection.isConnected || !connection.arrowEnabled}
                className="px-3 py-1 bg-red-600 hover:bg-red-700 rounded-lg text-sm disabled:opacity-50 transition-all hover:scale-105"
              >
                🛑 Stop
              </button>
            </div>

            {/* ROVER Telemetry with Arrow indicator */}
            {roverTelemetry && (
              <div
                className={`p-3 rounded-xl mb-4 border transition-all ${
                  isDataFresh(roverTelemetry.timestamp)
                    ? "bg-green-500/10 border-green-500/30"
                    : "bg-gray-500/10 border-gray-500/30"
                }`}
              >
                <div className="flex items-center justify-between mb-2">
                  <div className="text-sm font-medium text-green-200">
                    Rover Status
                  </div>
                  <span className="text-xs bg-purple-500/20 text-purple-300 px-2 py-1 rounded">
                    🏹 Arrow Data
                  </span>
                </div>
                <div className="grid grid-cols-3 gap-4 text-sm">
                  <div>
                    <div className="text-green-200 font-medium">Position</div>
                    <div className="text-white">
                      ({roverTelemetry.position[0].toFixed(2)},{" "}
                      {roverTelemetry.position[1].toFixed(2)})
                    </div>
                  </div>
                  <div>
                    <div className="text-green-200 font-medium">Heading</div>
                    <div className="text-white">
                      {((roverTelemetry.yaw * 180) / Math.PI).toFixed(1)}°
                    </div>
                  </div>
                  <div>
                    <div className="text-green-200 font-medium">Velocity</div>
                    <div className="text-white">
                      {roverTelemetry.velocity.toFixed(2)} m/s
                    </div>
                  </div>
                </div>
                <div className="text-xs text-gray-400 mt-2">
                  Last update:{" "}
                  {new Date(roverTelemetry.timestamp).toLocaleTimeString()}
                </div>
              </div>
            )}

            {/* ROVER Controls */}
            <div className="grid grid-cols-3 gap-4 mb-4">
              <div>
                <label className="text-sm text-green-200 block mb-1">
                  Throttle
                </label>
                <input
                  type="number"
                  value={roverControls.throttle}
                  onChange={(e) =>
                    setRoverControls((prev) => ({
                      ...prev,
                      throttle: parseFloat(e.target.value) || 0,
                    }))
                  }
                  step="0.1"
                  min="-1.0"
                  max="1.0"
                  className="w-full p-2 bg-black/30 border border-green-500/30 rounded-lg text-sm text-white focus:border-green-400 focus:outline-none"
                />
                <div className="text-xs text-gray-400 mt-1">-1.0 to 1.0</div>
              </div>
              <div>
                <label className="text-sm text-green-200 block mb-1">
                  Brake
                </label>
                <input
                  type="number"
                  value={roverControls.brake}
                  onChange={(e) =>
                    setRoverControls((prev) => ({
                      ...prev,
                      brake: parseFloat(e.target.value) || 0,
                    }))
                  }
                  step="0.1"
                  min="0.0"
                  max="1.0"
                  className="w-full p-2 bg-black/30 border border-green-500/30 rounded-lg text-sm text-white focus:border-green-400 focus:outline-none"
                />
                <div className="text-xs text-gray-400 mt-1">0.0 to 1.0</div>
              </div>
              <div>
                <label className="text-sm text-green-200 block mb-1">
                  Steering
                </label>
                <input
                  type="number"
                  value={roverControls.steering}
                  onChange={(e) =>
                    setRoverControls((prev) => ({
                      ...prev,
                      steering: parseFloat(e.target.value) || 0,
                    }))
                  }
                  step="1.0"
                  min="-15.0"
                  max="15.0"
                  className="w-full p-2 bg-black/30 border border-green-500/30 rounded-lg text-sm text-white focus:border-green-400 focus:outline-none"
                />
                <div className="text-xs text-gray-400 mt-1">-15° to 15°</div>
              </div>
            </div>

            <button
              onClick={() =>
                sendRoverCommand(
                  roverControls.throttle,
                  roverControls.brake,
                  roverControls.steering,
                )
              }
              disabled={!connection.isConnected}
              className="w-full py-2 bg-green-600 hover:bg-green-700 rounded-lg font-medium mb-4 disabled:opacity-50 transition-all hover:scale-[1.02]"
            >
              Send Custom Command
            </button>

            {/* ROVER Quick Controls with Keyboard Indicators */}
            <div className="text-sm text-green-200 mb-2 flex items-center justify-between">
              <span>Quick Movements</span>
              {keyboardEnabled && (
                <span className="text-xs text-purple-300">
                  ⌨️ Arrow Keys, J/L, Space, X
                </span>
              )}
            </div>
            <div className="grid grid-cols-3 gap-2">
              <button
                onClick={() => sendRoverCommand(0.2, 0.0, 5.0)}
                disabled={!connection.isConnected}
                className={`py-2 bg-green-600 hover:bg-green-700 rounded-lg text-sm disabled:opacity-50 transition-all hover:scale-105 ${
                  activeKeys["ArrowLeft"] ? "ring-2 ring-yellow-400" : ""
                }`}
              >
                ↖ Left {keyboardEnabled && "(←)"}
              </button>
              <button
                onClick={() => sendRoverCommand(0.3, 0.0, 0.0)}
                disabled={!connection.isConnected}
                className={`py-2 bg-green-600 hover:bg-green-700 rounded-lg text-sm disabled:opacity-50 transition-all hover:scale-105 ${
                  activeKeys["ArrowUp"] ? "ring-2 ring-yellow-400" : ""
                }`}
              >
                ↑ Forward {keyboardEnabled && "(↑)"}
              </button>
              <button
                onClick={() => sendRoverCommand(0.2, 0.0, -5.0)}
                disabled={!connection.isConnected}
                className={`py-2 bg-green-600 hover:bg-green-700 rounded-lg text-sm disabled:opacity-50 transition-all hover:scale-105 ${
                  activeKeys["ArrowRight"] ? "ring-2 ring-yellow-400" : ""
                }`}
              >
                ↗ Right {keyboardEnabled && "(→)"}
              </button>

              <button
                onClick={() => sendRoverCommand(0.0, 0.0, 5.0)}
                disabled={!connection.isConnected}
                className={`py-2 bg-blue-600 hover:bg-blue-700 rounded-lg text-sm disabled:opacity-50 transition-all hover:scale-105 ${
                  activeKeys["KeyJ"] ? "ring-2 ring-yellow-400" : ""
                }`}
              >
                ← Steer {keyboardEnabled && "(J)"}
              </button>
              <button
                onClick={() => sendRoverCommand(0.0, 1.0, 0.0)}
                disabled={!connection.isConnected}
                className={`py-2 bg-orange-600 hover:bg-orange-700 rounded-lg text-sm disabled:opacity-50 transition-all hover:scale-105 ${
                  activeKeys["Space"] ? "ring-2 ring-yellow-400" : ""
                }`}
              >
                🛑 Brake {keyboardEnabled && "(Space)"}
              </button>
              <button
                onClick={() => sendRoverCommand(0.0, 0.0, -5.0)}
                disabled={!connection.isConnected}
                className={`py-2 bg-blue-600 hover:bg-blue-700 rounded-lg text-sm disabled:opacity-50 transition-all hover:scale-105 ${
                  activeKeys["KeyL"] ? "ring-2 ring-yellow-400" : ""
                }`}
              >
                → Steer {keyboardEnabled && "(L)"}
              </button>

              <div></div>
              <button
                onClick={() => sendRoverCommand(-0.2, 0.0, 0.0)}
                disabled={!connection.isConnected}
                className={`py-2 bg-yellow-600 hover:bg-yellow-700 rounded-lg text-sm disabled:opacity-50 transition-all hover:scale-105 ${
                  activeKeys["ArrowDown"] ? "ring-2 ring-yellow-400" : ""
                }`}
              >
                ↓ Reverse {keyboardEnabled && "(↓)"}
              </button>
              <button
                onClick={() => sendRoverCommand(0.0, 0.0, 0.0)}
                disabled={!connection.isConnected}
                className={`py-2 bg-red-600 hover:bg-red-700 rounded-lg text-sm disabled:opacity-50 transition-all hover:scale-105 ${
                  activeKeys["KeyX"] ? "ring-2 ring-yellow-400" : ""
                }`}
              >
                🛑 Stop {keyboardEnabled && "(X)"}
              </button>
            </div>
          </div>
        </div>

        {/* Additional Controls Bar */}
        <div className="mt-6 bg-black/20 backdrop-blur-sm rounded-2xl border border-white/10 p-4">
          <div className="flex justify-center items-center space-x-4">
            <button
              onClick={() => socketRef.current?.emit("get_status")}
              disabled={!connection.isConnected}
              className="px-4 py-2 bg-purple-600 hover:bg-purple-700 rounded-lg text-sm disabled:opacity-50 transition-all hover:scale-105"
            >
              📊 System Status
            </button>

            <button
              onClick={() => loadSchemas()}
              disabled={!connection.isConnected}
              className="px-4 py-2 bg-indigo-600 hover:bg-indigo-700 rounded-lg text-sm disabled:opacity-50 transition-all hover:scale-105"
            >
              🏹 Reload Schemas
            </button>

            <button
              onClick={() =>
                socketRef.current?.emit("ping", { timestamp: Date.now() })
              }
              disabled={!connection.isConnected}
              className="px-4 py-2 bg-cyan-600 hover:bg-cyan-700 rounded-lg text-sm disabled:opacity-50 transition-all hover:scale-105"
            >
              📡 Ping Test
            </button>

            <div className="text-sm text-gray-400 hidden md:block">
              {connection.clientId &&
                `Client: ${connection.clientId.slice(0, 8)}...`}
              {connection.arrowEnabled && (
                <span className="ml-2 text-purple-300">| 🏹 Arrow Active</span>
              )}
            </div>

            {/* Emergency Stop with Keyboard Indicator */}
            <button
              onClick={emergencyStopAll}
              disabled={!connection.isConnected || !connection.arrowEnabled}
              className={`px-4 py-2 bg-red-600 hover:bg-red-700 rounded-lg text-sm font-bold disabled:opacity-50 disabled:cursor-not-allowed transition-all hover:scale-105 border-2 border-red-400 ${
                activeKeys["Escape"] ? "ring-2 ring-yellow-400" : ""
              }`}
            >
              🛑 Emergency Stop{" "}
              {keyboardEnabled && connection.arrowEnabled && "(ESC)"}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
};

export default RoboRoverController;
