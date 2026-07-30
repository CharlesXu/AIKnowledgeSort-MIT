import assert from "node:assert/strict";
import { test } from "node:test";
import { verifyInventory } from "./verify-provenance-contract.mjs";

// Keep this under Node's native runner because it exercises filesystem policy.
const sentinelBytes = Buffer.from("license-review-sentinel\n");
const sentinelSha256 =
  "c039df0eb2c0b8ec8404ef1235c2979c9bde160ecc7de3f4bd5b98e39816b376";

function inventory(asset) {
  return {
    schemaVersion: 1,
    roots: ["release-assets"],
    assets: [asset],
  };
}

test("rejects an uncleared third-party release asset", () => {
  const violations = verifyInventory(
    inventory({
      path: "release-assets/sentinel.txt",
      sha256: sentinelSha256,
      origin: "thirdParty",
      license: "MIT",
    }),
    new Map([["release-assets/sentinel.txt", sentinelBytes]]),
  );

  assert.ok(violations.some((violation) => violation.includes("clearance")));
});

test("accepts the same third-party asset only with explicit clearance evidence", () => {
  const violations = verifyInventory(
    inventory({
      path: "release-assets/sentinel.txt",
      sha256: sentinelSha256,
      origin: "thirdParty",
      license: "MIT",
      clearance: {
        decision: "cleared",
        evidenceId: "REVIEW-2026-07-31-SENTINEL",
        reviewedBy: "release-reviewer",
        reviewedAt: "2026-07-31",
      },
    }),
    new Map([["release-assets/sentinel.txt", sentinelBytes]]),
  );

  assert.deepEqual(violations, []);
});

test("rejects unlisted and byte-changed release assets", () => {
  const releaseInventory = inventory({
    path: "release-assets/sentinel.txt",
    sha256: sentinelSha256,
    origin: "cleanroomGenerated",
    license: "MIT",
    authorizationId: "CLEANROOM-TEST-DATA",
  });

  const changed = verifyInventory(
    releaseInventory,
    new Map([["release-assets/sentinel.txt", Buffer.from("changed\n")]]),
  );
  assert.ok(changed.some((violation) => violation.includes("SHA-256")));

  const unlisted = verifyInventory(
    releaseInventory,
    new Map([
      ["release-assets/sentinel.txt", sentinelBytes],
      ["release-assets/unlisted.txt", Buffer.from("unlisted\n")],
    ]),
  );
  assert.ok(unlisted.some((violation) => violation.includes("not inventoried")));
});
