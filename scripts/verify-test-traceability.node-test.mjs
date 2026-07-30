import assert from "node:assert/strict";
import { mkdtemp, mkdir, rm, symlink, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import test from "node:test";

import { verifyTestTraceability } from "./verify-test-traceability.mjs";

async function fixture() {
  const root = await mkdtemp(path.join(tmpdir(), "aiks-traceability-"));
  await mkdir(path.join(root, "docs"), { recursive: true });
  await mkdir(path.join(root, "src"), { recursive: true });
  await mkdir(path.join(root, ".github", "workflows"), { recursive: true });
  await writeFile(
    path.join(root, "docs", "TEST_VECTORS.json"),
    JSON.stringify({
      schemaVersion: 1,
      vectors: [{ id: "SAFE-001-example", requirements: ["SAFE-001"] }],
    }),
  );
  await writeFile(
    path.join(root, "src", "example.test.ts"),
    'test("cleanup stays disabled", () => {});\n',
  );
  await writeFile(
    path.join(root, ".github", "workflows", "ci.yml"),
    "run: npm test -- --run\n",
  );
  return root;
}

async function writeEvidence(root, evidence) {
  await writeFile(
    path.join(root, "docs", "TEST_EVIDENCE.json"),
    JSON.stringify({ schemaVersion: 1, vectors: evidence }),
  );
}

test("accepts one exact vector anchored to a CI-executed test", async () => {
  const root = await fixture();
  await writeEvidence(root, [{
    id: "SAFE-001-example",
    anchors: [{
      suite: "frontend",
      file: "src/example.test.ts",
      marker: 'test("cleanup stays disabled"',
    }],
  }]);

  const summary = await verifyTestTraceability(root);
  assert.deepEqual(summary, { vectors: 1, anchors: 1 });
});

test("rejects missing, duplicate, unknown, and stale vector evidence", async () => {
  const root = await fixture();

  await writeEvidence(root, []);
  await assert.rejects(
    verifyTestTraceability(root),
    /missing evidence for SAFE-001-example/,
  );

  await writeEvidence(root, [{
    id: "SAFE-001-example",
    anchors: [{
      suite: "frontend",
      file: "src/example.test.ts",
      marker: "stale marker",
    }],
  }]);
  await assert.rejects(verifyTestTraceability(root), /marker is absent/);

  await writeEvidence(root, [
    {
      id: "SAFE-001-example",
      anchors: [{
        suite: "frontend",
        file: "src/example.test.ts",
        marker: 'test("cleanup stays disabled"',
      }],
    },
    { id: "SAFE-001-example", anchors: [] },
    { id: "SAFE-999-unknown", anchors: [] },
  ]);
  await assert.rejects(
    verifyTestTraceability(root),
    /duplicate evidence for SAFE-001-example; unknown vector SAFE-999-unknown/,
  );
});

test("rejects evidence whose suite is not executed by CI", async () => {
  const root = await fixture();
  await writeFile(path.join(root, ".github", "workflows", "ci.yml"), "run: npm run build\n");
  await writeEvidence(root, [{
    id: "SAFE-001-example",
    anchors: [{
      suite: "frontend",
      file: "src/example.test.ts",
      marker: 'test("cleanup stays disabled"',
    }],
  }]);

  await assert.rejects(
    verifyTestTraceability(root),
    /suite frontend is not executed by CI/,
  );
});

test("rejects an evidence file that does not belong to its declared suite", async () => {
  const root = await fixture();
  await writeEvidence(root, [{
    id: "SAFE-001-example",
    anchors: [{
      suite: "rust",
      file: "src/example.test.ts",
      marker: 'test("cleanup stays disabled"',
    }],
  }]);

  await assert.rejects(
    verifyTestTraceability(root),
    /evidence file does not belong to suite rust/,
  );
});

test("rejects evidence reached through a linked parent directory", async () => {
  const root = await fixture();
  await mkdir(path.join(root, "real-src"));
  await writeFile(
    path.join(root, "real-src", "example.test.ts"),
    'test("cleanup stays disabled", () => {});\n',
  );
  await rm(path.join(root, "src"), { recursive: true });
  await symlink("real-src", path.join(root, "src"), "dir");
  await writeEvidence(root, [{
    id: "SAFE-001-example",
    anchors: [{
      suite: "frontend",
      file: "src/example.test.ts",
      marker: 'test("cleanup stays disabled"',
    }],
  }]);

  await assert.rejects(
    verifyTestTraceability(root),
    /evidence must be a regular non-link file/,
  );
});
