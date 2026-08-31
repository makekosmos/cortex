const inheritedEnvironmentKeys = [
  "APPDATA",
  "CARGO_HOME",
  "CARGO_INCREMENTAL",
  "CARGO_TARGET_DIR",
  "CI",
  "COMSPEC",
  "LOCALAPPDATA",
  "PATH",
  "PATHEXT",
  "PROGRAMDATA",
  "ProgramFiles",
  "ProgramFiles(x86)",
  "RUSTC_WRAPPER",
  "RUSTFLAGS",
  "RUSTUP_HOME",
  "SystemRoot",
  "TEMP",
  "TMP",
  "USERPROFILE",
  "WINDIR",
];

export const hostE2eEnvironment = (overrides: NodeJS.ProcessEnv = {}): NodeJS.ProcessEnv =>
  Object.assign(
    Object.fromEntries(
      inheritedEnvironmentKeys.flatMap((key) => {
        const value = process.env[key];
        return value === undefined ? [] : [[key, value]];
      }),
    ),
    overrides,
  );
