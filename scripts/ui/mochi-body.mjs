// Mochi with a body picture of the user's own (owner, 2026-10-18: "zmienianie grafiki mochi ale
// właśnie tylko ta grafika bez oczu i ust i emocji"). On a test copy of its own (virtual modules,
// never the focus): Mochi on the dial, a cut-out picture (an orange cat) as its body, as a user
// would set it: the dial's settings show "Its body" with the picture's thumbnail, Change and
// Remove; the dial in the window shows the picture; Remove brings Mochi's own body back.
// The system's file dialog behind "Pick a picture" cannot be driven: the picture is imported the
// way that dialog's answer is (image_import), the rest is clicked.
//   node scripts/ui/mochi-body.mjs     # screenshots in logs/mochi-body-*.png
import { chromium } from "playwright-core";
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

const root = path.resolve(path.dirname(new URL(import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1")), "..", "..");
const dir = path.join(os.tmpdir(), "hubdeck-mochi-body");
const PORT = 9479;
const ps = (...a) => execFileSync("powershell", ["-NoProfile", "-ExecutionPolicy", "Bypass", "-File", path.join(root, "scripts", "automation.ps1"), "-Dir", dir, "-CdpPort", String(PORT), ...a], { encoding: "utf8" });
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const results = [];
const check = (what, ok, note = "") => (results.push({ ok }), console.log(`${ok ? "ok  " : "FAIL"} ${what}${note ? ` (${note})` : ""}`));
// the cat: orange, with ears, on a see-through background
const cat = path.join(os.tmpdir(), "hubdeck-mochi-body-cat.svg");
fs.writeFileSync(cat, `<svg xmlns="http://www.w3.org/2000/svg" width="300" height="280" viewBox="0 0 300 280"><path d="M60 110 L70 20 L125 75 Q150 68 175 75 L230 20 L240 110 Q265 160 250 215 Q235 270 150 272 Q65 270 50 215 Q35 160 60 110Z" fill="#f2a24a"/><ellipse cx="150" cy="232" rx="58" ry="34" fill="#ffe7cf"/></svg>`);
ps("-Stop");
ps("-Fresh");
const browser = await chromium.connectOverCDP(`http://127.0.0.1:${PORT}`);
let page;
for (let i = 0; i < 40 && !page; i++) {
  page = browser.contexts()[0]?.pages().find((p) => /tauri.localhost|localhost:1460/.test(p.url()));
  if (!page) await sleep(500);
}
await page.setViewportSize({ width: 1280, height: 800 });
const invoke = (cmd, a) => page.evaluate(([c, x]) => window.__TAURI_INTERNALS__.invoke(c, x), [cmd, a ?? {}]);
const token = fs.readFileSync(path.join(dir, "api-token"), "utf8").trim();
const port = fs.readFileSync(path.join(dir, "api-http-port"), "utf8").trim();
const api = (route, body) => fetch(`http://127.0.0.1:${port}/api/${route}`, { method: body ? "POST" : "GET", headers: { Authorization: `Bearer ${token}`, "Content-Type": "application/json" }, body: body ? JSON.stringify(body) : undefined }).then((r) => r.json());
const dial = () => page.locator('g.dk-ctl[aria-label^="Dial"]').first();
// orange pixels of the dial as the window draws it
const orange = async () => {
  const shot = (await dial().screenshot()).toString("base64");
  return page.evaluate(
    (b64) =>
      new Promise((done) => {
        const img = new Image();
        img.onload = () => {
          const c = document.createElement("canvas");
          c.width = img.width;
          c.height = img.height;
          const g = c.getContext("2d");
          g.drawImage(img, 0, 0);
          const d = g.getImageData(0, 0, c.width, c.height).data;
          let n = 0;
          for (let i = 0; i < d.length; i += 4) if (d[i] > 200 && d[i + 1] > 120 && d[i + 1] < 190 && d[i + 2] < 110) n++;
          done(n);
        };
        img.src = `data:image/png;base64,${b64}`;
      }),
    shot,
  );
};
try {
  await page.waitForSelector("g.dk-ctl", { timeout: 20000 });
  const state = await invoke("engine_state");
  const profile = state.profiles.find((p) => p.id === state.active) ?? state.profiles[0];
  // the copy of the owner's data has his own Mochi picture on the Fun page: this walk starts without one
  for (const [c, bnd] of Object.entries((await api(`profiles/${profile.id}`)).controls ?? {}))
    if (bnd.action === "core.pet" && bnd.params?.body) await api(`profiles/${profile.id}/controls`, { control: c, binding: { ...bnd, params: { ...bnd.params, body: "" } } });
  await api(`profiles/${profile.id}/controls`, { control: "dial", binding: { action: "core.pet", params: { name: "Mochi" } } });
  await sleep(1500);
  const before = await orange();
  await dial().dispatchEvent("click");
  const field = page.getByText("Its body", { exact: true });
  await field.waitFor({ timeout: 8000 }).catch(() => {});
  check('the dial\'s settings show "Its body"', (await field.count()) > 0);
  check('with "Pick a picture" while it has none', (await page.locator("button", { hasText: "Pick a picture" }).count()) === 1);
  await page.screenshot({ path: path.join(root, "logs", "mochi-body-1-none.png") });
  // what the file dialog's answer does: the picture is copied in, its name goes in the field
  const name = await invoke("image_import", { path: cat });
  await api(`profiles/${profile.id}/controls`, { control: "dial", binding: { action: "core.pet", params: { name: "Mochi", body: name } } });
  await page.waitForSelector("img.picture-thumb", { timeout: 8000 }).catch(() => {});
  const thumb = await page.locator("img.picture-thumb").getAttribute("src").catch(() => null);
  check("its thumbnail shows the picture", !!thumb?.startsWith("data:image/"));
  check("with Change and Remove", (await page.getByRole("button", { name: "Change", exact: true }).count()) === 1 && (await page.getByRole("button", { name: "Remove", exact: true }).count()) === 1);
  // the Fit field (owner's 2a, 2026-10-18): four moods drawn by the engine, Size, a drag, Reset
  await page.waitForSelector(".pet-fit-shots img", { timeout: 8000 }).catch(() => {});
  const shots = await page.locator(".pet-fit img").count();
  check("Fit shows Mochi with the picture in four moods", shots === 4, `${shots} previews`);
  const bigBefore = await page.locator(".pet-fit-shots img").first().getAttribute("src");
  await page.locator(".pet-fit input[type=range]").fill("60");
  await sleep(900);
  const fitAfterSize = (await api(`profiles/${profile.id}`)).controls?.dial?.params?.bodyFit;
  check("Size changes the picture and is saved", fitAfterSize?.size === 60 && (await page.locator(".pet-fit-shots img").first().getAttribute("src")) !== bigBefore, JSON.stringify(fitAfterSize));
  const box = await page.locator(".pet-fit-shots img").first().boundingBox();
  await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
  await page.mouse.down();
  await page.mouse.move(box.x + box.width / 2 + 30, box.y + box.height / 2 + 12, { steps: 5 });
  await page.mouse.up();
  await sleep(900);
  const moved = (await api(`profiles/${profile.id}`)).controls?.dial?.params?.bodyFit;
  // a preview's pixel is 240 / its width of the dial's
  const want = { x: Math.round((30 * 240) / box.width), y: Math.round((12 * 240) / box.width) };
  check("dragging a preview moves the picture", Math.abs(moved?.x - want.x) <= 1 && Math.abs(moved?.y - want.y) <= 1, `${JSON.stringify(moved)}, wanted about ${JSON.stringify(want)}`);
  await page.screenshot({ path: path.join(root, "logs", "mochi-body-2b-fit.png") });
  // no words cut or past the panel, in the dark and the light look
  for (const theme of ["dark", "light"]) {
    await page.evaluate((th) => document.documentElement.setAttribute("data-theme", th), theme);
    await sleep(300);
    const cut = await page.evaluate(() => {
      const panel = document.querySelector(".pet-fit").closest("aside, .inspector, section") ?? document.body;
      const edge = panel.getBoundingClientRect().right;
      return [...document.querySelectorAll(".pet-fit *")].filter((el) => el.scrollWidth > el.clientWidth + 1 && getComputedStyle(el).overflow !== "visible" || el.getBoundingClientRect().right > edge + 1).map((el) => el.tagName + "." + el.className);
    });
    check(`Fit has nothing cut or past the panel (${theme})`, cut.length === 0, cut.join(", "));
    await page.screenshot({ path: path.join(root, "logs", `mochi-body-2c-fit-${theme}.png`) });
  }
  await page.evaluate(() => document.documentElement.removeAttribute("data-theme"));
  await page.locator(".pet-fit").getByRole("button", { name: "Reset" }).click();
  await sleep(900);
  const reset = (await api(`profiles/${profile.id}`)).controls?.dial?.params?.bodyFit;
  check("Reset puts it back", reset?.size === 100 && reset?.x === 0 && reset?.y === 0, JSON.stringify(reset));
  await sleep(600);
  const withCat = await orange();
  check("the dial in the window shows the picture as Mochi's body", withCat > before + 500, `orange pixels ${before} -> ${withCat}`);
  await page.screenshot({ path: path.join(root, "logs", "mochi-body-2-cat.png") });
  await page.getByRole("button", { name: "Remove", exact: true }).click();
  await sleep(1500);
  const after = await orange();
  check("Remove brings Mochi's own body back", after < before + 200 && (await page.locator("button", { hasText: "Pick a picture" }).count()) === 1, `orange pixels ${after}`);
  await page.screenshot({ path: path.join(root, "logs", "mochi-body-3-removed.png") });
  // as on the owner's Fun page (2026-10-18): the picture on the key that puts Mochi on the dial
  await api(`profiles/${profile.id}/controls`, { control: "dial", binding: null });
  await api(`profiles/${profile.id}/controls`, { control: "keys.10", binding: { action: "core.pet", params: { press: "dial", body: name }, look: { label: "Mochi" } } });
  await sleep(800);
  await api("test/input", { kind: "keys", index: 10, gesture: "press" });
  await sleep(2000);
  const onDial = (await api(`profiles/${profile.id}`)).controls?.dial?.action;
  const fromKey = await orange();
  check("a key with the picture puts Mochi on the dial with that body", onDial === "core.pet" && fromKey > before + 500, `dial ${onDial}, orange pixels ${fromKey}`);
  await page.screenshot({ path: path.join(root, "logs", "mochi-body-4-from-key.png") });
  // the dial's own settings show that picture and change it on the key (owner, 2026-10-18: the dial
  // showed his picture and its settings said "Pick a picture")
  await dial().dispatchEvent("click");
  await page.waitForSelector(".pet-fit img", { timeout: 8000 }).catch(() => {});
  const dialThumb = await page.locator("img.picture-thumb").count();
  const big = await page.locator(".pet-fit-shots img").first().boundingBox();
  check("the dial's settings show the key's picture, four big previews", dialThumb === 1 && (await page.locator(".pet-fit-shots img").count()) === 4 && (big?.width ?? 0) >= 120, `thumb ${dialThumb}, preview ${Math.round(big?.width ?? 0)} px`);
  await page.locator(".pet-fit input[type=range]").fill("80");
  await sleep(1200);
  const keyFit = (await api(`profiles/${profile.id}`)).controls?.["keys.10"]?.params?.bodyFit;
  check("Size on the dial changes the key's picture", keyFit?.size === 80, JSON.stringify(keyFit));
  await page.screenshot({ path: path.join(root, "logs", "mochi-body-5-dial-shared.png") });
} finally {
  await browser.close().catch(() => {});
  ps("-Stop");
}
const failed = results.filter((r) => !r.ok).length;
console.log(failed ? `\nFAILED: ${failed} of ${results.length}` : `\npassed ${results.length}/${results.length}`);
process.exit(failed ? 1 : 0);
