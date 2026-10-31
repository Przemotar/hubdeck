// Poland's golden autumn behind the icons, one painted view through the fifteen windows, laid out by
// the rows of keys: the top row a deepening violet-amber sky with thin clouds lit from below, the
// middle row a dense autumn forest against the low sun (birch gold, maple red, oak rust, spruce dark
// green; every crown a few clumps of leaves, its edges towards the sun glowing), the bottom row the
// trunks with beams of light between them and a meadow with their long shadows. A white birch on
// the left frames it. Kept mid-dark behind the icons so they stay readable. Over the icons, a warm
// light on the side facing the sun.
const SUN = vec2<f32>(150.0, 118.0);
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
fn smin(a: f32, b: f32, k: f32) -> f32 {
    let h = clamp(0.5 + 0.5 * (b - a) / k, 0.0, 1.0);
    return mix(b, a, h) - k * h * (1.0 - h);
}
const CLUMPS = array<vec3<f32>, 5>(
    vec3<f32>(-0.42, 0.12, 0.58),
    vec3<f32>(0.4, 0.06, 0.6),
    vec3<f32>(0.0, -0.36, 0.62),
    vec3<f32>(0.06, 0.24, 0.66),
    vec3<f32>(-0.22, -0.18, 0.5),
);
// a deciduous crown round c (radius r) made of clumps of leaves, each with a ragged edge, swaying a
// little at the top; .x the distance (px, negative inside), .y the leaves' texture 0..1
fn crown(p: vec2<f32>, c: vec2<f32>, r: f32, seed: f32) -> vec2<f32> {
    let lift = clamp((c.y + r - p.y) / (2.0 * r), 0.0, 1.0);
    let sway = vec2<f32>(sin(f.time * 0.55 + seed * 3.0), 0.0) * 1.2 * lift;
    let q = (p - c - sway) / r;
    var d = 1e3;
    for (var i = 0; i < 5; i++) {
        let cl = CLUMPS[i];
        let o = cl.xy + vec2<f32>(hash(vec2<f32>(seed, f32(i))) - 0.5, hash(vec2<f32>(f32(i), seed)) - 0.5) * 0.18;
        d = smin(d, length(q - o) - cl.z, 0.18);
    }
    let tex = fbm(q * 4.0 + seed * 7.3);
    // the edge from the two low octaves only: the high ones are finer than a pixel on the big
    // crowns and left a ragged, stair-stepped outline
    let edge = 0.66 * vnoise(q * 4.0 + seed * 7.3) + 0.34 * vnoise(q * 8.0 + seed * 7.3 * 2.03 + vec2<f32>(17.1, 9.3));
    d += 0.24 * (edge - 0.5) + 0.05 * (vnoise(q * 6.0 + seed) - 0.5);
    return vec2<f32>(d * r, tex);
}
fn spruce(p: vec2<f32>, base: vec2<f32>, h: f32, seed: f32) -> f32 {
    let q = p - base;
    let t = clamp(-q.y / h, 0.0, 1.0);
    let tier = fract(t * 7.0 + seed);
    let w = h * 0.24 * (1.0 - t) * (0.8 + 0.2 * smoothstep(0.0, 0.35, tier) * (1.0 - 0.5 * tier)) + 0.9 * (vnoise(vec2<f32>(p.x * 0.3, p.y * 0.45) + seed) - 0.5);
    return max(abs(q.x) - w, max(q.y, -q.y - h));
}
struct Tree {
    c: vec2<f32>,
    r: f32,
    kind: i32,
    row: i32,
}
// the forest in three rows, the back row first in the list; kind 0 birch, 1 maple, 2 oak, 3 spruce
fn tree_at(i: i32) -> Tree {
    let fi = f32(i);
    var row = 0;
    var n = fi;
    var count = 16.0;
    if (i >= 16) { row = 1; n = fi - 16.0; count = 13.0; }
    if (i >= 29) { row = 2; n = fi - 29.0; count = 9.0; }
    let span = 520.0 / count;
    let x = (n + 0.25 + 0.5 * hash(vec2<f32>(fi, 3.0))) * span - 20.0;
    let r = mix(20.0, 34.0, f32(row) / 2.0) * (0.8 + 0.4 * hash(vec2<f32>(fi, 7.0)));
    let y = mix(140.0, 178.0, f32(row) / 2.0) + 10.0 * hash(vec2<f32>(fi, 5.0));
    let s = hash(vec2<f32>(fi, 9.0));
    var kind = 1;
    if (s < 0.3) { kind = 0; } else if (s < 0.58) { kind = 1; } else if (s < 0.86) { kind = 2; } else { kind = 3; }
    return Tree(vec2<f32>(x, y), r, kind, row);
}
fn leaf_colour(kind: i32, tex: f32, seed: f32) -> vec3<f32> {
    let v = hash(vec2<f32>(seed, 13.0));
    switch kind {
        case 0: { return mix(vec3<f32>(0.95, 0.66, 0.08), vec3<f32>(1.0, 0.84, 0.3), tex); }
        case 1: { return mix(mix(vec3<f32>(0.72, 0.06, 0.04), vec3<f32>(0.95, 0.28, 0.04), v), vec3<f32>(1.0, 0.5, 0.08), tex * 0.5); }
        case 2: { return mix(vec3<f32>(0.66, 0.3, 0.06), vec3<f32>(0.92, 0.55, 0.12), tex); }
        default: { return mix(vec3<f32>(0.04, 0.14, 0.09), vec3<f32>(0.1, 0.26, 0.13), tex); }
    }
}
@compute @workgroup_size(16, 16)
fn under(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= f.size.x || id.y >= f.size.y) { return; }
    let p = vec2<f32>(f32(id.x), f32(id.y)) + 0.5;
    var c = px(i32(id.x), i32(id.y)).rgb;
    let to_sun = p - SUN;
    let ds = length(to_sun);
    // --- the sky: deep violet above, rose, amber at the forest's edge; the sun low, its bloom held in
    let v = clamp(p.y / 135.0, 0.0, 1.0);
    var col = mix(vec3<f32>(0.16, 0.12, 0.27), vec3<f32>(0.55, 0.24, 0.26), smoothstep(0.0, 0.6, v));
    col = mix(col, vec3<f32>(0.95, 0.55, 0.18), smoothstep(0.5, 1.0, v));
    col += vec3<f32>(1.0, 0.6, 0.22) * exp(-ds / 70.0) * 0.45;
    col = mix(col, vec3<f32>(1.0, 0.93, 0.7), smoothstep(13.0, 9.0, ds));
    col += vec3<f32>(1.0, 0.8, 0.5) * exp(-ds / 16.0) * 0.35;
    // thin clouds lit gold and rose from below, drifting slowly
    let cl = fbm(vec2<f32>(p.x * 0.01 - f.time * 0.008, p.y * 0.075 + 3.0));
    let cloud = smoothstep(0.5, 0.68, cl) * smoothstep(118.0, 30.0, p.y);
    let lit_under = smoothstep(0.5, 0.75, fbm(vec2<f32>(p.x * 0.01 - f.time * 0.008, p.y * 0.075 + 3.25)));
    let cloud_col = mix(vec3<f32>(0.35, 0.2, 0.33), vec3<f32>(1.0, 0.56, 0.32), (1.0 - lit_under) * (0.35 + 0.65 * exp(-ds / 260.0)));
    col = mix(col, cloud_col, cloud * 0.75);
    // --- the forest: the nearest crown that covers the pixel; trunks under the front rows
    var covered = false;
    var trunk = 1e3;
    for (var i = 37; i >= 0; i--) {
        let t = tree_at(i);
        if (abs(p.x - t.c.x) > t.r * 1.5 || p.y < t.c.y - t.r * 1.6 || p.y > t.c.y + t.r * 2.6) { continue; }
        let fi = f32(i);
        if (t.row > 0 && p.y > t.c.y) {
            let w = mix(1.6, 3.4, f32(t.row) / 2.0);
            trunk = min(trunk, abs(p.x - t.c.x - sin(p.y * 0.07 + fi) * 0.8) - w);
        }
        if (covered) { continue; }
        var d = 1e3;
        var tex = 0.5;
        if (t.kind == 3) {
            d = spruce(p, vec2<f32>(t.c.x, t.c.y + t.r * 0.9), t.r * 2.5, fi);
            tex = fbm(p * 0.25 + fi);
        } else {
            let cr = crown(p, t.c, t.r, fi);
            d = cr.x;
            tex = cr.y;
        }
        if (d > 0.9) { continue; }
        let leafc = leaf_colour(t.kind, tex, fi);
        // against the light: the body in shade, clumps light and dark, the edge facing the sun
        // glowing through the leaves, sparks of sun through gaps
        let toward = normalize(SUN - t.c);
        let rel = (p - t.c) / t.r;
        let facing = clamp(dot(rel, toward) * 0.7 + 0.35, 0.0, 1.0);
        let depth = f32(t.row) / 2.0;
        var tc = leafc * mix(0.74, 0.7, depth) * (0.55 + 0.75 * tex) * (0.6 + 0.4 * facing);
        // clumps of leaves inside the crown: lighter tops, darker undersides, small flecks
        let clump = vnoise(rel * 7.0 + vec2<f32>(fi * 3.1, 0.0));
        let fleck = vnoise(rel * 16.0 + vec2<f32>(0.0, fi * 1.7));
        tc *= mix(1.0, 0.72 + 0.45 * clump + 0.18 * (fleck - 0.5), smoothstep(-1.0, -3.0, d));
        let rim = smoothstep(-3.5, 0.0, d) * facing;
        tc = mix(tc, mix(leafc, vec3<f32>(1.0, 0.82, 0.4), 0.35) * 1.15, rim * mix(0.55, 0.95, depth));
        // sun through the gaps: soft round glints, a few per crown, on the side facing the sun
        let gc = floor(p / 5.0);
        let gp = (gc + vec2<f32>(hash(gc + fi), hash(gc + fi + 1.3))) * 5.0;
        let gap = smoothstep(0.9, 0.95, hash(gc + fi * 1.7)) * smoothstep(1.6, 0.4, length(p - gp));
        tc += vec3<f32>(1.0, 0.82, 0.48) * gap * facing * smoothstep(-t.r * 0.7, -3.0, d) * 0.55;
        // the back rows in the warm haze of the low sun
        tc = mix(tc, vec3<f32>(0.85, 0.48, 0.24), f32(2 - t.row) * 0.2 * (0.6 + 0.4 * exp(-ds / 200.0)));
        col = mix(col, tc, smoothstep(0.9, -0.9, d));
        if (d < -0.9) { covered = true; }
    }
    // --- the ground below the forest: trunks dark against the light, beams between them, a meadow
    let ground = 214.0 + 2.5 * sin(p.x * 0.031) + 1.5 * sin(p.x * 0.11);
    if (!covered && p.y > 150.0) {
        col = mix(col, vec3<f32>(0.13, 0.06, 0.05), smoothstep(0.7, -0.7, trunk));
    }
    if (p.y > ground) {
        let g = fbm(vec2<f32>(p.x * 0.05, p.y * 0.35));
        var grass = mix(vec3<f32>(0.32, 0.2, 0.07), vec3<f32>(0.7, 0.48, 0.16), g);
        // the low sun catches the tips: a warm sheen towards the sun's side
        grass += vec3<f32>(0.4, 0.24, 0.06) * exp(-abs(p.x - SUN.x) / 140.0) * smoothstep(ground, ground + 6.0, p.y) * 0.6;
        // long shadows of the front trunks, falling from the sun towards you
        for (var i = 29; i < 38; i++) {
            let t = tree_at(i);
            let dir = normalize(vec2<f32>(t.c.x, ground) - SUN);
            let rel = p - vec2<f32>(t.c.x, ground);
            let along = dot(rel, dir);
            let across = abs(rel.x * dir.y - rel.y * dir.x);
            grass *= 1.0 - 0.45 * smoothstep(3.5 + along * 0.06, 1.5, across) * step(0.0, along) * exp(-along / 120.0);
        }
        // fallen leaves on the grass
        let cell = floor(p / vec2<f32>(9.0, 5.0));
        let h = hash(cell);
        if (h > 0.78) {
            let lc = leaf_colour(i32(h * 30.0) % 3, hash(cell + 1.7), h) * 0.85;
            let lp = (cell + vec2<f32>(0.3 + 0.4 * hash(cell + 3.1), 0.5)) * vec2<f32>(9.0, 5.0);
            let e = (p - lp) / vec2<f32>(2.6, 1.4);
            grass = mix(grass, lc, smoothstep(1.0, 0.6, length(e)));
        }
        col = mix(col, grass, smoothstep(ground - 0.8, ground + 0.8, p.y));
    }
    // beams of the low sun between the trunks, coming towards you
    let ang = atan2(to_sun.y, to_sun.x);
    let beam = smoothstep(0.45, 0.85, vnoise(vec2<f32>(ang * 26.0, f.time * 0.04))) * smoothstep(0.0, 40.0, to_sun.y);
    col += vec3<f32>(1.0, 0.7, 0.32) * beam * exp(-ds / 220.0) * select(0.0, 0.3, !covered);
    // motes of dust glinting in the light
    let mc = floor((p + vec2<f32>(f.time * 3.0, -f.time * 1.5)) / 13.0);
    let mh = hash(mc);
    if (mh > 0.9) {
        let mp = (mc + vec2<f32>(hash(mc + 1.3), hash(mc + 2.7))) * 13.0 - vec2<f32>(f.time * 3.0, -f.time * 1.5);
        let tw = 0.5 + 0.5 * sin(f.time * (1.5 + mh * 3.0) + mh * 40.0);
        col += vec3<f32>(1.0, 0.86, 0.56) * smoothstep(1.2, 0.0, length(p - mp)) * tw * 0.6 * exp(-ds / 240.0);
    }
    // --- near: a white birch on the left, round in the light from the right, horizontal dark
    // marks and patches
    let bc = 22.0 + 3.0 * sin(p.y * 0.012);
    let bw = 8.5 - 1.5 * p.y / 272.0;
    let bx = abs(p.x - bc) - bw;
    if (bx < 0.9) {
        let u = (p.x - bc) / bw;
        var b = mix(vec3<f32>(0.5, 0.47, 0.44), vec3<f32>(0.95, 0.92, 0.86), smoothstep(-1.0, 0.6, u));
        b += vec3<f32>(0.35, 0.2, 0.06) * smoothstep(0.4, 1.0, u);
        let dash = smoothstep(0.66, 0.74, vnoise(vec2<f32>(p.x * 0.18 + 3.0, p.y * 0.9)));
        let blot = smoothstep(0.62, 0.7, fbm(vec2<f32>(p.x * 0.08, p.y * 0.05) + 9.0));
        b *= 1.0 - 0.85 * max(dash, blot);
        col = mix(col, b, smoothstep(0.9, -0.9, bx));
    }
    // dark by design (owner's "1 a", 2026-10-27): a soft knee holds the bright parts near 40 %, so
    // icons of any colour stay readable on it
    let lum = dot(col, vec3<f32>(0.299, 0.587, 0.114));
    if (lum > 0.36) { col *= (0.36 + (lum - 0.36) * 0.3) / lum; }
    col = clamp(col, vec3<f32>(0.0), vec3<f32>(1.0));
    c = mix(c, col, f.strength / 0.6 * 0.97 * (1.0 - keep_bg(c)));
    put(id.xy, c);
}
@compute @workgroup_size(16, 16)
fn over(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= f.size.x || id.y >= f.size.y) { return; }
    var c = px(i32(id.x), i32(id.y)).rgb;
    let here = cover(i32(id.x), i32(id.y));
    if (here > 0.0) {
        // the icon's edge towards the sun catches the light; the rest warms a little
        let p = vec2<f32>(f32(id.x), f32(id.y));
        let toward = normalize(SUN - p);
        let edge = here - cover(i32(p.x + toward.x * 2.0), i32(p.y + toward.y * 2.0));
        let k = f.strength / 0.6;
        c = mix(c, c * vec3<f32>(1.06, 0.97, 0.86), here * 0.45 * k);
        c += vec3<f32>(1.0, 0.72, 0.36) * max(-edge, 0.0) * 0.5 * k;
    }
    put(id.xy, c);
}
