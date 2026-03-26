/**
 * useConnection - Subscribe to socket connection state
 */

import { useState, useEffect, useCallback } from "react";
import { SocketService } from "../services";
import type { ConnectionStatus, SocketAuth } from "../adapters/factory/interfaces";

export interface UseConnectionReturn {
  status: ConnectionStatus;
  isConnected: boolean;
  clientId: string | null;
  authError: string | null;
  connect: (url: string, auth?: SocketAuth) => void;
  disconnect: () => void;
}

export const useConnection = (): UseConnectionReturn => {
  const [status, setStatus] = useState<ConnectionStatus>("disconnected");
  const [clientId, setClientId] = useState<string | null>(null);
  const [authError, setAuthError] = useState<string | null>(null);

  useEffect(() => {
    const unsubscribe = SocketService.onStatusChange((newStatus, newClientId) => {
      setStatus(newStatus);
      setClientId(newClientId ?? null);
      if (newStatus !== "error") setAuthError(null);
    });

    try {
      setStatus(SocketService.getStatus());
    } catch {
      // Service not initialized yet
    }

    return unsubscribe;
  }, []);

  const connect = useCallback((url: string, auth?: SocketAuth) => {
    SocketService.connect(url, auth);
  }, []);

  const disconnect = useCallback(() => {
    SocketService.disconnect();
  }, []);

  return {
    status,
    isConnected: status === "connected",
    clientId,
    authError,
    connect,
    disconnect,
  };
};
