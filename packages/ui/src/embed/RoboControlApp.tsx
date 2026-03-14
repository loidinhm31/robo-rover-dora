/**
 * RoboControlApp - Main embeddable component
 *
 * Initializes all services and provides the application shell.
 * Supports both standalone and qm-hub-app embedded modes.
 */

import React, { useEffect, useMemo } from "react";
import {
  setSocketService,
  setRoverCommandService,
  setTrackingService,
  setFleetService,
  setTelemetryService,
  setMediaService,
  setVoiceService,
  type AllServices,
} from "../adapters/factory";
import type { ISocketService } from "../adapters/factory/interfaces/ISocketService";
import type { IRoverCommandService } from "../adapters/factory/interfaces/IRoverCommandService";
import type { ITrackingService } from "../adapters/factory/interfaces/ITrackingService";
import type { IFleetService } from "../adapters/factory/interfaces/IFleetService";
import type { ITelemetryService } from "../adapters/factory/interfaces/ITelemetryService";
import type { IMediaService } from "../adapters/factory/interfaces/IMediaService";
import type { IVoiceService } from "../adapters/factory/interfaces/IVoiceService";
import { AppShell } from "../components/templates";
import { RoboRoverControl } from "../components/pages";

const DEFAULT_SOCKET_URL = "http://localhost:3030";

export interface RoboControlAppAdapters {
  socket: ISocketService;
  roverCommand: IRoverCommandService;
  tracking: ITrackingService;
  fleet: IFleetService;
  telemetry: ITelemetryService;
  media: IMediaService;
  voice: IVoiceService;
}

export interface RoboControlAppProps {
  /** Service adapters — required for standalone, optional when embedded */
  adapters?: RoboControlAppAdapters;
  /** Socket server URL (defaults to localhost:3030) */
  socketUrl?: string;
  /** Socket.IO authentication credentials */
  auth?: {
    username: string;
    password: string;
  };
  /** Additional className for the root element */
  className?: string;
  /** Children to render inside the AppShell */
  children?: React.ReactNode;

  // Standard qm-hub embed props
  /** SSO tokens from qm-hub-app (accepted for interface consistency) */
  authTokens?: { accessToken: string; refreshToken: string; userId: string };
  /** Whether this app is embedded in qm-hub-app */
  embedded?: boolean;
  /** Use parent's BrowserRouter (no-op — no internal routing) */
  useRouter?: boolean;
  /** Route prefix in parent router */
  basePath?: string;
  /** Callback to notify parent on logout */
  onLogoutRequest?: () => void;
  /** Register cleanup function called on parent logout */
  registerLogoutCleanup?: (cleanup: () => void) => () => void;
}

export const RoboControlApp: React.FC<RoboControlAppProps> = ({
  adapters,
  socketUrl,
  auth,
  className,
  children,
  embedded: _embedded,
  registerLogoutCleanup,
}) => {
  const resolvedSocketUrl = socketUrl || DEFAULT_SOCKET_URL;

  // Initialize services synchronously before first render
  const services = useMemo<AllServices | null>(() => {
    if (!adapters) return null;

    setSocketService(adapters.socket);
    setRoverCommandService(adapters.roverCommand);
    setTrackingService(adapters.tracking);
    setFleetService(adapters.fleet);
    setTelemetryService(adapters.telemetry);
    setMediaService(adapters.media);
    setVoiceService(adapters.voice);

    return {
      socket: adapters.socket,
      roverCommand: adapters.roverCommand,
      tracking: adapters.tracking,
      fleet: adapters.fleet,
      telemetry: adapters.telemetry,
      media: adapters.media,
      voice: adapters.voice,
    };
  }, [adapters]);

  // Auto-connect on mount
  useEffect(() => {
    if (services) {
      services.socket.connect(resolvedSocketUrl, auth);
    }

    return () => {
      services?.socket.disconnect();
    };
  }, [resolvedSocketUrl, auth, services]);

  // Register socket disconnect as cleanup on parent logout
  useEffect(() => {
    if (!registerLogoutCleanup || !services) return;

    const unregister = registerLogoutCleanup(() => {
      services.socket.disconnect();
    });

    return unregister;
  }, [registerLogoutCleanup, services]);

  if (!services) {
    // No adapters — fall back to Pattern A (direct socket, production-proven path)
    return (
      <div className={className}>
        <RoboRoverControl socketUrl={resolvedSocketUrl} auth={auth} />
      </div>
    );
  }

  return (
    <div className={className}>
      <AppShell socketUrl={resolvedSocketUrl} auth={auth}>
        {children}
      </AppShell>
    </div>
  );
};
