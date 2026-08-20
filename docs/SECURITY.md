# LumaSift Windows Security Model

## Non-Negotiable Controls

- A candidate is actionable only after full SHA-256 exact-content proof.
- Resolution is quarantine-first; no automatic permanent deletion is permitted.
- Purge requires a separate explicit owner action.
- User-selected file categories are preserved through cancellation, error, resume, and review paths.
- Paths, NAS credentials, signing material, tokens, and private certificates must not be committed, logged, or exposed through companion UI state.

## Credential and Trust Handling

The Windows coordinator runs with least privilege and validates input paths against approved sources.

## Release Security

Checksums accompany packaged release artifacts. Production signing must be accurately labelled: a TEST APK, simulator package, or unsigned archive is not a production mobile distribution artifact.
