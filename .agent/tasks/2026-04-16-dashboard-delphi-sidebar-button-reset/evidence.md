# Evidence

## Verification summary

- AC1 `PASS`: `apps/dashboard/src/global.css` now applies the same effective button reset Delphi uses, removing native button backgrounds from sidebar actions rendered as `<button>`.
- AC2 `PASS`: Dashboard typecheck and build passed after the reset change.

## Commands

```powershell
Set-Location D:\Personal\Hobby\Coding\kosmos\apps\dashboard; .\node_modules\.bin\tsc.exe
Set-Location D:\Personal\Hobby\Coding\kosmos\apps\dashboard; .\node_modules\.bin\vite.exe build --configLoader native
```

## Result

- `tsc.exe`: passed
- `vite build --configLoader native`: passed
