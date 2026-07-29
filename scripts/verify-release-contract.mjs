import { readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import path from "node:path";

const repositoryRoot = fileURLToPath(new URL("..", import.meta.url));

async function read(relativePath) {
  return readFile(path.join(repositoryRoot, relativePath), "utf8");
}

function cargoPackageVersion(manifest) {
  const packageSection = manifest.match(/^\[package\]\s*$([\s\S]*?)(?=^\[|\z)/m);
  return packageSection?.[1].match(/^version\s*=\s*"([^"]+)"\s*$/m)?.[1] ?? null;
}

function requireText(violations, text, pattern, label) {
  if (!pattern.test(text)) violations.push(label);
}

const [packageText, cargoText, tauriText, workflow] = await Promise.all([
  read("package.json"),
  read("src-tauri/Cargo.toml"),
  read("src-tauri/tauri.conf.json"),
  read(".github/workflows/ci.yml"),
]);

const packageJson = JSON.parse(packageText);
const tauriConfig = JSON.parse(tauriText);
const cargoVersion = cargoPackageVersion(cargoText);
const violations = [];

if (
  cargoVersion === null
  || packageJson.version !== cargoVersion
  || tauriConfig.version !== cargoVersion
) {
  violations.push("package, Cargo, and Tauri versions must match");
}
if (tauriConfig.bundle?.active !== true) {
  violations.push("Tauri bundling must be active");
}

const platformContracts = [
  ["macos-14", "app,dmg", "AIKnowledgeSort-macOS"],
  ["ubuntu-22.04", "deb,appimage", "AIKnowledgeSort-Linux"],
  ["windows-2022", "nsis", "AIKnowledgeSort-Windows"],
];
for (const [os, bundles, artifactName] of platformContracts) {
  requireText(
    violations,
    workflow,
    new RegExp(`- os: ${os}[\\s\\S]*?bundles: ${bundles}[\\s\\S]*?artifact_name: ${artifactName}`),
    `${os} bundle matrix entry is missing`,
  );
}

requireText(
  violations,
  workflow,
  /npm run tauri build -- --ci --no-sign --bundles \$\{\{ matrix\.bundles \}\}/,
  "matrix bundle build command is missing",
);
requireText(
  violations,
  workflow,
  /scripts\/run-desktop-smoke\.mjs \$\{\{ matrix\.executable \}\}/,
  "desktop startup smoke command is missing",
);
requireText(
  violations,
  workflow,
  /uses: actions\/upload-artifact@[0-9a-f]{40}\s+# v4/,
  "upload-artifact must be pinned to a full v4 commit SHA",
);
requireText(
  violations,
  workflow,
  /if-no-files-found:\s*error/,
  "missing bundles must fail artifact upload",
);
requireText(
  violations,
  workflow,
  /^permissions:\s*\n\s+contents:\s+read\s*$/m,
  "workflow permissions must remain contents read-only",
);

if (violations.length > 0) {
  console.error("Release contract failed:");
  for (const violation of violations) console.error(`- ${violation}`);
  process.exitCode = 1;
} else {
  console.log("Release contract passed: macOS app/DMG, Linux DEB/AppImage, Windows NSIS.");
}
