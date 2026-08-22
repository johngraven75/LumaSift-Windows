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

test("Windows scan classifies by extension before canonicalize to prevent reparse-point stalls", () => {
  // classify() must appear before path.canonicalize() inside source_files so that
  // the expensive OS file-open is skipped for non-media entries.  On Windows,
  // canonicalize() opens every file via GetFinalPathNameByHandleW; a junction point
  // targeting an offline network share can block for 30+ seconds.
  const sourceFilesStart = engine.indexOf("fn source_files(");
  assert.ok(sourceFilesStart !== -1, "source_files function must exist");
  // Capture body up to the next fn declaration so no magic length is needed.
  const nextFnIdx = engine.indexOf("\nfn ", sourceFilesStart + 1);
  const sourceFilesBody = nextFnIdx !== -1
    ? engine.slice(sourceFilesStart, nextFnIdx)
    : engine.slice(sourceFilesStart);
  const classifyIdx = sourceFilesBody.indexOf("classify(path, &selected)");
  const canonicalizeIdx = sourceFilesBody.indexOf("path.canonicalize()");
  assert.ok(classifyIdx !== -1, "classify(path, &selected) must appear in source_files");
  assert.ok(canonicalizeIdx !== -1, "path.canonicalize() must appear in source_files");
  assert.ok(
    classifyIdx < canonicalizeIdx,
    `classify (offset ${classifyIdx}) must precede canonicalize (offset ${canonicalizeIdx}) to avoid blocking on every non-media file`
  );
});
