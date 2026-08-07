import { chromium } from "playwright";

const baseURL = process.env.PLAYWRIGHT_BASE_URL ?? "http://web_gateway:8080";
const outputPath = process.env.VDA5050_LIVE_RECORD_PATH ?? "/output/live-demo.webm";
const browser = await chromium.launch({ headless: true });
const context = await browser.newContext({
  viewport: { width: 1440, height: 1000 },
  recordVideo: { dir: "/tmp/vda5050-video", size: { width: 1440, height: 1000 } }
});
const page = await context.newPage();
await page.goto(baseURL, { waitUntil: "networkidle" });
await page.locator("#run-status").waitFor({ state: "visible" });
await page.locator("#evidence-verdict").waitFor({ state: "visible" });
await page.waitForFunction(() => document.querySelector("#run-status")?.textContent === "DIAGNOSED");

const timeline = page.locator("#timeline");
const maximum = Number(await timeline.getAttribute("max"));
for (let frame = 0; frame <= 100; frame += 1) {
  const value = Math.round((maximum * frame) / 100);
  await timeline.evaluate((element, nextValue) => {
    element.value = String(nextValue);
    element.dispatchEvent(new Event("input", { bubbles: true }));
  }, value);
  await page.waitForTimeout(70);
}
await page.locator("#follow-live").click();
await page.locator("#passive-verdict").scrollIntoViewIfNeeded();
await page.waitForTimeout(2_000);

const video = page.video();
await context.close();
if (video === null) throw new Error("Playwright video was not created");
await video.saveAs(outputPath);
await browser.close();
