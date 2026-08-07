import { expect, test } from "@playwright/test";

test("renders the sealed MQTT incident and both Doctor evidence boundaries", async ({ page }) => {
  await page.goto("/");

  await expect(page).toHaveTitle("VDA 5050 Live Lab");
  await expect(page.getByText("SYNTHETIC", { exact: true })).toBeVisible();
  await expect(page.getByText("DOCTOR OFFLINE", { exact: true })).toBeVisible();
  await expect(page.locator("#run-status")).toHaveText("DIAGNOSED", { timeout: 10_000 });
  await expect(page.getByText("demo-001", { exact: true })).toBeVisible();
  await expect(page.getByText("demo-002", { exact: true })).toBeVisible();
  await expect(page.locator("#passive-verdict")).toHaveText("INCONCLUSIVE");
  await expect(page.locator("#passive-target")).toHaveText("UNRESOLVED");
  await expect(page.locator("#evidence-verdict")).toHaveText("FAIL");
  await expect(page.locator("#evidence-target")).toHaveText("MOBILE_ROBOT");
  await expect(page.getByText("CONNECTION BROKEN", { exact: true })).toBeVisible();
  await expect(page.getByText("RECONNECTED WITHOUT ONLINE", { exact: true })).toBeVisible();

  const timeline = page.locator("#timeline");
  await timeline.fill("0");
  await expect(page.locator("#cursor-label")).toHaveText("REPLAY");
  await page.locator("#follow-live").click();
  await expect(page.locator("#cursor-label")).toHaveText("LIVE EDGE");
});
