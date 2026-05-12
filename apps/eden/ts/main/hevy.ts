import electron from "electron";

const { BrowserWindow } = electron;

const HEVY_API_BASE = "https://api.hevyapp.com";

const WEB_API_KEY = "shelobs_hevy_web";

function appHeaders(authToken: string): Record<string, string> {
  return {
    Accept: "application/json, text/plain, */*",

    "Content-Type": "application/json",

    Origin: "https://hevy.com",

    Referer: "https://hevy.com/",

    "Hevy-Platform": "web",

    "User-Agent":
      "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",

    authorization: `Bearer ${authToken}`,

    "x-api-key": WEB_API_KEY,
  };
}

export type HevyLoginResponse =
  | { ok: true; authToken: string; username: string }
  | { ok: false; error: string };

export function hevyLoginViaBrowser(): Promise<HevyLoginResponse> {
  return new Promise((resolve) => {
    const loginWindow = new BrowserWindow({
      width: 500,

      height: 700,

      title: "Войти в Hevy",

      autoHideMenuBar: true,

      webPreferences: {
        nodeIntegration: false,

        contextIsolation: true,
      },
    });

    let resolved = false;
    const tokenTimers = new Set<ReturnType<typeof setTimeout>>();

    const clearTokenTimers = () => {
      tokenTimers.forEach((timer) => clearTimeout(timer));
      tokenTimers.clear();
    };

    const scheduleTokenExtraction = (delayMs: number) => {
      const timer = setTimeout(() => {
        tokenTimers.delete(timer);
        tryExtractToken();
      }, delayMs);
      tokenTimers.add(timer);
    };

    const tryExtractToken = () => {
      if (resolved) return;

      loginWindow.webContents

        .executeJavaScript(`
        (() => {
          try {
            const cookies = document.cookie.split(';').map(c => c.trim());
            let token = '';
            let username = '';
            for (const c of cookies) {
              if (c.startsWith('access-token=')) {
                token = c.substring('access-token='.length);
              }
            }
            // Get username from ACCOUNT_LOCAL_STORAGE_KEY
            try {
              const acc = JSON.parse(localStorage.getItem('ACCOUNT_LOCAL_STORAGE_KEY') || '{}');
              username = acc.username || '';
            } catch {}
            if (token) return JSON.stringify({ token, username });
            return '';
          } catch(e) { return ''; }
        })()
      `)

        .then((result: string) => {
          if (resolved || !result) return;

          try {
            const data = JSON.parse(result) as {
              token: string;
              username: string;
            };

            if (data.token) {
              resolved = true;
              clearTokenTimers();

              loginWindow.close();

              resolve({
                ok: true,
                authToken: data.token,
                username: data.username,
              });
            }
          } catch {}
        })

        .catch(() => {});
    };

    loginWindow.webContents.on("did-navigate", () => {
      scheduleTokenExtraction(1000);
      scheduleTokenExtraction(3000);
      scheduleTokenExtraction(5000);
    });

    loginWindow.webContents.on("did-navigate-in-page", () => {
      scheduleTokenExtraction(1000);
      scheduleTokenExtraction(3000);
    });

    loginWindow.on("closed", () => {
      clearTokenTimers();

      if (!resolved) {
        resolved = true;

        resolve({ ok: false, error: "Login window closed" });
      }
    });

    loginWindow.loadURL("https://hevy.com/login");
  });
}

export async function hevyFetchAccount(authToken: string) {
  const res = await fetch(`${HEVY_API_BASE}/account`, {
    headers: appHeaders(authToken),
  });

  if (!res.ok) {
    throw new Error(`Failed to fetch account: ${res.status}`);
  }

  return res.json();
}

export async function hevyGetWorkoutCount(authToken: string): Promise<number> {
  const res = await fetch(`${HEVY_API_BASE}/workout_count`, {
    headers: appHeaders(authToken),
  });

  if (!res.ok) {
    throw new Error(`Failed to fetch workout count: ${res.status}`);
  }

  const data = (await res.json()) as { workout_count: number };

  return data.workout_count;
}

export async function hevyFetchWorkouts(
  authToken: string,

  username: string,

  offset: number = 0,

  limit: number = 20,
) {
  const url = `${HEVY_API_BASE}/user_workouts_paged?username=${encodeURIComponent(username)}&limit=${limit}&offset=${offset}`;

  const res = await fetch(url, { headers: appHeaders(authToken) });

  if (!res.ok) {
    throw new Error(`Failed to fetch workouts: ${res.status}`);
  }

  return res.json();
}

export async function hevyFetchAllWorkouts(
  authToken: string,
  username: string,
) {
  const allWorkouts: unknown[] = [];

  const limit = 20;

  let offset = 0;

  while (true) {
    const data = (await hevyFetchWorkouts(
      authToken,
      username,
      offset,
      limit,
    )) as Record<string, unknown>;

    const workouts = Array.isArray(data)
      ? data
      : ((data.workouts as unknown[]) ?? []);

    if (!Array.isArray(workouts) || workouts.length === 0) break;

    allWorkouts.push(...workouts);

    if (workouts.length < limit) break;

    offset += workouts.length;
  }

  return allWorkouts;
}
