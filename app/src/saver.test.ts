import { describe, expect, it } from "vitest";
import { rnd, saverBody } from "./saver";

describe("the window's screensaver", () => {
  it("draws the same pseudo-random field as saver.rs (u32 arithmetic)", () => {
    // the same values are pinned in saver.rs `rnd_matches_the_window`
    expect([[0, 1], [3, 1], [97, 4], [69, 9], [1234, 11]].map(([i, s]) => rnd(i, s))).toEqual([0.9995, 0.7379, 0.492, 0.1734, 0.0383]);
  });
  it("draws every effect and moves", () => {
    for (const e of ["rain", "stars", "waves"]) {
      const a = saverBody(e, 1, "#7c6cf0");
      expect(a).toBeTruthy();
      expect(saverBody(e, 1.4, "#7c6cf0")).not.toEqual(a);
    }
    expect(saverBody("nope", 0, "#fff")).toBeNull();
  });
});
