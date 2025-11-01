import React, { useCallback, useEffect, useRef, useState } from "react";
import { Socket } from "socket.io-client";
import { Mic, MicOff, Volume2, Send, Radio, Headphones, AlertCircle, Shield } from "lucide-react";

// Declare Tauri global for TypeScript
declare global {
  interface Window {
    __TAURI__?: any;
  }
}

interface VoiceControlsProps {
  socket: Socket | null;
  isConnected: boolean;
  onLog?: (message: string, type?: "info" | "success" | "error" | "warning") => void;
}

type VoiceMode = "idle" | "voice_commands" | "walkie_talkie";

export const VoiceControls: React.FC<VoiceControlsProps> = ({
  socket,
  isConnected,
  onLog,
}) => {
  // TTS state
  const [ttsText, setTtsText] = useState("");
  const [isSendingTTS, setIsSendingTTS] = useState(false);

  // Voice mode state
  const [voiceMode, setVoiceMode] = useState<VoiceMode>("idle");
  const [audioLevel, setAudioLevel] = useState(0);
  const [permissionDenied, setPermissionDenied] = useState(false);
  const [permissionError, setPermissionError] = useState<string>("");

  // Audio refs
  const audioContextRef = useRef<AudioContext | null>(null);
  const processorRef = useRef<ScriptProcessorNode | null>(null);
  const analyserRef = useRef<AnalyserNode | null>(null);
  const mediaStreamRef = useRef<MediaStream | null>(null);
  const animationFrameRef = useRef<number | null>(null);

  // Send TTS command
  const sendTTS = useCallback(() => {
    if (!isConnected || !socket || !ttsText.trim()) {
      onLog?.("Cannot send TTS - not connected or empty text", "error");
      return;
    }

    setIsSendingTTS(true);
    socket.emit("tts_command", { text: ttsText.trim() });
    onLog?.(`TTS: "${ttsText.trim()}"`, "success");

    setTimeout(() => {
      setTtsText("");
      setIsSendingTTS(false);
    }, 300);
  }, [isConnected, socket, ttsText, onLog]);

  // Handle Enter key for TTS
  const handleTTSKeyPress = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === "Enter" && !e.shiftKey) {
        e.preventDefault();
        sendTTS();
      }
    },
    [sendTTS]
  );

  // Visualize audio level
  const visualizeAudioLevel = useCallback(() => {
    if (!analyserRef.current) return;

    const analyser = analyserRef.current;
    const dataArray = new Uint8Array(analyser.frequencyBinCount);
    analyser.getByteFrequencyData(dataArray);

    const average = dataArray.reduce((a, b) => a + b, 0) / dataArray.length;
    setAudioLevel(average / 255);

    animationFrameRef.current = requestAnimationFrame(visualizeAudioLevel);
  }, []);

  // Request microphone permission
  const requestMicrophonePermission = useCallback(async (): Promise<MediaStream | null> => {
    setPermissionDenied(false);
    setPermissionError("");

    try {

      // Check if we're in a secure context
      if (!window.isSecureContext && window.location.hostname !== "localhost") {
        setPermissionError("Microphone requires HTTPS or localhost");
        onLog?.("Microphone requires HTTPS or localhost", "error");
        return null;
      }

      // Request microphone with optimal settings
      const stream = await navigator.mediaDevices.getUserMedia({
        audio: {
          echoCancellation: true,
          noiseSuppression: true,
          autoGainControl: true,
          sampleRate: 16000,
          channelCount: 1,
        },
      });

      return stream;
    } catch (error) {
      console.error("Microphone permission error:", error);

      if (error instanceof Error) {
        if (error.name === "NotAllowedError") {
          setPermissionDenied(true);
          setPermissionError(
              "Microphone permission denied. Please allow microphone access in your browser settings."
          );
        } else if (error.name === "NotFoundError") {
          setPermissionError("No microphone found. Please connect a microphone.");
          onLog?.("No microphone device found", "error");
        } else if (error.name === "NotReadableError") {
          setPermissionError(
            "Microphone is already in use by another application."
          );
          onLog?.("Microphone already in use", "error");
        } else {
          setPermissionError(`Microphone error: ${error.message}`);
          onLog?.(`Microphone error: ${error.message}`, "error");
        }
      }

      return null;
    }
  }, [onLog]);

  // Create AudioWorklet processor with configurable buffer size
  const createAudioWorkletProcessor = useCallback((bufferSize: number, processorName: string) => {
    const processorCode = `
      class ${processorName} extends AudioWorkletProcessor {
        constructor() {
          super();
          this.bufferSize = ${bufferSize};
          this.buffer = new Float32Array(this.bufferSize);
          this.bufferIndex = 0;
        }

        process(inputs, outputs, parameters) {
          const input = inputs[0];
          if (input.length > 0) {
            const channelData = input[0];

            for (let i = 0; i < channelData.length; i++) {
              this.buffer[this.bufferIndex] = channelData[i];
              this.bufferIndex++;

              if (this.bufferIndex >= this.bufferSize) {
                // Send buffer to main thread
                this.port.postMessage({
                  type: 'audio-data',
                  audioData: new Float32Array(this.buffer)
                });

                // Reset buffer
                this.bufferIndex = 0;
                this.buffer.fill(0);
              }
            }
          }

          return true;
        }
      }

      registerProcessor('${processorName}', ${processorName});
    `;

    const blob = new Blob([processorCode], { type: "application/javascript" });
    return URL.createObjectURL(blob);
  }, []);

  // Start voice command mode
  const startVoiceCommands = useCallback(async () => {
    if (!isConnected || !socket) {
      onLog?.("Cannot start voice commands - not connected", "error");
      return;
    }

    const stream = await requestMicrophonePermission();
    if (!stream) return;

    try {
      const audioContext = new AudioContext({ sampleRate: 16000 });
      const source = audioContext.createMediaStreamSource(stream);
      const analyser = audioContext.createAnalyser();
      analyser.fftSize = 256;
      source.connect(analyser);

      // Create and load AudioWorklet processor (4096 buffer for voice commands)
      const processorUrl = createAudioWorkletProcessor(4096, "VoiceCommandProcessor");

      try {
        await audioContext.audioWorklet.addModule(processorUrl);
        onLog?.("Voice command AudioWorklet loaded", "success");

        // Create AudioWorkletNode
        const workletNode = new AudioWorkletNode(
          audioContext,
          "VoiceCommandProcessor"
        );

        // Handle messages from the audio worklet
        workletNode.port.onmessage = (event) => {
          if (event.data.type === "audio-data" && socket?.connected) {
            const audioData = event.data.audioData;

            // Send to speech recognition
            socket.emit("voice_command_audio", {
              audio_data: Array.from(audioData),
            });
          }
        };

        source.connect(workletNode);

        audioContextRef.current = audioContext;
        processorRef.current = workletNode as any;
        analyserRef.current = analyser;
        mediaStreamRef.current = stream;

        setVoiceMode("voice_commands");
        onLog?.("Voice command mode started - speak your commands", "success");
        visualizeAudioLevel();

        // Clean up the blob URL
        URL.revokeObjectURL(processorUrl);
      } catch (workletError) {
        onLog?.(`Failed to load AudioWorklet: ${workletError}`, "warning");
        // Fallback: AudioWorklet not supported, will fail gracefully
        stream.getTracks().forEach((track) => track.stop());
        audioContext.close();
      }
    } catch (error) {
      console.error("Failed to start voice commands:", error);
      onLog?.("Failed to initialize voice command mode", "error");
    }
  }, [
    isConnected,
    socket,
    onLog,
    requestMicrophonePermission,
    visualizeAudioLevel,
    createAudioWorkletProcessor,
  ]);

  // Start walkie-talkie mode
  const startWalkieTalkie = useCallback(async () => {
    if (!isConnected || !socket) {
      onLog?.("Cannot start voice commands - not connected", "error");
      return;
    }

    const stream = await requestMicrophonePermission();
    if (!stream) return;

    try {
      const audioContext = new AudioContext({ sampleRate: 16000 });

      const source = audioContext.createMediaStreamSource(stream);
      const analyser = audioContext.createAnalyser();
      analyser.fftSize = 256;
      source.connect(analyser);

      // Create and load AudioWorklet processor (1024 buffer for low-latency walkie-talkie)
      const processorUrl = createAudioWorkletProcessor(1024, "WalkieTalkieProcessor");
      try {
        await audioContext.audioWorklet.addModule(processorUrl);
        onLog?.("Walkie-talkie AudioWorklet loaded", "success");

        // Create AudioWorkletNode
        const workletNode = new AudioWorkletNode(
          audioContext,
          "WalkieTalkieProcessor"
        );

        // Handle messages from the audio worklet
        workletNode.port.onmessage = (event) => {
          if (event.data.type === "audio-data" && socket?.connected) {
            const audioData = event.data.audioData;

            // Send to audio playback
            socket.emit("audio_stream", {
              audio_data: Array.from(audioData),
            });
          }
        };

        source.connect(workletNode);

        audioContextRef.current = audioContext;
        processorRef.current = workletNode as any;
        analyserRef.current = analyser;
        mediaStreamRef.current = stream;

        setVoiceMode("walkie_talkie");
        onLog?.("Walkie-talkie mode started - speak to rover", "success");
        visualizeAudioLevel();

        // // Clean up the blob URL
        // URL.revokeObjectURL(processorUrl);
      } catch (workletError) {
        onLog?.(`Failed to load AudioWorklet: ${workletError}`, "warning");
        // Fallback: AudioWorklet not supported, will fail gracefully
        stream.getTracks().forEach((track) => track.stop());
        audioContext.close();
      }
    } catch (error) {
      console.error("Failed to start walkie-talkie:", error);
      onLog?.("Failed to initialize walkie-talkie mode", "error");
    }
  }, [
    isConnected,
    socket,
    onLog,
    requestMicrophonePermission,
    visualizeAudioLevel,
    createAudioWorkletProcessor,
  ]);

  // Stop all voice modes
  const stopVoiceMode = useCallback(() => {
    if (animationFrameRef.current) {
      cancelAnimationFrame(animationFrameRef.current);
      animationFrameRef.current = null;
    }

    if (processorRef.current) {
      processorRef.current.disconnect();
      processorRef.current = null;
    }

    if (audioContextRef.current) {
      audioContextRef.current.close();
      audioContextRef.current = null;
    }

    if (mediaStreamRef.current) {
      mediaStreamRef.current.getTracks().forEach((track) => track.stop());
      mediaStreamRef.current = null;
    }

    analyserRef.current = null;
    setAudioLevel(0);

    // Store previous mode for logging
    const previousMode = voiceMode;

    // Update state first
    setVoiceMode("idle");

    // Defer log to avoid "Cannot update component during render" error
    // This ensures the log happens after React completes the state update
    setTimeout(() => {
      if (previousMode === "voice_commands") {
        onLog?.("Voice command mode stopped", "info");
      } else if (previousMode === "walkie_talkie") {
        onLog?.("Walkie-talkie mode stopped", "info");
      }
    }, 0);
  }, [voiceMode, onLog]);

  // Cleanup on disconnect
  useEffect(() => {
    if (!isConnected && voiceMode !== "idle") {
      stopVoiceMode();
    }
  }, [isConnected, voiceMode, stopVoiceMode]);

  // Cleanup on unmount only
  useEffect(() => {
    return () => {
      // Cleanup function that runs only on unmount
      if (animationFrameRef.current) {
        cancelAnimationFrame(animationFrameRef.current);
      }
      if (processorRef.current) {
        processorRef.current.disconnect();
      }
      if (audioContextRef.current) {
        audioContextRef.current.close();
      }
      if (mediaStreamRef.current) {
        mediaStreamRef.current.getTracks().forEach((track) => track.stop());
      }
    };
  }, []); // Empty deps - only run on mount/unmount

  return (
    <div className="glass-card rounded-3xl shadow-2xl p-4 md:p-6 space-y-4">
      <div className="flex items-center gap-2 mb-2">
        <Volume2 className="w-6 h-6 md:w-8 md:h-8 text-orange-400" />
        <h2 className="text-2xl md:text-3xl font-bold text-white">
          VOICE COMMUNICATION
        </h2>
      </div>

      {/* Permission Error Banner */}
      {permissionDenied && (
        <div className="glass-card-light rounded-xl p-4 border-l-4 border-red-500 bg-red-500/10">
          <div className="flex items-start gap-3">
            <Shield className="w-6 h-6 text-red-400 flex-shrink-0 mt-0.5" />
            <div className="flex-1">
              <h3 className="font-bold text-red-300 mb-1">
                Microphone Permission Denied
              </h3>
              <p className="text-sm text-white/80 mb-2">{permissionError}</p>
            </div>
          </div>
        </div>
      )}

      {permissionError && !permissionDenied && (
        <div className="glass-card-light rounded-xl p-3 flex items-center gap-2 text-yellow-300 border border-yellow-400/30">
          <AlertCircle className="w-5 h-5 flex-shrink-0" />
          <span className="text-sm">{permissionError}</span>
        </div>
      )}

      {/* Type 1: Text-to-Speech */}
      <div className="glass-card-light rounded-2xl p-4 space-y-3">
        <div className="flex items-center gap-2">
          <Volume2 className="w-5 h-5 text-orange-300" />
          <h3 className="text-lg font-semibold text-white">
            Text-to-Speech
          </h3>
        </div>
        <p className="text-sm text-white/60">
          Type text to speak through rover speakers
        </p>

        <div className="flex gap-2">
          <input
            type="text"
            value={ttsText}
            onChange={(e) => setTtsText(e.target.value)}
            onKeyDown={handleTTSKeyPress}
            placeholder="Enter text to speak..."
            disabled={!isConnected}
            className="flex-1 px-4 py-3 bg-black/30 border border-white/20 rounded-xl text-white placeholder-white/40 focus:outline-none focus:border-orange-400 focus:ring-2 focus:ring-orange-400/50 transition-all disabled:opacity-50 disabled:cursor-not-allowed"
          />
          <button
            onClick={sendTTS}
            disabled={!isConnected || !ttsText.trim() || isSendingTTS}
            className="px-6 py-3 btn-gradient-orange rounded-xl font-semibold flex items-center gap-2 disabled:opacity-50 disabled:cursor-not-allowed transition-all hover:scale-105 active:scale-95"
          >
            <Send className={`w-5 h-5 ${isSendingTTS ? "animate-pulse" : ""}`} />
            Speak
          </button>
        </div>
      </div>

      {/* Type 2: Web UI Voice Commands */}
      <div className="glass-card-light rounded-2xl p-4 space-y-3">
        <div className="flex items-center gap-2">
          <Headphones className="w-5 h-5 text-blue-400" />
          <h3 className="text-lg font-semibold text-white">
            Voice Commands (Web Microphone)
          </h3>
        </div>
        <p className="text-sm text-white/60">
          Use your computer's microphone to control the rover with voice
        </p>

        <button
          onClick={voiceMode === "voice_commands" ? stopVoiceMode : startVoiceCommands}
          disabled={!isConnected || voiceMode === "walkie_talkie"}
          className={`w-full py-4 rounded-xl font-bold text-lg flex items-center justify-center gap-3 transition-all disabled:opacity-50 disabled:cursor-not-allowed ${
            voiceMode === "voice_commands"
              ? "bg-gradient-to-br from-red-600 via-red-500 to-orange-500 hover:shadow-[0_0_30px_rgba(239,68,68,0.7)] animate-pulse"
              : "btn-gradient-cyan hover:scale-105"
          } active:scale-95`}
        >
          {voiceMode === "voice_commands" ? (
            <>
              <MicOff className="w-6 h-6" />
              Stop Voice Commands
            </>
          ) : (
            <>
              <Mic className="w-6 h-6" />
              Start Voice Commands
            </>
          )}
        </button>

        {voiceMode === "voice_commands" && (
          <div className="space-y-2">
            <div className="flex justify-between text-xs text-white/60">
              <span>Audio Level</span>
              <span className="font-mono">{(audioLevel * 100).toFixed(0)}%</span>
            </div>
            <div className="h-2 bg-black/40 rounded-full overflow-hidden">
              <div
                className="h-full bg-gradient-to-r from-blue-500 via-cyan-500 to-blue-600 transition-all duration-100"
                style={{ width: `${audioLevel * 100}%` }}
              />
            </div>
            <div className="flex items-center gap-2 text-xs text-blue-300">
              <Headphones className="w-4 h-4 animate-pulse" />
              <span className="font-semibold">
                Listening for voice commands...
              </span>
            </div>
          </div>
        )}

        <div className="text-xs text-white/50">
          Say commands like: "move forward", "turn left", "track person"
        </div>
      </div>

      {/* Type 3: Walkie-Talkie Mode */}
      <div className="glass-card-light rounded-2xl p-4 space-y-3">
        <div className="flex items-center gap-2">
          <Radio className="w-5 h-5 text-green-400" />
          <h3 className="text-lg font-semibold text-white">
            Walkie-Talkie Mode
          </h3>
        </div>
        <p className="text-sm text-white/60">
          Stream your voice directly to rover speakers in real-time
        </p>

        <button
          disabled={!isConnected || voiceMode === "voice_commands"}
          onClick={voiceMode === "walkie_talkie" ? stopVoiceMode : startWalkieTalkie}
          className={`w-full py-4 rounded-xl font-bold text-lg flex items-center justify-center gap-3 transition-all disabled:opacity-50 disabled:cursor-not-allowed ${
            voiceMode === "walkie_talkie"
              ? "bg-gradient-to-br from-red-600 via-red-500 to-orange-500 hover:shadow-[0_0_30px_rgba(239,68,68,0.7)] animate-pulse"
              : "btn-gradient-green hover:scale-105"
          } active:scale-95`}
        >
          {voiceMode === "walkie_talkie" ? (
            <>
              <MicOff className="w-6 h-6" />
              Stop Talking
            </>
          ) : (
            <>
              <Mic className="w-6 h-6" />
              Talk to Rover
            </>
          )}
        </button>

        {voiceMode === "walkie_talkie" && (
          <div className="space-y-2">
            <div className="flex justify-between text-xs text-white/60">
              <span>Audio Level</span>
              <span className="font-mono">{(audioLevel * 100).toFixed(0)}%</span>
            </div>
            <div className="h-2 bg-black/40 rounded-full overflow-hidden">
              <div
                className="h-full bg-gradient-to-r from-green-500 via-yellow-500 to-red-500 transition-all duration-100"
                style={{ width: `${audioLevel * 100}%` }}
              />
            </div>
            <div className="flex items-center gap-2 text-xs text-green-300">
              <Radio className="w-4 h-4 animate-pulse" />
              <span className="font-semibold">
                Streaming live to rover speakers...
              </span>
            </div>
          </div>
        )}

        <div className="text-xs text-white/50">
          Use cases: Remote communication, announcements, intercom
        </div>
      </div>

      {/* Connection Warning */}
      {!isConnected && (
        <div className="glass-card-light rounded-xl p-3 flex items-center gap-2 text-yellow-300 border border-yellow-400/30">
          <Radio className="w-5 h-5" />
          <span className="text-sm font-medium">
            Connect to enable voice features
          </span>
        </div>
      )}
    </div>
  );
};
