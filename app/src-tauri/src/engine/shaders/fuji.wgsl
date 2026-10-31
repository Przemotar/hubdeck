// Japan A, after the owner's reference picture (2026-10-26, "to jest japoński klimat"): a bright,
// flat, saturated day held dark by design. The top row: a blue sky with soft clouds, a big
// red-orange sun low in the middle behind the mountain, cherry branches with dense pink blossom
// and dark red buds coming in from both top corners; the middle row: Mount Fuji, deep blue with a
// wide snow cap and shaded ridges, a vermilion torii standing in the water on the right; the bottom
// row: the calm lake mirroring the mountain, the sun and the torii, a thin line of trees on the
// shore. On the left a red banner hangs from a string with 日本 in black.
const SHORE = 196.0;
const TORII_FOOT = 222.0;
const SUN_C = vec2<f32>(240.0, 98.0);
const SUN_R = 33.0;
const PEAK = vec2<f32>(240.0, 104.0);
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
    let cx = 404.0;
    let top = TORII_FOOT - 96.0;
    // the kasagi rises towards both ends
    let ex = (p.x - cx) / 44.0;
    let lift = 4.0 * ex * ex;
    let kasagi = box(p + vec2<f32>(0.0, lift), vec2<f32>(cx, top), vec2<f32>(44.0 - 2.0 * ex * ex, 3.2));
    let shimaki = box(p + vec2<f32>(0.0, lift * 0.6), vec2<f32>(cx, top + 6.0), vec2<f32>(38.0, 2.6));
    let nuki = box(p, vec2<f32>(cx, top + 22.0), vec2<f32>(35.0, 2.4));
    let post = box(p, vec2<f32>(cx, top + 14.0), vec2<f32>(2.2, 6.0));
    // pillars lean in a little towards the top
    let py = clamp((TORII_FOOT - p.y) / 96.0, 0.0, 1.0);
    let lx = cx - 25.0 + 2.0 * py;
    let rx = cx + 25.0 - 2.0 * py;
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
    let cloud = smoothstep(0.56, 0.66, cn) * smoothstep(120.0, 20.0, p.y);
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
        let snowline = 0.58 - 0.16 * pow(ridge, 3.0) - 0.05 * vnoise(vec2<f32>(p.x * 0.2, 1.0));
        let snow = smoothstep(snowline - 0.015, snowline + 0.015, h);
        var s = mix(vec3<f32>(0.96, 0.97, 1.0), vec3<f32>(0.66, 0.74, 0.9), smoothstep(-10.0, 40.0, dx) * 0.8 + (1.0 - ridge) * 0.25);
        m = mix(m, s, snow);
        col = mix(col, m, smoothstep(1.0, -1.0, md));
    }
    // the thin line of trees on the shore
    let trees = SHORE - 3.0 - 3.0 * fbm(vec2<f32>(p.x * 0.18, 4.0)) - 2.0 * smoothstep(0.7, 0.9, vnoise(vec2<f32>(p.x * 0.5, 2.0)));
    if (p.y > trees) {
        col = mix(col, mix(vec3<f32>(0.08, 0.26, 0.14), vec3<f32>(0.18, 0.42, 0.2), vnoise(vec2<f32>(p.x * 0.4, p.y))), smoothstep(trees - 0.8, trees + 0.8, p.y));
    }
    return col;
}
fn torii_col(t: vec2<f32>, p: vec2<f32>) -> vec3<f32> {
    let red = mix(vec3<f32>(0.86, 0.22, 0.08), vec3<f32>(0.62, 0.12, 0.05), smoothstep(395.0, 425.0, p.x));
    return mix(red, vec3<f32>(0.08, 0.06, 0.06), t.y);
}
// the cherry branches: dark twigs from both top corners
fn branches(p: vec2<f32>) -> f32 {
    var d = 1e3;
    // left: from the corner to the right, a twig hanging down
    d = min(d, seg(p, vec2<f32>(-8.0, 6.0), vec2<f32>(48.0, 22.0)).x - 3.2);
    d = min(d, seg(p, vec2<f32>(48.0, 22.0), vec2<f32>(104.0, 30.0)).x - 2.2);
    d = min(d, seg(p, vec2<f32>(104.0, 30.0), vec2<f32>(146.0, 26.0)).x - 1.3);
    d = min(d, seg(p, vec2<f32>(48.0, 22.0), vec2<f32>(70.0, 52.0)).x - 1.6);
    d = min(d, seg(p, vec2<f32>(70.0, 52.0), vec2<f32>(78.0, 82.0)).x - 1.0);
    d = min(d, seg(p, vec2<f32>(78.0, 82.0), vec2<f32>(74.0, 104.0)).x - 0.8);
    d = min(d, seg(p, vec2<f32>(104.0, 30.0), vec2<f32>(118.0, 48.0)).x - 1.0);
    // right: from the corner down the side
    d = min(d, seg(p, vec2<f32>(490.0, 2.0), vec2<f32>(430.0, 18.0)).x - 3.2);
    d = min(d, seg(p, vec2<f32>(430.0, 18.0), vec2<f32>(370.0, 20.0)).x - 2.0);
    d = min(d, seg(p, vec2<f32>(370.0, 20.0), vec2<f32>(338.0, 30.0)).x - 1.2);
    d = min(d, seg(p, vec2<f32>(452.0, 12.0), vec2<f32>(462.0, 58.0)).x - 1.8);
    d = min(d, seg(p, vec2<f32>(462.0, 58.0), vec2<f32>(456.0, 110.0)).x - 1.1);
    d = min(d, seg(p, vec2<f32>(456.0, 110.0), vec2<f32>(466.0, 150.0)).x - 0.8);
    d = min(d, seg(p, vec2<f32>(462.0, 70.0), vec2<f32>(476.0, 96.0)).x - 0.8);
    d = min(d, seg(p, vec2<f32>(430.0, 18.0), vec2<f32>(420.0, 50.0)).x - 1.2);
    return d;
}
// a blossom of five notched petals, radius r, turned by a; .x distance, .y how far out (0..1)
fn blossom(q: vec2<f32>, r: f32, a: f32) -> vec2<f32> {
    let ang = atan2(q.y, q.x) + a;
    let petal = 0.5 + 0.5 * cos(ang * 5.0);
    let notch = smoothstep(0.92, 1.0, petal) * 0.18;
    let edge = r * (0.62 + 0.38 * sqrt(petal) - notch);
    return vec2<f32>(length(q) - edge, length(q) / r);
}
// the red banner on the left with 日本: .x distance to the cloth, the cloth's colour in .yzw
fn banner(p: vec2<f32>, tm: f32) -> vec4<f32> {
    let sway = 1.0 * sin(tm * 0.8) * clamp((p.y - 96.0) / 80.0, 0.0, 1.0);
    let q = p - vec2<f32>(sway, 0.0);
    // it ends above the left key's label (it hid the start of "Lubię to")
    let cloth = box(q, vec2<f32>(14.0, 124.0), vec2<f32>(9.5, 26.0));
    if (cloth > 1.0) { return vec4<f32>(cloth, 0.0, 0.0, 0.0); }
    var col = vec3<f32>(0.74, 0.08, 0.07) * (0.92 + 0.12 * vnoise(vec2<f32>(q.x * 0.3, q.y * 0.08)));
    // folds of the cloth
    col *= 0.9 + 0.1 * sin(q.x * 0.6 + q.y * 0.05);
    // 日: a box with a middle bar
    let a = abs(box(q, vec2<f32>(14.0, 111.0), vec2<f32>(4.6, 6.6))) - 1.0;
    let a2 = seg(q, vec2<f32>(9.8, 111.0), vec2<f32>(18.2, 111.0)).x - 0.9;
    // 本: a long bar, a stem, two sweeps, a short bar across the stem
    var b = seg(q, vec2<f32>(7.5, 127.0), vec2<f32>(20.5, 127.0)).x - 1.0;
    b = min(b, seg(q, vec2<f32>(14.0, 121.0), vec2<f32>(14.0, 145.0)).x - 1.0);
    b = min(b, seg(q, vec2<f32>(13.6, 128.0), vec2<f32>(7.0, 138.0)).x - 0.9);
    b = min(b, seg(q, vec2<f32>(14.4, 128.0), vec2<f32>(21.0, 138.0)).x - 0.9);
    b = min(b, seg(q, vec2<f32>(10.5, 138.0), vec2<f32>(17.5, 138.0)).x - 0.85);
    let ink = min(min(a, a2), b);
    col = mix(col, vec3<f32>(0.05, 0.03, 0.03), smoothstep(0.6, -0.6, ink));
    return vec4<f32>(cloth, col);
}
@compute @workgroup_size(16, 16)
fn under(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= f.size.x || id.y >= f.size.y) { return; }
    let p = vec2<f32>(f32(id.x), f32(id.y)) + 0.5;
    var c = px(i32(id.x), i32(id.y)).rgb;
    let tm = f.time * f.speed;
    var col: vec3<f32>;
    if (p.y < SHORE) {
        col = far(p, tm);
    } else {
        // the lake: the far scene mirrored, broken by slow ripples, deeper blue towards you
        let depth = (p.y - SHORE) / (272.0 - SHORE);
        let wob = 1.2 * sin(p.y * 0.32 + tm * 1.1) * (0.3 + depth) + 1.2 * (vnoise(vec2<f32>(p.x * 0.05, p.y * 0.5 + tm * 0.3)) - 0.5);
        let m = vec2<f32>(p.x + wob, 2.0 * SHORE - p.y);
        col = far(m, tm) * vec3<f32>(0.72, 0.8, 0.92);
        col = mix(col, vec3<f32>(0.1, 0.24, 0.44), 0.25 + 0.4 * depth);
        // bright streaks along the ripples
        let streak = smoothstep(0.72, 0.9, vnoise(vec2<f32>(p.x * 0.04 + tm * 0.05, p.y * 1.3)));
        col += vec3<f32>(0.6, 0.7, 0.8) * streak * 0.12;
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
    // the banner with 日本 and its string
    let bn = banner(p, tm);
    if (bn.x < 1.0) {
        col = mix(col, bn.yzw, smoothstep(0.8, -0.8, bn.x));
    }
    let string = seg(p, vec2<f32>(0.0, 97.0), vec2<f32>(34.0, 100.0)).x - 0.5;
    col = mix(col, vec3<f32>(0.25, 0.2, 0.15), smoothstep(0.7, -0.7, string));
    // the cherry branches and their blossom, in front of everything
    if (p.y < 162.0 && (p.x < 170.0 || p.x > 320.0)) {
        let bd = branches(p);
        col = mix(col, vec3<f32>(0.2, 0.1, 0.09), smoothstep(0.8, -0.8, bd));
        // blossom in cells along the twigs: the nearest flower over this pixel
        let cell = 6.0;
        let g = floor(p / cell);
        var best = 1e3;
        var bc = vec3<f32>(0.0);
        for (var j = -1; j <= 1; j++) {
            for (var i = -1; i <= 1; i++) {
                let gc = g + vec2<f32>(f32(i), f32(j));
                let h = hash(gc * 1.31 + 7.0);
                let ctr = (gc + vec2<f32>(0.2 + 0.6 * hash(gc + 1.7), 0.2 + 0.6 * hash(gc + 3.1))) * cell;
                let near = branches(ctr);
                if (near > 8.0 || h < 0.18) { continue; }
                let bud = h < 0.3;
                let r = select(2.6 + 1.8 * hash(gc + 5.3), 1.5, bud);
                let bl = blossom(p - ctr, r, h * 6.28);
                if (bl.x < best && bl.x < 0.8) {
                    best = bl.x;
                    if (bud) {
                        bc = vec3<f32>(0.62, 0.08, 0.2);
                    } else {
                        // pale pink petals, deeper at the heart, a dark red eye
                        bc = mix(vec3<f32>(0.86, 0.38, 0.55), vec3<f32>(1.0, 0.8, 0.88), smoothstep(0.15, 0.85, bl.y));
                        bc = mix(vec3<f32>(0.55, 0.06, 0.18), bc, smoothstep(0.1, 0.28, bl.y));
                        // the side turned from the sun a shade darker
                        bc *= 0.88 + 0.12 * hash(gc + 9.0);
                    }
                }
            }
        }
        col = mix(col, bc, smoothstep(0.7, -0.7, best));
    }
    // dark by design: rich but not bright, a soft knee near 40 %
    let lum = dot(col, vec3<f32>(0.299, 0.587, 0.114));
    if (lum > 0.38) { col *= (0.38 + (lum - 0.38) * 0.32) / lum; }
    col = clamp(col, vec3<f32>(0.0), vec3<f32>(1.0));
    c = mix(c, col, f.strength / 0.6 * 0.97 * (1.0 - keep_bg(c)));
    put(id.xy, c);
}
