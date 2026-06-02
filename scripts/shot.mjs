// TEMP diagnostic — drive system Chrome, capture console + errors + render.
import puppeteer from "puppeteer-core";

const CHROME =
  "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";
const url = process.argv[2] || "http://localhost:8080/pitch-preview";
const out = process.argv[3] || "/tmp/pp.png";

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
page.on("console", (m) => logs.push(`[console.${m.type()}] ${m.text()}`));
page.on("pageerror", (e) => logs.push(`[pageerror] ${e.message}\n${e.stack || ""}`));
page.on("requestfailed", (r) =>
  logs.push(`[requestfailed] ${r.url()} :: ${r.failure()?.errorText}`),
);

try {
  await page.goto(url, { waitUntil: "networkidle0", timeout: 20000 });
} catch (e) {
  logs.push(`[goto-error] ${e.message}`);
}
await new Promise((r) => setTimeout(r, 1500));

const rootHtml = await page.evaluate(() => {
  const el = document.getElementById("root");
  return el ? el.innerHTML.slice(0, 400) : "NO #root";
});
const svgCount = await page.evaluate(() => document.querySelectorAll("svg").length);

await page.screenshot({ path: out });
console.log("URL:", url);
console.log("svgCount:", svgCount);
console.log("rootHtml[0..400]:", JSON.stringify(rootHtml));
console.log("---- console/errors ----");
console.log(logs.length ? logs.join("\n") : "(no console output / errors)");
await browser.close();
