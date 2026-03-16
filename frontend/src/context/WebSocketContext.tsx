import {
  createContext,
  type PropsWithChildren,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
} from 'react';
import { API_BASE_URL } from '../constants';
import type { WsStatus } from '../types';

type MessageListener = (event: MessageEvent<ArrayBuffer | string>) => void;

interface WebSocketContextValue {
  status: WsStatus;
  addMessageListener(listener: MessageListener): () => void;
  sendJson(payload: Record<string, unknown>): void;
}

const WebSocketContext = createContext<WebSocketContextValue | null>(null);

export function WebSocketProvider({ children }: PropsWithChildren): JSX.Element {
  const socketRef = useRef<WebSocket | null>(null);
  const reconnectTimerRef = useRef<number | null>(null);
  const attemptRef = useRef(0);
  const listenersRef = useRef(new Set<MessageListener>());
  const [status, setStatus] = useState<WsStatus>('connecting');

  const connect = useCallback(() => {
    setStatus('connecting');
    const url = new URL(API_BASE_URL);
    url.protocol = url.protocol === 'https:' ? 'wss:' : 'ws:';
    url.pathname = '/v1/ws';

    const socket = new WebSocket(url);
    socket.binaryType = 'arraybuffer';

    socket.addEventListener('open', () => {
      attemptRef.current = 0;
      socketRef.current = socket;
      setStatus('connected');
    });

    socket.addEventListener('message', (event) => {
      listenersRef.current.forEach((listener) => listener(event as MessageEvent<ArrayBuffer | string>));
    });

    const scheduleReconnect = () => {
      if (reconnectTimerRef.current !== null) {
        window.clearTimeout(reconnectTimerRef.current);
      }

      setStatus('disconnected');
      const delay = Math.min(30_000, 1_000 * 2 ** attemptRef.current);
      attemptRef.current += 1;
      reconnectTimerRef.current = window.setTimeout(connect, delay);
    };

    socket.addEventListener('close', scheduleReconnect);
    socket.addEventListener('error', () => {
      socket.close();
    });
  }, []);

  useEffect(() => {
    connect();

    return () => {
      if (reconnectTimerRef.current !== null) {
        window.clearTimeout(reconnectTimerRef.current);
      }
      socketRef.current?.close();
    };
  }, [connect]);

  const addMessageListener = useCallback((listener: MessageListener) => {
    listenersRef.current.add(listener);
    return () => {
      listenersRef.current.delete(listener);
    };
  }, []);

  const sendJson = useCallback((payload: Record<string, unknown>) => {
    if (socketRef.current?.readyState === WebSocket.OPEN) {
      socketRef.current.send(JSON.stringify(payload));
    }
  }, []);

  const value = useMemo(
    () => ({
      status,
      addMessageListener,
      sendJson,
    }),
    [addMessageListener, sendJson, status],
  );

  return <WebSocketContext.Provider value={value}>{children}</WebSocketContext.Provider>;
}

export function useWebSocketContext(): WebSocketContextValue {
  const context = useContext(WebSocketContext);
  if (!context) {
    throw new Error('useWebSocketContext must be used within WebSocketProvider');
  }

  return context;
}
