# Problems

## Playwright CLI Environment Block

The requested Playwright CLI diagnosis could not complete in this environment because browser launch fails before capture:

- `spawn EPERM` while launching Playwright Chromium headless shell

Impact:

- no screenshot artifact
- no runtime paint/computed-style capture from Playwright itself

Mitigation applied in code despite the runtime block:

- removed accidental sidebar collapse on near-click resize-handle release
- reduced eager renderer boot by lazy-loading route modules
