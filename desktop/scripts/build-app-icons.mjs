#!/usr/bin/env node
// Regenerate every product icon deterministically from the committed PNG
// sources in desktop/build/icons/ — never export .ico files by hand.
//   product icon      icons/mundus.png    → icon.ico, tray.ico, app-icons/mundus.*
//   bundled apps      icons/<app>.png     → app-icons/<app>.{png,ico}
import { mkdirSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import pngToIco from "png-to-ico";
import sharp from "sharp";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const sources = path.join(root, "build", "icons");
const target = path.join(root, "build", "app-icons");
mkdirSync(target, { recursive: true });

const ICO_SIZES = [16, 24, 32, 48, 64, 128, 256];

const icon = async (name, sourceFile) => {
  const image = sharp(path.join(sources, sourceFile));
  const png = path.join(target, `${name}.png`);
  const fullSize = await image.png().toBuffer();
  await sharp(fullSize).resize(256, 256).png({ compressionLevel: 9 }).toFile(png);
  writeFileSync(path.join(target, `${name}.ico`), await pngToIco(fullSize));
};

const toIco = async (sourceFile, outFile) => {
  const image = sharp(path.join(sources, sourceFile));
  const variants = await Promise.all(
    ICO_SIZES.map((size) => image.clone().resize(size, size).png().toBuffer()),
  );
  writeFileSync(outFile, await pngToIco(variants));
};

await Promise.all([
  icon("mundus", "mundus.png"),
  icon("memoria", "memoria.png"),
  icon("agenda", "agenda.png"),
  icon("dictation", "dictation.png"),
  // Installer + uninstaller icon and the Engine tray icon.
  toIco("mundus.png", path.join(root, "build", "icon.ico")),
  toIco("mundus.png", path.join(root, "build", "tray.ico")),
]);
