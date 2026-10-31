// Fuji as one picture in the 480x480 world square (`world()`, as the Halloween scenes): lying the
// deck shows its band y 104..376, upright its column x 104..376. Three zones (owner, 2026-10-29: "te
// główne rzeczy są w tym 3x3 środkowym, ale po bokach i na górze też są istotne rzeczy, które
// dopełniają kadr i po prostu są przycinane"): the centre 3x3, seen both ways, holds what makes the
// scene (the mountain, the sun, the torii, the banner); the sides, seen lying only, a black pine on
// the left shore and a pagoda on a hill on the right, reeds in the water; the top and the bottom,
// seen upright only, cranes in the sky and the branches coming down, a boatman on the lake and
// reeds in front; the corners are never seen.
// Japan A, after the owner's reference picture (2026-10-26, "to jest japoński klimat"): a bright,
// flat, saturated day held dark by design. The top row: a blue sky with soft clouds, a big
// red-orange sun low in the middle behind the mountain, cherry branches with dense pink blossom
// and dark red buds coming in from both top corners; the middle row: Mount Fuji, deep blue with a
// wide snow cap and shaded ridges, a vermilion torii standing in the water on the right; the bottom
// row: the calm lake mirroring the mountain, the sun and the torii, a thin line of trees on the
// shore. On the left a red banner hangs from a string with 日本 in black.
// Laid out on the key windows (owner, 2026-10-29: "ten łuk ma podporę na łączeniu - jak to ładnie
// skomponować?"): the centre 3x3 windows stand at x and y 100..180, 200..280, 300..380 of the square
// in both turns (within 4 px), the gaps between them at 180..200 and 280..300. The horizon runs
// through the middle of the middle row, the sun and the peak sit in the row above, their mirror in
// the row below; every line that matters (the shore, the torii's beams and feet, the waterline under
// the boat) lies inside a window, and only trunks, pillars and slopes cross a gap.
const SHORE = 240.0;
const TORII_FOOT = 342.0;
const TORII_TOP = 224.0;
const TORII_X = 340.0;
// the sun centred in its window (the row above the horizon runs y 104..179 lying, 101..180 upright),
// the peak just under its middle
const SUN_C = vec2<f32>(240.0, 141.0);
const SUN_R = 24.0;
const PEAK = vec2<f32>(240.0, 148.0);
const BANNER_X = 118.0;
const BANNER_Y = 14.0;
fn fbm(p: vec2<f32>) -> f32 {
    var v = 0.0;
    var a = 0.5;
    var q = p;
    for (var i = 0; i < 4; i++) {
        v += a * vnoise(q);
        q = q * 2.03 + vec2<f32>(17.1, 9.3);
        a *= 0.5;
    }
    return v;
}
fn seg(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>) -> vec2<f32> {
    let pa = p - a;
    let ba = b - a;
    let h = clamp(dot(pa, ba) / dot(ba, ba), 0.0, 1.0);
    return vec2<f32>(length(pa - ba * h), h);
}
fn box(p: vec2<f32>, c: vec2<f32>, r: vec2<f32>) -> f32 {
    let d = abs(p - c) - r;
    return length(max(d, vec2<f32>(0.0))) + min(max(d.x, d.y), 0.0);
}
// the mountain's top edge at x: concave flanks, a small flat crater with notches
fn fuji_top(x: f32) -> f32 {
    let t = clamp(abs(x - PEAK.x) / 215.0, 0.0, 1.0);
    let h = (SHORE - PEAK.y) * pow(1.0 - t, 1.9);
    let crater = 1.2 * sin(x * 0.9) * smoothstep(9.0, 0.0, abs(x - PEAK.x));
    return SHORE - h + crater;
}
// the torii at TORII_FOOT: the black curved top beam (kasagi) over a red one, the lower cross beam
// (nuki), the two pillars and the short middle post; .x distance, .y 1 for black, 0 for red
fn torii(p: vec2<f32>) -> vec2<f32> {
    let cx = TORII_X;
    let top = TORII_TOP;
    let hgt = TORII_FOOT - TORII_TOP;
    // the kasagi rises towards both ends; it and the pillars fit one column of windows
    let ex = (p.x - cx) / 37.0;
    let lift = 3.6 * ex * ex;
    let kasagi = box(p + vec2<f32>(0.0, lift), vec2<f32>(cx, top), vec2<f32>(37.0 - 1.6 * ex * ex, 3.2));
    let shimaki = box(p + vec2<f32>(0.0, lift * 0.6), vec2<f32>(cx, top + 6.0), vec2<f32>(32.0, 2.6));
    let nuki = box(p, vec2<f32>(cx, top + 28.0), vec2<f32>(30.0, 2.4));
    let post = box(p, vec2<f32>(cx, top + 17.0), vec2<f32>(2.2, 8.0));
    // pillars lean in a little towards the top
    let py = clamp((TORII_FOOT - p.y) / hgt, 0.0, 1.0);
    let lx = cx - 22.0 + 2.0 * py;
    let rx = cx + 22.0 - 2.0 * py;
    let pw = 3.6 - 0.6 * py;
    let pillars = min(max(abs(p.x - lx) - pw, max(p.y - TORII_FOOT, top - p.y)), max(abs(p.x - rx) - pw, max(p.y - TORII_FOOT, top - p.y)));
    let red = min(min(shimaki, nuki), min(post, pillars));
    if (kasagi < red) { return vec2<f32>(kasagi, 1.0); }
    return vec2<f32>(red, 0.0);
}
// the far scene (the sky, the clouds, the sun, the mountain, the trees on the shore): what the lake
// mirrors
fn far(p: vec2<f32>, tm: f32) -> vec3<f32> {
    let v = clamp(p.y / SHORE, 0.0, 1.0);
    var col = mix(vec3<f32>(0.2, 0.42, 0.74), vec3<f32>(0.56, 0.74, 0.88), smoothstep(0.0, 1.0, v));
    // soft white clouds drifting, flat-bottomed like in a print
    let cn = fbm(vec2<f32>(p.x * 0.012 - tm * 0.006, p.y * 0.04));
    let cloud = smoothstep(0.56, 0.66, cn) * smoothstep(150.0, 20.0, p.y);
    col = mix(col, vec3<f32>(0.96, 0.97, 1.0) * (0.92 + 0.08 * smoothstep(0.56, 0.75, cn)), cloud * 0.85);
    // the sun: a flat red-orange disc, a soft warm air round it
    let ds = length(p - SUN_C);
    col = mix(col, col + vec3<f32>(0.25, 0.12, 0.0), exp(-max(ds - SUN_R, 0.0) / 30.0) * 0.6);
    col = mix(col, mix(vec3<f32>(0.96, 0.32, 0.1), vec3<f32>(0.9, 0.18, 0.08), smoothstep(0.0, SUN_R, ds)), smoothstep(SUN_R + 0.8, SUN_R - 0.8, ds));
    // the mountain
    let top = fuji_top(p.x);
    let md = top - p.y;
    if (md < 1.0) {
        let dx = p.x - PEAK.x;
        let h = clamp((SHORE - p.y) / (SHORE - PEAK.y), 0.0, 1.0);
        // ridges running down from the top, lit on the left, shaded on the right
        let ang = atan2(dx, (p.y - PEAK.y) + 6.0);
        let ridge = 0.5 + 0.5 * sin(ang * 26.0 + 2.0 * vnoise(vec2<f32>(ang * 6.0, p.y * 0.05)));
        var m = mix(vec3<f32>(0.1, 0.17, 0.38), vec3<f32>(0.2, 0.3, 0.55), ridge * 0.6 + 0.2 * smoothstep(40.0, -60.0, dx));
        m = mix(m, vec3<f32>(0.14, 0.24, 0.44), smoothstep(0.35, 0.0, h) * 0.5);
        // the snow cap: wide, its lower edge running down the ridges in tongues
        let snowline = 0.76 - 0.14 * pow(ridge, 3.0) - 0.05 * vnoise(vec2<f32>(p.x * 0.2, 1.0));
        let snow = smoothstep(snowline - 0.015, snowline + 0.015, h);
        var s = mix(vec3<f32>(0.96, 0.97, 1.0), vec3<f32>(0.66, 0.74, 0.9), smoothstep(-10.0, 40.0, dx) * 0.8 + (1.0 - ridge) * 0.25);
        m = mix(m, s, snow);
        col = mix(col, m, smoothstep(1.0, -1.0, md));
    }
    // low hills on the far shore at both sides, hazed; the right one carries a pagoda
    let hill = SHORE - 64.0 * exp(-pow((p.x - 446.0) / 38.0, 2.0)) - 18.0 * exp(-pow((p.x - 392.0) / 30.0, 2.0)) - 14.0 * exp(-pow((p.x - 40.0) / 50.0, 2.0)) - 6.0 * exp(-pow((p.x - 150.0) / 40.0, 2.0));
    if (p.y > hill - 1.0) {
        // forested: rounded tree tops along its line, crowns in rows down its face, hazed blue with
        // distance at the top, greener near the shore
        let crowns = 1.6 * abs(sin(p.x * 0.55 + 1.7 * vnoise(vec2<f32>(p.x * 0.1, 1.0))));
        let edge = hill - crowns;
        let rowsv = 0.6 * vnoise(vec2<f32>(p.x * 0.16, p.y * 0.22)) + 0.4 * vnoise(vec2<f32>(p.x * 0.32, p.y * 0.4) + 5.0);
        var hc = mix(vec3<f32>(0.22, 0.34, 0.46), vec3<f32>(0.1, 0.24, 0.18), smoothstep(edge, SHORE, p.y));
        hc *= 0.86 + 0.2 * rowsv;
        hc += vec3<f32>(0.06, 0.08, 0.03) * smoothstep(0.6, 0.8, rowsv) * smoothstep(SHORE, edge, p.y);
        col = mix(col, hc, smoothstep(edge - 0.8, edge + 0.8, p.y));
    }
    let pg = pagoda(p);
    if (pg.x < 1.0) {
        col = mix(col, mix(pg.yzw, vec3<f32>(0.42, 0.56, 0.74), 0.22), smoothstep(0.8, -0.8, pg.x));
    }
    // the thin line of trees on the shore
    let trees = SHORE - 3.0 - 3.0 * fbm(vec2<f32>(p.x * 0.18, 4.0)) - 2.0 * smoothstep(0.7, 0.9, vnoise(vec2<f32>(p.x * 0.5, 2.0)));
    if (p.y > trees) {
        col = mix(col, mix(vec3<f32>(0.08, 0.26, 0.14), vec3<f32>(0.18, 0.42, 0.2), vnoise(vec2<f32>(p.x * 0.4, p.y))), smoothstep(trees - 0.8, trees + 0.8, p.y));
    }
    return col;
}
// a tapered limb from a to b, thickness r0 at a and r1 at b: .x distance, .y 0..1 lit (its upper
// side faces the light), .z the distance along it (px)
fn limb(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>, r0: f32, r1: f32) -> vec3<f32> {
    let pa = p - a;
    let ba = b - a;
    let h = clamp(dot(pa, ba) / dot(ba, ba), 0.0, 1.0);
    let off = pa - ba * h;
    let rr = mix(r0, r1, h);
    let up = clamp(-off.y / max(rr, 0.5), -1.0, 1.0);
    return vec3<f32>(length(off) - rr, 0.5 + 0.5 * up, h * length(ba));
}
// the cherry limbs: from, to (x, y, x, y), and their thickness at each end
const LIMB_AT = array<vec4<f32>, 30>(
    vec4<f32>(-20.0, 20.0, 60.0, 62.0), vec4<f32>(60.0, 62.0, 120.0, 84.0), vec4<f32>(120.0, 84.0, 176.0, 92.0), vec4<f32>(176.0, 92.0, 204.0, 88.0),
    vec4<f32>(-20.0, 128.0, 40.0, 108.0), vec4<f32>(40.0, 108.0, 60.0, 62.0), vec4<f32>(40.0, 108.0, 52.0, 150.0), vec4<f32>(52.0, 150.0, 46.0, 176.0),
    vec4<f32>(150.0, -20.0, 140.0, 30.0), vec4<f32>(140.0, 30.0, 120.0, 84.0), vec4<f32>(120.0, 84.0, 132.0, 100.0), vec4<f32>(90.0, 74.0, 80.0, 96.0),
    vec4<f32>(176.0, 92.0, 182.0, 106.0), vec4<f32>(140.0, 30.0, 170.0, 40.0), vec4<f32>(60.0, 62.0, 70.0, 40.0), vec4<f32>(500.0, 20.0, 420.0, 62.0),
    vec4<f32>(420.0, 62.0, 360.0, 84.0), vec4<f32>(360.0, 84.0, 304.0, 92.0), vec4<f32>(304.0, 92.0, 276.0, 88.0), vec4<f32>(500.0, 128.0, 440.0, 108.0),
    vec4<f32>(440.0, 108.0, 420.0, 62.0), vec4<f32>(440.0, 108.0, 430.0, 128.0), vec4<f32>(330.0, -20.0, 340.0, 30.0), vec4<f32>(340.0, 30.0, 360.0, 84.0),
    vec4<f32>(352.0, 86.0, 346.0, 130.0), vec4<f32>(346.0, 130.0, 352.0, 166.0), vec4<f32>(360.0, 84.0, 372.0, 102.0), vec4<f32>(304.0, 92.0, 298.0, 106.0),
    vec4<f32>(340.0, 30.0, 310.0, 40.0), vec4<f32>(420.0, 62.0, 410.0, 40.0),
);
const LIMB_R = array<vec2<f32>, 30>(
    vec2<f32>(5.0, 4.2), vec2<f32>(4.2, 3.2), vec2<f32>(2.6, 1.6), vec2<f32>(1.5, 0.7),
    vec2<f32>(3.4, 2.7), vec2<f32>(2.7, 2.4), vec2<f32>(1.4, 0.8), vec2<f32>(0.8, 0.5),
    vec2<f32>(3.2, 2.6), vec2<f32>(2.6, 2.2), vec2<f32>(1.2, 0.5), vec2<f32>(1.2, 0.5),
    vec2<f32>(0.9, 0.4), vec2<f32>(1.2, 0.6), vec2<f32>(1.2, 0.5), vec2<f32>(5.0, 4.2),
    vec2<f32>(4.2, 3.2), vec2<f32>(2.6, 1.6), vec2<f32>(1.5, 0.7), vec2<f32>(3.4, 2.7),
    vec2<f32>(2.7, 2.4), vec2<f32>(1.2, 0.6), vec2<f32>(3.2, 2.6), vec2<f32>(2.6, 2.2),
    vec2<f32>(1.6, 1.0), vec2<f32>(1.0, 0.5), vec2<f32>(1.2, 0.5), vec2<f32>(0.9, 0.4),
    vec2<f32>(1.2, 0.6), vec2<f32>(1.2, 0.5),
);
// the cherry branches: gnarled, tapering limbs; .x distance, .y lit, .z along
fn branches(p: vec2<f32>) -> vec3<f32> {
    // a slow wander off the straight line, so no limb is a ruled stick
    let pw = p + (vec2<f32>(vnoise(p * 0.09), vnoise(p * 0.09 + 7.3)) - 0.5) * 3.2;
    var best = vec3<f32>(1e3, 0.5, 0.0);
    for (var i = 0; i < 30; i++) {
        let l = LIMB_AT[i];
        let r = LIMB_R[i];
        let e = limb(pw, l.xy, l.zw, r.x, r.y);
        if (e.x < best.x) { best = e; }
    }
    return best;
}
// the branches near p: the nearest limb as `branches` gives it, and up to four limbs within 20 px
// (the blossom looks only at those)
struct Near {
    best: vec3<f32>,
    // the limbs themselves, not their indices: indexing the constant table again for every flower
    // copied it whole each time (a frame cost 25 times the original's)
    at: array<vec4<f32>, 4>,
    r: array<vec2<f32>, 4>,
    n: i32,
}
fn branches_near(p: vec2<f32>) -> Near {
    let pw = p + (vec2<f32>(vnoise(p * 0.09), vnoise(p * 0.09 + 7.3)) - 0.5) * 3.2;
    var best = vec3<f32>(1e3, 0.5, 0.0);
    var at = array<vec4<f32>, 4>(vec4<f32>(0.0), vec4<f32>(0.0), vec4<f32>(0.0), vec4<f32>(0.0));
    var rs = array<vec2<f32>, 4>(vec2<f32>(0.0), vec2<f32>(0.0), vec2<f32>(0.0), vec2<f32>(0.0));
    var n = 0;
    let table = LIMB_AT;
    let radii = LIMB_R;
    for (var i = 0; i < 30; i++) {
        let l = table[i];
        // a limb whose box is far from here: neither drawn nor near
        if (pw.x < min(l.x, l.z) - 24.0 || pw.x > max(l.x, l.z) + 24.0 || pw.y < min(l.y, l.w) - 24.0 || pw.y > max(l.y, l.w) + 24.0) { continue; }
        let r = radii[i];
        let e = limb(pw, l.xy, l.zw, r.x, r.y);
        if (e.x < best.x) { best = e; }
        if (e.x < 20.0 && n < 4) {
            at[n] = l;
            rs[n] = r;
            n += 1;
        }
    }
    return Near(best, at, rs, n);
}
// how near a point is to the limbs listed (no wander: a flower's place, not a pixel)
fn near_limbs(c: vec2<f32>, nb: Near) -> f32 {
    var d = 1e3;
    for (var k = 0; k < nb.n; k++) {
        let l = nb.at[k];
        let r = nb.r[k];
        d = min(d, limb(c, l.xy, l.zw, r.x, r.y).x);
    }
    return d;
}
// cherry bark: dark red-brown, its upper side catching the light, the horizontal lenticels
fn bark(b: vec3<f32>, p: vec2<f32>) -> vec3<f32> {
    var c = mix(vec3<f32>(0.11, 0.05, 0.05), vec3<f32>(0.36, 0.2, 0.17), b.y);
    let band = smoothstep(0.82, 0.97, abs(sin(b.z * 0.9 + 2.0 * vnoise(p * 0.2)))) * step(0.45, vnoise(vec2<f32>(b.z * 0.25, 3.0)));
    c = mix(c, vec3<f32>(0.5, 0.36, 0.32), band * 0.45 * b.y);
    // the rim a touch darker, so a limb reads against the sky
    return c * (0.75 + 0.25 * smoothstep(0.0, -1.4, b.x));
}
// a cherry blossom: five petals with the notch at each tip, radius r, turned by a and squashed by
// sq (seen at an angle); .x distance, .y how far out along its petal (0..1), .z across the petal
// (-1..1), .w distance from the heart / r
fn flower(q0: vec2<f32>, r: f32, a: f32, sq: f32) -> vec4<f32> {
    let cs = vec2<f32>(cos(a), sin(a));
    var q = vec2<f32>(q0.x * cs.x + q0.y * cs.y, -q0.x * cs.y + q0.y * cs.x);
    q.y /= sq;
    let sector = 1.2566371;
    let th = atan2(q.y, q.x);
    let k0 = floor(th / sector + 0.5);
    var best = vec4<f32>(1e3, 0.0, 0.0, length(q) / r);
    // the petal this side of the pixel and its neighbour on the nearer side
    let side = select(-1.0, 1.0, th / sector - k0 > 0.0);
    for (var j = 0; j < 2; j++) {
        let ang = (k0 + f32(j) * side) * sector;
        let ca = vec2<f32>(cos(ang), sin(ang));
        let u = vec2<f32>(q.x * ca.x + q.y * ca.y, -q.x * ca.y + q.y * ca.x);
        let e = (u - vec2<f32>(0.55 * r, 0.0)) / vec2<f32>(0.5 * r, 0.36 * r);
        var dp = (length(e) - 1.0) * 0.36 * r;
        let notch = length(u - vec2<f32>(1.07 * r, 0.0)) - 0.16 * r;
        dp = max(dp, -notch);
        if (dp < best.x) { best = vec4<f32>(dp, clamp(u.x / r, 0.0, 1.0), clamp(u.y / (0.36 * r), -1.0, 1.0), length(q) / r); }
    }
    best.x *= min(sq, 1.0);
    return best;
}
// a blossom's colour: deep pink at the heart to near white at the tips, a faint midrib, the side
// away from the light shaded, the stamens a ring of gold round a crimson eye
fn flower_col(fl: vec4<f32>, shade: f32) -> vec3<f32> {
    var c = mix(vec3<f32>(0.8, 0.27, 0.46), vec3<f32>(0.98, 0.68, 0.8), smoothstep(0.08, 0.5, fl.y));
    c = mix(c, vec3<f32>(1.0, 0.91, 0.95), smoothstep(0.55, 0.95, fl.y));
    c *= 1.0 - 0.1 * smoothstep(0.25, 0.0, abs(fl.z)) * smoothstep(0.15, 0.4, fl.y);
    c *= 0.9 + 0.1 * fl.z * shade;
    // each petal's own edge, so the five read apart
    c *= 0.86 + 0.14 * smoothstep(0.0, -0.7, fl.x);
    c = mix(c, vec3<f32>(0.6, 0.08, 0.22), smoothstep(0.24, 0.1, fl.w));
    c = mix(c, vec3<f32>(1.0, 0.84, 0.42), smoothstep(0.07, 0.0, abs(fl.w - 0.27)) * 0.85);
    return c;
}
// a Japanese black pine on the left shore (seen lying only), leaning out over the water: a crooked,
// tapering trunk with plated bark and flat clouds of needles at its branch ends; .x distance, .yzw
fn pine(p: vec2<f32>) -> vec4<f32> {
    if (p.x > 112.0 || p.y < 110.0 || p.y > 380.0) { return vec4<f32>(1e3, 0.0, 0.0, 0.0); }
    let pw = p + (vec2<f32>(vnoise(p * 0.12), vnoise(p * 0.12 + 5.0)) - 0.5) * 1.4;
    // from the bank at the bottom left, leaning out over the water, the pads in the windows
    var tr = limb(pw, vec2<f32>(22.0, 362.0), vec2<f32>(30.0, 320.0), 5.4, 4.6);
    var segs = array<vec4<f32>, 9>(
        vec4<f32>(30.0, 320.0, 24.0, 286.0), vec4<f32>(24.0, 286.0, 38.0, 250.0), vec4<f32>(38.0, 250.0, 58.0, 222.0), vec4<f32>(58.0, 222.0, 84.0, 210.0),
        vec4<f32>(38.0, 250.0, 14.0, 236.0), vec4<f32>(58.0, 222.0, 50.0, 180.0), vec4<f32>(50.0, 180.0, 60.0, 150.0), vec4<f32>(50.0, 180.0, 22.0, 160.0),
        vec4<f32>(26.0, 300.0, 50.0, 320.0),
    );
    var rads = array<vec2<f32>, 9>(vec2<f32>(4.6, 4.0), vec2<f32>(4.0, 3.3), vec2<f32>(3.3, 2.5), vec2<f32>(2.4, 1.5), vec2<f32>(2.0, 1.1), vec2<f32>(2.4, 1.7), vec2<f32>(1.7, 1.0), vec2<f32>(1.5, 0.9), vec2<f32>(1.6, 0.9));
    for (var i = 0; i < 9; i++) {
        let e = limb(pw, segs[i].xy, segs[i].zw, rads[i].x, rads[i].y);
        if (e.x < tr.x) { tr = e; }
    }
    // the pads: centre, half width, height; each a cloud of needles, domed and lumpy on top, flat
    // underneath, as the gardeners shape a black pine
    var pads = array<vec4<f32>, 6>(
        vec4<f32>(80.0, 206.0, 17.0, 9.5), vec4<f32>(60.0, 146.0, 16.0, 9.0), vec4<f32>(14.0, 232.0, 14.0, 8.0),
        vec4<f32>(22.0, 156.0, 12.0, 7.0), vec4<f32>(52.0, 316.0, 12.0, 6.5), vec4<f32>(46.0, 260.0, 10.0, 6.0),
    );
    var pd = 1e3;
    var lit = 0.0;
    var needle = 0.5;
    var low = 0.0;
    for (var i = 0; i < 6; i++) {
        let pad = pads[i];
        if (abs(p.x - pad.x) > pad.z + 8.0 || abs(p.y - pad.y) > pad.w + 8.0) { continue; }
        var dc = 1e3;
        for (var k = 0; k < 7; k++) {
            let t = f32(k) / 3.0 - 1.0;
            let cc = vec2<f32>(pad.x + t * pad.z * 0.78, pad.y - pad.w * 0.28 * (1.0 - t * t) + 1.6 * (hash(vec2<f32>(f32(k), f32(i) + 0.5)) - 0.5));
            let rc = pad.w * (0.5 + 0.32 * (1.0 - abs(t))) * (0.85 + 0.3 * hash(vec2<f32>(f32(i), f32(k) + 9.0)));
            let dk = length(p - cc) - rc;
            // a smooth union, so the lumps flow into one cloud
            let hk = clamp(0.5 + 0.5 * (dk - dc) / 3.0, 0.0, 1.0);
            dc = mix(dk, dc, hk) - 3.0 * hk * (1.0 - hk);
        }
        // needles fanning up from the pad's root: fine streaks, and a bristly rim
        let root = vec2<f32>(pad.x, pad.y + pad.w * 1.6);
        let fan = atan2(p.x - root.x, root.y - p.y);
        let st = 0.5 + 0.5 * sin(fan * 70.0 + 3.0 * vnoise(p * 0.5 + f32(i)));
        dc += 0.7 * (st - 0.5);
        dc = max(dc, p.y - (pad.y + pad.w * 0.42 + 1.2 * (vnoise(vec2<f32>(p.x * 0.35, f32(i))) - 0.5)));
        if (dc < pd) {
            pd = dc;
            lit = clamp((pad.y + pad.w * 0.4 - p.y) / (pad.w * 1.2), 0.0, 1.0);
            needle = st;
            low = smoothstep(pad.y - pad.w * 0.1, pad.y + pad.w * 0.42, p.y);
        }
    }
    if (pd < 0.8) {
        var g = mix(vec3<f32>(0.03, 0.09, 0.06), vec3<f32>(0.24, 0.4, 0.2), lit * lit);
        g *= 0.8 + 0.35 * needle;
        // glints of light on the needle tips at the top
        g += vec3<f32>(0.12, 0.14, 0.06) * step(0.9, hash(floor(p * 1.3))) * lit;
        g *= 1.0 - 0.5 * low;
        return vec4<f32>(pd, g);
    }
    var bk = mix(vec3<f32>(0.09, 0.07, 0.06), vec3<f32>(0.34, 0.27, 0.22), tr.y);
    bk *= 1.0 - 0.35 * smoothstep(0.55, 0.72, vnoise(vec2<f32>(tr.z * 0.45, pw.x * 0.5 + pw.y * 0.5)));
    return vec4<f32>(tr.x, bk * (0.8 + 0.2 * smoothstep(0.0, -1.2, tr.x)));
}
// a three-storey pagoda on the right hill (seen lying only): a stone plinth, vermilion storeys with
// dark doors and posts, gold brackets under each roof, wide slate roofs with tile ribs and up-turned
// eaves, the spire with its rings; .x distance, .yzw its colour (painted back to front)
fn pagoda(p: vec2<f32>) -> vec4<f32> {
    // on the hill's top (y 176), the whole of it in the row of windows above the horizon
    let q = (p - vec2<f32>(446.0, 178.0)) / 0.78;
    if (abs(q.x) > 28.0 || q.y > 3.0 || q.y < -96.0) { return vec4<f32>(1e3, 0.0, 0.0, 0.0); }
    var dm = 1e3;
    var col = vec3<f32>(0.0);
    let plinth = box(q, vec2<f32>(0.0, -1.5), vec2<f32>(13.0, 2.5));
    if (plinth < 0.5) { col = vec3<f32>(0.42, 0.42, 0.45) * (0.85 + 0.15 * step(0.0, -q.y - 2.5)); }
    dm = min(dm, plinth);
    var bottoms = array<vec3<f32>, 3>(vec3<f32>(-4.0, 14.0, 9.5), vec3<f32>(-26.0, 12.0, 7.8), vec3<f32>(-46.0, 11.0, 6.4));
    for (var i = 0; i < 3; i++) {
        let fi = f32(i);
        let yb = bottoms[i].x;
        let hh = bottoms[i].y;
        let bw = bottoms[i].z;
        let body = box(q, vec2<f32>(0.0, yb - hh * 0.5), vec2<f32>(bw, hh * 0.5));
        if (body < 0.5) {
            var c = vec3<f32>(0.66, 0.17, 0.08);
            // posts at the corners and beside the door, a dark door, a railing round its foot
            c *= 1.0 - 0.3 * smoothstep(0.9, 0.3, abs(abs(q.x) - bw * 0.55));
            let door = box(q, vec2<f32>(0.0, yb - hh * 0.42), vec2<f32>(2.0, hh * 0.32));
            c = mix(c, vec3<f32>(0.08, 0.05, 0.05), smoothstep(0.5, -0.5, door));
            c = mix(c, vec3<f32>(0.78, 0.62, 0.38), smoothstep(0.6, 0.0, abs(q.y - (yb - 2.2))) * 0.8);
            // lit from the left
            c *= 0.8 + 0.25 * smoothstep(bw, -bw, q.x);
            col = c;
        }
        dm = min(dm, body);
        // the gold bracket band under the eaves
        let yr = yb - hh;
        let brk = box(q, vec2<f32>(0.0, yr - 0.8), vec2<f32>(bw + 2.0, 1.0));
        if (brk < 0.5) { col = vec3<f32>(0.62, 0.48, 0.2) * (0.8 + 0.2 * step(0.5, fract(q.x * 0.6))); }
        dm = min(dm, brk);
        // the roof: concave, wide at the eaves, its ends turned up
        let w = 21.0 - fi * 3.4;
        let ye = yr - 1.6;
        let yt = ye - 6.5;
        let t = clamp((q.y - yt) / (ye - yt), 0.0, 1.0);
        let wy = mix(w * 0.4, w, pow(t, 1.6));
        let ybot = ye - 3.4 * pow(clamp(abs(q.x) / w, 0.0, 1.0), 3.0);
        let roof = max(max(abs(q.x) - wy, yt - q.y), q.y - ybot);
        if (roof < 0.5) {
            var c = mix(vec3<f32>(0.1, 0.1, 0.13), vec3<f32>(0.22, 0.22, 0.27), smoothstep(-w, w, -q.x));
            c *= 0.8 + 0.3 * smoothstep(0.35, 0.05, abs(fract(q.x / 2.0) - 0.5));
            // the eave's edge catches the light
            c = mix(c, vec3<f32>(0.42, 0.42, 0.46), smoothstep(1.0, 0.0, ybot - q.y) * 0.6);
            col = c;
        }
        dm = min(dm, roof);
    }
    // the spire: a pole with rings and a jewel on top
    let pole = box(q, vec2<f32>(0.0, -76.0), vec2<f32>(0.8, 13.0));
    if (pole < 0.5) {
        var c = vec3<f32>(0.42, 0.33, 0.16);
        c = mix(c, vec3<f32>(0.66, 0.54, 0.26), step(0.55, fract(-q.y / 2.6)) * step(q.y, -66.0) * step(-84.0, q.y));
        col = c;
    }
    dm = min(dm, pole);
    let jewel = length(q - vec2<f32>(0.0, -90.0)) - 1.8;
    if (jewel < 0.5) { col = vec3<f32>(0.75, 0.6, 0.3); }
    dm = min(dm, jewel);
    // the rings stand out from the pole
    let rings = max(box(q, vec2<f32>(0.0, -75.0), vec2<f32>(2.0, 9.0)), 0.4 - abs(fract(-q.y / 2.6) - 0.5) * 2.0);
    if (rings < 0.5) { col = vec3<f32>(0.55, 0.44, 0.2); }
    dm = min(dm, rings);
    return vec4<f32>(dm * 0.78, col);
}
// a triangle's distance (negative inside)
fn tri(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>, c: vec2<f32>) -> f32 {
    let e0 = b - a;
    let e1 = c - b;
    let e2 = a - c;
    let v0 = p - a;
    let v1 = p - b;
    let v2 = p - c;
    let pq0 = v0 - e0 * clamp(dot(v0, e0) / dot(e0, e0), 0.0, 1.0);
    let pq1 = v1 - e1 * clamp(dot(v1, e1) / dot(e1, e1), 0.0, 1.0);
    let pq2 = v2 - e2 * clamp(dot(v2, e2) / dot(e2, e2), 0.0, 1.0);
    let sg = sign(e0.x * e2.y - e0.y * e2.x);
    let d0 = vec2<f32>(dot(pq0, pq0), sg * (v0.x * e0.y - v0.y * e0.x));
    let d1 = vec2<f32>(dot(pq1, pq1), sg * (v1.x * e1.y - v1.y * e1.x));
    let d2 = vec2<f32>(dot(pq2, pq2), sg * (v2.x * e2.y - v2.y * e2.x));
    let d = min(min(d0, d1), d2);
    return -sqrt(d.x) * sign(d.y);
}
// a sampan with its boatman (seen upright only): a wooden hull of planks, the bow rising to a point,
// an arched roof of reed mats amidships, the man in indigo at the stern with a straw hat, poling;
// .x distance, .yzw colour
fn boat(p: vec2<f32>, c: vec2<f32>) -> vec4<f32> {
    let q = (p - c) / 1.35;
    if (abs(q.x) > 30.0 || q.y < -34.0 || q.y > 26.0) { return vec4<f32>(1e3, 0.0, 0.0, 0.0); }
    var col = vec3<f32>(0.0);
    var dm = 1e3;
    // the pole first: behind the man, into the water ahead of the stern
    let pole = seg(q, vec2<f32>(20.0, -30.0), vec2<f32>(2.0, 22.0)).x - 0.55;
    if (pole < 0.4) { col = vec3<f32>(0.58, 0.5, 0.3); }
    dm = min(dm, pole);
    // the hull: the gunwale rising to the bow (left) and a little at the stern, a rounded keel
    let u = q.x / 24.0;
    let top = -3.0 - 7.0 * pow(clamp(-u - 0.45, 0.0, 1.0) / 0.55, 2.0) - 1.5 * pow(clamp(u - 0.6, 0.0, 1.0) / 0.4, 2.0);
    let bot = 3.5 - 4.5 * u * u - 5.0 * pow(clamp(-u - 0.55, 0.0, 1.0) / 0.45, 1.5);
    let hull = max(max(top - q.y, q.y - bot), abs(q.x) - 24.0 - 3.0 * clamp(-u, 0.0, 1.0));
    if (hull < 0.4) {
        var hc = vec3<f32>(0.34, 0.21, 0.12);
        // planks along it, the gunwale's rail lit
        hc *= 0.82 + 0.18 * step(0.5, fract((q.y - top) / 2.2));
        hc = mix(hc, vec3<f32>(0.55, 0.38, 0.22), smoothstep(1.2, 0.0, q.y - top));
        hc *= 0.85 + 0.15 * smoothstep(20.0, -20.0, q.x);
        col = hc;
    }
    dm = min(dm, hull);
    // the arched mat roof amidships
    let rq = q - vec2<f32>(-3.0, -3.0);
    let arch = max(length(rq / vec2<f32>(9.0, 9.5)) - 1.0, rq.y) * 9.0;
    let open = length(rq / vec2<f32>(6.0, 6.5)) - 1.0;
    let roof = max(arch, -max(open * 6.0, -rq.x - 3.0));
    if (roof < 0.4) {
        var mc = vec3<f32>(0.6, 0.46, 0.24);
        mc *= 0.8 + 0.2 * step(0.5, fract(atan2(rq.y, rq.x) * 6.0));
        mc *= 0.75 + 0.35 * smoothstep(9.0, -9.0, rq.x + rq.y);
        col = mc;
    }
    dm = min(dm, roof);
    let inside = max(open * 6.0, rq.y);
    if (inside < 0.4 && roof > 0.0) { col = vec3<f32>(0.08, 0.06, 0.05); dm = min(dm, inside); }
    // the boatman at the stern: a robe, arms on the pole, a head under a wide conical hat
    let mq = q - vec2<f32>(15.0, 0.0);
    let robe = max(box(mq, vec2<f32>(0.0, -10.0), vec2<f32>(3.2 - 0.06 * (-mq.y - 3.0), 7.0)), mq.y + 3.0);
    let arm = seg(mq, vec2<f32>(-1.0, -14.0), vec2<f32>(3.6, -18.0)).x - 1.1;
    if (min(robe, arm) < 0.4) {
        col = vec3<f32>(0.14, 0.18, 0.34) * (0.75 + 0.35 * smoothstep(3.0, -3.0, mq.x)) * (0.9 + 0.1 * step(0.5, fract(mq.y * 0.4)));
    }
    dm = min(dm, min(robe, arm));
    let face = length(mq - vec2<f32>(0.2, -19.2)) - 1.9;
    if (face < 0.4) { col = vec3<f32>(0.72, 0.52, 0.38); }
    dm = min(dm, face);
    let hq = mq - vec2<f32>(0.2, -21.0);
    let hat = max(abs(hq.x) * 0.42 + hq.y + 0.2, -hq.y - 3.6) * 0.9;
    if (hat < 0.4) {
        var hc = vec3<f32>(0.72, 0.6, 0.34) * (0.7 + 0.35 * smoothstep(5.0, -5.0, hq.x));
        hc *= 0.9 + 0.1 * step(0.5, fract(atan2(hq.y + 3.6, hq.x) * 5.0));
        hc = mix(hc, vec3<f32>(0.3, 0.22, 0.12), smoothstep(0.6, 0.0, abs(hq.y + 0.4)) * 0.7);
        col = hc;
    }
    dm = min(dm, hat);
    return vec4<f32>(dm * 1.35, col);
}
// reeds standing in the water from the line `base`, between x0 and x1: tapering, arching blades in
// two greens, a few feathery plumes; .x distance, .yzw colour
fn reeds(p: vec2<f32>, x0: f32, x1: f32, base: f32, tm: f32) -> vec4<f32> {
    if (p.x < x0 - 16.0 || p.x > x1 + 16.0 || p.y > base + 2.0 || p.y < base - 84.0) { return vec4<f32>(1e3, 0.0, 0.0, 0.0); }
    var best = vec4<f32>(1e3, 0.0, 0.0, 0.0);
    for (var i = 0; i < 12; i++) {
        let fi = f32(i);
        let x = mix(x0, x1, (fi + 0.6 * hash(vec2<f32>(fi, x0))) / 12.0);
        let h = 38.0 + 40.0 * hash(vec2<f32>(fi, x1));
        let bend = (hash(vec2<f32>(x0, fi)) - 0.5) * 22.0 + 2.5 * sin(tm * 0.7 + fi * 1.3);
        let t = clamp((base - p.y) / h, 0.0, 1.0);
        let bx = x + bend * t * t;
        let w = 1.7 * (1.0 - t) + 0.25;
        let across = (p.x - bx) / w;
        let dd = max(abs(p.x - bx) - w, max(p.y - base, base - h - p.y));
        if (dd < best.x) {
            var g = mix(vec3<f32>(0.1, 0.2, 0.08), vec3<f32>(0.3, 0.42, 0.14), clamp(0.5 - across * 0.5, 0.0, 1.0));
            g = mix(g, vec3<f32>(0.42, 0.38, 0.2), hash(vec2<f32>(fi, 7.0)) * 0.4 * t);
            best = vec4<f32>(dd, g);
        }
        // a plume on some: soft, tan, leaning with the blade
        if (hash(vec2<f32>(fi, 9.0)) > 0.55) {
            let pc = vec2<f32>(x + bend, base - h);
            let pq = p - pc - vec2<f32>(bend * 0.08, -5.0);
            let pl = length(pq / vec2<f32>(2.4, 6.0)) - 1.0 + 0.25 * (vnoise(p * 1.1) - 0.5);
            if (pl * 2.4 < best.x) { best = vec4<f32>(pl * 2.4, vec3<f32>(0.66, 0.56, 0.42) * (0.8 + 0.3 * vnoise(p * 0.8))); }
        }
    }
    return best;
}
fn torii_col(t: vec2<f32>, p: vec2<f32>) -> vec3<f32> {
    let red = mix(vec3<f32>(0.86, 0.22, 0.08), vec3<f32>(0.62, 0.12, 0.05), smoothstep(TORII_X - 9.0, TORII_X + 21.0, p.x));
    return mix(red, vec3<f32>(0.08, 0.06, 0.06), t.y);
}
// the red banner on the left with 日本: .x distance to the cloth, the cloth's colour in .yzw
fn banner(p: vec2<f32>, tm: f32) -> vec4<f32> {
    // drawn at its old height, 104 px down the square
    let sway = 1.0 * sin(tm * 0.8) * clamp((p.y - 110.0) / 80.0, 0.0, 1.0);
    let q = p - vec2<f32>(sway, BANNER_Y);
    // it ends above the left key's label (it hid the start of "Lubię to")
    let cloth = box(q, vec2<f32>(BANNER_X, 124.0), vec2<f32>(9.5, 26.0));
    if (cloth > 1.0) { return vec4<f32>(cloth, 0.0, 0.0, 0.0); }
    var col = vec3<f32>(0.74, 0.08, 0.07) * (0.92 + 0.12 * vnoise(vec2<f32>(q.x * 0.3, q.y * 0.08)));
    // folds of the cloth
    col *= 0.9 + 0.1 * sin(q.x * 0.6 + q.y * 0.05);
    // 日: a box with a middle bar
    let a = abs(box(q, vec2<f32>(BANNER_X, 111.0), vec2<f32>(4.6, 6.6))) - 1.0;
    let a2 = seg(q, vec2<f32>(BANNER_X - 4.2, 111.0), vec2<f32>(BANNER_X + 4.2, 111.0)).x - 0.9;
    // 本: a long bar, a stem, two sweeps, a short bar across the stem
    var b = seg(q, vec2<f32>(BANNER_X - 6.5, 127.0), vec2<f32>(BANNER_X + 6.5, 127.0)).x - 1.0;
    b = min(b, seg(q, vec2<f32>(BANNER_X, 121.0), vec2<f32>(BANNER_X, 145.0)).x - 1.0);
    b = min(b, seg(q, vec2<f32>(BANNER_X - 0.4, 128.0), vec2<f32>(BANNER_X - 7.0, 138.0)).x - 0.9);
    b = min(b, seg(q, vec2<f32>(BANNER_X + 0.4, 128.0), vec2<f32>(BANNER_X + 7.0, 138.0)).x - 0.9);
    b = min(b, seg(q, vec2<f32>(BANNER_X - 3.5, 138.0), vec2<f32>(BANNER_X + 3.5, 138.0)).x - 0.85);
    let ink = min(min(a, a2), b);
    col = mix(col, vec3<f32>(0.05, 0.03, 0.03), smoothstep(0.6, -0.6, ink));
    return vec4<f32>(cloth, col);
}
fn rot(v: vec2<f32>, a: f32) -> vec2<f32> {
    let c = cos(a);
    let s = sin(a);
    return vec2<f32>(v.x * c + v.y * s, -v.x * s + v.y * c);
}
// a gust of wind now and then (about every 23 s, a few seconds long): 0 calm .. 1
fn gust_at(t: f32) -> f32 {
    return pow(max(sin(t * 0.27 + 1.3), 0.0), 8.0);
}
// a cherry petal at q (its own space, the tip up), radius r, narrowed by `narrow` (seen edge-on as it
// turns), edge soft by aa: .x coverage, .y 0 at the base .. 1 at the tip
fn petal_cov(q: vec2<f32>, r: f32, narrow: f32, aa: f32) -> vec2<f32> {
    let u = vec2<f32>(q.x / max(narrow, 0.2), q.y);
    let t = clamp((r - u.y) / (2.0 * r), 0.0, 1.0);
    let wd = r * 0.82 * pow(t, 0.5) * (1.0 - 0.12 * t) + 0.35;
    var d = max(abs(u.x) - wd, abs(u.y) - r);
    d = max(d, -(length(u - vec2<f32>(0.0, -r * 1.06)) - r * 0.24));
    return vec2<f32>(smoothstep(aa, -aa, d * max(narrow, 0.2)), t);
}
fn petal_col(t: f32, back: bool) -> vec3<f32> {
    var c = mix(vec3<f32>(0.93, 0.56, 0.68), vec3<f32>(1.0, 0.92, 0.95), smoothstep(0.1, 0.9, t));
    if (back) { c *= vec3<f32>(0.88, 0.84, 0.88); }
    return c;
}
// Each petal's state this frame, the same for every pixel: worked out once per 16x16 group of pixels
// into `PET` (one thread a petal), then every pixel only draws (the per-pixel version cost 19 of
// 41 ms a frame on the software renderer). a: x, y, its turn, its flip (in the air) or its time on
// the water; b: radius, depth 0 far .. 1 near, its fade in (air) or sinking (water), 0 none / 1 air /
// 2 on the water.
const NPET: u32 = 40u;
var<workgroup> PET: array<vec4<f32>, 80>;
// the petals that reach this group's 16x16 pixels, so a pixel looks at those only
var<workgroup> NEAR: array<u32, 40>;
var<workgroup> NNEAR: atomic<u32>;
fn petal_state(i: i32, t: f32, gust: f32) {
    let fi = f32(i);
    var life = 0.0;
    var o = vec2<f32>(0.0);
    var seed = 0.0;
    var kick = vec2<f32>(0.0);
    var shaken = false;
    var none = false;
    if (i < 24) {
        let period = 16.0 + 10.0 * hash(vec2<f32>(fi, 1.7));
        let cyc = (t + hash(vec2<f32>(fi, 3.3)) * period) / period;
        let n = floor(cyc);
        life = fract(cyc) * period;
        seed = fi * 7.31 + n * 1.97;
        // off a blossom along one of the two trees' arms
        let s = hash(vec2<f32>(seed, 1.0));
        let left = hash(vec2<f32>(seed, 2.0)) < 0.5;
        o = select(vec2<f32>(mix(460.0, 276.0, s), 54.0 + 40.0 * s), vec2<f32>(mix(20.0, 204.0, s), 54.0 + 40.0 * s), left) + (vec2<f32>(hash(vec2<f32>(seed, 3.0)), hash(vec2<f32>(seed, 4.0))) - 0.5) * vec2<f32>(16.0, 22.0);
    } else {
        // a press: a handful off the branch above the key, thrown out and falling (the two latest
        // presses, eight petals each)
        let k = (i - 24) / 8;
        let b = f.aux[k];
        life = t - b.z;
        none = b.w < 0.5 || life < 0.0 || life > 12.0;
        seed = fi * 3.7 + b.z * 13.1;
        let kc = world(b.xy);
        // off the nearer tree's blossoms above the key (none hang over the sun)
        let bx = select(max(kc.x, 282.0), min(kc.x, 198.0), kc.x < 240.0);
        o = vec2<f32>(bx + (hash(vec2<f32>(seed, 5.0)) - 0.5) * 40.0, 70.0 + 40.0 * hash(vec2<f32>(seed, 6.0)));
        kick = (vec2<f32>(hash(vec2<f32>(seed, 7.0)), hash(vec2<f32>(seed, 8.0))) - vec2<f32>(0.5, 0.8)) * vec2<f32>(70.0, 30.0);
        shaken = true;
    }
    let z = hash(vec2<f32>(seed, 9.0));
    let vy = mix(12.0, 24.0, z) * (0.85 + 0.3 * hash(vec2<f32>(seed, 10.0)));
    let drift = (hash(vec2<f32>(seed, 11.0)) - 0.4) * 8.0;
    let swing = mix(6.0, 14.0, z);
    let w = 1.1 + 0.8 * hash(vec2<f32>(seed, 12.0));
    let ph = hash(vec2<f32>(seed, 13.0)) * 6.28;
    // its landing: on the lake, lower (nearer) for a nearer petal
    let yland = SHORE + 8.0 + z * z * 200.0;
    let tland = (yland - o.y) / vy;
    let kicked = kick * (1.0 - exp(-min(life, tland) * 1.5)) / 1.5;
    let r = mix(2.4, 5.8, z);
    var a = vec4<f32>(0.0);
    var bb = vec4<f32>(0.0);
    if (!none && life < tland) {
        // in the air: a pendulum swing across, slower at the ends of each swing, turning over
        let x = o.x + drift * life + swing * sin(w * life + ph) + kicked.x + gust * mix(20.0, 45.0, z);
        let y = o.y + vy * life - 3.0 * abs(cos(w * life + ph)) + kicked.y;
        let ang = 0.7 * cos(w * life + ph) + gust * 0.8 + hash(vec2<f32>(seed, 14.0)) * 6.28;
        let flip = cos(life * (1.4 + 1.6 * hash(vec2<f32>(seed, 15.0))) + ph);
        a = vec4<f32>(x, y, ang, flip);
        bb = vec4<f32>(r, z, smoothstep(0.0, 0.4, life), 1.0);
    } else if (!none) {
        // on the water: drifting, a small ring at first, then sinking
        let fl = life - tland;
        let stay = select(9.0 + 6.0 * hash(vec2<f32>(seed, 16.0)), 3.0, shaken);
        if (fl <= stay) {
            let xl = o.x + drift * tland + swing * sin(w * tland + ph) + kicked.x + 3.0 * fl * (hash(vec2<f32>(seed, 17.0)) - 0.5);
            a = vec4<f32>(xl, yland + 0.6 * sin(fl * 1.3 + ph), hash(vec2<f32>(seed, 18.0)) * 6.28 + fl * 0.1, fl);
            bb = vec4<f32>(r, z, smoothstep(stay - 2.0, stay, fl), 2.0);
        }
    }
    PET[2 * i] = a;
    PET[2 * i + 1] = bb;
}
// whether petal state (a, bb) reaches the square's box lo..hi
fn petal_reaches(a: vec4<f32>, bb: vec4<f32>, lo: vec2<f32>, hi: vec2<f32>) -> bool {
    if (bb.w < 0.5) { return false; }
    let m = select(vec2<f32>(bb.x + 3.0), vec2<f32>(bb.x * 2.6 + 12.0, 8.0), bb.w > 1.5);
    return a.x + m.x >= lo.x && a.x - m.x <= hi.x && a.y + m.y >= lo.y && a.y - m.y <= hi.y;
}
// Petals coming off the blossoms on both trees (owner, 2026-10-29: "płatki odrywają się od kwiatów",
// "spadają jak prawdziwy płatek", "podmuch wiatru", "pływają po jeziorze", "wciśnięcie strząsa
// garść płatków"): each falls swinging and turning over, the nearer ones bigger and softer, leans with
// the gusts, lands on the lake at its own depth with a small ring and floats there a while, then
// sinks; a press shakes a handful off the branch above the key. Drawn last, the floating ones too
// (one pass over the petals).
fn petals(p: vec2<f32>, base: vec3<f32>) -> vec3<f32> {
    var col = base;
    // one pass for both: nothing floats above the shore
    let water = p.y > SHORE - 6.0;
    let n = atomicLoad(&NNEAR);
    for (var j = 0u; j < n; j++) {
        let i = NEAR[j];
        let a = PET[2u * i];
        let bb = PET[2u * i + 1u];
        let r = bb.x;
        let z = bb.y;
        let floating = bb.w > 1.5;
        if (bb.w < 0.5 || (floating && !water)) { continue; }
        if (!floating) {
            if (abs(p.x - a.x) > r + 3.0 || abs(p.y - a.y) > r + 3.0) { continue; }
            let q = rot(p - a.xy, a.z);
            let pc = petal_cov(q, r, 0.3 + 0.7 * abs(a.w), mix(0.6, 1.5, z * z));
            col = mix(col, petal_col(pc.y, a.w < 0.0), pc.x * mix(0.6, 0.95, z) * bb.z);
        } else {
            if (abs(p.x - a.x) > r * 2.6 + 12.0 || abs(p.y - a.y) > 8.0) { continue; }
            let fl = a.w;
            let sink = bb.z;
            if (fl < 2.5) {
                let rq = (p - a.xy) / vec2<f32>(1.0, 0.32);
                let rr = abs(length(rq) - fl * 7.0);
                col += vec3<f32>(0.4, 0.5, 0.6) * smoothstep(1.1, 0.0, rr) * (1.0 - fl / 2.5) * 0.4 * mix(0.6, 1.0, z);
            }
            let q = rot((p - a.xy) / vec2<f32>(1.0, 0.45), a.z);
            let pc = petal_cov(q, r, 1.0, 0.7);
            col = mix(col, mix(petal_col(pc.y, false), col, sink * 0.7), pc.x * 0.9 * (1.0 - sink * 0.7));
        }
    }
    return col;
}
@compute @workgroup_size(16, 16)
fn under(@builtin(global_invocation_id) id: vec3<u32>, @builtin(local_invocation_index) li: u32, @builtin(workgroup_id) wg: vec3<u32>) {
    // the petals' state, once for the group, before any pixel returns; then the list of those that
    // reach the group's pixels
    if (li == 0u) { atomicStore(&NNEAR, 0u); }
    if (li < NPET) { petal_state(i32(li), f.time * f.speed, gust_at(f.time * f.speed)); }
    workgroupBarrier();
    if (li < NPET) {
        let c0 = world(vec2<f32>(wg.xy * 16u));
        let c1 = world(vec2<f32>(wg.xy * 16u + 16u));
        if (petal_reaches(PET[2u * li], PET[2u * li + 1u], min(c0, c1), max(c0, c1))) {
            NEAR[atomicAdd(&NNEAR, 1u)] = li;
        }
    }
    workgroupBarrier();
    if (id.x >= f.size.x || id.y >= f.size.y) { return; }
    let p = world(vec2<f32>(f32(id.x), f32(id.y)) + 0.5);
    var c = px(i32(id.x), i32(id.y)).rgb;
    let tm = f.time * f.speed;
    var col: vec3<f32>;
    if (p.y < SHORE) {
        col = far(p, tm);
    } else {
        // the lake: the far scene mirrored, broken by slow ripples, deeper blue towards you
        let depth = clamp((p.y - SHORE) / 120.0, 0.0, 1.0);
        let wob = 1.2 * sin(p.y * 0.32 + tm * 1.1) * (0.3 + depth) + 1.2 * (vnoise(vec2<f32>(p.x * 0.05, p.y * 0.5 + tm * 0.3)) - 0.5);
        let m = vec2<f32>(p.x + wob, 2.0 * SHORE - p.y);
        col = far(m, tm) * vec3<f32>(0.72, 0.8, 0.92);
        col = mix(col, vec3<f32>(0.1, 0.24, 0.44), 0.25 + 0.4 * depth);
        // bright streaks along the ripples
        let streak = smoothstep(0.72, 0.9, vnoise(vec2<f32>(p.x * 0.04 + tm * 0.05, p.y * 1.3)));
        col += vec3<f32>(0.6, 0.7, 0.8) * streak * 0.12;
    }
    // the near bank at the bottom left (seen lying only): dark earth and rocks, grass on its top, the
    // pine standing on it
    let bank = 352.0 - 0.0045 * (p.x - 4.0) * (p.x - 4.0) + 3.0 * (vnoise(vec2<f32>(p.x * 0.12, 3.0)) - 0.5);
    if (p.x < 96.0 && p.y > bank - 1.0) {
        var bc2 = mix(vec3<f32>(0.2, 0.17, 0.13), vec3<f32>(0.08, 0.07, 0.06), smoothstep(bank, bank + 20.0, p.y));
        let rock = vnoise(p * vec2<f32>(0.12, 0.2));
        bc2 = mix(bc2, vec3<f32>(0.34, 0.33, 0.32) * (0.7 + 0.4 * rock), smoothstep(0.55, 0.62, rock) * 0.8);
        bc2 = mix(bc2, vec3<f32>(0.2, 0.32, 0.12), smoothstep(bank + 3.5, bank, p.y) * 0.9);
        col = mix(col, bc2, smoothstep(bank - 0.8, bank + 0.8, p.y));
    }
    let pn = pine(p);
    if (pn.x < 1.0) {
        col = mix(col, pn.yzw, smoothstep(0.8, -0.8, pn.x));
    }
    // the torii in the water, and its reflection below its feet
    let t = torii(p);
    if (t.x < 1.0 && p.y < TORII_FOOT) {
        col = mix(col, torii_col(t, p), smoothstep(0.8, -0.8, t.x));
    }
    if (p.y >= TORII_FOOT) {
        let wob = 0.9 * sin(p.y * 0.32 + f.time * 1.1) + 0.5 * (vnoise(vec2<f32>(p.x * 0.05, p.y * 0.25 + f.time * 0.3)) - 0.5);
        let mp = vec2<f32>(p.x + wob, 2.0 * TORII_FOOT - p.y);
        let tr = torii(mp);
        if (tr.x < 1.0) {
            col = mix(col, torii_col(tr, mp) * vec3<f32>(0.7, 0.72, 0.85), smoothstep(0.8, -0.8, tr.x) * 0.75);
        }
    }
    // the top, seen upright only: a far skein of birds crossing the sky slowly, small dark silhouettes
    // in a V as in a woodblock print (owner, 2026-10-29: the cranes "wyglądają strasznie słabo", "1 a")
    if (p.y < 100.0) {
        let lead = vec2<f32>(400.0 - fract(tm * 0.011 + 0.35) * 380.0, 46.0 + 5.0 * sin(tm * 0.21));
        if (abs(p.x - lead.x) < 80.0) {
            var bd = 1e3;
            for (var k = 0; k < 7; k++) {
                let fk = f32(k);
                let side = select(-1.0, 1.0, k % 2 == 1);
                let rank = floor((fk + 1.0) * 0.5);
                let bc = lead + vec2<f32>(rank * 11.0, side * rank * 6.5) + vec2<f32>(sin(tm * 0.7 + fk) * 1.2, cos(tm * 0.5 + fk * 2.0) * 0.9);
                let sz = 4.6 - 0.25 * rank;
                let flap = sin(tm * 3.2 + fk * 1.3);
                let q = (p - bc) / sz;
                // a body and two bowed wings, their tips rising and falling
                let lw = min(seg(q, vec2<f32>(0.0, 0.0), vec2<f32>(-0.45, -0.32 - 0.25 * flap)).x, seg(q, vec2<f32>(-0.45, -0.32 - 0.25 * flap), vec2<f32>(-0.95, -0.12 - 0.55 * flap)).x);
                let rw = min(seg(q, vec2<f32>(0.0, 0.0), vec2<f32>(0.45, -0.32 - 0.25 * flap)).x, seg(q, vec2<f32>(0.45, -0.32 - 0.25 * flap), vec2<f32>(0.95, -0.12 - 0.55 * flap)).x);
                let body = length(q / vec2<f32>(0.22, 0.14)) - 1.0;
                let taper = mix(0.13, 0.05, clamp(abs(q.x), 0.0, 1.0));
                bd = min(bd, min(min(lw, rw) - taper, body * 0.14) * sz);
            }
            col = mix(col, vec3<f32>(0.13, 0.15, 0.22), smoothstep(0.55, -0.45, bd) * 0.9);
        }
    }
    // the bottom, seen upright only: a boatman poling across, his reflection under him
    if (p.y > 376.0) {
        let bc = vec2<f32>(236.0 + 8.0 * sin(tm * 0.07), 452.0);
        let bt = boat(p, bc);
        if (bt.x < 1.0) {
            col = mix(col, bt.yzw, smoothstep(0.6, -0.6, bt.x));
        }
        let wob = 1.2 * sin(p.y * 0.5 + tm * 1.3);
        let mr = boat(vec2<f32>(p.x + wob, 2.0 * (bc.y + 5.0) - p.y), bc);
        if (mr.x < 1.0 && p.y > bc.y + 5.0) { col = mix(col, mix(col, mr.yzw, 0.35) * 0.55, smoothstep(0.7, -0.7, mr.x) * 0.8); }
        // a ring of ripples round the hull
        let rr = length((p - bc - vec2<f32>(0.0, 6.0)) / vec2<f32>(1.0, 0.25));
        col += vec3<f32>(0.5, 0.6, 0.7) * smoothstep(1.2, 0.0, abs(rr - 36.0 - 6.0 * fract(tm * 0.2))) * 0.12 * smoothstep(60.0, 30.0, rr);
    }
    // reeds: in front at the bottom (upright), in the water at both sides (lying)
    var rd = reeds(p, 108.0, 152.0, 482.0, tm);
    let rd2 = reeds(p, 328.0, 372.0, 486.0, tm);
    if (rd2.x < rd.x) { rd = rd2; }
    let rd3 = reeds(p, 56.0, 78.0, 380.0, tm);
    if (rd3.x < rd.x) { rd = rd3; }
    let rd4 = reeds(p, 408.0, 472.0, 382.0, tm);
    if (rd4.x < rd.x) { rd = rd4; }
    if (rd.x < 1.0) {
        col = mix(col, rd.yzw, smoothstep(0.6, -0.6, rd.x));
    }
    // the banner with 日本 and its string
    let bn = banner(p, tm);
    if (bn.x < 1.0) {
        col = mix(col, bn.yzw, smoothstep(0.8, -0.8, bn.x));
    }
    let string = seg(p, vec2<f32>(BANNER_X - 16.0, 97.0 + BANNER_Y), vec2<f32>(BANNER_X + 20.0, 100.0 + BANNER_Y)).x - 0.5;
    col = mix(col, vec3<f32>(0.25, 0.2, 0.15), smoothstep(0.7, -0.7, string));
    // the cherry branches and their blossom, in front of everything
    // the branches stay above the sun, at the top of the row it stands in
    // the branches sway a little, more in a gust of wind
    let gust = gust_at(tm);
    let pb = p - vec2<f32>(gust * 2.6 * sin(tm * 2.3 + p.y * 0.04) + 0.35 * sin(tm * 1.1 + p.x * 0.05), 0.4 * gust * sin(tm * 3.1 + p.x * 0.03));
    if (p.y < 190.0 && (p.x < 214.0 || p.x > 266.0)) {
        let nb = branches_near(pb);
        let bd = nb.best;
        // the blossom's soft shadow on the limbs and the sky behind
        let cell = 7.0;
        let g = floor(pb / cell);
        var best = vec4<f32>(1e3, 0.0, 0.0, 0.0);
        var shadow = 1e3;
        var bud = false;
        var shade = 1.0;
        for (var j = -1; j <= 1; j++) {
            for (var i = -1; i <= 1; i++) {
                // no flower reaches this far from a limb
                if (nb.n == 0 || nb.best.x > 12.0) { break; }
                let gc = g + vec2<f32>(f32(i), f32(j));
                let h = hash(gc * 1.31 + 7.0);
                let ctr = (gc + vec2<f32>(0.15 + 0.7 * hash(gc + 1.7), 0.15 + 0.7 * hash(gc + 3.1))) * cell;
                let near = near_limbs(ctr, nb);
                if (near > 6.5 || h < 0.1) { continue; }
                // this cell's flower (or its shadow) cannot reach the pixel
                if (length(pb - ctr) > 9.5) { continue; }
                let a = h * 6.28 + hash(gc + 2.2) * 3.0;
                if (h < 0.24) {
                    // a bud: a deep pink oval on a short stalk, its tip lighter
                    let ca = vec2<f32>(cos(a), sin(a));
                    let bq = pb - ctr;
                    let lq = vec2<f32>(bq.x * ca.x + bq.y * ca.y, -bq.x * ca.y + bq.y * ca.x);
                    let bd2 = (length(lq / vec2<f32>(1.5, 2.4)) - 1.0) * 1.5;
                    if (bd2 < best.x) { best = vec4<f32>(bd2, clamp(-lq.y / 2.4 * 0.5 + 0.5, 0.0, 1.0), 0.0, 0.0); bud = true; }
                    continue;
                }
                let r = 3.6 + 2.0 * hash(gc + 5.3);
                let sq = 0.68 + 0.32 * hash(gc + 6.1);
                let fl = flower(pb - ctr, r, a, sq);
                if (fl.x < best.x) { best = fl; bud = false; shade = select(-1.0, 1.0, hash(gc + 8.0) > 0.5); }
                // its shadow: soft, so a disc does
                shadow = min(shadow, length(pb - ctr - vec2<f32>(1.2, 1.8)) - r * 0.85);
            }
        }
        col *= 1.0 - 0.3 * smoothstep(1.2, -1.0, shadow);
        col = mix(col, bark(bd, pb), smoothstep(0.7, -0.7, bd.x));
        var bc: vec3<f32>;
        if (bud) {
            bc = mix(vec3<f32>(0.5, 0.06, 0.16), vec3<f32>(0.92, 0.42, 0.58), best.y);
        } else {
            bc = flower_col(best, shade);
        }
        col = mix(col, bc, smoothstep(0.6, -0.6, best.x));
    }
    // the falling petals, in front of everything
    col = petals(p, col);
    // dark by design: rich but not bright, a soft knee near 40 %
    let lum = dot(col, vec3<f32>(0.299, 0.587, 0.114));
    if (lum > 0.38) { col *= (0.38 + (lum - 0.38) * 0.32) / lum; }
    col = clamp(col, vec3<f32>(0.0), vec3<f32>(1.0));
    c = mix(c, col, f.strength / 0.6 * 0.97 * (1.0 - keep_bg(c)));
    put(id.xy, c);
}
