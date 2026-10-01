# Agenda Engine-contract fixtures (KOS-297)

These files are the exact objects `agenda-gpui`'s `mapping::write` produces
(fresh write + edit-path write per scenario). They are **generated, not
hand-written**.

To regenerate after changing Agenda's write path:

1. In the agenda-gpui checkout: `AGENDA_EMIT_FIXTURES=1 cargo test engine_payload_fixtures`
2. Copy `fixtures/engine/*.json` over this directory.
3. Re-run `cargo nextest run -p ark-core -E 'test(agenda_write_payloads)'`.

Agenda's own test fails when its committed copy drifts from `mapping::write`
output, with the same instruction — fixtures must be copied in the same change
as the write-path diff.

Last generated from makekosmos/agenda-gpui `kos-297` @ `2793d97`.
