//! Friendly OS computer name, shared by Engine identity and Manager display.
//! macOS GUI processes normally do not have HOSTNAME in their environment.

fn clean_name(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_owned())
}

#[cfg(unix)]
fn command_name(program: &str, args: &[&str]) -> Option<String> {
    let output = std::process::Command::new(program)
        .args(args)
        .stdin(std::process::Stdio::null())
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    clean_name(std::str::from_utf8(&output.stdout).ok()?)
}

pub fn system_device_name() -> String {
    #[cfg(target_os = "macos")]
    if let Some(name) = command_name("/usr/sbin/scutil", &["--get", "ComputerName"]) {
        return name;
    }
    for key in ["COMPUTERNAME", "HOSTNAME"] {
        if let Some(name) = std::env::var(key).ok().and_then(|value| clean_name(&value)) {
            return name;
        }
    }
    #[cfg(unix)]
    if let Some(name) = command_name("/bin/hostname", &[]) {
        return name;
    }
    "Устройство".into()
}

#[cfg(test)]
mod tests {
    use super::clean_name;

    #[test]
    fn system_name_keeps_unicode_and_removes_command_newline() {
        assert_eq!(
            clean_name("MacBook Pro — user\n"),
            Some("MacBook Pro — user".into())
        );
        assert!(clean_name(" \n\t").is_none());
    }
}
