/**
 * ISocketService - Low-level WebSocket connection management
 * Handles connect/disconnect lifecycle and raw event emission/subscription
 */

export type ConnectionStatus = "disconnected" | "connecting" | "connected" | "error";

export interface SocketAuth {
  username?: string;
  password?: string;
  token?: string;
}

export interface ISocketService {
  /**
   * Connect to the socket server
   * @param url The server URL
   * @param auth Optional authentication credentials
   */
  connect(url: string, auth?: SocketAuth): void;

  /**
   * Disconnect from the socket server
   */
  disconnect(): void;

  /**
   * Get current connection status
   */
  getStatus(): ConnectionStatus;

  /**
   * Subscribe to connection status changes
   * @returns Unsubscribe function
   */
  onStatusChange(callback: (status: ConnectionStatus, clientId?: string) => void): () => void;

  /**
   * Emit an event to the server
   * @param event Event name
   * @param data Event data
   */
  emit(event: string, data: unknown): void;

  /**
   * Subscribe to an event from the server
   * @param event Event name
   * @param handler Event handler
   * @returns Unsubscribe function
   */
  on<T = unknown>(event: string, handler: (data: T) => void): () => void;
}
