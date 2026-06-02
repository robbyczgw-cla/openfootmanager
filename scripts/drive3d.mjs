// TEMP — drive to a live match and freeze early to screenshot the 3D pitch.
import puppeteer from "puppeteer-core";
const CHROME = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";
const out = process.argv[2] || "/tmp/match3d.png";

const browser = await puppeteer.launch({
  executablePath: CHROME,
  headless: "new",
  args: [
    "--no-sandbox", "--use-gl=angle", "--use-angle=swiftshader",
    "--enable-unsafe-swiftshader", "--ignore-gpu-blocklist", "--window-size=1440,1000",
  ],
});
const page = await browser.newPage();
await page.setViewport({ width: 1440, height: 1000 });
const errs = [];
page.on("pageerror", (e) => errs.push(`[pageerror] ${e.message}`));

async function click(text) {
  const h = await page.evaluateHandle((t) => {
    const els = [...document.querySelectorAll('button, a, [role="button"]')];
    return els.find((e) => (e.innerText || "").trim().toLowerCase().includes(t.toLowerCase())) || null;
  }, text);
  const el = h.asElement();
  if (el) { await el.click(); return true; }
  return false;
}
const wait = (ms) => new Promise((r) => setTimeout(r, ms));

await page.goto("http://localhost:8080/", { waitUntil: "networkidle0", timeout: 20000 });
await wait(900);
await click("LOAD GAME"); await wait(700);
await click("career"); await wait(1600);
await click("Go to the Field"); await wait(700);
await click("CONFIRM"); await wait(1500);
await click("START MATCH"); await wait(150);
await click("FULL MATCH"); await wait(120);
await click("PAUSE"); await wait(150);
await click("SLOW"); await wait(2500); // resume slowly so players are mid-play
await click("PAUSE"); await wait(500);

await page.screenshot({ path: out });
const svgCount = await page.evaluate(() => document.querySelectorAll("svg").length);
const canvasCount = await page.evaluate(() => document.querySelectorAll("canvas").length);
console.log("url:", page.url(), "svg:", svgCount, "canvas:", canvasCount);
console.log("errs:", errs.join(" | ") || "none");
await browser.close();
