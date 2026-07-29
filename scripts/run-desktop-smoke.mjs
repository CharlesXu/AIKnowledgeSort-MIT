import { spawn } from "node:child_process";
import { stat } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repositoryRoot = fileURLToPath(new URL("..", import.meta.url));
const [requestedPath, ...extraArguments] = process.argv.slice(2);

if (requestedPath === undefined || extraArguments.length > 0 || path.isAbsolute(requestedPath)) {
  console.error("Desktop smoke requires exactly one repository-relative executable path.");
  process.exit(2);
}

const executablePath = path.resolve(repositoryRoot, requestedPath);
const relativePath = path.relative(repositoryRoot, executablePath);
if (relativePath.startsWith("..") || path.isAbsolute(relativePath)) {
  console.error("Desktop smoke executable must remain inside the repository.");
  process.exit(2);
}

let metadata;
try {
  metadata = await stat(executablePath);
} catch {
  console.error("Desktop smoke executable is unavailable.");
  process.exit(2);
}
if (!metadata.isFile()) {
  console.error("Desktop smoke target must be one executable file.");
  process.exit(2);
}

const child = spawn(executablePath, ["--desktop-smoke"], {
  env: process.env,
  stdio: "inherit",
});

let timedOut = false;
const timeout = setTimeout(() => {
  timedOut = true;
  child.kill();
}, 30_000);

child.once("error", () => {
  clearTimeout(timeout);
  console.error("Desktop smoke process could not be started.");
  process.exitCode = 1;
});

child.once("exit", (code, signal) => {
  clearTimeout(timeout);
  if (timedOut) {
    console.error("Desktop smoke did not become ready within 30 seconds.");
    process.exitCode = 1;
  } else if (code !== 0) {
    console.error(
      signal === null
        ? `Desktop smoke exited with status ${code ?? "unknown"}.`
        : "Desktop smoke was terminated before readiness.",
    );
    process.exitCode = 1;
  } else {
    console.log("Desktop smoke passed: Tauri runtime reached ready state.");
  }
});
