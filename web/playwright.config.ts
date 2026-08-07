import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: "./e2e",
  fullyParallel: false,
  retries: 0,
  reporter: "line",
  outputDir: "/tmp/vda5050-playwright-results",
  use: {
    baseURL: process.env.PLAYWRIGHT_BASE_URL ?? "http://web_gateway:8080",
    browserName: "chromium",
    screenshot: "only-on-failure",
    trace: "retain-on-failure"
  }
});
