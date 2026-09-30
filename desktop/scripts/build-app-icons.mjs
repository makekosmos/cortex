#!/usr/bin/env node
// Regenerate every product icon deterministically from the committed PNG
// sources in desktop/build/icons/ — never export .ico files by hand.
//   product icon      icons/mundus.png    → icon.ico, tray.ico, app-icons/mundus.*
//   bundled apps      icons/<app>.png     → app-icons/<app>.{png,ico}
//   installer bitmaps icons/mundus.png    → installer-assets/{header,welcome}.bmp
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

// 24-bit BMP from a top-to-bottom RGB buffer (Sharp's raw() layout). BMP
// stores rows bottom-up and pixels as BGR.
function writeBmp(file, width, height, rgb) {
  if (rgb.length !== width * height * 3) {
    throw new Error(`${file}: expected ${width}x${height} RGB, got ${rgb.length} bytes`);
  }
  const rowSize = Math.ceil((width * 3) / 4) * 4;
  const padding = rowSize - width * 3;
  const pixelDataSize = rowSize * height;
  const fileSize = 54 + pixelDataSize;
  const header = Buffer.alloc(54);
  header.write("BM", 0);
  header.writeUInt32LE(fileSize, 2);
  header.writeUInt32LE(0, 6);
  header.writeUInt32LE(54, 10);
  header.writeUInt32LE(40, 14);
  header.writeInt32LE(width, 18);
  header.writeInt32LE(height, 22);
  header.writeUInt16LE(1, 26);
  header.writeUInt16LE(24, 28);
  header.writeUInt32LE(0, 30);
  header.writeUInt32LE(pixelDataSize, 34);
  header.writeUInt32LE(2835, 38);
  header.writeUInt32LE(2835, 42);
  header.writeUInt32LE(0, 46);
  header.writeUInt32LE(0, 50);
  const parts = [header];
  for (let y = height - 1; y >= 0; y--) {
    const row = Buffer.from(rgb.subarray(y * width * 3, (y + 1) * width * 3));
    for (let x = 0; x < row.length; x += 3) [row[x], row[x + 2]] = [row[x + 2], row[x]];
    parts.push(row);
    if (padding > 0) parts.push(Buffer.alloc(padding));
  }
  writeFileSync(file, Buffer.concat(parts));
}

const ICON_BACKDROP = { r: 22, g: 20, b: 30 }; // matches the Mundus icon backdrop
const WIZARD_HEADER = { r: 255, g: 255, b: 255 }; // MUI2 header strip is COLOR_WINDOW

// Writes installer-assets/<file>: the icon (with its alpha edge) on a
// solid background at (left, top). The flatten runs as a second pipeline
// because Sharp applies composite last.
async function installerBitmap(file, { width, height, background, iconSize, left, top }) {
  const iconPng = await sharp(path.join(sources, "mundus.png"))
    .resize(iconSize, iconSize, { fit: "contain", background: { ...background, alpha: 0 } })
    .png()
    .toBuffer();
  const composed = await sharp({ create: { width, height, channels: 3, background } })
    .composite([{ input: iconPng, left, top }])
    .png()
    .toBuffer();
  const rgb = await sharp(composed).flatten({ background }).raw().toBuffer();
  writeBmp(path.join(root, "build", "installer-assets", file), width, height, rgb);
}

// Rendered at 2x the MUI2 control size (150x57 header, 164x314 welcome) and
// scaled down to fit by the installer, so they stay sharp at 200% scaling.
async function installerAssets() {
  mkdirSync(path.join(root, "build", "installer-assets"), { recursive: true });
  // Header strip of interior pages: a small icon near the right edge, as in
  // native Windows wizards (MUI_HEADERIMAGE_RIGHT).
  await installerBitmap("header.bmp", {
    width: 300,
    height: 114,
    background: WIZARD_HEADER,
    iconSize: 72,
    left: 300 - 72 - 24,
    top: 21,
  });
  // Welcome/finish sidebar in the icon's own backdrop colour.
  await installerBitmap("welcome.bmp", {
    width: 328,
    height: 628,
    background: ICON_BACKDROP,
    iconSize: 224,
    left: 52,
    top: 202,
  });
}

await Promise.all([
  icon("mundus", "mundus.png"),
  icon("memoria", "memoria.png"),
  icon("agenda", "agenda.png"),
  icon("dictation", "dictation.png"),
  // Installer + uninstaller icon and the Engine tray icon.
  toIco("mundus.png", path.join(root, "build", "icon.ico")),
  toIco("mundus.png", path.join(root, "build", "tray.ico")),
  installerAssets(),
]);
