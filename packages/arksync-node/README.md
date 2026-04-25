# @arksync/node

Compatibility package for the old ARK SDK name.

New Electron main / Node code should import from `@kepler/ark`:

```ts
import { ArkClient } from "@kepler/ark";
```

This package remains in the workspace only to avoid abruptly breaking older
imports. It re-exports the canonical SDK from `@kepler/ark`.
