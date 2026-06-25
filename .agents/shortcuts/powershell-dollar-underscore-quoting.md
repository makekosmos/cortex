# PowerShell Dollar Underscore Quoting

## Trigger

When a one-off PowerShell pipeline passed through the shell tool needs
`Where-Object { $_... }` or `ForEach-Object { $_... }`.

## Symptom

The command unexpectedly runs as `.ProcessName`, `.Line`, or `.Context...`,
and PowerShell reports those as missing commands.

## Do This

Avoid the quoting fight for diagnostics:

- Use Node for log/file inspection when possible.
- Use `cmd /c` for simple Windows filesystem checks.
- If PowerShell is required, escape `$` as a PowerShell literal before sending
  the command through the tool.

## Avoid

Do not keep retrying the same `$_` pipeline through nested JSON/PowerShell
quoting. It creates huge noisy output and hides the actual failure.

## Promote To Skill When

This recurs across multiple Windows command recipes.
