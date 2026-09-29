use super::*;
use std::collections::HashMap;

fn fake_env<'a>(map: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
    let map: HashMap<String, String> = map
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    move |key| map.get(key).cloned()
}

const STANDARD_ENV: &[(&str, &str)] = &[
    ("ProgramFiles", r"C:\Program Files"),
    ("ProgramFiles(x86)", r"C:\Program Files (x86)"),
    ("USERPROFILE", r"C:\Users\Kirill"),
    ("LOCALAPPDATA", r"C:\Users\Kirill\AppData\Local"),
    ("APPDATA", r"C:\Users\Kirill\AppData\Roaming"),
    ("TEMP", r"C:\Users\Kirill\AppData\Local\Temp"),
    ("PUBLIC", r"C:\Users\Public"),
];

#[test]
fn install_dir_lives_under_program_files() {
    let dir = install_dir(&fake_env(STANDARD_ENV)).unwrap();
    assert_eq!(dir, PathBuf::from(r"C:\Program Files\Kosmos\Service"));
}

#[test]
fn install_dir_rejects_user_writable_roots() {
    // The regression this guards: a service binary under %LOCALAPPDATA%
    // can be swapped by any user-mode process → SYSTEM escalation.
    let env = fake_env(&[
        ("ProgramFiles", r"C:\Users\Kirill\AppData\Local\Programs"),
        ("LOCALAPPDATA", r"C:\Users\Kirill\AppData\Local"),
        ("USERPROFILE", r"C:\Users\Kirill"),
    ]);
    assert!(install_dir(&env).is_err());
}

#[test]
fn install_dir_without_program_files_fails() {
    let env = fake_env(&[]);
    assert!(install_dir(&env).is_err());
}

#[test]
fn user_writable_detection_catches_profile_locations() {
    let env = fake_env(STANDARD_ENV);
    for bad in [
        r"C:\Users\Kirill\AppData\Local\Kosmos\Service",
        r"C:\Users\Kirill\AppData\Roaming\svc",
        r"C:\Users\Kirill\svc",
        r"C:\Users\Public\svc",
    ] {
        assert!(
            is_user_writable_location(Path::new(bad), &env),
            "missed user-writable path {bad}"
        );
    }
    assert!(!is_user_writable_location(
        Path::new(r"C:\Program Files\Kosmos\Service"),
        &env
    ));
    // Case-insensitive, separator-insensitive.
    assert!(is_user_writable_location(
        Path::new(r"c:/users/kirill/svc"),
        &env
    ));
}
