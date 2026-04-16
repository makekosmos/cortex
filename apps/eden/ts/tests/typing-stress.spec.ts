import { test, expect, _electron as electron, type Page } from "@playwright/test";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

type ElectronApp = Awaited<ReturnType<typeof electron.launch>>;

interface LaunchedApp {
  electronApp: ElectronApp;
  window: Page;
  homePath: string;
}

function parsePositiveInt(value: string | undefined, fallback: number) {
  const parsed = Number.parseInt(value ?? "", 10);
  if (!Number.isFinite(parsed) || parsed <= 0) {
    return fallback;
  }

  return parsed;
}

const STRESS_CHARS = parsePositiveInt(process.env.EDEN_STRESS_CHARS, 120_000);
const STRESS_LABEL = process.env.EDEN_STRESS_LABEL ?? "baseline";
const SHOULD_RUN = process.env.EDEN_RUN_STRESS_BENCHMARK === "1";

async function launchApp(
  homePath = fs.mkdtempSync(path.join(os.tmpdir(), "eden-stress-home-")),
  attempt = 0,
): Promise<LaunchedApp> {
  const electronApp = await electron.launch({
    args: ["."],
    env: {
      ...process.env,
      EDEN_BACKGROUND_LAUNCH: "1",
      HOME: homePath,
      NODE_ENV: "development",
    },
  });

  const window = await electronApp.firstWindow();
  await window.waitForLoadState("domcontentloaded");

  try {
    await window.waitForSelector(".app-container", { timeout: 10_000 });
    return { electronApp, window, homePath };
  } catch (error) {
    await electronApp.close();
    if (attempt >= 1) throw error;
    return launchApp(homePath, attempt + 1);
  }
}

async function ensureVault(launch: LaunchedApp, vaultPath: string): Promise<LaunchedApp> {
  await launch.window.evaluate(async (selectedVaultPath: string) => {
    await window.api.setVaultPath(selectedVaultPath);
    await window.api.updateSidebarConfig({
      widget: { width: 320, hidden: false },
    });
  }, vaultPath);

  await launch.electronApp.close();
  return launchApp(launch.homePath);
}

function buildStressText(targetChars: number) {
  const chunk =
    "Eden stress paragraph keeps typing pressure high while repeating realistic prose blocks, link markers, and calm writing flow. ";

  let text = "";
  while (text.length < targetChars) {
    text += chunk;
  }

  return text.slice(0, targetChars);
}

function buildStressDoc(targetChars: number) {
  const paragraphTarget = 320;
  const content: Array<Record<string, unknown>> = [];
  let remaining = targetChars;
  let paragraphIndex = 0;
  const structureCounts = {
    paragraphs: 0,
    headings: 0,
    blockquotes: 0,
    bulletLists: 0,
    codeBlocks: 0,
    wikilinkParagraphs: 0,
  };

  while (remaining > 0) {
    if (paragraphIndex % 24 === 0) {
      content.push({
        type: "heading",
        attrs: { level: 2 },
        content: [{ type: "text", text: `Stress section ${paragraphIndex / 24 + 1}` }],
      });
      structureCounts.headings += 1;
    }

    if (remaining > 0 && paragraphIndex % 24 === 6) {
      const quoteText = buildStressText(Math.min(220, remaining));
      content.push({
        type: "blockquote",
        content: [
          {
            type: "paragraph",
            content: [{ type: "text", text: quoteText }],
          },
        ],
      });
      remaining -= quoteText.length;
      structureCounts.blockquotes += 1;
    }

    if (remaining > 0 && paragraphIndex % 24 === 12) {
      const listItems = Array.from({ length: 3 }, (_, index) => {
        const itemText = buildStressText(Math.min(90, remaining));
        remaining -= itemText.length;
        return {
          type: "listItem",
          content: [
            {
              type: "paragraph",
              content: [{ type: "text", text: `${index + 1}. ${itemText}` }],
            },
          ],
        };
      });

      content.push({
        type: "bulletList",
        content: listItems,
      });
      structureCounts.bulletLists += 1;
    }

    if (remaining > 0 && paragraphIndex % 24 === 18) {
      const codeLines = [
        "function stressBenchmarkSection(index) {",
        "  const values = [];",
        "  for (let i = 0; i < 40; i += 1) values.push(index + i);",
        "  return values.join(', ');",
        "}",
      ];
      const codeText = codeLines.join("\n");
      content.push({
        type: "codeBlock",
        attrs: { language: "ts", wrap: true },
        content: [{ type: "text", text: codeText }],
      });
      remaining -= Math.min(remaining, codeText.length);
      structureCounts.codeBlocks += 1;
    }

    if (remaining <= 0) break;

    const paragraphChars = Math.min(paragraphTarget, remaining);
    const paragraphText =
      paragraphIndex % 10 === 0
        ? `${buildStressText(Math.max(0, paragraphChars - 32))} [[Stress Link ${paragraphIndex + 1}]]`
        : buildStressText(paragraphChars);
    content.push({
      type: "paragraph",
      content: [{ type: "text", text: paragraphText }],
    });

    remaining -= paragraphText.length;
    paragraphIndex += 1;
    structureCounts.paragraphs += 1;
    if (paragraphText.includes("[[")) {
      structureCounts.wikilinkParagraphs += 1;
    }
  }

  const totalChars = content.reduce((sum, node) => {
    const stack = [node];
    let nestedText = "";

    while (stack.length > 0) {
      const current = stack.pop();
      if (!current || typeof current !== "object") continue;

      if ("text" in current) {
        nestedText += String(current.text);
      }

      if ("content" in current && Array.isArray(current.content)) {
        for (const child of current.content) {
          stack.push(child as Record<string, unknown>);
        }
      }
    }

    return sum + nestedText.length;
  }, 0);

  return {
    doc: { type: "doc", content },
    paragraphCount: content.filter((node) => node.type === "paragraph").length,
    totalChars,
    structureCounts,
  };
}

async function seedStressNote(window: Page, title: string, targetChars: number) {
  const { doc, paragraphCount, totalChars, structureCounts } = buildStressDoc(targetChars);
  const now = Date.now();
  const contentJson = JSON.stringify(doc);

  await window.evaluate(
    async ({ noteTitle, noteContentJson, nowValue }: { noteTitle: string; noteContentJson: string; nowValue: number }) => {
      await window.api.saveEntry({
        id: crypto.randomUUID(),
        title: noteTitle,
        content_json: noteContentJson,
        created_at: nowValue,
        updated_at: nowValue,
        folder_id: null,
        type_id: null,
        header_layout: null,
        header_props_json: "{}",
        schema_version: 1,
      });
    },
    { noteTitle: title, noteContentJson: contentJson, nowValue: now },
  );

  return {
    paragraphCount,
    totalChars,
    contentBytes: Buffer.byteLength(contentJson, "utf8"),
    structureCounts,
  };
}

async function openStressNote(window: Page, title: string) {
  const note = window.locator(".widget-nav-item", { hasText: title }).first();
  const startedAt = performance.now();
  await note.click();
  await window.waitForSelector(".editor-wrapper", { state: "visible" });
  await window.waitForFunction(
    (expectedTitle) => {
      const input = document.querySelector<HTMLInputElement>(".title-input");
      return input?.value === expectedTitle;
    },
    title,
  );
  return Math.round((performance.now() - startedAt) * 100) / 100;
}

async function captureTypingSummary(window: Page, text: string) {
  await window.evaluate(() => {
    window.__edenPerf?.reset();
  });

  const editor = window.locator(".ProseMirror");
  await editor.click();
  await window.keyboard.type(text, { delay: 3 });
  await window.waitForTimeout(1_300);

  return window.evaluate(() => window.__edenPerf?.getSummary() ?? null);
}

async function toggleZenMode(window: Page) {
  const shortcut = process.platform === "darwin" ? "Meta+Alt+Z" : "Control+Alt+Z";
  await window.keyboard.press(shortcut);
}

test.describe("Eden typing stress benchmark", () => {
  test.skip(!SHOULD_RUN, "Run only when EDEN_RUN_STRESS_BENCHMARK=1");

  test("captures a 100k+ typing baseline artifact", async () => {
    test.setTimeout(120_000);

    const repoRoot = path.resolve(process.cwd(), "../../..");
    const artifactsDir = path.join(
      repoRoot,
      ".agent/tasks/2026-04-15-eden-100k-stress-benchmark/artifacts",
    );
    const vaultPath = fs.mkdtempSync(path.join(os.tmpdir(), "eden-stress-vault-"));
    const noteTitle = `Stress note ${STRESS_CHARS}`;
    const typingBurst = " benchmark burst text ".repeat(18);

    let launch: LaunchedApp | null = null;

    try {
      launch = await launchApp();
      launch = await ensureVault(launch, vaultPath);

      const seeded = await seedStressNote(launch.window, noteTitle, STRESS_CHARS);

      await launch.electronApp.close();
      launch = await launchApp(launch.homePath);
      await launch.window.waitForSelector(".widget-sidebar", { state: "visible" });

      const openMs = await openStressNote(launch.window, noteTitle);
      const normalSummary = await captureTypingSummary(launch.window, typingBurst);

      await toggleZenMode(launch.window);
      await launch.window.waitForSelector('[data-testid="zen-mode-exit"]', { state: "visible" });
      const zenSummary = await captureTypingSummary(launch.window, typingBurst);

      const artifact = {
        label: STRESS_LABEL,
        capturedAt: new Date().toISOString(),
        environment: {
          platform: process.platform,
          arch: process.arch,
          node: process.version,
        },
        scenario: {
          targetChars: STRESS_CHARS,
          totalChars: seeded.totalChars,
          paragraphCount: seeded.paragraphCount,
          contentBytes: seeded.contentBytes,
          structureCounts: seeded.structureCounts,
          typingBurstChars: typingBurst.length,
        },
        metrics: {
          openMs,
          normalSummary,
          zenSummary,
        },
      };

      fs.mkdirSync(artifactsDir, { recursive: true });
      const artifactPath = path.join(artifactsDir, `${STRESS_LABEL}-typing-stress-${STRESS_CHARS}.json`);
      fs.writeFileSync(artifactPath, JSON.stringify(artifact, null, 2));

      expect(normalSummary).not.toBeNull();
      expect(zenSummary).not.toBeNull();
      expect(normalSummary?.inputToNextPaint.count ?? 0).toBeGreaterThan(0);
      expect(zenSummary?.inputToNextPaint.count ?? 0).toBeGreaterThan(0);
      expect(seeded.totalChars).toBeGreaterThanOrEqual(STRESS_CHARS);
    } finally {
      if (launch) {
        await launch.electronApp.close();
        fs.rmSync(launch.homePath, { recursive: true, force: true });
      }

      fs.rmSync(vaultPath, { recursive: true, force: true });
    }
  });
});
