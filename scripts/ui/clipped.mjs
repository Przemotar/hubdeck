// Finds text the window cuts: buttons, tabs and chips whose words do not fit (scrollWidth over
// clientWidth) or that stick out of their row. Run on the test copy for every state you change:
//   node run.mjs clipped.mjs            (the window as it is now)
// The lead missed a cut "Assistant" tab once by looking only at screenshots of another state.
export async function clipped(page) {
  return page.evaluate(() => {
    const out = [];
    const sel = 'button, [role="tab"], .chip, .tab, .tool, .menu-row, .sec-title, .label, .dk-chip';
    for (const el of document.querySelectorAll(sel)) {
      const r = el.getBoundingClientRect();
      if (!r.width || !r.height) continue; // hidden
      const text = (el.innerText || el.getAttribute('aria-label') || '').trim().replace(/\s+/g, ' ').slice(0, 60);
      if (!text) continue;
      const cut = el.scrollWidth > el.clientWidth + 1;
      const parent = el.parentElement?.getBoundingClientRect();
      const out_of_row = parent && (r.right > parent.right + 1 || r.left < parent.left - 1) && getComputedStyle(el.parentElement).overflowX !== 'auto';
      if (cut || out_of_row) out.push({ text, cut, outOfRow: !!out_of_row, w: Math.round(r.width) });
    }
    return out;
  });
}

export default async (page) => clipped(page);
