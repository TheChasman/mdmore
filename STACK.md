<!-- stack-stamps: generated from .stack by `stamp`; edits are overwritten -->
# Stack

## rust
### Rust Stack Notes
- Edition: 2021
- Error handling: anyhow for binaries, thiserror for libraries
- Async: tokio when the project needs an async runtime; a project's own CLAUDE.md wins where it says otherwise (for example, no runtime)
- Prefer workspace Cargo.toml for multi-crate projects
