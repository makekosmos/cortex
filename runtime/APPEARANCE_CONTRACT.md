# Shared appearance RPC (Engine)

Call `appearance.get` with `{}`. Call `appearance.set` with a flat patch object, for example `{"mode":"light","follow_apps":true}`. Both return the usual Engine RPC envelope; its `data` is:

```json
{
  "settings": {
    "schema_version": 1,
    "mode": "dark",
    "light_theme": "default",
    "dark_theme": "default",
    "accent_source": "theme",
    "accent_color": null,
    "follow_apps": false,
    "material": "default",
    "font_family": "Inter",
    "font_size": 13,
    "revision": 0
  },
  "capabilities": {"materials": ["default", "frosted", "opaque"], "wallpaper_accent": true},
  "wallpaper_accent": null
}
```

The example capabilities are for macOS. Windows reports `default` and `opaque`; on detected builds 17763 and newer it also reports `acrylic`, and on detected Windows 11 builds 22000 and newer it also reports `mica`. Unknown Windows builds use the conservative pair. Linux reports `default`, `opaque` and `wallpaper_accent:false`. A rejected patch returns `ok:false` and leaves the persisted file and revision unchanged. The Engine assigns `revision` starting at 0 for defaults and increments it for each accepted patch. Clients cannot patch `revision` or `schema_version`. A patch is a partial object; unspecified fields keep their values. Clients should use the returned settings after a write and refresh on startup. There is no appearance change broadcast yet.

Theme values are stable Imago `ThemeDef.key` identifiers from the pinned table: `default`, `t3-chat`, `claymorphism`, `claude`, `graphite`, `amethyst-haze`, `vercel`. `font_family` must be a nonempty string of at most 128 bytes without control characters; `font_size` is a finite number from 11 through 18 (including 12.5). `accent_color` is `null` or `#RRGGBB`; selecting `custom` requires a color. `follow_apps` is the UI control labelled «Единый стиль приложений» and defaults to false, preserving each client's existing choice.

`wallpaper_accent` is computed from the **OS desktop picture**, never from an application's wallpaper. On macOS the Engine asks System Events for the current desktop's picture and samples a reduced image with `sips`. The first response after selecting `wallpaper` returns `wallpaper_accent:null` and `wallpaper_error:"desktop wallpaper accent is pending"`; clients can poll later. Subsequent calls reuse the last result for 30 seconds. A refresh runs in the background, retains the previous result during refresh, and combines concurrent requests into one lookup. Failed lookups are cached for the same period and reported as `wallpaper_error`. Both OS lookup and image conversion have five-second deadlines. The setting is stored in `<Engine data dir>/appearance-settings.json` on this device and is not synced through ARK.
