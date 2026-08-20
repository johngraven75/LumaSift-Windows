# LumaSift Windows Architecture

## Purpose and User Outcome

The Windows edition uses Tauri 2, React, TypeScript, and Rust. Its intended outcome is a comprehensible, safe duplicate-resolution experience that keeps user-selected categories, progress, proof evidence, and recovery state visible at every stage.

## Component Boundaries

| Layer | Responsibility | Contract / safety boundary |
| --- | --- | --- |
| Frontend | Select categories, initiate a scan or refresh a plan, show percentage/current item/evidence, and require confirmation for quarantine or purge. | The UI never implies that a candidate is a proven duplicate until the plan marks it exact. |
| Connector / integration | Local command and filesystem bridge. | Inputs are validated; source paths and credentials are never exposed in rendering state. |
| Backend | Sampled-hash candidacy, full SHA-256 proof, deterministic quality ranking, plan persistence, quarantine, and explicit purge protection. | Selection, cancellation, error, and recovery state preserve the exact user-selected scope. |

## Data and Recovery Lifecycle

1. The owner chooses source scope and allowed file categories.
2. The system reports progress without deleting any data.
3. A plan is reviewable only after full SHA-256 proof.
4. The owner explicitly approves a quarantine plan.
5. Permanent purge is a separate, deliberate action with an audit disposition.

## Compatibility and Rollback

Plans are versioned, human-readable records. Release packages retain a documented downgrade/rollback path. No upgrade may silently alter prior quarantine locations, selected-category scope, or credential storage semantics.
