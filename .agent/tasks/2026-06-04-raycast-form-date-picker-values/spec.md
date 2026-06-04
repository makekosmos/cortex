# Raycast Form.DatePicker Values

## Goal

Make `Form.DatePicker` handle Raycast-style `Date` values in the shim and host model.

## Acceptance Criteria

- `serializableProp(...)` preserves `Date` props as serializable strings.
- `Form.DatePicker` accepts `Date`, string, or null-ish default/value props in the shim.
- Host form model converts DatePicker defaults to `YYYY-MM-DD` for the native date input.
- DatePicker `onChange` callbacks receive `Date | null` instead of raw input strings.
- Focused unit verification covers serialization, model defaults, and callback conversion.
