import assert from "node:assert/strict";
import { chooseUsageIconRef } from "./store.ts";
import { HIDDEN_DASHBOARD_TYPE_IDS } from "./typeVisuals.ts";

// Regression: 2026-06-01. Stale tracked_apps.icon_ref paths rendered as
// broken images in Dashboard usage rows.
assert.equal(
  chooseUsageIconRef("C:\\stale\\icon.png", "data:image/png;base64,AAAA"),
  "data:image/png;base64,AAAA",
);

assert.equal(chooseUsageIconRef("C:\\stale\\icon.png", null), null);

assert.equal(chooseUsageIconRef("data:image/png;base64,BBBB", null), "data:image/png;base64,BBBB");

// Focus blocklists are settings data, not user-facing dashboard objects.
assert.equal(HIDDEN_DASHBOARD_TYPE_IDS.has("blocklist_obj"), true);
