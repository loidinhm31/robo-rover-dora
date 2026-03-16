import React, { useState } from "react";
import * as Popover from "@radix-ui/react-popover";
import { Link, Plug, Settings, Unplug, X } from "lucide-react";
import { detectMixedContent, suggestSecureUrl } from "../../utils/url-validation";

export interface SocketAuth {
  username: string;
  password: string;
}

export interface ServerSettingsProps {
  currentUrl: string;
  currentAuth?: SocketAuth;
  isConnected: boolean;
  onConnect: (url: string, auth: SocketAuth | undefined) => void;
  onDisconnect: () => void;
}

export const ServerSettings: React.FC<ServerSettingsProps> = ({
  currentUrl,
  currentAuth,
  isConnected,
  onConnect,
  onDisconnect,
}) => {
  const [draftUrl, setDraftUrl] = useState(currentUrl);
  const [draftUsername, setDraftUsername] = useState(currentAuth?.username ?? "");
  const [draftPassword, setDraftPassword] = useState(currentAuth?.password ?? "");

  const mixedContentWarning = detectMixedContent(draftUrl);

  const handleOpen = (open: boolean) => {
    if (open) {
      setDraftUrl(currentUrl);
      setDraftUsername(currentAuth?.username ?? "");
      setDraftPassword(currentAuth?.password ?? "");
    }
  };

  const buildAuth = (): SocketAuth | undefined => {
    const u = draftUsername.trim();
    const p = draftPassword.trim();
    return u && p ? { username: u, password: p } : undefined;
  };

  const handleConnect = () => {
    const trimmed = draftUrl.trim();
    if (trimmed) {
      onConnect(trimmed, buildAuth());
    }
  };

  return (
    <Popover.Root onOpenChange={handleOpen}>
      <Popover.Trigger asChild>
        <button
          className="p-2 rounded text-slate-400 hover:text-syntax-cyan hover:bg-slate-800/60 transition-colors cursor-pointer"
          title="Server settings"
          aria-label="Server settings"
        >
          <Settings className="w-4 h-4" />
        </button>
      </Popover.Trigger>

        <Popover.Content
          side="bottom"
          align="end"
          sideOffset={8}
          collisionPadding={12}
          className="z-50 w-80 bg-slate-900 border border-slate-700 rounded-lg shadow-2xl p-4 space-y-3 outline-none
            data-[state=open]:animate-in data-[state=closed]:animate-out
            data-[state=closed]:fade-out-0 data-[state=open]:fade-in-0
            data-[state=closed]:zoom-out-95 data-[state=open]:zoom-in-95
            data-[side=bottom]:slide-in-from-top-2"
        >
          {/* Header */}
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-2">
              <Settings className="w-4 h-4 text-syntax-cyan" />
              <span className="text-sm font-mono font-bold text-syntax-cyan">
                server_settings
              </span>
            </div>
            <Popover.Close asChild>
              <button
                className="text-slate-500 hover:text-slate-300 transition-colors cursor-pointer"
                aria-label="Close settings"
              >
                <X className="w-4 h-4" />
              </button>
            </Popover.Close>
          </div>

          {/* Current connection status */}
          <div className="bg-slate-900/80 border border-slate-700 rounded px-3 py-2 space-y-1">
            <div className="flex items-center gap-2">
              <div
                className={`w-2 h-2 rounded-full flex-shrink-0 ${
                  isConnected
                    ? "bg-syntax-green status-glow-green"
                    : "bg-syntax-red status-glow-red"
                }`}
              />
              <span
                className={`text-xs font-mono font-semibold ${
                  isConnected ? "text-syntax-green" : "text-syntax-red"
                }`}
              >
                {isConnected ? "[ONLINE]" : "[OFFLINE]"}
              </span>
            </div>
            <div className="flex items-center gap-1.5 text-xs font-mono text-slate-400 min-w-0">
              <Link className="w-3 h-3 flex-shrink-0 text-slate-500" />
              <span className="truncate">{currentUrl}</span>
            </div>
          </div>

          {/* URL input */}
          <div className="space-y-1.5">
            <label className="text-xs font-mono text-slate-400">
              <span className="text-syntax-orange">socket_url</span>
              <span className="text-slate-600">:</span>
            </label>
            <input
              type="text"
              value={draftUrl}
              onChange={(e) => setDraftUrl(e.target.value)}
              onKeyDown={(e) => { if (e.key === "Enter") handleConnect(); }}
              placeholder="http://localhost:3030"
              className="glass-input w-full px-3 py-2 rounded text-sm font-mono focus:outline-none focus:ring-2 focus:ring-cyan-400/50"
            />
            {mixedContentWarning && (
              <div className="flex items-start gap-2 bg-amber-500/10 border border-amber-500/30 rounded px-2.5 py-2 text-xs font-mono">
                <span className="text-amber-400 flex-shrink-0 mt-0.5">⚠</span>
                <div className="space-y-1">
                  <p className="text-amber-300">{mixedContentWarning}</p>
                  <button
                    type="button"
                    onClick={() => setDraftUrl(suggestSecureUrl(draftUrl))}
                    className="text-syntax-cyan hover:underline cursor-pointer"
                  >
                    → Use {suggestSecureUrl(draftUrl)}
                  </button>
                </div>
              </div>
            )}
          </div>

          {/* Auth inputs */}
          <div className="space-y-1.5">
            <label className="text-xs font-mono text-slate-400">
              <span className="text-syntax-purple">auth</span>
              <span className="text-slate-600">: {"{"}</span>
              <span className="text-slate-600 ml-1">optional</span>
              <span className="text-slate-600"> {"}"}</span>
            </label>
            <input
              type="text"
              value={draftUsername}
              onChange={(e) => setDraftUsername(e.target.value)}
              onKeyDown={(e) => { if (e.key === "Enter") handleConnect(); }}
              placeholder="username"
              autoComplete="off"
              className="glass-input w-full px-3 py-2 rounded text-sm font-mono focus:outline-none focus:ring-2 focus:ring-cyan-400/50"
            />
            <input
              type="password"
              value={draftPassword}
              onChange={(e) => setDraftPassword(e.target.value)}
              onKeyDown={(e) => { if (e.key === "Enter") handleConnect(); }}
              placeholder="password"
              autoComplete="current-password"
              className="glass-input w-full px-3 py-2 rounded text-sm font-mono focus:outline-none focus:ring-2 focus:ring-cyan-400/50"
            />
          </div>

          {/* Actions */}
          <div className="flex gap-2">
            <Popover.Close asChild>
              <button
                onClick={handleConnect}
                disabled={!draftUrl.trim()}
                className="btn-info flex-1 px-3 py-2 rounded flex items-center justify-center gap-1.5 text-sm font-mono font-bold disabled:opacity-50 disabled:cursor-not-allowed cursor-pointer"
              >
                <Plug className="w-3.5 h-3.5" />
                Connect
              </button>
            </Popover.Close>

            {isConnected && (
              <Popover.Close asChild>
                <button
                  onClick={onDisconnect}
                  className="btn-destructive px-3 py-2 rounded flex items-center justify-center gap-1.5 text-sm font-mono font-bold cursor-pointer"
                >
                  <Unplug className="w-3.5 h-3.5" />
                  Disconnect
                </button>
              </Popover.Close>
            )}
          </div>

          <p className="text-xs font-mono text-slate-600">
            // persisted in localStorage
          </p>

          <Popover.Arrow className="fill-slate-700" />
        </Popover.Content>
    </Popover.Root>
  );
};
