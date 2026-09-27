import { protocol } from "electron";
import { existsSync } from "node:fs";
import { readFile, stat } from "node:fs/promises";
import type { ArkClient } from "@kosmos/ark";
import { keplerLog } from "./logging";
import {
  APP_ICON_PROTOCOL,
  bufferToArrayBuffer,
  parseAppIconRequestUrl,
} from "./app-icon-protocol";
import {
  LOCAL_IMAGE_PROTOCOL,
  localImageMimeType,
  parseLocalImageRequestUrl,
  resolveLocalImagePath,
} from "../../shared/electron/local-image-protocol";
import { MARKDOWN_IMAGE_MAX_BYTES } from "../../shared/electron/markdown-vault";

interface MainProtocolOptions {
  awaitArkReady(timeoutMs?: number): Promise<ArkClient>;
}

const appIconBytesCache = new Map<string, Buffer>();

export function clearMainProtocolCaches(): void {
  appIconBytesCache.clear();
}

export function registerMainProtocols(options: MainProtocolOptions): void {
  registerAppIconProtocol(options);
  registerLocalImageProtocol();
}

function registerAppIconProtocol(options: MainProtocolOptions): void {
  protocol.handle(APP_ICON_PROTOCOL, async (request) => {
    let appId: string | null = null;
    try {
      appId = parseAppIconRequestUrl(request.url);
      if (!appId) {
        return new Response(null, { status: 404 });
      }

      const cached = appIconBytesCache.get(appId);
      if (cached) {
        return new Response(bufferToArrayBuffer(cached), {
          headers: {
            "content-type": "image/png",
            "cache-control": "max-age=3600",
          },
        });
      }

      const client = await options.awaitArkReady(5_000);
      // SAFETY: app_index.icon_path returns an object with an optional path.
      const resp = (await client.invokeOperation({
        operation: "app_index.icon_path",
        id: appId,
        // SAFETY: The surrounding boundary establishes this documented contract.
      })) as { path?: string };
      const iconPath = resp?.path ?? "";
      if (!iconPath || !existsSync(iconPath)) {
        return new Response(null, { status: 404 });
      }

      // См. postmortems.md § 2026-06-09: only visible <img> requests touch icon files.
      const bytes = await readFile(iconPath);
      appIconBytesCache.set(appId, bytes);
      return new Response(bufferToArrayBuffer(bytes), {
        headers: {
          "content-type": "image/png",
          "cache-control": "max-age=3600",
        },
      });
    } catch (e) {
      keplerLog.warn("app-icon", "kosmos-icon protocol lookup failed", {
        appId: appId ?? null,
        err: String(e),
      });
      return new Response(null, { status: 404 });
    }
  });
}

function registerLocalImageProtocol(): void {
  protocol.handle(LOCAL_IMAGE_PROTOCOL, async (request) => {
    let imagePath: string | null = null;
    try {
      imagePath = parseLocalImageRequestUrl(request.url);
      if (!imagePath) {
        return new Response(null, { status: 404 });
      }

      const resolvedPath = resolveLocalImagePath(imagePath);
      if (!resolvedPath) {
        return new Response(null, { status: 404 });
      }

      // Vault scan caps image size before linking, but the protocol handler
      // is reachable with any crafted URL — bound the buffered read too.
      const meta = await stat(resolvedPath);
      if (!meta.isFile() || meta.size > MARKDOWN_IMAGE_MAX_BYTES) {
        return new Response(null, { status: 404 });
      }
      const bytes = await readFile(resolvedPath);
      if (bytes.length > MARKDOWN_IMAGE_MAX_BYTES) {
        return new Response(null, { status: 404 });
      }
      return new Response(bufferToArrayBuffer(bytes), {
        headers: {
          "content-type": localImageMimeType(resolvedPath),
          "cache-control": "max-age=3600",
          "access-control-allow-origin": "*",
        },
      });
    } catch (e) {
      keplerLog.warn("local-image", "kosmos-local-image protocol lookup failed", {
        imagePath: imagePath ?? null,
        err: String(e),
      });
      return new Response(null, { status: 404 });
    }
  });
}
