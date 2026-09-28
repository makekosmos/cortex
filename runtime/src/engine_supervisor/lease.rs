pub(crate) fn take_desktop_lease_env() -> Result<Option<(u32, String)>, ()> {
    let credential = std::env::var("MUNDUS_DESKTOP_ROLE_CREDENTIAL").ok();
    let pid = std::env::var("MUNDUS_DESKTOP_ROLE_PID").ok();
    std::env::remove_var("MUNDUS_DESKTOP_ROLE_CREDENTIAL");
    std::env::remove_var("MUNDUS_DESKTOP_ROLE_PID");
    match (credential, pid) {
        (None, None) => Ok(None),
        (Some(credential), Some(pid)) => {
            let pid = pid.parse::<u32>().map_err(|_| ())?;
            if pid == 0 || credential.is_empty() || credential.len() > 128 {
                return Err(());
            }
            Ok(Some((pid, credential)))
        }
        _ => Err(()),
    }
}
