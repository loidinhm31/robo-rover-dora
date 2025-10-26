import React, { useEffect, useRef, useState } from "react";
import { Camera, Eye, EyeOff, Maximize2, Minimize2 } from "lucide-react";

interface VideoFrame {
  timestamp: number;
  frame_id: number;
  width: number;
  height: number;
  format: string;
  quality: number;
  data: string; // base64 encoded JPEG
  overlay_data?: {
    rover_position?: [number, number];
    rover_velocity?: number;
    arm_position?: number[];
    battery_level?: number;
    signal_strength?: number;
    timestamp_text: string;
  };
}

interface VideoStats {
  timestamp: number;
  frames_processed: number;
  frames_dropped: number;
  avg_frame_size_kb: number;
  avg_processing_time_ms: number;
  current_fps: number;
  bandwidth_kbps: number;
}

interface CameraViewerProps {
  isConnected: boolean;
  socket: any; // Socket.IO socket
  onClose?: () => void;
}

export const CameraViewer: React.FC<CameraViewerProps> = ({
                                                            isConnected,
                                                            socket,
                                                            onClose,
                                                          }) => {
  const [isStreaming, setIsStreaming] = useState(false);
  const [currentFrame, setCurrentFrame] = useState<VideoFrame | null>(null);
  const [videoStats, setVideoStats] = useState<VideoStats | null>(null);
  const [isFullscreen, setIsFullscreen] = useState(false);
  const [fps, setFps] = useState(0);

  const canvasRef = useRef<HTMLCanvasElement>(null);
  const imageRef = useRef<HTMLImageElement | null>(null);
  const frameCountRef = useRef(0);
  const lastFpsUpdateRef = useRef(Date.now());

  // Start/Stop video streaming
  const toggleStreaming = () => {
    if (!socket || !isConnected) return;

    if (isStreaming) {
      socket.emit("video_control", {
        command: "stop",
      });
      setIsStreaming(false);
    } else {
      socket.emit("video_control", {
        command: "start",
        max_fps: 30,
      });
      setIsStreaming(true);
    }
  };

  // Handle incoming video frames
  useEffect(() => {
    if (!socket || !isConnected) return;

    const handleVideoFrame = (frame: VideoFrame) => {
      setCurrentFrame(frame);

      // Update FPS counter
      frameCountRef.current++;
      const now = Date.now();
      const elapsed = now - lastFpsUpdateRef.current;

      if (elapsed >= 1000) {
        setFps(Math.round((frameCountRef.current / elapsed) * 1000));
        frameCountRef.current = 0;
        lastFpsUpdateRef.current = now;
      }

      // Render frame to canvas
      if (canvasRef.current) {
        const ctx = canvasRef.current.getContext("2d");
        if (ctx && frame.data) {
          // Create image from base64 data
          const img = new Image();
          img.onload = () => {
            // Set canvas size to match frame
            canvasRef.current!.width = frame.width;
            canvasRef.current!.height = frame.height;

            // Draw image
            ctx.drawImage(img, 0, 0, frame.width, frame.height);

            // Draw additional overlay info if needed
            if (frame.overlay_data) {
              ctx.font = "14px monospace";
              ctx.fillStyle = "#00ffff";
              ctx.strokeStyle = "#000000";
              ctx.lineWidth = 2;

              const text = `FPS: ${fps} | Frame: ${frame.frame_id}`;
              ctx.strokeText(text, 10, 25);
              ctx.fillText(text, 10, 25);
            }
          };
          img.src = `data:image/jpeg;base64,${frame.data}`;
          imageRef.current = img;
        }
      }
    };

    const handleVideoStats = (stats: VideoStats) => {
      setVideoStats(stats);
    };

    const handleVideoStatus = (status: { streaming: boolean; fps?: number }) => {
      setIsStreaming(status.streaming);
    };

    socket.on("video_frame", handleVideoFrame);
    socket.on("video_stats", handleVideoStats);
    socket.on("video_status", handleVideoStatus);

    return () => {
      socket.off("video_frame", handleVideoFrame);
      socket.off("video_stats", handleVideoStats);
      socket.off("video_status", handleVideoStatus);
    };
  }, [socket, isConnected, fps]);

  // Auto-start streaming when connected
  useEffect(() => {
    if (isConnected && socket && !isStreaming) {
      // Auto-start after a short delay
      const timer = setTimeout(() => {
        socket.emit("video_control", {
          command: "start",
          max_fps: 30,
        });
        setIsStreaming(true);
      }, 500);
      return () => clearTimeout(timer);
    }
  }, [isConnected, socket]);

  // Cleanup on unmount
  useEffect(() => {
    return () => {
      if (socket && isStreaming) {
        socket.emit("video_control", { command: "stop" });
      }
    };
  }, [socket, isStreaming]);

  return (
    <div className={`glass-card rounded-3xl shadow-2xl p-4 md:p-6 ${isFullscreen ? "fixed inset-4 z-50" : ""}`}>
      {/* Header */}
      <div className="flex items-center justify-between mb-4">
        <div className="flex items-center gap-2">
          <Camera className="w-6 h-6 md:w-8 md:h-8 text-green-400" />
          <h2 className="text-2xl md:text-3xl font-bold text-white">CAMERA</h2>
          {isStreaming && (
            <div className="flex items-center gap-1">
              <div className="w-2 h-2 bg-red-500 rounded-full animate-pulse" />
              <span className="text-xs text-red-400 font-semibold">LIVE</span>
            </div>
          )}
        </div>

        <div className="flex items-center gap-2">
          <button
            onClick={() => setIsFullscreen(!isFullscreen)}
            className="p-2 glass-card-light rounded-lg hover:bg-white/20 transition-all"
            title={isFullscreen ? "Exit fullscreen" : "Fullscreen"}
          >
            {isFullscreen ? (
              <Minimize2 className="w-5 h-5 text-white" />
            ) : (
              <Maximize2 className="w-5 h-5 text-white" />
            )}
          </button>

          {onClose && (
            <button
              onClick={onClose}
              className="p-2 glass-card-light rounded-lg hover:bg-white/20 transition-all"
              title="Close camera"
            >
              <EyeOff className="w-5 h-5 text-white" />
            </button>
          )}
        </div>
      </div>

      {/* Video Display */}
      <div className="glass-card-light rounded-2xl overflow-hidden relative">
        {!isConnected ? (
          <div className="aspect-video bg-gray-900/50 flex items-center justify-center">
            <div className="text-center">
              <Camera className="w-16 h-16 text-white/30 mx-auto mb-2" />
              <p className="text-white/50">Not Connected</p>
            </div>
          </div>
        ) : !isStreaming ? (
          <div className="aspect-video bg-gray-900/50 flex items-center justify-center">
            <div className="text-center">
              <Camera className="w-16 h-16 text-white/30 mx-auto mb-2" />
              <p className="text-white/50 mb-3">Camera Ready</p>
              <button
                onClick={toggleStreaming}
                className="btn-gradient px-6 py-2 rounded-xl"
              >
                Start Stream
              </button>
            </div>
          </div>
        ) : (
          <>
            <canvas
              ref={canvasRef}
              className="w-full h-auto bg-black"
              style={{ maxHeight: isFullscreen ? "calc(100vh - 200px)" : "500px" }}
            />

            {/* Stats Overlay */}
            {videoStats && (
              <div className="absolute top-2 right-2 glass-card-light rounded-lg p-2 text-xs">
                <div className="text-white/90 font-mono space-y-1">
                  <div>FPS: {fps}</div>
                  <div>Quality: {currentFrame?.quality || 0}%</div>
                  <div>Size: {videoStats.avg_frame_size_kb.toFixed(1)} KB</div>
                  <div>Bandwidth: {videoStats.bandwidth_kbps.toFixed(0)} Kbps</div>
                </div>
              </div>
            )}

            {/* Frame Info Overlay */}
            {currentFrame?.overlay_data && (
              <div className="absolute bottom-2 left-2 glass-card-light rounded-lg p-2 text-xs">
                <div className="text-white/90 font-mono space-y-1">
                  {currentFrame.overlay_data.rover_velocity !== undefined && (
                    <div>Velocity: {currentFrame.overlay_data.rover_velocity.toFixed(2)} m/s</div>
                  )}
                  {currentFrame.overlay_data.battery_level !== undefined && (
                    <div>Battery: {currentFrame.overlay_data.battery_level.toFixed(0)}%</div>
                  )}
                  <div className="text-white/60 text-[10px] mt-1">
                    {currentFrame.overlay_data.timestamp_text}
                  </div>
                </div>
              </div>
            )}
          </>
        )}
      </div>

      {/* Controls */}
      {isConnected && (
        <div className="mt-4 flex items-center justify-between">
          <button
            onClick={toggleStreaming}
            className={`px-4 py-2 rounded-xl font-semibold transition-all ${
              isStreaming
                ? "bg-red-500/20 text-red-400 hover:bg-red-500/30"
                : "btn-gradient"
            }`}
          >
            {isStreaming ? "Stop Stream" : "Start Stream"}
          </button>

          <div className="text-sm text-white/60">
            {currentFrame && (
              <>
                <span className="text-white/90 font-mono">{currentFrame.width}x{currentFrame.height}</span>
                <span className="mx-2">•</span>
                <span>Frame #{currentFrame.frame_id}</span>
              </>
            )}
          </div>
        </div>
      )}
    </div>
  );
};