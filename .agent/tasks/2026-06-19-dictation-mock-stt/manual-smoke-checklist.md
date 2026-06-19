# Manual dictation smoke checklist

## What is already proved

- Automated proof is complete in `.agent/tasks/2026-06-19-dictation-mock-stt/evidence.md`.
- The opt-in headless Groq smoke exists and is skipped only when its required env vars are missing.
- The remaining gap is human-observed real-device smoke on Windows and macOS.

## Prep

Run the real smoke from a visible Kosmos shell, not `KOSMOS_HEADLESS=1`.

Set these env vars before the opt-in Groq e2e run:

- `KOSMOS_TEST_GROQ_API_KEY`
- optional: `KOSMOS_TEST_DICTATION_AUDIO_B64`
- optional: `KOSMOS_TEST_DICTATION_EXPECTED_TRANSCRIPT_SUBSTRING`

Suggested PowerShell prep:

```powershell
$env:KOSMOS_TEST_GROQ_API_KEY = "<real Groq API key>"
$env:KOSMOS_TEST_DICTATION_AUDIO_B64 = "<base64-encoded WAV>"
$env:KOSMOS_TEST_DICTATION_EXPECTED_TRANSCRIPT_SUBSTRING = "<optional transcript fragment>"
```

If `KOSMOS_TEST_DICTATION_AUDIO_B64` is omitted, the wrapper and smoke test use a
tiny synthetic WAV fixture automatically.

## Opt-in real Groq smoke

Preferred wrapper:

```powershell
node scripts/run-dictation-groq-smoke.mjs
```

The wrapper only requires `KOSMOS_TEST_GROQ_API_KEY`; it synthesizes a tiny WAV
fixture itself when `KOSMOS_TEST_DICTATION_AUDIO_B64` is not set. Optional
`KOSMOS_TEST_DICTATION_EXPECTED_TRANSCRIPT_SUBSTRING` enables an exact transcript
substring assertion.

Direct Playwright command:

```powershell
node <resolved-playwright-cli> test --config platform/desktop/playwright.config.ts --reporter=line --output <temp> platform/desktop/e2e/dictation.spec.ts -g "opt-in real provider path works headless without microphone"
```

The wrapper resolves the installed Playwright CLI path at runtime, so the exact
path is printed before launch.

Pass criteria:

- command exits 0;
- `dictation.submit_audio` completes with `provider="groq"` and `injectMode="clipboard_only"`;
- clipboard ends with the transcript text;
- no microphone is required for this smoke.

Fail evidence:

- command that was run;
- missing env var names, if any;
- Playwright failure text;
- whether the clipboard stayed unchanged;
- whether the state machine left `pending` work behind.

## Windows live smoke

Use a real text target outside Kosmos, then:

1. Start Kosmos in the visible desktop shell.
2. Open Settings > Dictation and confirm the provider is `groq`.
3. Set `injectMode` to `auto_paste`.
4. Focus an external text field, trigger dictation from the configured global hotkey, and speak one short phrase.
5. Confirm the pill starts and stops without stray windows.
6. Confirm the transcript lands in the target app and focus returns to that app.
7. Switch `injectMode` to `clipboard_only`, repeat the same phrase, and confirm the transcript stays in the clipboard without a forced paste.
8. Change the dictation shortcut in Settings and confirm the App Commands row updates live.

Windows pass evidence:

- hotkey used;
- target app and field;
- microphone permission state;
- provider and inject mode;
- transcript result;
- clipboard result;
- whether focus returned correctly;
- screenshot or short screen recording if available.

Windows fail evidence:

- exact step that failed;
- permission prompt or OS dialog text;
- whether the pill appeared;
- whether text was pasted or only copied;
- any error toast or status text.

## macOS adapter sanity

1. Open Settings > Dictation.
2. Capture a modified shortcut and confirm the new accelerator is shown after save.
3. Grant microphone, accessibility, and input-monitoring permissions.
4. Run one short Groq dictation from a non-Kosmos app.
5. Repeat once with `clipboard_only` and once with `auto_paste`.
6. Confirm focus returns to the original target app after transcription.
7. If input monitoring is missing, the adapter should cancel cleanly instead of hanging in capture mode.

macOS pass evidence:

- current macOS version;
- shortcut before and after save;
- permission state for microphone, accessibility, and input monitoring;
- target app;
- transcript result;
- clipboard result;
- focus return;
- any adapter-specific notes.

macOS fail evidence:

- whether shortcut capture timed out;
- which permission was missing;
- whether capture stayed armed;
- whether the transcript injected into the wrong app;
- any OS dialog text or permission denial.
