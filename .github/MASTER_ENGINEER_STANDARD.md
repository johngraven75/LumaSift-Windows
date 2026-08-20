# LumaSift Master Engineering Standard

This document is mandatory for every human- or AI-assisted contribution to a LumaSift repository. It supplements `AGENTS.md`, the Total Automation Policy, and the repository review instructions. When requirements conflict, the safer and more restrictive requirement applies.

## Product Safety Invariants

LumaSift must only queue a file for resolution after exact-content proof. Candidate discovery may use sampled hashes, but final duplicate confirmation must use full SHA-256 hashing. Automated removal is prohibited: lower-ranked copies must move through recoverable quarantine after explicit owner approval, and permanent purge must remain a separate, explicit action.

User-selected file categories are authoritative. Supported categories are video, MP3 audio, DOCX documents, PDFs, and images. Cancellation, resume, plan review, and resolution application must preserve the selected-category contract and must not broaden a scan scope implicitly.

Remote or companion clients must not disclose raw local source paths or embed NAS credentials. Sensitive credentials, signing material, and long-lived tokens must never be committed, logged, placed in test fixtures, or added to release artifacts.

## Engineering Requirements

Before implementation, define the product goal, target user, constraints, measurable acceptance criteria, non-goals, affected public contracts, persistence effects, upgrade path, and rollback path. Organize the design into **Frontend**, **Connector / integration**, and **Backend** concerns where they apply.

Use narrow, typed contracts and validate inputs at all boundaries. Prefer deterministic ranking, explicit error handling, cancellation safety, bounded resource use, secure defaults, and observable state transitions. Do not add placeholders, fabricated integrations, mock production behavior, TODO-only code paths, silent fallbacks, or compatibility-breaking changes without an approved migration.

Keep commits atomic and conventional. Do not merge or release around a failing required check. Root-cause failures, preserve accepted functionality, commit the correction, and rerun the relevant validation on the resulting commit.

## Mandatory Evidence Before Publication

Every release candidate must include the following artifacts in version control:

| Artifact | Required evidence |
| --- | --- |
| Architecture documentation | Purpose, component boundaries, API/IPC contracts, trust boundaries, configuration, and recovery behavior. |
| Security documentation | Credential handling, path-redaction policy, quarantine and purge safety, and secret-management requirements. |
| Release notes | User-visible changes, compatibility, migration or installer notes, rollback procedure, and known limitations. |
| Automated workflows | Current build, test, package, checksum, and publication automation appropriate to the platform. |
| Validation | Formatting, lint/static analysis, unit tests, contract tests, build/package checks, and platform-appropriate user-flow checks. |

Production signing must be described accurately. A debug-signed Android APK, simulator package, or unsigned iOS archive must be labelled **TEST** and must never be presented as a store-ready production release.

## Completion Report

Every completed task must provide a report with these sections:

1. **Purpose** — outcome delivered and safety invariants protected.
2. **Frontend** — user-visible behavior and validation.
3. **Connector / integration** — API, IPC, storage, or platform integration and validation, or “not applicable.”
4. **Backend** — domain logic, hashing, planning, persistence, or recovery behavior and validation, or “not applicable.”
5. **Completion** — files changed, tests and package checks run, artifact checksums, release links, remaining limitations, and rollback position.
