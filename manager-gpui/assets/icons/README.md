# Device icons

SVG paths are from `@hugeicons/core-free-icons` **4.3.2** (Hugeicons, MIT):
https://www.npmjs.com/package/@hugeicons/core-free-icons/v/4.3.2

- `device-laptop.svg`: `LaptopIcon` (macOS)
- `device-monitor.svg`: `ComputerIcon` (Windows; generic computer fallback)
- `device-android.svg`: `SmartPhone01Icon`
- `device-iphone.svg`: `SmartphoneIcon`

The original 24×24 paths and 1.5px stroke are preserved. Icons are embedded by
`src/assets.rs`; GPUI applies the theme foreground color.

`appearance.svg` is an original palette outline created for this project, not
part of the Hugeicons set. It uses the same 24×24 view box and 1.5px stroke.
