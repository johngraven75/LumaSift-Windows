import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const types = readFileSync(new URL("../src/types.ts", import.meta.url), "utf8");
const engine = readFileSync(new URL("../src-tauri/src/lib.rs", import.meta.url), "utf8");

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
