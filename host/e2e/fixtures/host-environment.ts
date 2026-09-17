const inheritedEnvironmentKeys = [
  "APPDATA",
  "CARGO_HOME",
  "CARGO_INCREMENTAL",
  "CARGO_TARGET_DIR",
  "CI",
  "COMSPEC",
  "DBUS_SESSION_BUS_ADDRESS",
  "DISPLAY",
  "HOME",
  "LANG",
  "LOCALAPPDATA",
  "PATH",
  "PATHEXT",
  "PROGRAMDATA",
  "ProgramFiles",
  "ProgramFiles(x86)",
  "RUSTC_WRAPPER",
  "RUSTFLAGS",
  "RUSTUP_HOME",
  "SHELL",
  "SystemRoot",
  "TEMP",
  "TMP",
  "USER",
  "USERPROFILE",
  "WINDIR",
  "XAUTHORITY",
  "XDG_CONFIG_HOME",
  "XDG_RUNTIME_DIR",
];

export const hostE2eEnvironment = (overrides: NodeJS.ProcessEnv = {}): NodeJS.ProcessEnv =>
  Object.assign(
    Object.fromEntries(
      inheritedEnvironmentKeys.flatMap((key) => {
        const value = process.env[key];
        return value === undefined ? [] : [[key, value]];
      }),
    ),
    // An undefined override means "absent" so callers can strip defaults.
    Object.fromEntries(Object.entries(overrides).filter(([, value]) => value !== undefined)),
  );
