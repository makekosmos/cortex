# macOS / SwiftUI design notes

These notes summarize the visual direction for Eden based on Apple's HIG and current macOS app behavior.

## Core principles

- Hierarchy first: controls must support the content, not compete with it.
- Harmony with the system: corners, spacing, translucency, shadows, and control density should feel like macOS, not generic web UI.
- Consistency: the same surface language should apply to sidebar, dialogs, settings, editor chrome, and menus.
- Liquid materials are background treatment, not decoration. Blur and translucency should create depth without reducing clarity.
- Typography stays restrained and readable: content leads, chrome recedes.
- Motion stays quiet and purposeful: short fades, soft sheet transitions, no loud animation.
- Adaptive layout should preserve the same hierarchy on narrow widths instead of collapsing into a different visual language.

## Practical implementation rules for Eden

- Prefer one dominant window material with softer nested surfaces instead of many unrelated glass panels.
- Keep sidebar and settings as grouped list/form surfaces, closer to SwiftUI `List` and `Form`.
- Use thin separators, subtle strokes, and low-contrast fills before adding stronger shadows.
- Make editor chrome minimal: title, mode toggle, focus button. Content remains visually primary.
- Treat dialogs as sheets: compact width, strong title, light blur, low-noise actions.
- Hover states should be gentle and almost invisible until interaction is intentional.
- Keep code block controls understated and contextual.
- Avoid visual noise: no extra badges, no persistent status chips unless they communicate real risk.

## Performance guardrails

- Favor static CSS, variables, and simple transitions over heavy filters on many nested elements.
- Reuse derived data with memoized tree/sort structures instead of recomputing in render loops.
- Keep blur concentrated on major surfaces, not every child element.
- Prefer component boundaries around sidebar tree, dialogs, and settings sections to limit rerenders.

## What to avoid

- Generic web-card UI.
- Loud gradients and glossy effects everywhere.
- Multiple accent colors competing with content.
- Dense controls around the editor content.
- Layout changes that make the app feel unlike macOS when the window gets narrow.
