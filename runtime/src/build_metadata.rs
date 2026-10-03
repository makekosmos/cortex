//! Platform-independent product metadata shared by Engine and Manager builds.
//! Cargo's crate version is not a production product version.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BuildMetadata {
    pub version: &'static str,
    pub channel: &'static str,
}

impl BuildMetadata {
    pub fn label(self) -> String {
        version_label(self.version, self.channel)
    }
}

pub fn version_label(version: &str, channel: &str) -> String {
    if !version.is_empty() && channel == "dev" {
        format!("{version} (dev)")
    } else {
        version.to_owned()
    }
}

// KOS-278: the version always stays semver — the updater parses it — while
// an uninjected build marks itself on the "dev" channel, so the UI labels
// it "0.1.0 (dev)" instead of presenting a release version.
pub fn from_inputs(
    product: Option<&'static str>,
    channel: Option<&'static str>,
    crate_version: &'static str,
) -> BuildMetadata {
    match product.map(str::trim).filter(|v| !v.is_empty()) {
        Some(version) => BuildMetadata {
            version,
            channel: channel
                .filter(|v| matches!(*v, "stable" | "beta" | "nightly" | "dev"))
                .unwrap_or("stable"),
        },
        None => BuildMetadata {
            version: crate_version,
            channel: "dev",
        },
    }
}

pub fn compiled() -> BuildMetadata {
    from_inputs(
        option_env!("MUNDUS_PRODUCT_VERSION"),
        option_env!("MUNDUS_BUILD_CHANNEL"),
        env!("CARGO_PKG_VERSION"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bare_cargo_and_empty_metadata_are_explicitly_dev() {
        for product in [None, Some(""), Some("  ")] {
            let meta = from_inputs(product, Some("stable"), "0.1.0");
            assert_eq!(meta.version, "0.1.0");
            assert_eq!(meta.channel, "dev");
            assert_eq!(meta.label(), "0.1.0 (dev)");
        }
    }
    #[test]
    fn release_uses_real_product_version_without_dev_suffix() {
        let meta = from_inputs(Some("0.10.3"), None, "0.1.0");
        assert_eq!(meta.version, "0.10.3");
        assert_eq!(meta.channel, "stable");
        assert_eq!(meta.label(), "0.10.3");
        assert_eq!(
            from_inputs(Some("0.10.3"), Some("dev"), "0.1.0").label(),
            "0.10.3 (dev)"
        );
    }
}
