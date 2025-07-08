import React, { useState, useEffect, useCallback, useRef } from 'react';
import { io, Socket } from 'socket.io-client';

// Types
interface ArmTelemetry {
    type: 'arm_telemetry';
    end_effector_pose: number[];
    joint_angles?: number[];
    joint_velocities?: number[];
    is_moving: boolean;
    source?: string;
    timestamp: number;
}

interface RoverTelemetry {
    type: 'rover_telemetry';
    position: [number, number];
    yaw: number;
    velocity: number;
    timestamp: number;
}

interface LogEntry {
    timestamp: string;
    message: string;
    type: 'info' | 'success' | 'error' | 'warning';
}

interface ConnectionState {
    isConnected: boolean;
    clientId: string | null;
    commandsSent: number;
    commandsReceived: number;
}

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
    const [roverTelemetry, setRoverTelemetry] = useState<RoverTelemetry | null>(null);
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

    // Refs
    const socketRef = useRef<Socket | null>(null);
    const logEndRef = useRef<HTMLDivElement>(null);

    // Utility functions
    const addLog = useCallback((message: string, type: LogEntry['type'] = 'info') => {
        const timestamp = new Date().toLocaleTimeString();
        setLogs(prev => [...prev.slice(-49), { timestamp, message, type }]);
    }, []);

    const isDataFresh = (timestamp: number) => {
        return (Date.now() - timestamp) < 2000;
    };

    // Socket connection management
    const connect = useCallback(() => {
        if (socketRef.current) {
            socketRef.current.disconnect();
        }

        addLog('Connecting to Web Bridge...', 'info');
        const socket = io('http://127.0.0.1:8080', {
            forceNew: true,
            transports: ['websocket', 'polling']
        });

        socket.on('connect', () => {
            setConnection(prev => ({
                ...prev,
                isConnected: true,
                clientId: socket.id || null,
            }));
            addLog('Connected successfully', 'success');
        });

        socket.on('disconnect', () => {
            setConnection(prev => ({
                ...prev,
                isConnected: false,
                clientId: null,
            }));
            addLog('Disconnected from Web Bridge', 'error');
        });

        socket.on('status', (data) => {
            setConnection(prev => ({ ...prev, commandsReceived: prev.commandsReceived + 1 }));
            addLog(`Status: ${data.message || JSON.stringify(data)}`, 'info');
        });

        socket.on('error', (data) => {
            setConnection(prev => ({ ...prev, commandsReceived: prev.commandsReceived + 1 }));
            addLog(`Error: ${data.message || JSON.stringify(data)}`, 'error');
        });

        socket.on('telemetry', (data) => {
            setConnection(prev => ({ ...prev, commandsReceived: prev.commandsReceived + 1 }));

            if (data.type === 'arm_telemetry') {
                setArmTelemetry(data);
            } else if (data.type === 'rover_telemetry') {
                setRoverTelemetry(data);
            }
        });

        socket.on('pong', (data) => {
            addLog(`Ping: ${data.timestamp ? `${Date.now() - data.timestamp}ms` : 'OK'}`, 'success');
        });

        socket.on('connect_error', (error) => {
            addLog(`Connection error: ${error.message}`, 'error');
        });

        socketRef.current = socket;
    }, [addLog]);

    const disconnect = useCallback(() => {
        if (socketRef.current) {
            socketRef.current.disconnect();
            socketRef.current = null;
        }
    }, []);

    // Command functions
    const sendArmCommand = useCallback((type: string, params = {}) => {
        if (!socketRef.current?.connected) {
            addLog('Cannot send ARM command - not connected', 'error');
            return;
        }

        const command = { type, ...params };
        socketRef.current.emit('arm_command', command);
        setConnection(prev => ({ ...prev, commandsSent: prev.commandsSent + 1 }));
        addLog(`ARM: ${type}`, 'info');
    }, [addLog]);

    const sendRoverCommand = useCallback((throttle: number, brake: number, steering_angle: number) => {
        if (!socketRef.current?.connected) {
            addLog('Cannot send ROVER command - not connected', 'error');
            return;
        }

        const command = { throttle, brake, steering_angle };
        socketRef.current.emit('rover_command', command);
        setConnection(prev => ({ ...prev, commandsSent: prev.commandsSent + 1 }));
        addLog(`ROVER: T${throttle.toFixed(1)} B${brake.toFixed(1)} S${steering_angle.toFixed(1)}°`, 'info');
    }, [addLog]);

    // Emergency stop for both systems
    const emergencyStopAll = useCallback(() => {
        sendArmCommand('emergency_stop');
        sendRoverCommand(0.0, 1.0, 0.0);
        addLog('EMERGENCY STOP - ALL SYSTEMS', 'error');
    }, [sendArmCommand, sendRoverCommand, addLog]);

    // Component mount/unmount
    useEffect(() => {
        addLog('Robo Rover Controller initialized');

        return () => {
            disconnect();
        };
    }, [addLog, disconnect]);

    // Auto-scroll logs
    useEffect(() => {
        if (showLogs) {
            logEndRef.current?.scrollIntoView({ behavior: 'smooth' });
        }
    }, [logs, showLogs]);

    // Responsive breakpoint detection
    useEffect(() => {
        const handleResize = () => {
            setIsCompact(window.innerWidth < 768);
        };

        handleResize();
        window.addEventListener('resize', handleResize);
        return () => window.removeEventListener('resize', handleResize);
    }, []);

    // Format helpers
    const formatPoseValues = (pose: number[]) => {
        if (!pose || !Array.isArray(pose) || pose.length < 6) return null;
        const labels = ['X', 'Y', 'Z', 'R', 'P', 'Y'];
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
                            <div className={`flex items-center space-x-2 px-3 py-1 rounded-full text-sm ${
                                connection.isConnected
                                    ? 'bg-green-500/20 text-green-400 border border-green-500/30'
                                    : 'bg-red-500/20 text-red-400 border border-red-500/30'
                            }`}>
                                <div className={`w-2 h-2 rounded-full ${connection.isConnected ? 'bg-green-400' : 'bg-red-400'}`} />
                                <span>{connection.isConnected ? 'Connected' : 'Disconnected'}</span>
                            </div>
                        </div>

                        <div className="flex items-center space-x-2">
                            <div className="text-xs text-gray-400 hidden sm:block">
                                ↑{connection.commandsSent} ↓{connection.commandsReceived}
                            </div>

                            <button
                                onClick={() => setShowLogs(!showLogs)}
                                className="px-3 py-1 bg-gray-700 hover:bg-gray-600 rounded-lg text-sm transition-colors"
                            >
                                {showLogs ? 'Hide' : 'Logs'}
                            </button>

                            <button
                                onClick={connection.isConnected ? disconnect : connect}
                                className={`px-4 py-1 rounded-lg text-sm font-medium transition-all ${
                                    connection.isConnected
                                        ? 'bg-red-600 hover:bg-red-700 hover:scale-105'
                                        : 'bg-blue-600 hover:bg-blue-700 hover:scale-105'
                                }`}
                            >
                                {connection.isConnected ? 'Disconnect' : 'Connect'}
                            </button>

                            <button
                                onClick={emergencyStopAll}
                                disabled={!connection.isConnected}
                                className="px-4 py-1 bg-red-600 hover:bg-red-700 rounded-lg text-sm font-bold disabled:opacity-50 disabled:cursor-not-allowed transition-all hover:scale-105 border-2 border-red-400"
                            >
                                🛑 E-STOP
                            </button>
                        </div>
                    </div>
                </div>
            </div>

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
                                        log.type === 'error' ? 'text-red-400' :
                                            log.type === 'success' ? 'text-green-400' :
                                                log.type === 'warning' ? 'text-orange-400' :
                                                    'text-gray-300'
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
                <div className={`grid gap-6 ${isCompact ? 'grid-cols-1' : 'grid-cols-1 lg:grid-cols-2'}`}>

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
                            </h2>
                            <div className="flex space-x-2">
                                <button
                                    onClick={() => sendArmCommand('home')}
                                    disabled={!connection.isConnected}
                                    className="px-3 py-1 bg-green-600 hover:bg-green-700 rounded-lg text-sm disabled:opacity-50 transition-all hover:scale-105"
                                >
                                    🏠 Home
                                </button>
                                <button
                                    onClick={() => sendArmCommand('stop')}
                                    disabled={!connection.isConnected}
                                    className="px-3 py-1 bg-orange-600 hover:bg-orange-700 rounded-lg text-sm disabled:opacity-50 transition-all hover:scale-105"
                                >
                                    ⏹️ Stop
                                </button>
                            </div>
                        </div>

                        {/* ARM Telemetry */}
                        {armTelemetry && (
                            <div className={`p-3 rounded-xl mb-4 border transition-all ${
                                isDataFresh(armTelemetry.timestamp)
                                    ? 'bg-green-500/10 border-green-500/30'
                                    : 'bg-gray-500/10 border-gray-500/30'
                            }`}>
                                <div className="text-sm font-medium mb-2 text-blue-200">End Effector Pose</div>
                                {formatPoseValues(armTelemetry.end_effector_pose)}
                                {armTelemetry.joint_angles && (
                                    <>
                                        <div className="text-sm font-medium mt-2 mb-1 text-blue-200">Joint Angles</div>
                                        {formatJointValues(armTelemetry.joint_angles)}
                                    </>
                                )}
                                <div className="text-xs text-gray-400 mt-2">
                                    Last update: {new Date(armTelemetry.timestamp).toLocaleTimeString()}
                                </div>
                            </div>
                        )}

                        {/* ARM Controls */}
                        <div className="grid grid-cols-2 gap-4 mb-4">
                            <div>
                                <label className="text-sm text-blue-200 block mb-1">Position (m)</label>
                                <div className="grid grid-cols-3 gap-2">
                                    <input
                                        type="number"
                                        placeholder="X"
                                        value={armControls.x}
                                        onChange={(e) => setArmControls(prev => ({ ...prev, x: parseFloat(e.target.value) || 0 }))}
                                        step="0.001"
                                        className="w-full p-2 bg-black/30 border border-blue-500/30 rounded-lg text-sm text-white placeholder-gray-400 focus:border-blue-400 focus:outline-none"
                                    />
                                    <input
                                        type="number"
                                        placeholder="Y"
                                        value={armControls.y}
                                        onChange={(e) => setArmControls(prev => ({ ...prev, y: parseFloat(e.target.value) || 0 }))}
                                        step="0.001"
                                        className="w-full p-2 bg-black/30 border border-blue-500/30 rounded-lg text-sm text-white placeholder-gray-400 focus:border-blue-400 focus:outline-none"
                                    />
                                    <input
                                        type="number"
                                        placeholder="Z"
                                        value={armControls.z}
                                        onChange={(e) => setArmControls(prev => ({ ...prev, z: parseFloat(e.target.value) || 0 }))}
                                        step="0.001"
                                        className="w-full p-2 bg-black/30 border border-blue-500/30 rounded-lg text-sm text-white placeholder-gray-400 focus:border-blue-400 focus:outline-none"
                                    />
                                </div>
                            </div>
                            <div>
                                <label className="text-sm text-blue-200 block mb-1">Rotation (rad)</label>
                                <div className="grid grid-cols-3 gap-2">
                                    <input
                                        type="number"
                                        placeholder="Roll"
                                        value={armControls.roll}
                                        onChange={(e) => setArmControls(prev => ({ ...prev, roll: parseFloat(e.target.value) || 0 }))}
                                        step="0.1"
                                        className="w-full p-2 bg-black/30 border border-blue-500/30 rounded-lg text-sm text-white placeholder-gray-400 focus:border-blue-400 focus:outline-none"
                                    />
                                    <input
                                        type="number"
                                        placeholder="Pitch"
                                        value={armControls.pitch}
                                        onChange={(e) => setArmControls(prev => ({ ...prev, pitch: parseFloat(e.target.value) || 0 }))}
                                        step="0.1"
                                        className="w-full p-2 bg-black/30 border border-blue-500/30 rounded-lg text-sm text-white placeholder-gray-400 focus:border-blue-400 focus:outline-none"
                                    />
                                    <input
                                        type="number"
                                        placeholder="Yaw"
                                        value={armControls.yaw}
                                        onChange={(e) => setArmControls(prev => ({ ...prev, yaw: parseFloat(e.target.value) || 0 }))}
                                        step="0.1"
                                        className="w-full p-2 bg-black/30 border border-blue-500/30 rounded-lg text-sm text-white placeholder-gray-400 focus:border-blue-400 focus:outline-none"
                                    />
                                </div>
                            </div>
                        </div>

                        <button
                            onClick={() => sendArmCommand('cartesian_move', armControls)}
                            disabled={!connection.isConnected}
                            className="w-full py-2 bg-blue-600 hover:bg-blue-700 rounded-lg font-medium mb-4 disabled:opacity-50 transition-all hover:scale-[1.02]"
                        >
                            Send Custom Move
                        </button>

                        {/* ARM Quick Controls */}
                        <div className="text-sm text-blue-200 mb-2">Quick Movements (1cm)</div>
                        <div className="grid grid-cols-3 gap-2">
                            <div></div>
                            <button
                                onClick={() => sendArmCommand('cartesian_move', { x: 0.01, y: 0, z: 0, roll: 0, pitch: 0, yaw: 0 })}
                                disabled={!connection.isConnected}
                                className="py-2 bg-blue-600 hover:bg-blue-700 rounded-lg text-sm disabled:opacity-50 transition-all hover:scale-105"
                            >
                                ↑ X+
                            </button>
                            <div></div>

                            <button
                                onClick={() => sendArmCommand('cartesian_move', { x: 0, y: -0.01, z: 0, roll: 0, pitch: 0, yaw: 0 })}
                                disabled={!connection.isConnected}
                                className="py-2 bg-blue-600 hover:bg-blue-700 rounded-lg text-sm disabled:opacity-50 transition-all hover:scale-105"
                            >
                                ← Y-
                            </button>
                            <button
                                onClick={() => sendArmCommand('cartesian_move', { x: 0, y: 0, z: 0.01, roll: 0, pitch: 0, yaw: 0 })}
                                disabled={!connection.isConnected}
                                className="py-2 bg-green-600 hover:bg-green-700 rounded-lg text-sm disabled:opacity-50 transition-all hover:scale-105"
                            >
                                ↑ Z+
                            </button>
                            <button
                                onClick={() => sendArmCommand('cartesian_move', { x: 0, y: 0.01, z: 0, roll: 0, pitch: 0, yaw: 0 })}
                                disabled={!connection.isConnected}
                                className="py-2 bg-blue-600 hover:bg-blue-700 rounded-lg text-sm disabled:opacity-50 transition-all hover:scale-105"
                            >
                                → Y+
                            </button>

                            <div></div>
                            <button
                                onClick={() => sendArmCommand('cartesian_move', { x: -0.01, y: 0, z: 0, roll: 0, pitch: 0, yaw: 0 })}
                                disabled={!connection.isConnected}
                                className="py-2 bg-blue-600 hover:bg-blue-700 rounded-lg text-sm disabled:opacity-50 transition-all hover:scale-105"
                            >
                                ↓ X-
                            </button>
                            <button
                                onClick={() => sendArmCommand('cartesian_move', { x: 0, y: 0, z: -0.01, roll: 0, pitch: 0, yaw: 0 })}
                                disabled={!connection.isConnected}
                                className="py-2 bg-orange-600 hover:bg-orange-700 rounded-lg text-sm disabled:opacity-50 transition-all hover:scale-105"
                            >
                                ↓ Z-
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
                            </h2>
                            <button
                                onClick={() => sendRoverCommand(0.0, 0.0, 0.0)}
                                disabled={!connection.isConnected}
                                className="px-3 py-1 bg-red-600 hover:bg-red-700 rounded-lg text-sm disabled:opacity-50 transition-all hover:scale-105"
                            >
                                🛑 Stop
                            </button>
                        </div>

                        {/* ROVER Telemetry */}
                        {roverTelemetry && (
                            <div className={`p-3 rounded-xl mb-4 border transition-all ${
                                isDataFresh(roverTelemetry.timestamp)
                                    ? 'bg-green-500/10 border-green-500/30'
                                    : 'bg-gray-500/10 border-gray-500/30'
                            }`}>
                                <div className="grid grid-cols-3 gap-4 text-sm">
                                    <div>
                                        <div className="text-green-200 font-medium">Position</div>
                                        <div className="text-white">
                                            ({roverTelemetry.position[0].toFixed(2)}, {roverTelemetry.position[1].toFixed(2)})
                                        </div>
                                    </div>
                                    <div>
                                        <div className="text-green-200 font-medium">Heading</div>
                                        <div className="text-white">{(roverTelemetry.yaw * 180 / Math.PI).toFixed(1)}°</div>
                                    </div>
                                    <div>
                                        <div className="text-green-200 font-medium">Velocity</div>
                                        <div className="text-white">{roverTelemetry.velocity.toFixed(2)} m/s</div>
                                    </div>
                                </div>
                                <div className="text-xs text-gray-400 mt-2">
                                    Last update: {new Date(roverTelemetry.timestamp).toLocaleTimeString()}
                                </div>
                            </div>
                        )}

                        {/* ROVER Controls */}
                        <div className="grid grid-cols-3 gap-4 mb-4">
                            <div>
                                <label className="text-sm text-green-200 block mb-1">Throttle</label>
                                <input
                                    type="number"
                                    value={roverControls.throttle}
                                    onChange={(e) => setRoverControls(prev => ({ ...prev, throttle: parseFloat(e.target.value) || 0 }))}
                                    step="0.1"
                                    min="-1.0"
                                    max="1.0"
                                    className="w-full p-2 bg-black/30 border border-green-500/30 rounded-lg text-sm text-white focus:border-green-400 focus:outline-none"
                                />
                                <div className="text-xs text-gray-400 mt-1">-1.0 to 1.0</div>
                            </div>
                            <div>
                                <label className="text-sm text-green-200 block mb-1">Brake</label>
                                <input
                                    type="number"
                                    value={roverControls.brake}
                                    onChange={(e) => setRoverControls(prev => ({ ...prev, brake: parseFloat(e.target.value) || 0 }))}
                                    step="0.1"
                                    min="0.0"
                                    max="1.0"
                                    className="w-full p-2 bg-black/30 border border-green-500/30 rounded-lg text-sm text-white focus:border-green-400 focus:outline-none"
                                />
                                <div className="text-xs text-gray-400 mt-1">0.0 to 1.0</div>
                            </div>
                            <div>
                                <label className="text-sm text-green-200 block mb-1">Steering</label>
                                <input
                                    type="number"
                                    value={roverControls.steering}
                                    onChange={(e) => setRoverControls(prev => ({ ...prev, steering: parseFloat(e.target.value) || 0 }))}
                                    step="1.0"
                                    min="-15.0"
                                    max="15.0"
                                    className="w-full p-2 bg-black/30 border border-green-500/30 rounded-lg text-sm text-white focus:border-green-400 focus:outline-none"
                                />
                                <div className="text-xs text-gray-400 mt-1">-15° to 15°</div>
                            </div>
                        </div>

                        <button
                            onClick={() => sendRoverCommand(roverControls.throttle, roverControls.brake, roverControls.steering)}
                            disabled={!connection.isConnected}
                            className="w-full py-2 bg-green-600 hover:bg-green-700 rounded-lg font-medium mb-4 disabled:opacity-50 transition-all hover:scale-[1.02]"
                        >
                            Send Custom Command
                        </button>

                        {/* ROVER Quick Controls */}
                        <div className="text-sm text-green-200 mb-2">Quick Movements</div>
                        <div className="grid grid-cols-3 gap-2">
                            <button
                                onClick={() => sendRoverCommand(0.2, 0.0, 5.0)}
                                disabled={!connection.isConnected}
                                className="py-2 bg-green-600 hover:bg-green-700 rounded-lg text-sm disabled:opacity-50 transition-all hover:scale-105"
                            >
                                ↖ Left
                            </button>
                            <button
                                onClick={() => sendRoverCommand(0.3, 0.0, 0.0)}
                                disabled={!connection.isConnected}
                                className="py-2 bg-green-600 hover:bg-green-700 rounded-lg text-sm disabled:opacity-50 transition-all hover:scale-105"
                            >
                                ↑ Forward
                            </button>
                            <button
                                onClick={() => sendRoverCommand(0.2, 0.0, -5.0)}
                                disabled={!connection.isConnected}
                                className="py-2 bg-green-600 hover:bg-green-700 rounded-lg text-sm disabled:opacity-50 transition-all hover:scale-105"
                            >
                                ↗ Right
                            </button>

                            <button
                                onClick={() => sendRoverCommand(0.0, 0.0, 5.0)}
                                disabled={!connection.isConnected}
                                className="py-2 bg-blue-600 hover:bg-blue-700 rounded-lg text-sm disabled:opacity-50 transition-all hover:scale-105"
                            >
                                ← Steer
                            </button>
                            <button
                                onClick={() => sendRoverCommand(0.0, 1.0, 0.0)}
                                disabled={!connection.isConnected}
                                className="py-2 bg-orange-600 hover:bg-orange-700 rounded-lg text-sm disabled:opacity-50 transition-all hover:scale-105"
                            >
                                🛑 Brake
                            </button>
                            <button
                                onClick={() => sendRoverCommand(0.0, 0.0, -5.0)}
                                disabled={!connection.isConnected}
                                className="py-2 bg-blue-600 hover:bg-blue-700 rounded-lg text-sm disabled:opacity-50 transition-all hover:scale-105"
                            >
                                → Steer
                            </button>

                            <div></div>
                            <button
                                onClick={() => sendRoverCommand(-0.2, 0.0, 0.0)}
                                disabled={!connection.isConnected}
                                className="py-2 bg-yellow-600 hover:bg-yellow-700 rounded-lg text-sm disabled:opacity-50 transition-all hover:scale-105"
                            >
                                ↓ Reverse
                            </button>
                            <div></div>
                        </div>
                    </div>
                </div>

                {/* Additional Controls Bar */}
                <div className="mt-6 bg-black/20 backdrop-blur-sm rounded-2xl border border-white/10 p-4">
                    <div className="flex justify-center items-center space-x-4">
                        <button
                            onClick={() => socketRef.current?.emit('get_status')}
                            disabled={!connection.isConnected}
                            className="px-4 py-2 bg-purple-600 hover:bg-purple-700 rounded-lg text-sm disabled:opacity-50 transition-all hover:scale-105"
                        >
                            📊 System Status
                        </button>

                        <button
                            onClick={() => socketRef.current?.emit('ping', { timestamp: Date.now() })}
                            disabled={!connection.isConnected}
                            className="px-4 py-2 bg-cyan-600 hover:bg-cyan-700 rounded-lg text-sm disabled:opacity-50 transition-all hover:scale-105"
                        >
                            📡 Ping Test
                        </button>

                        <div className="text-sm text-gray-400 hidden md:block">
                            {connection.clientId && `Client: ${connection.clientId.slice(0, 8)}...`}
                        </div>
                    </div>
                </div>
            </div>
        </div>
    );
};

export default RoboRoverController;