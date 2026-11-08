//! The Manager's own names, in one place.
//!
//! A product rename must touch this file, `brand.mjs`/`engine-base::brand` on
//! the Engine side and the packaging scripts — not string literals scattered
//! through the tree. The client class is a role, not a framework: the Engine
//! accepts it next to the class managers installed before 0.10.27 send.

/// Window title, About heading and the packaged binary's description.
pub const PRODUCT_NAME: &str = "Mundus Manager";

/// Identity the pages and the platform use (nsh.json carries the same id).
pub const BUNDLE_ID: &str = "com.kazui.mundus.manager";

/// The header the class travels in. Neutral like the class itself: the Engine
/// reads the same name from `engine_base::brand::CLIENT_CLASS_HEADER`.
pub const CLIENT_CLASS_HEADER: &str = "X-Client-Class";

/// The Manager's role on the Engine wire. Empty of both the old and the current
/// product name, so a rename never reaches the protocol.
pub const CLIENT_CLASS: &str = "manager";
