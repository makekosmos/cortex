/**
 * electron-builder afterPack hook: embed icon в final exe.
 *
 * Зачем: `win.signAndEditExecutable: false` отрубает встроенный rcedit
 * у electron-builder (workaround под падение winCodeSign symlinks на Windows
 * без Developer Mode). Без него .exe выходит с дефолтной Electron-иконкой
 * → нет иконки в taskbar / Start Menu / Explorer. Здесь дёргаем rcedit
 * руками из npm-пакета `rcedit` (бандлит rcedit-x64.exe) и проставляем
 * иконку + version-string метаданные. productName читается из контекста,
 * хук переносим между приложениями Kepler без правок.
 */

const path = require("node:path");
const fs = require("node:fs");
const { rcedit } = require("rcedit");
const sharp = require("sharp");
const pngToIcoMod = require("png-to-ico");
const pngToIco = pngToIcoMod.default || pngToIcoMod;

async function ensureIco(sourcePath, icoPath, label) {
  if (!fs.existsSync(sourcePath)) {
    console.warn(`[afterPack] ${label} source not found: ${sourcePath}`);
    return false;
  }
  let needsBuild = !fs.existsSync(icoPath);
  if (!needsBuild) {
    const sourceMtime = fs.statSync(sourcePath).mtimeMs;
    const icoMtime = fs.statSync(icoPath).mtimeMs;
    if (sourceMtime > icoMtime) needsBuild = true;
  }
  if (!needsBuild) return true;
  console.log(`[afterPack] converting ${path.basename(sourcePath)} → ${path.basename(icoPath)}`);
  const buf = await pngToIco(sourcePath);
  fs.writeFileSync(icoPath, buf);
  return true;
}

async function ensureTrayIcoFromSvg(svgPath, icoPath) {
  if (!fs.existsSync(svgPath)) {
    console.warn(`[afterPack] tray icon source not found: ${svgPath}`);
    return false;
  }
  let needsBuild = !fs.existsSync(icoPath);
  if (!needsBuild) {
    const svgMtime = fs.statSync(svgPath).mtimeMs;
    const icoMtime = fs.statSync(icoPath).mtimeMs;
    if (svgMtime > icoMtime) needsBuild = true;
  }
  if (!needsBuild) return true;
  console.log(`[afterPack] converting ${path.basename(svgPath)} → ${path.basename(icoPath)}`);
  const pngBuffer = await sharp(fs.readFileSync(svgPath), { density: 384 })
    .resize(256, 256, {
      fit: "contain",
      background: { r: 0, g: 0, b: 0, alpha: 0 },
    })
    .png()
    .toBuffer();
  const icoBuffer = await pngToIco(pngBuffer);
  fs.writeFileSync(icoPath, icoBuffer);
  return true;
}

module.exports = async function afterPack(context) {
  if (context.electronPlatformName !== "win32") return;

  const appOutDir = context.appOutDir;
  const exeName = `${context.packager.appInfo.productFilename}.exe`;
  const exePath = path.join(appOutDir, exeName);
  if (!fs.existsSync(exePath)) {
    console.warn(`[afterPack] exe not found: ${exePath}`);
    return;
  }

  const buildDir = path.join(__dirname);
  const appPngPath = path.join(buildDir, "icon.png");
  const appIcoPath = path.join(buildDir, "icon.ico");
  const traySvgPath = path.join(buildDir, "tray.svg");
  const trayIcoPath = path.join(buildDir, "tray.ico");

  // Конвертируем PNG → ICO / SVG → ICO один раз и кэшируем рядом.
  // Перегенерируем только если источник новее ICO (или ICO отсутствует).
  const appIconReady = await ensureIco(appPngPath, appIcoPath, "app icon");
  const trayIconReady = await ensureTrayIcoFromSvg(traySvgPath, trayIcoPath);
  if (!appIconReady || !trayIconReady) return;

  const { productName, version, copyright, appId } = context.packager.appInfo;
  console.log(`[afterPack] embedding icon + metadata into ${exePath}`);
  await rcedit(exePath, {
    icon: appIcoPath,
    "version-string": {
      ProductName: productName,
      FileDescription: productName,
      CompanyName: "Kazui",
      LegalCopyright: copyright || `Copyright © ${new Date().getFullYear()} Kazui`,
      OriginalFilename: exeName,
      InternalName: appId || productName,
    },
    "file-version": version,
    "product-version": version,
  });
  console.log("[afterPack] done.");
};
