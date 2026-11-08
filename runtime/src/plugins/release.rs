//! What a manifest points at, in the shape the release machinery takes.

use super::manifest::PluginSource;
use crate::native_apps::ReleaseTarget;

/// The release a manifest points at, for the release machinery.
impl PluginSource {
    pub fn target(&self) -> Option<ReleaseTarget<'_>> {
        match self {
            PluginSource::Release {
                repository,
                tag_prefix,
                asset,
                bundle,
            } => Some(ReleaseTarget {
                repository,
                tag_prefix,
                asset_stem: asset,
                bundle_name: bundle.as_deref().unwrap_or(asset),
            }),
            PluginSource::Builtin => None,
        }
    }
}
