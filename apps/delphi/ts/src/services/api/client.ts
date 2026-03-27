import { normalizeApiUrl, normalizePassphrase } from '@/helpers/normalize';

const TOKEN_KEY = 'todofus.jwt';
const API_URL_KEY = 'todofus.apiUrl';
const PASSPHRASE_KEY = 'todofus.passphrase';
const REQUEST_TIMEOUT_MS = 12000;

function getCookie(key: string) {
  const escaped = key.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  const match = document.cookie.match(new RegExp(`(?:^|; )${escaped}=([^;]*)`));
  return match ? decodeURIComponent(match[1]) : null;
}

function setCookie(key: string, value: string) {
  const maxAge = 60 * 60 * 24 * 365;
  document.cookie = `${key}=${encodeURIComponent(value)}; path=/; max-age=${maxAge}; samesite=lax`;
}

function removeCookie(key: string) {
  document.cookie = `${key}=; path=/; max-age=0; samesite=lax`;
}

function storageGet(key: string) {
  try {
    const value = localStorage.getItem(key);
    if (value !== null) return value;
  } catch {
    // ignore
  }

  try {
    const value = sessionStorage.getItem(key);
    if (value !== null) return value;
  } catch {
    // ignore
  }

  try {
    return getCookie(key);
  } catch {
    return null;
  }
}

function storageSet(key: string, value: string) {
  try {
    localStorage.setItem(key, value);
  } catch {
    // ignore
  }

  try {
    sessionStorage.setItem(key, value);
  } catch {
    // ignore
  }

  try {
    setCookie(key, value);
  } catch {
    // ignore
  }
}

function storageRemove(key: string) {
  try {
    localStorage.removeItem(key);
  } catch {
    // ignore
  }

  try {
    sessionStorage.removeItem(key);
  } catch {
    // ignore
  }

  try {
    removeCookie(key);
  } catch {
    // ignore
  }
}

export class ApiError extends Error {
  status: number;

  constructor(message: string, status: number) {
    super(message);
    this.name = 'ApiError';
    this.status = status;
  }
}

export type TaskDto = {
  id: string;
  title: string;
  description: string | null;
  completed: boolean;
  priority: number;
  due_date: string | null;
  list_id: string | null;
  user_id: string;
  created_at: string;
  updated_at: string;
};

type LoginResponse = {
  token: string;
  userId: string;
};

export { normalizeApiUrl, normalizePassphrase };

export function getReadableError(error: unknown) {
  if (error instanceof ApiError) {
    if (error.status === 401) return 'Invalid passphrase.';
    return error.message || `Request failed (${error.status}).`;
  }
  if (error instanceof Error) return error.message;
  return 'Connection failed.';
}

export function getApiUrl() {
  const stored = storageGet(API_URL_KEY);
  const envValue = import.meta.env.VITE_API_URL;
  const defaultValue = 'https://213.165.58.219.nip.io';

  const storedLooksLocalhost = stored ? /^https?:\/\/(localhost|127\.0\.0\.1)(:\d+)?/i.test(stored) : false;

  const resolved =
    stored && !storedLooksLocalhost
      ? stored
      : envValue ?? defaultValue;

  return normalizeApiUrl(resolved);
}

export function setApiUrl(url: string) {
  storageSet(API_URL_KEY, normalizeApiUrl(url));
}

export function getToken() {
  return storageGet(TOKEN_KEY);
}

export function setToken(token: string) {
  storageSet(TOKEN_KEY, token);
}

export function clearToken() {
  storageRemove(TOKEN_KEY);
}

export function getSavedPassphrase() {
  return storageGet(PASSPHRASE_KEY);
}

export function clearSavedPassphrase() {
  storageRemove(PASSPHRASE_KEY);
}

async function request<T>(path: string, init?: RequestInit): Promise<T> {
  const token = getToken();
  const headers = new Headers(init?.headers);
  if (init?.body !== undefined) {
    headers.set('Content-Type', 'application/json');
  }

  if (token) {
    headers.set('Authorization', `Bearer ${token}`);
  }

  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), REQUEST_TIMEOUT_MS);

  let response: Response;
  try {
    response = await fetch(`${getApiUrl()}${path}`, {
      ...init,
      headers,
      signal: controller.signal,
    });
  } catch (error) {
    if (error instanceof DOMException && error.name === 'AbortError') {
      throw new ApiError('Request timeout. Check server availability.', 408);
    }
    throw error;
  } finally {
    clearTimeout(timeout);
  }

  if (!response.ok) {
    const text = await response.text();
    let message = text || `Request failed: ${response.status}`;

    try {
      const parsed = JSON.parse(text) as { message?: string };
      if (parsed?.message) {
        message = parsed.message;
      }
    } catch {
      // keep text as-is
    }

    throw new ApiError(message, response.status);
  }

  if (response.status === 204) {
    return undefined as T;
  }

  return response.json() as Promise<T>;
}

export async function loginWithPassphrase(passphrase: string) {
  const normalizedPassphrase = normalizePassphrase(passphrase);

  const response = await fetch(`${getApiUrl()}/auth/login`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ passphrase: normalizedPassphrase }),
  });

  if (!response.ok) {
    let message = 'Invalid passphrase';
    try {
      const parsed = (await response.json()) as { message?: string };
      if (parsed?.message) {
        message = parsed.message;
      }
    } catch {
      // ignore invalid JSON body
    }
    throw new ApiError(message, response.status);
  }

  const data = (await response.json()) as LoginResponse;
  setToken(data.token);
  storageSet(PASSPHRASE_KEY, normalizedPassphrase);
  return data;
}

export function buildTasksWsUrl(apiUrl: string, token: string) {
  const baseUrl = normalizeApiUrl(apiUrl).replace(/^http/, 'ws');
  return `${baseUrl}/ws?token=${encodeURIComponent(token)}`;
}

export function connectTasksWebSocket(
  onMessage: (data: unknown) => void,
  onClose?: () => void,
  onOpen?: () => void,
) {
  const token = getToken();
  if (!token) return null;

  const wsUrl = buildTasksWsUrl(getApiUrl(), token);
  const socket = new WebSocket(wsUrl);

  socket.onopen = () => {
    if (onOpen) onOpen();
  };

  socket.onmessage = (event) => {
    try {
      onMessage(JSON.parse(event.data));
    } catch {
      // ignore malformed messages
    }
  };

  socket.onclose = () => {
    if (onClose) onClose();
  };

  return socket;
}

export const api = {
  getTasks: () => request<TaskDto[]>('/tasks'),
  createTask: (title: string) =>
    request<TaskDto>('/tasks', {
      method: 'POST',
      body: JSON.stringify({ title }),
    }),
  updateTask: (id: string, patch: Partial<Pick<TaskDto, 'title' | 'completed'>>) =>
    request<TaskDto>(`/tasks/${id}`, {
      method: 'PATCH',
      body: JSON.stringify(patch),
    }),
  deleteTask: (id: string) =>
    request<{ success: boolean }>(`/tasks/${id}`, {
      method: 'DELETE',
    }),
};
