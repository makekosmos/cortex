import { defineConfig, devices } from "@playwright/test";
import path from "path";
import { fileURLToPath } from "url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, "..");

export default defineConfig({
    testDir: "./tests/e2e",
    timeout: 30_000,
    expect: {
        timeout: 10_000,
    },
    outputDir: "test-results",
    retries: process.env.CI ? 1 : 0,
    use: {
        baseURL: "http://127.0.0.1:8010",
        trace: "on-first-retry",
        screenshot: "only-on-failure",
        video: "retain-on-failure",
    },
    projects: [
        {
            name: "chromium",
            use: { ...devices["Desktop Chrome"] },
        },
    ],
    webServer: {
        // We serve the built UI from the FastAPI server, so e2e runs against a single origin.
        command:
            "python3 core/generate_demo_db.py --output examples/demo.db --overwrite --count 200 && " +
            "(cd ui && bun run build) && " +
            "./.venv/bin/uvicorn server.app:app --host 127.0.0.1 --port 8010 --log-level warning",
        url: "http://127.0.0.1:8010/health",
        cwd: repoRoot,
        // Avoid clashing with a manually running dev server on 8000.
        reuseExistingServer: false,
        timeout: 120_000,
        env: {
            LIFE_DB_PATH: path.join(repoRoot, "examples", "demo.db"),
            LIFE_API_KEY: "test-secret",
        },
    },
});
