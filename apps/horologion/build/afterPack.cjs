/**
 * electron-builder afterPack hook: embed icon into Strontium.exe.
 *
 * Зачем: `win.signAndEditExecutable: false` отрубает встроенный rcedit
 * у electron-builder (workaround под падение winCodeSign symlinks на Windows
 * без Developer Mode). Без него .exe выходит с дефолтной Electron-иконкой
 * → нет иконки в taskbar / Start Menu / Explorer. Здесь дёргаем rcedit
 * руками из npm-пакета `rcedit` (бандлит rcedit-x64.exe).
 */

const path = require("node:path");
const fs = require("node:fs");
const { rcedit } = require("rcedit");
const pngToIcoMod = require("png-to-ico");
const pngToIco = pngToIcoMod.default || pngToIcoMod;

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
  const pngPath = path.join(buildDir, "icon.png");
  const icoPath = path.join(buildDir, "icon.ico");

  // Конвертируем PNG → ICO один раз и кэшируем рядом. Перегенерируем
  // только если ICO старше PNG (или отсутствует).
  let needsBuild = !fs.existsSync(icoPath);
  if (!needsBuild) {
    const pngMtime = fs.statSync(pngPath).mtimeMs;
    const icoMtime = fs.statSync(icoPath).mtimeMs;
    if (pngMtime > icoMtime) needsBuild = true;
  }
  if (needsBuild) {
    console.log("[afterPack] converting icon.png → icon.ico");
    const buf = await pngToIco(pngPath);
    fs.writeFileSync(icoPath, buf);
  }

  const { productName, version, copyright, appId } = context.packager.appInfo;
  console.log(`[afterPack] embedding icon + metadata into ${exePath}`);
  await rcedit(exePath, {
    icon: icoPath,
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
