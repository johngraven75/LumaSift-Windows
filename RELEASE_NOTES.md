# LumaSift Windows Release Notes

## 0.1.0 — Standalone Coordinator Prerelease

### Included

- A public, standalone Tauri desktop application with a dedicated LumaSift React workspace.
- Explicit file-category selection for videos, MP3 audio, DOCX/PDF documents, and images.
- Recursive local, external-drive, and UNC NAS source scanning with Windows-native NAS connection support.
- Two-phase duplicate proof: sampled content identifies candidates and full SHA-256 hashing gates every actionable exact-content group.
- Live scanning progress, current-file visibility, deterministic disposition evidence, reviewable plans, safe cancellation, and audit records.
- Quarantine-first application that re-hashes every candidate immediately before moving it, prevents overwrite, and keeps permanent purge as a separately confirmed action.
- Master engineering governance, automated policy verification, frontend contract checks, Rust unit tests, Windows packaging workflow, and release checksum automation.

### Validation Status

- `python3 scripts/verify_governance.py` passes.
- `npm run test:all` passes at the current commit.
- Local Rust unit execution is blocked by the sandbox’s Cargo 1.75 toolchain because current transitive dependencies require Cargo Edition 2024 parsing. The release workflow uses a current Rust toolchain on `windows-latest` and remains the authoritative package gate.

### Distribution Status

The `windows-v0.1.0` workflow publishes an explicitly labelled prerelease containing MSI and NSIS installers plus a SHA-256 checksum manifest after its Windows-hosted validation and packaging steps complete.

### Rollback

Before release publication, revert the standalone application commit while preserving the governance baseline. After a release tag exists, install the previous verified MSI or NSIS asset and retain the associated checksum file. Quarantine data remains outside installer directories and is not removed by an application rollback.
