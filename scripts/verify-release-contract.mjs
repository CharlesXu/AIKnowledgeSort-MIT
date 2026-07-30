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

const [
  packageText,
  cargoText,
  tauriText,
  workflow,
  profileCommands,
  vaultCommands,
  agentAccessCommands,
  mainCapabilityText,
] = await Promise.all([
  read("package.json"),
  read("src-tauri/Cargo.toml"),
  read("src-tauri/tauri.conf.json"),
  read(".github/workflows/ci.yml"),
  read("src-tauri/src/profiles/mod.rs"),
  read("src-tauri/src/vault/mod.rs"),
  read("src-tauri/src/agent_access/mod.rs"),
  read("src-tauri/capabilities/main.json"),
]);

const packageJson = JSON.parse(packageText);
const tauriConfig = JSON.parse(tauriText);
const mainCapability = JSON.parse(mainCapabilityText);
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
const mainPermissions = mainCapability.permissions ?? [];
for (const permission of [
  "core:event:allow-listen",
  "core:event:allow-unlisten",
]) {
  if (!mainPermissions.includes(permission)) {
    violations.push(`main capability must include ${permission}`);
  }
}
for (const overbroadPermission of [
  "core:event:default",
  "core:event:allow-emit",
  "core:event:allow-emit-to",
]) {
  if (mainPermissions.includes(overbroadPermission)) {
    violations.push(`main capability must not include ${overbroadPermission}`);
  }
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
  /smoke_prefix: dbus-run-session -- xvfb-run -a env NO_AT_BRIDGE=1/,
  "Linux desktop smoke must provide a session D-Bus and disable the AT-SPI bridge",
);
requireText(
  violations,
  workflow,
  /uses: actions\/upload-artifact@[0-9a-f]{40}\s+# v7/,
  "upload-artifact must be pinned to a full v7 commit SHA",
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
for (const [source, label] of [
  [profileCommands, "profile commands"],
  [vaultCommands, "Vault commands"],
  [agentAccessCommands, "Agent access commands"],
]) {
  if (/blocking_pick_(?:file|files|folder|folders)\s*\(/.test(source)) {
    violations.push(`${label} must not block the application thread on native dialogs`);
  }
}

if (violations.length > 0) {
  console.error("Release contract failed:");
  for (const violation of violations) console.error(`- ${violation}`);
  process.exitCode = 1;
} else {
  console.log("Release contract passed: macOS app/DMG, Linux DEB/AppImage, Windows NSIS.");
}
