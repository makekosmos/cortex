//! Pipe-client image validation (KOS-269): the service runs as SYSTEM, so
//! it must never take a program path from the request. The rule's program
//! is derived from the pipe *client's* process image and must be a
//! `mundus-engine.exe` inside the calling user's own versioned install dir.

use std::path::{Component, Path, PathBuf};

use crate::brand;
use crate::install_layout::engine_root_of_exe;

/// `<profile>\AppData\Local\<Brand>\Engine` — the only install root whose
/// `versions\<semver>\mundus-engine.exe` the service may open the firewall
/// for. `profile` is the *pipe client's* profile dir resolved under
/// impersonation, so the path is always scoped to the calling user.
pub fn engine_install_root(profile_dir: &Path) -> PathBuf {
    profile_dir
        .join("AppData")
        .join("Local")
        .join(brand::LOCAL_DIR_NAME)
        .join(brand::ENGINE_DIR_NAME)
}

/// Case-insensitive, component-wise path equality (Windows semantics):
/// `\\?\` verbatim prefixes are never produced here — callers reject them
/// first — so component comparison is exact enough for our fixed shape.
fn paths_equal_ci(a: &Path, b: &Path) -> bool {
    let ac: Vec<String> = a
        .components()
        .map(|c| c.as_os_str().to_string_lossy().to_lowercase())
        .collect();
    let bc: Vec<String> = b
        .components()
        .map(|c| c.as_os_str().to_string_lossy().to_lowercase())
        .collect();
    ac == bc
}

/// Validate the pipe client's process image as the program a firewall rule
/// may point at. Accepted: `<profile>\AppData\Local\<Brand>\Engine\
/// versions\<semver>\mundus-engine.exe` — nothing else. The service runs as
/// SYSTEM, so a client-supplied or loosely-checked path would be a primitive
/// for opening the firewall to an arbitrary binary.
pub fn validated_engine_image(profile_dir: &Path, image_path: &Path) -> Result<PathBuf, String> {
    for component in image_path.components() {
        // UNC (`\\host\share\…`) and verbatim (`\\?\…`, `\\.\…`) paths are
        // refused outright: the rule must scope to a local drive-letter
        // path. `VerbatimDisk` is still rejected — it is the same path
        // spelled `\\?\C:\…` and we never produce that form ourselves.
        if let Component::Prefix(prefix) = component {
            if !matches!(prefix.kind(), std::path::Prefix::Disk(_)) {
                return Err(format!(
                    "refusing non-local image path {}",
                    image_path.display()
                ));
            }
        }
        if matches!(component, Component::ParentDir) {
            return Err(format!(
                "refusing traversal in image path {}",
                image_path.display()
            ));
        }
    }
    // `components()` silently normalizes `.` away — scan the raw segments so
    // `a\.\b` is rejected too: the rule name is the only accepted shape.
    if image_path
        .to_string_lossy()
        .split(['\\', '/'])
        .any(|seg| seg == "." || seg == "..")
    {
        return Err(format!(
            "refusing traversal in image path {}",
            image_path.display()
        ));
    }
    // `engine_root_of_exe` also enforces the `mundus-engine.exe` name and
    // the `versions/<semver>` parent shape.
    let root = engine_root_of_exe(image_path)
        .ok_or_else(|| format!("{} is not a versioned Engine install", image_path.display()))?;
    let expected = engine_install_root(profile_dir);
    if !paths_equal_ci(&root, &expected) {
        return Err(format!(
            "{} is outside the caller's Engine install root {}",
            image_path.display(),
            expected.display()
        ));
    }
    Ok(image_path.to_path_buf())
}
