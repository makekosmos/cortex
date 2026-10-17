# KOS-424 usage application boundary

- [x] Move the usage write use case, mutation DTO, and storage session contract to `ark-application`.
- [x] Keep the existing `ark_core::application::usage` path as a facade for callers.
- [x] Run application tests, the 28 usage-focused service/storage behavior tests, dependency-tree, strict Clippy, format, source-size, and diff gates.
- [ ] Commit this boundary independently and keep KOS-424 open.
