import { createRequire } from "node:module";

const { chromium } = createRequire("/oracle/package.json")("playwright-core");

export const FIXED_CLOCK = new Date("2026-11-26T00:00:00Z");
export const LAUNCH_ARGS = ["--font-render-hinting=none", "--disable-lcd-text"];

export async function launch() {
  return chromium.launch({ args: LAUNCH_ARGS });
}

export async function newPage(browser, { width = 800, height = 1200, dpr = 2, theme = "light" } = {}) {
  const context = await browser.newContext({
    viewport: { width, height },
    deviceScaleFactor: dpr,
    colorScheme: theme,
    locale: "en-US",
    timezoneId: "UTC",
  });
  await context.clock.setFixedTime(FIXED_CLOCK);
  const page = await context.newPage();
  return { context, page };
}
