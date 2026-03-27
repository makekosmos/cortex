import './global.css';
import { useEffect, useState } from 'react';
import AuthOverlay from '@/components/AuthOverlay';
import QuickEntry from '@/components/QuickEntry';
import QuickOpen from '@/components/QuickOpen';
import SideBar from '@/components/sideBar';
import AppRoutes from '@/routes';
import {
  api,
  ApiError,
  clearSavedPassphrase,
  clearToken,
  connectTasksWebSocket,
  getReadableError,
  getSavedPassphrase,
  getToken,
  loginWithPassphrase,
  normalizeApiUrl,
  normalizePassphrase,
  setApiUrl,
} from '@/services/api/client';
import { fromTaskDto } from '@/services/api/tasks';
import {
  loadTasksFromWebStorage,
  saveTasksToWebStorage,
} from '@/services/storage/tasks.web';
import { arkSync, getArkApiKey, getArkUrl } from '@/services/sync/ark-client';
import useTask from '@/store/tasks';

type TaskSocketEvent = { type: string; payload?: unknown };
type ConnectionState = 'online' | 'syncing' | 'offline';

function isTaskPayload(value: unknown): value is Parameters<typeof fromTaskDto>[0] {
  if (!value || typeof value !== 'object') return false;
  const payload = value as Record<string, unknown>;
  return (
    typeof payload.id === 'string' &&
    typeof payload.title === 'string' &&
    typeof payload.completed === 'boolean' &&
    typeof payload.created_at === 'string'
  );
}

function ConnectionBadge({ state }: { state: ConnectionState }) {
  const label =
    state === 'online' ? 'Synced' : state === 'syncing' ? 'Syncing...' : 'Offline';
  const color =
    state === 'online'
      ? 'bg-emerald-600/85'
      : state === 'syncing'
        ? 'bg-amber-600/85'
        : 'bg-rose-700/85';

  return (
    <div className={`fixed top-3 right-3 z-40 rounded-md px-2 py-1 text-xs text-white ${color}`}>
      {label}
    </div>
  );
}

function App() {
  const setTasks = useTask((s) => s.setTasks);
  const tasks = useTask((s) => s.tasks);
  const setHydrated = useTask((s) => s.setHydrated);
  const hydrated = useTask((s) => s.hydrated);
  const upsertTask = useTask((s) => s.upsertTask);
  const removeTaskById = useTask((s) => s.removeTaskById);

  const [authRequired, setAuthRequired] = useState(false);
  const [authBusy, setAuthBusy] = useState(false);
  const [authError, setAuthError] = useState<string | null>(null);
  const [bootstrapNonce, setBootstrapNonce] = useState(0);
  const [connectionState, setConnectionState] = useState<ConnectionState>('syncing');

  useEffect(() => {
    let mounted = true;

    (async () => {
      const cached = loadTasksFromWebStorage();
      if (cached.length > 0 && mounted) {
        setTasks(cached);
      }

      try {
        if (!getToken()) {
          const savedPassphrase = getSavedPassphrase();
          if (!savedPassphrase) {
            if (mounted) {
              setAuthRequired(true);
              setConnectionState('offline');
            }
            return;
          }
          await loginWithPassphrase(savedPassphrase);
        }

        const fetched = await api.getTasks();
        if (!mounted) return;
        setTasks(fetched.map(fromTaskDto));
        setAuthRequired(false);
        setAuthError(null);
        setConnectionState('syncing');
      } catch (error) {
        if (!mounted) return;

        if (error instanceof ApiError && error.status === 401) {
          clearToken();
          clearSavedPassphrase();
        }

        setAuthRequired(true);
        setAuthError(getReadableError(error));
        setConnectionState('offline');
      } finally {
        if (mounted) {
          setHydrated(true);
        }
      }
    })();

    return () => {
      mounted = false;
    };
  }, [bootstrapNonce, setHydrated, setTasks]);

  useEffect(() => {
    if (!hydrated || authRequired || !getToken()) return;

    let socket: WebSocket | null = null;
    let reconnectTimer: ReturnType<typeof setTimeout> | null = null;
    let stopped = false;

    const handleEvent = (raw: unknown) => {
      const event = raw as TaskSocketEvent;
      if (
        (event.type === 'task.created' || event.type === 'task.updated') &&
        isTaskPayload(event.payload)
      ) {
        upsertTask(fromTaskDto(event.payload));
      }
      if (
        event.type === 'task.deleted' &&
        event.payload &&
        typeof event.payload === 'object' &&
        'id' in event.payload &&
        typeof event.payload.id === 'string'
      ) {
        removeTaskById(event.payload.id);
      }
    };

    const connect = () => {
      if (stopped) return;
      setConnectionState('syncing');

      socket = connectTasksWebSocket(
        handleEvent,
        () => {
          if (stopped) return;
          setConnectionState('offline');
          reconnectTimer = setTimeout(connect, 2000);
        },
        () => {
          setConnectionState('online');
        },
      );

      if (!socket) {
        setConnectionState('offline');
      }
    };

    connect();

    return () => {
      stopped = true;
      if (reconnectTimer) clearTimeout(reconnectTimer);
      socket?.close();
    };
  }, [authRequired, hydrated, removeTaskById, upsertTask]);

  useEffect(() => {
    if (!hydrated) return;
    saveTasksToWebStorage(tasks);
  }, [hydrated, tasks]);

  // Auto-connect to Ark on startup if paired
  useEffect(() => {
    const url = getArkUrl();
    const key = getArkApiKey();
    if (url && key && !arkSync.isConnected) {
      arkSync.connect(url, key);
    }
  }, []);

  const handleAuthSubmit = async (input: { apiUrl: string; passphrase: string }) => {
    const apiUrl = normalizeApiUrl(input.apiUrl);
    const passphrase = normalizePassphrase(input.passphrase);

    if (!apiUrl) {
      setAuthError('API URL is required.');
      return;
    }
    if (!passphrase) {
      setAuthError('Passphrase is required.');
      return;
    }

    setAuthBusy(true);
    setAuthError(null);
    setConnectionState('syncing');

    try {
      setApiUrl(apiUrl);
      clearToken();
      await loginWithPassphrase(passphrase);
      setAuthRequired(false);
      setBootstrapNonce((value) => value + 1);
    } catch (error) {
      setAuthError(getReadableError(error));
      setConnectionState('offline');
    } finally {
      setAuthBusy(false);
    }
  };

  return (
    <>
      <ConnectionBadge state={connectionState} />
      <main className="flex min-h-0 min-w-0 flex-1">
        <SideBar />
        <AppRoutes />
      </main>
      <QuickEntry />
      <QuickOpen />
      {authRequired ? (
        <AuthOverlay busy={authBusy} errorMessage={authError} onSubmit={handleAuthSubmit} />
      ) : null}
    </>
  );
}

export default App;
