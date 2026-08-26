#!/usr/bin/env node
import { mkdirSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import pngToIco from "png-to-ico";
import sharp from "sharp";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const source = path.join(root, "build", "icon.png");
const target = path.join(root, "build", "app-icons");
mkdirSync(target, { recursive: true });
const icon = async (name, image) => {
  const png = path.join(target, `${name}.png`);
  const fullSize = await image.png().toBuffer();
  await sharp(fullSize).resize(256, 256).png({ compressionLevel: 9 }).toFile(png);
  writeFileSync(path.join(target, `${name}.ico`), await pngToIco(fullSize));
};
await Promise.all([
  icon("kosmos", sharp(source)),
  icon("memoria", sharp(source).tint("#7c00ff")),
  icon("agenda", sharp(source).tint("#27c7b4")),
  icon("arcadia", sharp(source).tint("#ef3f52")),
  icon("dictation", sharp(source).tint("#c65df7")),
]);
