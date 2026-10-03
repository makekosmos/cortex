import { createHash } from "node:crypto";
import {
  cpSync,
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import path from "node:path";
import { spawnSync } from "node:child_process";

const desktopRoot = path.resolve(import.meta.dirname, "..");
const repoRoot = path.resolve(desktopRoot, "..", "..");
const outRoot = path.join(repoRoot, ".tmp", "engine-addons");

const PYTHON_VERSION = "3.11.9";
const FASTER_WHISPER_VERSION = "1.2.1";
const CUBLAS_VERSION = "12.9.2.10";
const CUDNN_VERSION = "9.23.2.1";

const RUNTIME_ARCHIVE = "mundus-faster-whisper-runtime-win-x64.zip";
const CUDA_ARCHIVE = "mundus-cuda-libs-cuda12-cudnn9.zip";

function run(cmd, args, opts = {}) {
  const result = spawnSync(cmd, args, {
    cwd: opts.cwd ?? repoRoot,
    stdio: "inherit",
    shell: false,
    ...opts,
  });
  if (result.status !== 0) {
    throw new Error(`${cmd} ${args.join(" ")} failed with exit ${result.status}`);
  }
}

async function download(url, target) {
  if (existsSync(target)) return;
  console.log(`[runtime] download ${url}`);
  mkdirSync(path.dirname(target), { recursive: true });
  const response = await fetch(url);
  if (!response.ok) {
    throw new Error(`download failed ${response.status}: ${url}`);
  }
  const buffer = Buffer.from(await response.arrayBuffer());
  writeFileSync(target, buffer);
}

function expandZip(zipPath, targetDir) {
  rmSync(targetDir, { recursive: true, force: true });
  mkdirSync(targetDir, { recursive: true });
  run("tar", ["-xf", zipPath, "-C", targetDir]);
}

function zipDir(sourceDir, archivePath) {
  rmSync(archivePath, { force: true });
  run("tar", ["-a", "-c", "-f", archivePath, "-C", sourceDir, "."]);
}

function sha256(file) {
  return createHash("sha256").update(readFileSync(file)).digest("hex");
}

function patchEmbeddedPython(root) {
  const pth = path.join(root, "python311._pth");
  let text = readFileSync(pth, "utf8");
  text = text.replace(/^#?import site$/m, "import site");
  if (!text.includes("Lib/site-packages")) {
    text = `${text.trimEnd()}\nLib/site-packages\n`;
  }
  writeFileSync(pth, text);
}

function findDirs(root, suffix) {
  const found = [];
  function walk(dir) {
    if (dir.replace(/\\/g, "/").endsWith(suffix)) found.push(dir);
    for (const child of readdirSync(dir)) {
      const next = path.join(dir, child);
      if (statSync(next).isDirectory()) walk(next);
    }
  }
  walk(root);
  return found;
}

function copyFirstDir(root, suffix, target) {
  const [source] = findDirs(root, suffix);
  if (!source) throw new Error(`missing wheel directory: ${suffix}`);
  rmSync(target, { recursive: true, force: true });
  cpSync(source, target, { recursive: true });
}

async function buildRuntime() {
  const downloads = path.join(outRoot, "downloads");
  const stage = path.join(outRoot, "stage", "faster-whisper-runtime-win-x64");
  const archive = path.join(outRoot, RUNTIME_ARCHIVE);
  const pythonZip = path.join(downloads, `python-${PYTHON_VERSION}-embed-amd64.zip`);
  const getPip = path.join(downloads, "get-pip.py");

  await download(
    `https://www.python.org/ftp/python/${PYTHON_VERSION}/python-${PYTHON_VERSION}-embed-amd64.zip`,
    pythonZip,
  );
  await download("https://bootstrap.pypa.io/get-pip.py", getPip);

  expandZip(pythonZip, stage);
  patchEmbeddedPython(stage);
  const python = path.join(stage, "python.exe");
  run(python, [getPip, "--no-warn-script-location"]);
  run(python, [
    "-m",
    "pip",
    "install",
    "--no-warn-script-location",
    "--upgrade",
    `faster-whisper==${FASTER_WHISPER_VERSION}`,
  ]);
  run(python, [
    "-c",
    "from faster_whisper import WhisperModel; import ctranslate2; print('runtime-ok', ctranslate2.__version__)",
  ]);
  zipDir(stage, archive);
  return { archive, sha256: sha256(archive) };
}

async function buildCuda() {
  const wheels = path.join(outRoot, "wheels");
  const extracted = path.join(outRoot, "stage", "cuda-wheels");
  const stage = path.join(outRoot, "stage", "cuda-libs-cuda12-cudnn9");
  const archive = path.join(outRoot, CUDA_ARCHIVE);

  rmSync(wheels, { recursive: true, force: true });
  rmSync(extracted, { recursive: true, force: true });
  rmSync(stage, { recursive: true, force: true });
  mkdirSync(wheels, { recursive: true });
  mkdirSync(extracted, { recursive: true });
  mkdirSync(stage, { recursive: true });

  run("python", [
    "-m",
    "pip",
    "download",
    "--only-binary=:all:",
    "--dest",
    wheels,
    `nvidia-cublas-cu12==${CUBLAS_VERSION}`,
    `nvidia-cudnn-cu12==${CUDNN_VERSION}`,
  ]);
  for (const file of readdirSync(wheels).filter((name) => name.endsWith(".whl"))) {
    expandZip(path.join(wheels, file), path.join(extracted, file.replace(/\.whl$/, "")));
  }
  copyFirstDir(extracted, "nvidia/cublas/bin", path.join(stage, "cublas", "bin"));
  copyFirstDir(extracted, "nvidia/cudnn/bin", path.join(stage, "cudnn", "bin"));
  zipDir(stage, archive);
  return { archive, sha256: sha256(archive) };
}

const runtime = await buildRuntime();
const cuda = await buildCuda();
const manifest = {
  fasterWhisperVersion: FASTER_WHISPER_VERSION,
  pythonVersion: PYTHON_VERSION,
  cublasVersion: CUBLAS_VERSION,
  cudnnVersion: CUDNN_VERSION,
  runtime,
  cuda,
};
writeFileSync(path.join(outRoot, "manifest.json"), JSON.stringify(manifest, null, 2));
console.log(JSON.stringify(manifest, null, 2));
