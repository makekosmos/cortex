# Caption glyphs

`window-minimize.svg`, `window-maximize.svg`, `window-restore.svg` and
`close.svg` are taken unchanged from Zeron (`zeronsh/zeron`,
`crates/ui/assets/icons/`), so the Manager's caption buttons draw the same
shapes Zeron's titlebar does: a 16-point box, 1.25 stroke, round caps, the
shape itself 6.5 points across at 4.75…11.25.

Most of Zeron's set is **Solar Icons** (Linear weight) by 480 Design, licensed
under [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/) —
attribution: "Solar Icons by 480 Design".

They are embedded with `include_bytes!` and painted with `Window::paint_svg`
from `manager/src/chrome.rs`, so a maximized window draws `window-restore.svg`
in the same frame the maximized state changes.
