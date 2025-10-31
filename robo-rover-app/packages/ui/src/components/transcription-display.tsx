import React, { useEffect, useState } from "react";
import { SpeechTranscription } from "../types/robo-rover.js";
import { Mic, MicOff, Volume2 } from "lucide-react";

interface TranscriptionDisplayProps {
  transcription: SpeechTranscription | null;
  isAudioActive: boolean;
  maxHistory?: number;
  onStartAudio?: () => void;
  onStopAudio?: () => void;
}

interface TranscriptionHistoryItem extends SpeechTranscription {
  id: string;
}

export const TranscriptionDisplay: React.FC<TranscriptionDisplayProps> = ({
  transcription,
  isAudioActive,
  maxHistory = 5,
  onStartAudio,
  onStopAudio,
}) => {
  const [history, setHistory] = useState<TranscriptionHistoryItem[]>([]);
  const [isAnimating, setIsAnimating] = useState(false);

  useEffect(() => {
    if (transcription) {
      // Add new transcription to history with unique ID
      const newItem: TranscriptionHistoryItem = {
        ...transcription,
        id: `${transcription.timestamp}-${Math.random()}`,
      };

      setHistory((prev) => {
        const updated = [newItem, ...prev];
        return updated.slice(0, maxHistory);
      });

      // Trigger animation
      setIsAnimating(true);
      const timer = setTimeout(() => setIsAnimating(false), 300);
      return () => clearTimeout(timer);
    }
  }, [transcription, maxHistory]);

  const getConfidenceColor = (confidence: number) => {
    if (confidence >= 0.8) return "text-green-400";
    if (confidence >= 0.6) return "text-yellow-400";
    return "text-orange-400";
  };

  const getConfidenceBadge = (confidence: number) => {
    if (confidence >= 0.8) return "bg-green-500/20 text-green-400";
    if (confidence >= 0.6) return "bg-yellow-500/20 text-yellow-400";
    return "bg-orange-500/20 text-orange-400";
  };

  return (
    <div className="bg-slate-800/50 backdrop-blur-sm rounded-lg border border-slate-700/50 overflow-hidden">
      {/* Header */}
      <div className="flex items-center justify-between px-4 py-3 border-b border-slate-700/50 bg-slate-800/70">
        <div className="flex items-center gap-3">
          {isAudioActive ? (
            <>
              <Mic className="w-5 h-5 text-green-400" />
              <span className="text-sm font-medium text-slate-200">
                Live Transcription
              </span>
              {/* Animated pulse for active audio */}
              <div className="flex gap-1">
                <div className="w-1 h-3 bg-green-400 rounded-full animate-pulse" />
                <div
                  className="w-1 h-3 bg-green-400 rounded-full animate-pulse"
                  style={{ animationDelay: "0.1s" }}
                />
                <div
                  className="w-1 h-3 bg-green-400 rounded-full animate-pulse"
                  style={{ animationDelay: "0.2s" }}
                />
              </div>
            </>
          ) : (
            <>
              <MicOff className="w-5 h-5 text-slate-500" />
              <span className="text-sm font-medium text-slate-400">
                Audio Inactive
              </span>
            </>
          )}
        </div>

        <div className="flex items-center gap-2">
          {transcription && (
            <div className="flex items-center gap-2 text-xs text-slate-400">
              <Volume2 className="w-4 h-4" />
              <span>
                {(transcription.duration_ms / 1000).toFixed(1)}s
              </span>
            </div>
          )}

          {/* Audio control buttons */}
          {onStartAudio && onStopAudio && (
            <button
              onClick={isAudioActive ? onStopAudio : onStartAudio}
              className={`ml-2 px-3 py-1 rounded-md text-xs font-medium transition-all ${
                isAudioActive
                  ? "bg-red-500/20 text-red-400 hover:bg-red-500/30"
                  : "bg-green-500/20 text-green-400 hover:bg-green-500/30"
              }`}
            >
              {isAudioActive ? "Stop Audio" : "Start Audio"}
            </button>
          )}
        </div>
      </div>

      {/* Current Transcription */}
      {transcription && (
        <div
          className={`px-4 py-4 border-b border-slate-700/50 bg-slate-900/30 transition-all duration-300 ${
            isAnimating ? "bg-blue-500/10 scale-[1.01]" : ""
          }`}
        >
          <div className="flex items-start gap-3">
            <div className="flex-1">
              <p className="text-base font-medium text-slate-100 leading-relaxed">
                {transcription.text}
              </p>
              <div className="flex items-center gap-3 mt-2">
                <span
                  className={`inline-flex items-center px-2 py-1 rounded-md text-xs font-medium ${getConfidenceBadge(transcription.confidence)}`}
                >
                  {(transcription.confidence * 100).toFixed(0)}% confidence
                </span>
                <span className="text-xs text-slate-500">
                  {transcription.language.toUpperCase()}
                </span>
                <span className="text-xs text-slate-500">
                  {new Date(transcription.timestamp).toLocaleTimeString()}
                </span>
              </div>
            </div>
          </div>
        </div>
      )}

      {/* Transcription History */}
      {history.length > 0 && (
        <div className="max-h-[300px] overflow-y-auto">
          {history.slice(1).map((item, index) => (
            <div
              key={item.id}
              className={`px-4 py-3 border-b border-slate-700/30 hover:bg-slate-800/30 transition-colors ${
                index === history.length - 2 ? "border-b-0" : ""
              }`}
              style={{ opacity: 1 - index * 0.15 }}
            >
              <div className="flex items-start gap-2">
                <div className="flex-1">
                  <p className="text-sm text-slate-300 leading-relaxed">
                    {item.text}
                  </p>
                  <div className="flex items-center gap-2 mt-1">
                    <span
                      className={`text-xs ${getConfidenceColor(item.confidence)}`}
                    >
                      {(item.confidence * 100).toFixed(0)}%
                    </span>
                    <span className="text-xs text-slate-600">
                      {new Date(item.timestamp).toLocaleTimeString()}
                    </span>
                  </div>
                </div>
              </div>
            </div>
          ))}
        </div>
      )}

      {/* Empty State */}
      {!transcription && history.length === 0 && (
        <div className="px-4 py-12 text-center">
          <Mic className="w-12 h-12 text-slate-600 mx-auto mb-3" />
          <p className="text-sm text-slate-400">
            {isAudioActive
              ? "Waiting for speech..."
              : "Start audio capture to see transcriptions"}
          </p>
        </div>
      )}
    </div>
  );
};

export default TranscriptionDisplay;
