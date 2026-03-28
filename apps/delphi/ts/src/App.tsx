import './global.css';
import { useEffect, useState } from 'react';
import AuthOverlay from '@/components/AuthOverlay';
import QuickEntry from '@/components/QuickEntry';
import QuickOpen from '@/components/QuickOpen';
import SideBar from '@/components/sideBar';
import AppRoutes from '@/routes';
import {
  loadTasksFromWebStorage,
  saveTasksToWebStorage,
} from '@/services/storage/tasks.web';
import {
  arkChangeToTask,
  arkChangeToProject,
  arkChangeEventType,
  arkSync,
  getArkApiKey,
  getArkUrl,
  setArkApiKey,
  setArkUrl,
  fetchTasksFromArk,
  fetchProjectsFromArk,
} from '@/services/sync/ark-client';
import { parseConnectionString } from '@/services/sync/pairing';
import useTask from '@/store/tasks';
import useTodoStore from '@/store/todos';

type ConnectionState = 'online' | 'syncing' | 'offline';

function ConnectionDot({ state }: { state: ConnectionState }) {
  const color =
    state === 'online'
      ? 'bg-emerald-500'
      : state === 'syncing'
        ? 'bg-amber-500 animate-pulse'
        : 'bg-rose-500';
  const title =
    state === 'online' ? 'Подключено' : state === 'syncing' ? 'Подключение...' : 'Нет связи';

  return (
    <div
      className={`fixed top-4 right-4 z-40 h-2.5 w-2.5 rounded-full ${color}`}
      title={title}
    />
  );
}

function App() {
  const setTasks = useTask((s) => s.setTasks);
  const tasks = useTask((s) => s.tasks);
  const setHydrated = useTask((s) => s.setHydrated);
  const hydrated = useTask((s) => s.hydrated);
  const upsertTask = useTask((s) => s.upsertTask);
  const removeTaskById = useTask((s) => s.removeTaskById);
  const setProjects = useTodoStore((s) => s.setProjects);
  const upsertProject = useTodoStore((s) => s.upsertProject);
  const removeProject = useTodoStore((s) => s.removeProject);
  const [authRequired, setAuthRequired] = useState(false);
  const [authBusy, setAuthBusy] = useState(false);
  const [authError, setAuthError] = useState<string | null>(null);
  const [bootstrapNonce, setBootstrapNonce] = useState(0);
  const [connectionState, setConnectionState] = useState<ConnectionState>('syncing');

  useEffect(() => {
    const cached = loadTasksFromWebStorage();
    if (cached.length > 0) {
      setTasks(cached);
    }

    // Subscribe to sync changes BEFORE connecting so we don't miss initial sync_changes
    const unsubStatus = arkSync.onStatus((connected) => {
      setConnectionState(connected ? (arkSync.isSynced ? 'online' : 'syncing') : 'offline');
    });

    const unsubChange = arkSync.onChange((change) => {
      const eventType = arkChangeEventType(change);

      if (eventType === 'project') {
        if (change.change_type === 'delete') {
          removeProject(change.event_id);
          return;
        }
        const project = arkChangeToProject(change);
        if (project) upsertProject(project);
        return;
      }

      // Default: handle as task
      if (change.change_type === 'delete') {
        removeTaskById(change.event_id);
        return;
      }
      const task = arkChangeToTask(change);
      if (task) upsertTask(task);
    });

    const url = getArkUrl();
    const key = getArkApiKey();

    if (!url || !key) {
      setAuthRequired(true);
      setConnectionState('offline');
      setHydrated(true);
      return () => { unsubStatus(); unsubChange(); };
    }

    // Load tasks and projects from Ark HTTP API (source of truth in Electron where localStorage is disabled)
    fetchTasksFromArk().then((tasks) => {
      if (tasks.length > 0) setTasks(tasks);
    });
    fetchProjectsFromArk().then((projects) => {
      if (projects.length > 0) setProjects(projects);
    });

    // Connect WebSocket for realtime sync
    if (!arkSync.isConnected) {
      arkSync.connect(url, key);
    }
    setAuthRequired(false);
    setAuthError(null);
    setConnectionState('syncing');
    setHydrated(true);

    return () => {
      unsubStatus();
      unsubChange();
    };
  }, [bootstrapNonce, setHydrated, setTasks, upsertTask, removeTaskById, setProjects, upsertProject, removeProject]);

  useEffect(() => {
    if (!hydrated) return;
    saveTasksToWebStorage(tasks);
  }, [hydrated, tasks]);

  const handleAuthSubmit = async (connectionCode: string) => {
    const parsed = parseConnectionString(connectionCode);

    if (!parsed) {
      setAuthError('Неверный формат. Ожидается: ark://host:port?key=...');
      return;
    }

    setAuthBusy(true);
    setAuthError(null);
    setConnectionState('syncing');

    try {
      setArkUrl(parsed.server_url);
      setArkApiKey(parsed.api_key);
      arkSync.connect(parsed.server_url, parsed.api_key);
      setAuthRequired(false);
      setBootstrapNonce((value) => value + 1);
    } catch (error) {
      setAuthError(error instanceof Error ? error.message : 'Ошибка подключения');
      setConnectionState('offline');
    } finally {
      setAuthBusy(false);
    }
  };

  return (
    <>
      <ConnectionDot state={connectionState} />
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
