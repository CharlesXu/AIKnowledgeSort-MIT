import { createHash } from "node:crypto";
import { lstat, readFile, readdir } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import path from "node:path";

const repositoryRoot = fileURLToPath(new URL("..", import.meta.url));
const inventoryPath = "docs/RELEASE_ASSET_PROVENANCE.json";
const sha256Pattern = /^[0-9a-f]{64}$/;
const allowedOrigins = new Set([
  "authorizedFirstParty",
  "cleanroomGenerated",
  "thirdParty",
]);

function isSafeRelativePath(value) {
  return typeof value === "string"
    && value.length > 0
    && value === value.replaceAll("\\", "/")
    && !value.startsWith("/")
    && !value.split("/").includes("..")
    && path.posix.normalize(value) === value;
}

function insideRoot(assetPath, root) {
  return assetPath === root || assetPath.startsWith(`${root}/`);
}

function nonEmpty(value) {
  return typeof value === "string" && value.trim().length > 0;
}

export function verifyInventory(inventory, files) {
  const violations = [];
  if (inventory?.schemaVersion !== 1) {
    violations.push("provenance inventory schemaVersion must be 1");
  }
  const roots = Array.isArray(inventory?.roots) ? inventory.roots : [];
  const assets = Array.isArray(inventory?.assets) ? inventory.assets : [];
  if (roots.length === 0 || roots.some((root) => !isSafeRelativePath(root))) {
    violations.push("provenance inventory roots must contain safe relative paths");
  }

  const entries = new Map();
  for (const asset of assets) {
    if (!isSafeRelativePath(asset?.path)) {
      violations.push("provenance asset path must be a safe relative path");
      continue;
    }
    if (entries.has(asset.path)) {
      violations.push(`${asset.path} is inventoried more than once`);
      continue;
    }
    entries.set(asset.path, asset);
    if (!roots.some((root) => insideRoot(asset.path, root))) {
      violations.push(`${asset.path} is outside every declared asset root`);
    }
    if (!sha256Pattern.test(asset.sha256 ?? "")) {
      violations.push(`${asset.path} must record lowercase SHA-256`);
    }
    if (!allowedOrigins.has(asset.origin)) {
      violations.push(`${asset.path} has an unsupported origin`);
    }
    if (!nonEmpty(asset.license)) {
      violations.push(`${asset.path} must record its license`);
    }
    if (asset.origin === "thirdParty") {
      const clearance = asset.clearance;
      if (
        clearance?.decision !== "cleared"
        || !nonEmpty(clearance.evidenceId)
        || !nonEmpty(clearance.reviewedBy)
        || !/^\d{4}-\d{2}-\d{2}$/.test(clearance.reviewedAt ?? "")
      ) {
        violations.push(`${asset.path} requires explicit third-party clearance evidence`);
      }
    } else if (!nonEmpty(asset.authorizationId)) {
      violations.push(`${asset.path} requires first-party authorization evidence`);
    }
  }

  for (const [filePath, bytes] of files) {
    const asset = entries.get(filePath);
    if (asset === undefined) {
      violations.push(`${filePath} is not inventoried`);
      continue;
    }
    const actual = createHash("sha256").update(bytes).digest("hex");
    if (actual !== asset.sha256) {
      violations.push(`${filePath} SHA-256 does not match the reviewed asset`);
    }
  }
  for (const assetPath of entries.keys()) {
    if (!files.has(assetPath)) {
      violations.push(`${assetPath} is inventoried but missing`);
    }
  }
  return violations;
}

async function collectFiles(relativeRoot, files, violations) {
  const absoluteRoot = path.join(repositoryRoot, relativeRoot);
  let rootMetadata;
  let entries;
  try {
    rootMetadata = await lstat(absoluteRoot);
    if (rootMetadata.isSymbolicLink()) {
      violations.push(`${relativeRoot} asset root must not be a symbolic link`);
      return;
    }
    if (!rootMetadata.isDirectory()) {
      violations.push(`${relativeRoot} asset root must be a directory`);
      return;
    }
    entries = await readdir(absoluteRoot, { withFileTypes: true });
  } catch {
    violations.push(`${relativeRoot} asset root is missing or unreadable`);
    return;
  }
  for (const entry of entries) {
    const relativePath = path.posix.join(relativeRoot, entry.name);
    const absolutePath = path.join(repositoryRoot, relativePath);
    const metadata = await lstat(absolutePath);
    if (metadata.isSymbolicLink()) {
      violations.push(`${relativePath} must not be a symbolic link`);
    } else if (metadata.isDirectory()) {
      await collectFiles(relativePath, files, violations);
    } else if (metadata.isFile()) {
      files.set(relativePath, await readFile(absolutePath));
    } else {
      violations.push(`${relativePath} must be a regular file`);
    }
  }
}

async function main() {
  const inventory = JSON.parse(
    await readFile(path.join(repositoryRoot, inventoryPath), "utf8"),
  );
  const files = new Map();
  const violations = [];
  for (const root of inventory.roots ?? []) {
    if (isSafeRelativePath(root)) {
      await collectFiles(root, files, violations);
    }
  }
  violations.push(...verifyInventory(inventory, files));

  if (violations.length > 0) {
    console.error("Release provenance contract failed:");
    for (const violation of violations) console.error(`- ${violation}`);
    process.exitCode = 1;
  } else {
    console.log(
      `Release provenance contract passed: ${files.size} reviewed assets, no uncleared material.`,
    );
  }
}

if (
  process.argv[1]
  && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)
) {
  await main();
}
