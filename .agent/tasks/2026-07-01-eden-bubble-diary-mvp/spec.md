# Eden Bubble Diary MVP

## Goal

Build the first diary layer inside Eden: a quiet time-ordered log of the
thoughts written directly into the diary. This intentionally starts without
external object/event sources so the base capture loop can be evaluated first.

## Scope

- Make the existing Diary sidebar entry open the timeline surface.
- Use existing Kosmos visual tokens/components where they fit.
- Keep the diary surface quiet: no large visible title/eyebrow and a bottom
  chat-style input for new thoughts.
- Persist locally added diary thoughts in browser storage so they survive a
  renderer refresh during visual/product iteration.
- Keep ARK writes/schema untouched for this MVP.

## Non-goals

- No AI extraction, auto-linking, or database normalization.
- No focus/Pomodoro/game/training/event sources in the diary timeline yet.
- No filter/source visibility UI yet.
- No backlinks graph or cross-object reference browser yet.
- No mobile capture/offline upload.
- No new ARK object type, sync table, or direct SQLite write.
- No full editor rewrite.

## Acceptance Criteria

**AC1.** Eden's Diary sidebar entry opens the timeline surface; it is not a
separate "Bubbles" navigation item.

**AC2.** The diary screen starts as a readable empty timeline and only shows
entries written directly in the diary composer.

**AC3.** Bubbles show block-level tags as chips, not inline hashtag text.

**AC4.** The screen includes a bottom chat-style composer/mock input that
demonstrates adding a new bubble into the timeline without persisting it to ARK.

**AC5.** Locally added bubbles survive component remount/page refresh via
browser-local storage.

**AC6.** The implementation uses Eden/Vue component boundaries, existing Kosmos
visual tokens/components, and does not add a new dependency.

**AC7.** Verification includes a targeted build/type check and a visual/browser
check or a clear reason why visual verification could not be completed.
