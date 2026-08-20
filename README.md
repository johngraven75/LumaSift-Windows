# LumaSift for Windows

LumaSift for Windows is the **standalone coordinator** for safe duplicate resolution. It scans local folders, external drives, and connected NAS shares for user-selected videos, MP3 audio files, DOCX documents, PDFs, and images. It builds a reviewable plan before changing any files.

## Safety Model

LumaSift never treats filename similarity as duplicate proof. It first creates low-cost collision candidates from file size and sampled content, then calculates a **full SHA-256 digest** for every candidate. Only exact-content groups may enter the resolution plan.

Each exact group retains one deterministic copy and records evidence for the decision. The lower-ranked exact copies are **queued for quarantine**, not automatically erased. On application, each file is hashed again immediately before it is moved. Permanent erase is a separate operation requiring the exact confirmation `ERASE LUMASIFT QUARANTINE`.

## NAS Sources

Use a Windows UNC path such as `\\server\share`, provide a permitted Windows or NAS account, and choose whether Windows should remember the connection. The native Windows networking API receives the credentials directly in memory; LumaSift does not write the password to plans, logs, source lists, or release artifacts.

## Development

| Check | Command |
| --- | --- |
| Governance gate | `python3 scripts/verify_governance.py` |
| Frontend lint and contract tests | `npm run test:all` |
| Rust tests | `cd src-tauri && cargo test` |
| Tauri package | `npm run package` |

The hosted Windows release workflow performs these checks on a current Windows Rust toolchain, packages MSI and NSIS installers, writes SHA-256 checksums, and publishes a prerelease artifact set.

## Architecture and Security

Read [Architecture](docs/ARCHITECTURE.md), [Security](docs/SECURITY.md), [Release Notes](RELEASE_NOTES.md), and the enforced [Master Engineering Standard](.github/MASTER_ENGINEER_STANDARD.md) before contributing.

## Current Distribution Scope

`windows-v0.1.0` is an installer-ready **prerelease** target once the Windows CI workflow completes. The first public release remains explicitly prerelease while real Windows-hosted package evidence is collected.
