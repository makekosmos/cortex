use super::*;

pub(super) fn granted_path_scope(
    grant: &Grant,
    method: &WorkerMethod,
    path: &Path,
) -> Option<String> {
    let capability = match method {
        WorkerMethod::FilesystemRootOpen => return None,
        WorkerMethod::FilesystemRead
        | WorkerMethod::FilesystemList
        | WorkerMethod::FilesystemPoll => "filesystem.read",
        WorkerMethod::FilesystemWrite
        | WorkerMethod::FilesystemDelete
        | WorkerMethod::FilesystemCreateDir => "filesystem.write",
        WorkerMethod::ProcessSpawn => "process.spawn",
        _ => return None,
    };
    grant
        .scopes
        .get(capability)?
        .iter()
        .find(|root| path.starts_with(root))
        .cloned()
}
