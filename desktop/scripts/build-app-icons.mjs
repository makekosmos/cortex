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

// 24-bit BMP from a top-to-bottom RGB buffer (Sharp's raw() layout).
function writeBmp(file, width, height, rgb) {
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
    parts.push(rgb.subarray(y * width * 3, (y + 1) * width * 3));
    if (padding > 0) parts.push(Buffer.alloc(padding));
  }
  writeFileSync(file, Buffer.concat(parts));
}

async function renderInstallerBitmap(width, height, iconSize) {
  const background = { r: 22, g: 20, b: 30 }; // matches the Mundus icon backdrop
  const iconSource = sharp(path.join(sources, "mundus.png"))
    .resize(iconSize, iconSize, { fit: "contain", background })
    .raw()
    .toBuffer({ resolveWithObject: true });
  const base = sharp({
    create: { width, height, channels: 3, background },
  }).raw();
  const { data, info } = await iconSource;
  // Composite only if the icon rendered to the expected size; otherwise fall
  // back to the plain background (should never happen with a valid source).
  if (info.width === iconSize && info.height === iconSize) {
    const composite = await base
      .composite([
        { input: data, raw: { width: iconSize, height: iconSize, channels: 3 }, gravity: "center" },
      ])
      .raw()
      .toBuffer();
    return composite;
  }
  return base.raw().toBuffer();
}

async function installerAssets() {
  const assetsDir = path.join(root, "build", "installer-assets");
  mkdirSync(assetsDir, { recursive: true });
  // MUI2 header image is shown at the top-right of interior pages.
  const header = await renderInstallerBitmap(150, 57, 40);
  writeBmp(path.join(assetsDir, "header.bmp"), 150, 57, header);
  // Welcome/finish sidebar bitmap.
  const welcome = await renderInstallerBitmap(164, 314, 128);
  writeBmp(path.join(assetsDir, "welcome.bmp"), 164, 314, welcome);
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
