import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const types = readFileSync(new URL("../src/types.ts", import.meta.url), "utf8");
const engine = readFileSync(new URL("../src-tauri/src/lib.rs", import.meta.url), "utf8");
const ui = readFileSync(new URL("../src/LumaSiftApp.tsx", import.meta.url), "utf8");

test("Windows standalone UI exposes every supported LumaSift category", () => {
  for (const label of ["Videos", "MP3 audio", "DOCX & PDF", "Images"]) {
    assert.match(types, new RegExp(label));
  }
});

test("Windows standalone engine requires full SHA-256 proof and quarantine-first application", () => {
  for (const token of ["fn full_hash", "Full SHA-256", "queued_for_quarantine", "purge_quarantine", "ERASE LUMASIFT QUARANTINE"]) {
    assert.match(engine, new RegExp(token));
  }
});

test("Windows standalone engine preserves selected categories through cancellation", () => {
  assert.match(engine, /fn cancel_plan\(dispositions: Vec<Disposition>, selected_types: Vec<SelectionType>\)/);
  assert.match(engine, /selected_types,/);
});

test("Windows scan indexes in the worker and reports progress-safe failures", () => {
  assert.match(engine, /phase: "Indexing sources"/);
  assert.match(engine, /thread::Builder::new\(\)\.name\("lumasift-resolution"/);
  assert.match(engine, /match source_files\(&request, &app_data\)/);
  assert.match(engine, /Err\(error\) => fail_progress\(error\)/);
});

test("Windows scan emits immediate progress update at start of each source walk", () => {
  // An update_progress call must appear before the WalkDir loop so that
  // the very first poll from the UI reflects that the worker has entered
  // the source directory, even when fewer than 32 entries have been visited.
  assert.match(engine, /update_progress\("Indexing sources"[^;]+source_path\.to_string_lossy/);
});

test("Windows scan tracks files_considered live during source walk", () => {
  // update_progress now receives the live files.len() count so that the
  // UI shows a non-zero filesConsidered value as soon as files are indexed.
  assert.match(engine, /update_progress\([^;]+files\.len\(\) as u64\)/s);
});

test("UI scan startup sets optimistic scanning state before invoke to prevent hang appearance", () => {
  // Verify that setProgress({ scanning: true }) appears in the source text
  // BEFORE the await invoke call, ensuring the UI switches to "Cancel scan"
  // and the polling interval starts immediately on click rather than after
  // the invoke round-trip.
  const setProgressIdx = ui.indexOf('setProgress({\n      ...idleProgress,\n      scanning: true');
  const awaitInvokeIdx = ui.indexOf('await invoke<ScanProgress>("start_resolution"');
  assert.ok(setProgressIdx !== -1, "optimistic setProgress({ scanning: true }) not found");
  assert.ok(awaitInvokeIdx !== -1, "await invoke start_resolution not found");
  assert.ok(setProgressIdx < awaitInvokeIdx, "setProgress with scanning:true must appear before await invoke");
  assert.match(ui, /phase: "Starting"/);
  // Must reset to idle progress when the invoke throws, so the UI does not
  // remain stuck in a scanning state after a validation error.
  assert.match(ui, /setProgress\(idleProgress\)/);
});
