//! macOS apply half of the self-updater (KOS-377): mount the downloaded
//! DMG, copy its `.app` next to the running bundle, and hand the swap +
//! relaunch to a detached helper script — the running Manager and Engine
//! (this process) must quit before the bundle is replaced, and the helper
//! outlives both. Pure planning ([`MacInstallPlan`], [`bundle_for_exe`],
//! [`helper_script`], [`read_bundle_id`]) is testable off-mac; only the thin
//! process layer ([`apply_update`]) spawns `hdiutil` / `ditto` / `/bin/sh`.
//!
//! The bundle is replaced in place — `/Applications/Mundus Manager.app` or
//! wherever the user put it — keeping the bundle id and the DMG build's
//! ad-hoc signature class, so Gatekeeper sees a drag-install equivalent.
//! Staging lives in a sibling directory of the target, so the final `mv` is
//! a same-volume rename and the preflight `create_dir` already proves write
//! permission or fails with a clear error.
use std::path::{Path, PathBuf};

use super::UpdaterError;

/// Everything the apply needs, derived once from the running bundle path.
/// `staging_dir` is a sibling of `app` — same volume, so the final `mv` is
/// an atomic rename — and holds the ditto target (`<AppName>.app`), the
/// swap backup (`old.app`), the DMG mountpoint (`mnt`; the DMG's volume
/// name is versioned, e.g. "Mundus 0.10.11", so it is never guessed) and
/// the helper (`apply.sh`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MacInstallPlan {
    pub app: PathBuf,
    pub staging_dir: PathBuf,
    pub staged_app: PathBuf,
    pub backup_app: PathBuf,
    pub mount_dir: PathBuf,
    pub script_path: PathBuf,
}

/// `<App>.app` for a binary at `<App>.app/Contents/MacOS/<exe>` — the DMG
/// layout `desktop/scripts/package-macos-dmg.mjs` ships. Anything else
/// (dev build, bare binary) is not updatable this way.
pub(crate) fn bundle_for_exe(exe: &Path) -> Result<PathBuf, UpdaterError> {
    let bundle = exe
        .parent()
        .and_then(|macos| macos.parent())
        .and_then(|contents| contents.parent())
        .filter(|path| path.extension().is_some_and(|ext| ext == "app"))
        .ok_or_else(|| {
            UpdaterError::Io(format!(
                "updater: {} is not inside a .app bundle — update via a manual DMG install",
                exe.display()
            ))
        })?;
    Ok(bundle.to_path_buf())
}

/// The .app this Engine runs from. `MUNDUS_UPDATER_APP_BUNDLE` overrides it
/// so tests (and manual runs outside /Applications) can point the apply at
/// a scratch bundle.
fn installed_app_bundle() -> Result<PathBuf, UpdaterError> {
    if let Some(path) = std::env::var_os("MUNDUS_UPDATER_APP_BUNDLE") {
        return Ok(PathBuf::from(path));
    }
    let exe = std::env::current_exe().map_err(|error| UpdaterError::Io(error.to_string()))?;
    bundle_for_exe(&exe)
}

impl MacInstallPlan {
    pub(crate) fn for_app(app: &Path) -> Result<Self, UpdaterError> {
        let app_name = app
            .file_name()
            .ok_or_else(|| UpdaterError::Io("updater: app bundle path has no name".into()))?;
        let parent = app.parent().ok_or_else(|| {
            UpdaterError::Io(format!(
                "updater: {} has no parent directory",
                app.display()
            ))
        })?;
        for path in [app, parent] {
            if path.to_string_lossy().contains('\'') {
                return Err(UpdaterError::Io(format!(
                    "updater: path {} contains a quote — cannot embed it in the apply script",
                    path.display()
                )));
            }
        }
        let staging_dir = parent.join(format!(".mundus-update-{}", std::process::id()));
        Ok(Self {
            app: app.to_path_buf(),
            staged_app: staging_dir.join(app_name),
            backup_app: staging_dir.join("old.app"),
            mount_dir: staging_dir.join("mnt"),
            script_path: staging_dir.join("apply.sh"),
            staging_dir,
        })
    }
}

/// `CFBundleIdentifier` from `Contents/Info.plist` — a plain string lookup,
/// enough for the flat plist `package-macos-dmg.mjs` writes.
pub(crate) fn read_bundle_id(app: &Path) -> Result<String, UpdaterError> {
    let plist = std::fs::read_to_string(app.join("Contents/Info.plist"))
        .map_err(|error| UpdaterError::Io(error.to_string()))?;
    let key = "<key>CFBundleIdentifier</key>";
    let after = plist
        .split_once(key)
        .and_then(|(_, rest)| rest.split_once("<string>"))
        .and_then(|(_, rest)| rest.split_once("</string>"))
        .map(|(id, _)| id.trim())
        .filter(|id| !id.is_empty());
    after.map(str::to_string).ok_or_else(|| {
        UpdaterError::Io(format!(
            "updater: no CFBundleIdentifier in {}/Contents/Info.plist",
            app.display()
        ))
    })
}

/// The detached helper: shut the Engine down gracefully through its own
/// control channel (`--shutdown`, same as the NSIS installer's stop step —
/// osascript would cost an Automation consent prompt), wait for every
/// process running from the bundle to die (TERM, then KILL), swap staged
/// over installed, relaunch, clean up. Runs under `/bin/sh`; everything is
/// single-quoted literals baked at generation time — paths with quotes are
/// rejected by [`MacInstallPlan::for_app`].
pub(crate) fn helper_script(plan: &MacInstallPlan, bundle_id: &str) -> String {
    format!(
        concat!(
            "#!/bin/sh\n",
            "# Mundus self-update helper — detached; outlives Engine and Manager.\n",
            "set -u\n",
            "APP='{app}'\n",
            "STAGED='{staged}'\n",
            "BACKUP='{backup}'\n",
            "STAGING='{staging}'\n",
            "BUNDLE_ID='{bundle_id}'\n",
            "\"$APP/Contents/MacOS/mundus-engine\" --shutdown >/dev/null 2>&1\n",
            "i=0\n",
            "while [ \"$i\" -lt 150 ]; do\n",
            "  pgrep -f \"$APP/Contents/MacOS/\" >/dev/null 2>&1 || break\n",
            "  i=$((i + 1)); sleep 0.2\n",
            "done\n",
            "pids=$(pgrep -f \"$APP/Contents/MacOS/\" 2>/dev/null) && kill $pids 2>/dev/null\n",
            "sleep 1\n",
            "pids=$(pgrep -f \"$APP/Contents/MacOS/\" 2>/dev/null) && kill -9 $pids 2>/dev/null\n",
            "if mv \"$APP\" \"$BACKUP\" && mv \"$STAGED\" \"$APP\"; then\n",
            "  xattr -dr com.apple.quarantine \"$APP\" >/dev/null 2>&1\n",
            "  open \"$APP\" || open -b \"$BUNDLE_ID\"\n",
            "  rm -rf \"$BACKUP\"\n",
            "  rm -rf \"$STAGING\"\n",
            "else\n",
            "  [ -d \"$APP\" ] || mv \"$BACKUP\" \"$APP\" 2>/dev/null\n",
            "  rm -rf \"$STAGED\" 2>/dev/null\n",
            "  echo 'mundus updater: bundle swap failed' >&2\n",
            "  exit 1\n",
            "fi\n"
        ),
        app = plan.app.display(),
        staged = plan.staged_app.display(),
        backup = plan.backup_app.display(),
        staging = plan.staging_dir.display(),
        bundle_id = bundle_id,
    )
}

/// The single `*.app` at a mounted DMG's root.
fn find_app_in(dir: &Path) -> Result<PathBuf, UpdaterError> {
    let mut apps = std::fs::read_dir(dir)
        .map_err(|error| UpdaterError::Io(error.to_string()))?
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "app"));
    match (apps.next(), apps.next()) {
        (Some(app), None) => Ok(app),
        (None, _) => Err(UpdaterError::Io(format!(
            "updater: no .app inside {}",
            dir.display()
        ))),
        (Some(_), Some(_)) => Err(UpdaterError::Io(format!(
            "updater: more than one .app inside {}",
            dir.display()
        ))),
    }
}

fn run(command: &str, args: &[&std::ffi::OsStr]) -> Result<(), UpdaterError> {
    let output = std::process::Command::new(command)
        .args(args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .output()
        .map_err(|error| UpdaterError::Io(format!("{command}: {error}")))?;
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    Err(UpdaterError::Io(format!(
        "{command} exited with {}: {}",
        output.status,
        stderr.trim()
    )))
}

/// Mount → stage → detach → spawn the detached helper. Blocking process +
/// file work; callers run it on the blocking pool.
fn apply_update_blocking(dmg: &Path, app: &Path) -> Result<(), UpdaterError> {
    let plan = MacInstallPlan::for_app(app)?;
    let cleanup = |plan: &MacInstallPlan| {
        let _ = std::fs::remove_dir_all(&plan.staging_dir);
    };

    std::fs::create_dir_all(&plan.mount_dir).map_err(|error| {
        UpdaterError::Io(format!(
            "updater: cannot stage next to {}: {error}",
            app.display()
        ))
    })?;
    let result = stage_from_dmg(dmg, &plan);
    if let Err(error) = result {
        cleanup(&plan);
        return Err(error);
    }

    let current_id = read_bundle_id(&plan.app)?;
    let staged_id = read_bundle_id(&plan.staged_app)?;
    if staged_id != current_id {
        cleanup(&plan);
        return Err(UpdaterError::Io(format!(
            "updater: staged app id {staged_id} != installed {current_id} — refusing to swap"
        )));
    }

    std::fs::write(&plan.script_path, helper_script(&plan, &current_id))
        .map_err(|error| UpdaterError::Io(error.to_string()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&plan.script_path, std::fs::Permissions::from_mode(0o755))
            .map_err(|error| UpdaterError::Io(error.to_string()))?;
    }

    // Detached: own process group, all stdio null — the helper must keep
    // running after it kills this Engine and the Manager.
    let mut command = std::process::Command::new("/bin/sh");
    command
        .arg(&plan.script_path)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    command
        .spawn()
        .map_err(|error| UpdaterError::Io(format!("spawn update helper: {error}")))?;
    Ok(())
}

/// `hdiutil attach` the DMG at the plan's mountpoint, `ditto` the .app it
/// contains into staging, detach. On any failure the mount is released
/// before the error propagates.
fn stage_from_dmg(dmg: &Path, plan: &MacInstallPlan) -> Result<(), UpdaterError> {
    run(
        "hdiutil",
        &[
            std::ffi::OsStr::new("attach"),
            std::ffi::OsStr::new("-nobrowse"),
            std::ffi::OsStr::new("-readonly"),
            std::ffi::OsStr::new("-mountpoint"),
            plan.mount_dir.as_os_str(),
            dmg.as_os_str(),
        ],
    )?;
    let result = (|| {
        let source = find_app_in(&plan.mount_dir)?;
        run("ditto", &[source.as_os_str(), plan.staged_app.as_os_str()])
    })();
    let detach = run(
        "hdiutil",
        &[std::ffi::OsStr::new("detach"), plan.mount_dir.as_os_str()],
    );
    match (result, detach) {
        (Err(error), _) => Err(error),
        (Ok(()), Err(error)) => Err(error),
        (Ok(()), Ok(())) => Ok(()),
    }
}

/// Apply the downloaded DMG to the running .app. Resolves the bundle,
/// stages, spawns the detached swap helper and returns — the helper then
/// quits this process, swaps and relaunches. Every failure before the
/// helper is spawned surfaces as an [`UpdaterError`] for the UI.
pub(crate) async fn apply_update(dmg: &Path) -> Result<(), UpdaterError> {
    let app = installed_app_bundle()?;
    let dmg = dmg.to_path_buf();
    tokio::task::spawn_blocking(move || apply_update_blocking(&dmg, &app))
        .await
        .map_err(|error| UpdaterError::Io(error.to_string()))?
}

#[cfg(test)]
#[path = "macos_tests.rs"]
mod tests;
