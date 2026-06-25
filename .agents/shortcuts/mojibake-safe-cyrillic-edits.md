# Mojibake Safe Cyrillic Edits

## Trigger

When editing files with Russian/Cyrillic strings or comments, or after
`apply_patch` / rewrites on Windows.

## Symptom

Strings like `РѕР±`, `Р’Рµ`, `СЃ`, or `вЂ` show up in source files, not just in
terminal output.

## Do This

Use Unicode-aware Node reads or `JSON.stringify`, inspect actual code points, and
compare against already-correct Cyrillic before retyping. Prefer preserving or
moving existing valid strings, or typing fresh UTF-8 text. Search touched files
for common mojibake sequences after the edit.

```powershell
rtk node -e "const fs=require('fs'); const p=process.argv[1]; const s=fs.readFileSync(p,'utf8'); console.log(JSON.stringify(s));" path\to\file
rtk proxy node -e "const fs=require('fs'); const files=process.argv.slice(1); const markers=['\uFFFD','\u00D0\u00A0','\u00D0\u009F','\u00D0\u00B0','\u00D0\u00B1','\u00D0\u00B2','\u00D0\u00B3','\u00D0\u00B4','\u00D0\u00B5','\u00D0\u00B8','\u00D0\u00BA','\u00D0\u00BB','\u00D0\u00BC','\u00D0\u00BD','\u00D0\u00BE','\u00D0\u00BF','\u00D1\u0080','\u00D1\u0081','\u00D1\u0082','\u00D1\u008C','\u00D0\u0098','\u00D0\u00AF','\u0432\u0402','\u00C2','\u00C3']; let bad=false; for(const file of files){ const text=fs.readFileSync(file,'utf8'); for(const marker of markers){ if(text.includes(marker)){ console.log(file+' '+JSON.stringify(marker)); bad=true; break; } } } process.exit(bad?1:0);" path\to\touched-file.ts
```

For a broader source sweep, walk source roots and keep markers escaped so the
command itself is independent of terminal encoding.

## Avoid

Do not trust PowerShell-rendered Cyrillic output. Do not retype from mojibake
snippets in the terminal. Do not claim the file is clean without inspecting the
actual file text or bytes.

## Promote To Skill When

This becomes a recurring Windows text-editing failure mode across multiple
tasks.
