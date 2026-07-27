import { defineConfig, devices } from "@playwright/test";

const e2eUrl = "http://127.0.0.1:1422";

export default defineConfig({
  testDir: "./e2e",
  fullyParallel: true,
  reporter: "line",
  use: {
    baseURL: e2eUrl,
    trace: "retain-on-failure",
  },
  projects: [
    {
      name: "chromium",
      use: {
        ...devices["Desktop Chrome"],
        viewport: { width: 1600, height: 900 },
      },
    },
  ],
  webServer: {
    command: "npm run dev -- --host 127.0.0.1 --port 1422",
    reuseExistingServer: !process.env.CI,
    timeout: 120_000,
    url: e2eUrl,
  },
});
