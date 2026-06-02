// TEMP e2e driver — load the app, optionally click a sequence of buttons by
// visible text (passed as args), then dump clickable elements + screenshot.
// Usage: node scripts/drive.mjs "/tmp/out.png" "Neues Spiel" "Weiter" ...
import puppeteer from "puppeteer-core";

const CHROME = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";
const out = process.argv[2] || "/tmp/drive.png";
const clicks = process.argv.slice(3);

const browser = await puppeteer.launch({
  executablePath: CHROME,
  headless: "new",
  args: [
    "--no-sandbox",
    "--use-gl=angle",
    "--use-angle=swiftshader",
    "--enable-unsafe-swiftshader",
    "--ignore-gpu-blocklist",
    "--window-size=1440,1000",
  ],
});
const page = await browser.newPage();
await page.setViewport({ width: 1440, height: 1000 });
const logs = [];
page.on("pageerror", (e) => logs.push(`[pageerror] ${e.message}`));

await page.goto("http://localhost:8080/", { waitUntil: "networkidle0", timeout: 20000 });
await new Promise((r) => setTimeout(r, 1200));

async function clickByText(text) {
  const handle = await page.evaluateHandle((t) => {
    const els = [...document.querySelectorAll('button, a, [role="button"], [role="tab"]')];
    return els.find((e) => (e.innerText || "").trim().toLowerCase().includes(t.toLowerCase())) || null;
  }, text);
  const el = handle.asElement();
  if (!el) return false;
  await el.click();
  return true;
}

for (const c of clicks) {
  const ok = await clickByText(c);
  logs.push(`click "${c}" -> ${ok ? "ok" : "NOT FOUND"} (url=${page.url()})`);
  await new Promise((r) => setTimeout(r, 1500));
}

// Settle, then capture two screenshots ~1.2s apart to reveal motion.
await new Promise((r) => setTimeout(r, 3000));
await page.screenshot({ path: out });
const out2 = out.replace(/\.png$/, "-2.png");
await new Promise((r) => setTimeout(r, 1200));
await page.screenshot({ path: out2 });

const clickable = await page.evaluate(() => {
  const els = [...document.querySelectorAll('button, a, [role="button"], [role="tab"]')];
  return els
    .map((e) => (e.innerText || "").replace(/\s+/g, " ").trim())
    .filter((t) => t.length > 0 && t.length < 60)
    .slice(0, 50);
});
const svgCount = await page.evaluate(() => document.querySelectorAll("svg").length);
await page.screenshot({ path: out });
console.log("url:", page.url());
console.log("svgCount:", svgCount);
console.log("clickable:", JSON.stringify([...new Set(clickable)], null, 0));
console.log("logs:", logs.join(" | "));
await browser.close();
