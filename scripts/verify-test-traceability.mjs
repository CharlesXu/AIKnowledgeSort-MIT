import { lstat, readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import path from "node:path";

const repositoryRoot = fileURLToPath(new URL("..", import.meta.url));

const suiteCommands = Object.freeze({
  rust: "cargo test --manifest-path src-tauri/Cargo.toml --lib",
  "rust-integration": "cargo test --manifest-path src-tauri/Cargo.toml --test mcp_runtime_smoke",
  frontend: "npm test -- --run",
  e2e: "npm run e2e",
  release: "npm run verify:release",
  provenance: "npm run verify:release",
});

const suiteFilePatterns = Object.freeze({
  rust: /^src-tauri\/src\/.+\.rs$/u,
  "rust-integration": /^src-tauri\/tests\/.+\.rs$/u,
  frontend: /^src\/.+\.test\.[cm]?[jt]sx?$/u,
  e2e: /^e2e\/.+\.spec\.[cm]?[jt]s$/u,
  release: /^(?:\.github\/workflows\/ci\.yml|scripts\/verify-release-contract\.mjs)$/u,
  provenance: /^(?:docs\/RELEASE_ASSET_PROVENANCE\.json|scripts\/verify-provenance-contract(?:\.node-test)?\.mjs)$/u,
});

function safeRelativePath(value) {
  return typeof value === "string"
    && value.length > 0
    && !path.isAbsolute(value)
    && !value.split(/[\\/]/u).includes("..");
}

async function readJson(root, relativePath) {
  return JSON.parse(await readFile(path.join(root, relativePath), "utf8"));
}

async function isRegularNonLinkPath(root, relativePath) {
  const components = relativePath.split("/");
  for (let index = 1; index <= components.length; index += 1) {
    const metadata = await lstat(path.join(root, ...components.slice(0, index)));
    if (metadata.isSymbolicLink()) return false;
    if (index === components.length && !metadata.isFile()) return false;
  }
  return true;
}

export async function verifyTestTraceability(root = repositoryRoot) {
  const [contract, evidence, workflow] = await Promise.all([
    readJson(root, "docs/TEST_VECTORS.json"),
    readJson(root, "docs/TEST_EVIDENCE.json"),
    readFile(path.join(root, ".github/workflows/ci.yml"), "utf8"),
  ]);
  const violations = [];

  if (contract.schemaVersion !== 1 || !Array.isArray(contract.vectors)) {
    violations.push("test vector contract must use schema version 1");
  }
  if (evidence.schemaVersion !== 1 || !Array.isArray(evidence.vectors)) {
    violations.push("test evidence must use schema version 1");
  }

  const contractIds = new Set();
  for (const vector of contract.vectors ?? []) {
    if (typeof vector.id !== "string" || contractIds.has(vector.id)) {
      violations.push(`invalid or duplicate contract vector ${String(vector.id)}`);
    } else {
      contractIds.add(vector.id);
    }
  }

  const evidenceIds = new Set();
  let anchorCount = 0;
  for (const vector of evidence.vectors ?? []) {
    if (typeof vector.id !== "string") {
      violations.push("evidence vector id must be a string");
      continue;
    }
    if (evidenceIds.has(vector.id)) {
      violations.push(`duplicate evidence for ${vector.id}`);
      continue;
    }
    evidenceIds.add(vector.id);
    if (!contractIds.has(vector.id)) {
      violations.push(`unknown vector ${vector.id}`);
    }
    if (!Array.isArray(vector.anchors) || vector.anchors.length === 0) {
      violations.push(`evidence for ${vector.id} must contain at least one anchor`);
      continue;
    }

    for (const anchor of vector.anchors) {
      anchorCount += 1;
      const command = suiteCommands[anchor.suite];
      if (command === undefined) {
        violations.push(`${vector.id} uses unknown suite ${String(anchor.suite)}`);
      } else if (!workflow.includes(command)) {
        violations.push(`suite ${anchor.suite} is not executed by CI`);
      }
      if (!safeRelativePath(anchor.file)) {
        violations.push(`${vector.id} has an unsafe evidence path`);
        continue;
      }
      if (command !== undefined && !suiteFilePatterns[anchor.suite].test(anchor.file)) {
        violations.push(`${vector.id} evidence file does not belong to suite ${anchor.suite}`);
        continue;
      }
      if (typeof anchor.marker !== "string" || anchor.marker.length < 8) {
        violations.push(`${vector.id} has an invalid evidence marker`);
        continue;
      }
      const absolutePath = path.join(root, anchor.file);
      try {
        if (!(await isRegularNonLinkPath(root, anchor.file))) {
          violations.push(`${vector.id} evidence must be a regular non-link file`);
          continue;
        }
        const source = await readFile(absolutePath, "utf8");
        if (!source.includes(anchor.marker)) {
          violations.push(`${vector.id} marker is absent from ${anchor.file}`);
        }
      } catch {
        violations.push(`${vector.id} evidence file is unavailable: ${anchor.file}`);
      }
    }
  }

  for (const id of contractIds) {
    if (!evidenceIds.has(id)) violations.push(`missing evidence for ${id}`);
  }

  if (violations.length > 0) {
    throw new Error(`Test traceability failed: ${violations.join("; ")}`);
  }
  return { vectors: contractIds.size, anchors: anchorCount };
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  try {
    const summary = await verifyTestTraceability();
    console.log(
      `Test traceability passed: ${summary.vectors} vectors, ${summary.anchors} executable evidence anchors.`,
    );
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exitCode = 1;
  }
}
