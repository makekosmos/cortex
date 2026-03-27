# Web UI

Svelte (Vite) UI for browsing the database through the HTTP API.

## Dev (separate dev server)

```bash
cd ui
bun install
bun run dev
```

Open `http://localhost:5173`, then set:
- Server URL: `http://127.0.0.1:8000`
- API key: your `LIFE_API_KEY`

## Build (served by the Python server)

```bash
cd ui
bun install
bun run build
```

After that, the server serves `ui/dist` automatically at `http://127.0.0.1:8000/`.

## E2E tests (Playwright)

Install browsers once:

```bash
cd ui
bun run test:e2e:install
```

Run tests:

```bash
cd ui
bun run test:e2e
```

Note: e2e automatically builds the UI, generates `examples/demo.db`, and starts the FastAPI server on `127.0.0.1:8010`.

Note: there is a legacy `ui/src-tauri/` directory in the repo history; it is no longer used by the UI.
