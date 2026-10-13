// A whole interview talk in Polish, answered the way the owner would (session 6), with a
// screenshot after every turn.
import path from "node:path";
const SHOTS = process.env.SHOTS;
const ANSWERS = [
  "Głównie praca z AI i agentami (Claude Code, kilka sesji naraz), do tego Spotify i czytanie newsów w Firefoxie. W gry teraz prawie nie gram.",
  "Przełączanie się między sesjami agentów i szukanie, która czeka na mnie. I ciągłe ogarnianie muzyki.",
  "Sesje Claude Code, Spotify (graj, lubię, głośność), Firefox z newsami, dyktowanie.",
  "Spokojnie, ale z prawdziwymi ikonami aplikacji. Bez fajerwerków.",
  "Wystarczy pytań, pokaż pomysły.",
];
export default async (page) => {
  page.setDefaultTimeout(240000);
  await page.evaluate(() => localStorage.setItem("hubdeck.language", "pl"));
  await page.reload();
  await page.locator(".lib-tabs .tab").nth(2).click();
  await page.locator(".as-intro .tool.primary").click();
  await page.locator(".as-consent").waitFor();
  const log = [];
  const waitTurn = async () => {
    await page.waitForTimeout(1500);
    await page.waitForFunction(() => !document.querySelector(".as-work.is-live"), null, { timeout: 240000 });
  };
  const t0 = Date.now();
  await page.locator(".as-starter").first().click();
  for (let turn = 0; turn <= ANSWERS.length; turn++) {
    await waitTurn();
    const said = await page.locator(".as-msg.is-assistant:not(.is-cards)").last().textContent().catch(() => "");
    const choices = await page.locator(".as-choices .chip").allTextContents();
    const cards = await page.locator(".as-carousel .as-sketch").count();
    log.push({ turn, s: Math.round((Date.now() - t0) / 1000), said, choices, cards });
    await page.evaluate(() => document.querySelector(".as-body")?.scrollTo(0, 1e9));
    await page.screenshot({ path: path.join(SHOTS, `iv-${turn}.png`), clip: { x: 0, y: 0, width: 500, height: 1025 } });
    if (cards > 0 || turn === ANSWERS.length) break;
    await page.fill(".as-input input", ANSWERS[turn]);
    await page.locator(".as-input button[type=submit]").click();
    log.push({ you: ANSWERS[turn] });
  }
  return log;
};
