import assert from "node:assert/strict";
import test from "node:test";
import { cleanArguments, reportArguments, scanArguments } from "./commands";

const filters = { olderThan: "7d", minSize: "100MiB" };

test("scan remains machine-readable and root-scoped", () => {
  assert.deepEqual(scanArguments("/workspace", filters), [
    "scan", "--format", "json", "--older-than", "7d", "--min-size", "100MiB", "/workspace",
  ]);
});

test("cleanup is dry-run until separately confirmed", () => {
  assert.equal(cleanArguments("/workspace", filters, false)[1], "--dry-run");
  assert.equal(cleanArguments("/workspace", filters, true)[1], "--yes");
});

test("report command writes one explicit destination", () => {
  assert.deepEqual(reportArguments("/workspace", "/tmp/report.html", filters).slice(0, 5), [
    "scan", "--format", "html", "--output", "/tmp/report.html",
  ]);
});
