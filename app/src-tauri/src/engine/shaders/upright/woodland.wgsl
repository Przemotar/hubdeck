// The mushroom hunt as one picture in the 480x480 world square (`world()`; owner, 2026-10-29: "wolałbym
// poprawić ten mushroom hunt … fajniej, ładniej ale optymalniej", under 35 ms a frame on the software
// renderer with the mushrooms): an old mixed forest in the morning. The trunks stand through the
// whole height, so lying the deck sees a wide stand of them and upright a tall one; the canopy closes
// over the top (upright, its whole top row), the forest floor fills the bottom row in both turns
// (the mushrooms layer grows there). Pines with plated bark going orange high up, smooth grey beeches
// with lichen, roots and moss at their feet, three depths, the far ones pale in the haze; shafts of
// sun through gaps in the canopy that breathe slowly, flecks of light moving on the floor and the
// trunks when the wind stirs; ground mist between the trunks; ferns, moss, needles, cones and
// fallen leaves on the floor. The falling leaves are the `leaves` layer over it, as before (owner:
// "wolałem te poprzednie liście").
// Cheap by construction: a pixel looks only at the trunks standing in its own strip, a fern only
// near its own place.
const GROUND = 318.0;
const LDIR = vec2<f32>(-0.33, 0.94);
fn rot(v: vec2<f32>, a: f32) -> vec2<f32> {
    let c = cos(a);
    let s = sin(a);
    return vec2<f32>(v.x * c + v.y * s, -v.x * s + v.y * c);
}
fn fbm3(p: vec2<f32>) -> f32 {
    return 0.57 * vnoise(p) + 0.29 * vnoise(p * 2.03 + vec2<f32>(17.1, 9.3)) + 0.14 * vnoise(p * 4.1 + vec2<f32>(3.7, 21.2));
}
fn seg(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>) -> vec2<f32> {
    let pa = p - a;
    let ba = b - a;
    let h = clamp(dot(pa, ba) / dot(ba, ba), 0.0, 1.0);
    return vec2<f32>(length(pa - ba * h), h);
}
fn ground_at(x: f32) -> f32 {
    return GROUND + 3.0 * sin(x * 0.021) + 2.0 * sin(x * 0.067 + 1.0);
}
// a gust now and then (about every 19 s): 0 calm .. 1
fn gust_at(t: f32) -> f32 {
    return pow(max(sin(t * 0.33 + 0.6), 0.0), 6.0);
}
// --- the trunks: depth d (0 far, 1 middle, 2 near) in cells of a width; the one in cell c
struct Trunk {
    x: f32,
    w: f32,
    lean: f32,
    pine: bool,
    here: bool,
    base: f32,
}
fn trunk_of(d: i32, c: f32) -> Trunk {
    let fd = f32(d);
    let cw = select(select(92.0, 46.0, d == 1), 25.0, d == 0);
    let h1 = hash(vec2<f32>(c, 3.1 + fd * 11.0));
    let h2 = hash(vec2<f32>(c, 5.7 + fd * 11.0));
    let h3 = hash(vec2<f32>(c, 8.3 + fd * 11.0));
    var t: Trunk;
    t.x = (c + 0.2 + 0.6 * h1) * cw;
    t.w = select(select(mix(12.0, 16.0, h2), mix(5.5, 8.5, h2), d == 1), mix(2.0, 3.4, h2), d == 0);
    t.lean = (h3 - 0.5) * 0.06;
    t.pine = h3 < 0.55;
    t.here = h2 < select(0.85, 0.8, d == 2);
    t.base = GROUND + (fd - 1.0) * 5.0;
    return t;
}
// a trunk's colour at p, or alpha 0: .rgb colour, .a coverage
fn trunk_at(p: vec2<f32>, t: Trunk, d: i32) -> vec4<f32> {
    if (!t.here || p.y > t.base + 4.0) { return vec4<f32>(0.0); }
    let x = t.x + (p.y - 240.0) * t.lean + 1.6 * (vnoise(vec2<f32>(p.y * 0.011, t.x)) - 0.5);
    // wider at the foot: a root flare, a few roots spreading
    let foot = smoothstep(t.base - 40.0, t.base, p.y);
    let w = t.w * (1.0 + 0.35 * foot * foot + select(0.0, 0.6, d == 2) * pow(foot, 4.0) * (0.6 + 0.4 * sin((p.x - x) * 0.5)));
    let u = (p.x - x) / w;
    let cov = smoothstep(1.0 + 1.0 / w, 1.0 - 1.0 / w, abs(u));
    if (cov <= 0.0) { return vec4<f32>(0.0); }
    // round: lit from the upper left, the far side in shade, a rim of light on the lit edge
    let n = vec2<f32>(u, sqrt(max(1.0 - u * u, 0.0)));
    let lit = clamp(0.45 - u * 0.55, 0.0, 1.0);
    var bark: vec3<f32>;
    if (t.pine) {
        // pine: grey-brown plates and dark cracks low down, thin orange bark high up
        // plates: cells stretched along the trunk, each its own shade, dark cracks between
        let bq = vec2<f32>(u * 3.2 + t.x * 0.37, p.y * 0.06 / max(t.w / 9.0, 0.6));
        let bc = floor(bq);
        var d1 = 9.0;
        var d2 = 9.0;
        var shade = 0.5;
        for (var j = -1; j <= 1; j++) {
            for (var i = -1; i <= 1; i++) {
                let g = bc + vec2<f32>(f32(i), f32(j));
                let o = vec2<f32>(hash(g + 0.7), hash(g + 2.9)) * 0.8 + 0.1;
                let dd = length((bq - g - o) * vec2<f32>(1.0, 2.4));
                if (dd < d1) { d2 = d1; d1 = dd; shade = hash(g + 5.1); } else if (dd < d2) { d2 = dd; }
            }
        }
        bark = mix(vec3<f32>(0.2, 0.15, 0.11), vec3<f32>(0.4, 0.3, 0.21), shade);
        bark *= 0.6 + 0.4 * smoothstep(0.0, 0.14, d2 - d1);
        bark *= 0.9 + 0.2 * vnoise(vec2<f32>(u * 9.0, p.y * 0.4));
        bark = mix(bark, vec3<f32>(0.66, 0.36, 0.17), smoothstep(200.0, 40.0, p.y) * 0.8);
    } else {
        // beech: smooth grey, pale lichen patches, faint rings
        bark = vec3<f32>(0.44, 0.45, 0.42) * (0.92 + 0.12 * vnoise(vec2<f32>(u * 2.0 + t.x, p.y * 0.03)));
        bark = mix(bark, vec3<f32>(0.6, 0.64, 0.52), smoothstep(0.66, 0.74, vnoise(vec2<f32>(u * 3.0 + t.x * 3.0, p.y * 0.08))) * 0.5);
        bark *= 1.0 - 0.1 * smoothstep(0.8, 0.95, vnoise(vec2<f32>(p.x * 0.2, p.y * 0.7)));
    }
    var c = bark * mix(0.32, 1.1, lit);
    c += vec3<f32>(0.5, 0.36, 0.18) * smoothstep(-0.55, -0.9, u) * 0.35;
    // moss climbing the shaded side at the foot
    let moss = smoothstep(t.base - 70.0, t.base - 10.0, p.y + 25.0 * vnoise(vec2<f32>(u * 2.0, p.y * 0.06))) * smoothstep(-0.3, 0.6, u);
    c = mix(c, vec3<f32>(0.16, 0.27, 0.07) * (0.6 + 0.5 * lit), moss * 0.85);
    return vec4<f32>(c, cov);
}
// --- a fern at its root b: three arching fronds of leaflets; .x distance (px), .y along 0..1
fn fern(p: vec2<f32>, b: vec2<f32>, s: f32, seed: f32, sway: f32) -> vec2<f32> {
    let q = p - b;
    if (abs(q.x) > 46.0 * s || q.y > 3.0 || q.y < -42.0 * s) { return vec2<f32>(1e3, 0.0); }
    var best = vec2<f32>(1e3, 0.0);
    for (var f = 0; f < 3; f++) {
        let ff = f32(f) - 1.0;
        let len = (30.0 + 8.0 * hash(vec2<f32>(seed, ff))) * s * (1.0 - 0.15 * abs(ff));
        let lean = ff * 0.85 + 0.12 * sin(sway + ff);
        // the frond's spine: an arch out and over, as three straight pieces
        let p0 = vec2<f32>(0.0);
        let p1 = vec2<f32>(lean * len * 0.25, -len * 0.45);
        let p2 = vec2<f32>(lean * len * 0.7, -len * 0.75);
        let p3 = vec2<f32>(lean * len * 1.15, -len * 0.62);
        var dd = seg(q, p0, p1);
        var t = dd.y * 0.33;
        let e1 = seg(q, p1, p2);
        if (e1.x < dd.x) { dd = e1; t = 0.33 + e1.y * 0.33; }
        let e2 = seg(q, p2, p3);
        if (e2.x < dd.x) { dd = e2; t = 0.66 + e2.y * 0.34; }
        // the leaflets: wide near the middle, shorter towards the tip, in a comb along the spine
        let reach = len * 0.2 * sin(3.14159 * clamp(t * 1.05, 0.0, 1.0)) * (1.0 - 0.3 * t);
        let comb = 0.55 + 0.45 * abs(sin(t * len * 0.42));
        let df = dd.x - reach * comb;
        if (df < best.x) { best = vec2<f32>(df, t); }
    }
    return best;
}
fn leaf_col(kind: f32, t: f32, back: bool) -> vec3<f32> {
    var c: vec3<f32>;
    if (kind < 0.5) {
        c = mix(vec3<f32>(0.86, 0.58, 0.12), vec3<f32>(0.98, 0.8, 0.3), t);
    } else if (kind < 1.5) {
        c = mix(vec3<f32>(0.5, 0.24, 0.07), vec3<f32>(0.74, 0.44, 0.13), t);
    } else {
        c = mix(vec3<f32>(0.7, 0.12, 0.05), vec3<f32>(0.95, 0.4, 0.08), t);
    }
    if (back) { c *= 0.82; }
    return c;
}
@compute @workgroup_size(16, 16)
fn under(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= f.size.x || id.y >= f.size.y) { return; }
    let tm = f.time * f.speed;
    let gust = gust_at(tm);
    let p = world(vec2<f32>(f32(id.x), f32(id.y)) + 0.5);
    var c = px(i32(id.x), i32(id.y)).rgb;
    let gy = ground_at(p.x);
    // --- the far forest in the morning haze: pale gold high up, deep green low down, a few far
    // autumn crowns as soft dabs
    let v = clamp(p.y / 480.0, 0.0, 1.0);
    var col = mix(vec3<f32>(0.66, 0.6, 0.4), vec3<f32>(0.12, 0.15, 0.07), smoothstep(0.15, 0.75, v));
    col += vec3<f32>(0.3, 0.24, 0.1) * exp(-length(p - vec2<f32>(80.0, -30.0)) / 220.0);
    let dab = fbm3(vec2<f32>(p.x * 0.035, p.y * 0.05) + 5.0);
    col = mix(col, mix(vec3<f32>(0.66, 0.52, 0.26), vec3<f32>(0.6, 0.4, 0.2), vnoise(p * 0.015)), smoothstep(0.5, 0.75, dab) * smoothstep(300.0, 140.0, p.y) * 0.22);
    // --- the trunks, far to near: only those standing in this pixel's strip
    for (var d = 0; d < 3; d++) {
        let cw = select(select(92.0, 46.0, d == 1), 25.0, d == 0);
        let cell = floor(p.x / cw);
        let haze = f32(2 - d) / 2.0;
        for (var k = -1; k <= 1; k++) {
            let tr = trunk_of(d, cell + f32(k));
            let tc = trunk_at(p, tr, d);
            if (tc.a > 0.0) {
                let hc = mix(tc.rgb, col * 0.95 + vec3<f32>(0.03, 0.04, 0.02), haze * 0.6);
                col = mix(col, hc, tc.a);
            }
        }
    }
    // --- the canopy: a dark roof with gaps of sky, densest at the top (no leaves on the trees, owner:
    // "nie muszą być na drzewach i z nich spadać, bo tu to jakoś sztucznie wygląda")
    let roofn = fbm3(vec2<f32>(p.x * 0.035 + tm * 0.01 + gust * 0.04, p.y * 0.05));
    let roof = smoothstep(0.42, 0.62, roofn) * smoothstep(150.0, 10.0, p.y);
    col = mix(col, vec3<f32>(0.06, 0.1, 0.04), roof * 0.85);
    let gapc = smoothstep(0.32, 0.22, roofn) * smoothstep(110.0, 0.0, p.y);
    col = mix(col, vec3<f32>(0.85, 0.9, 0.7), gapc * 0.7);
    // --- the forest floor: moss and needles, cones, fallen leaves; darker towards you
    if (p.y > gy - 2.0) {
        let g = fbm3(vec2<f32>(p.x * 0.08, p.y * 0.14));
        var fl = mix(vec3<f32>(0.11, 0.16, 0.05), vec3<f32>(0.27, 0.32, 0.1), g);
        // moss in fine tufts, needle-brown patches between
        fl *= 0.85 + 0.3 * vnoise(p * vec2<f32>(0.6, 0.9));
        fl = mix(fl, vec3<f32>(0.38, 0.26, 0.12), smoothstep(0.58, 0.72, vnoise(p * vec2<f32>(0.03, 0.06) + 9.0)) * 0.5);
        // a litter of small things in cells: needles, a cone, a fallen leaf
        let cs = vec2<f32>(7.0, 4.0);
        let cell = floor(p / cs);
        let h = hash(cell);
        let lp = (cell + vec2<f32>(0.25 + 0.5 * hash(cell + 3.1), 0.3 + 0.4 * hash(cell + 4.2))) * cs;
        let lq = rot(p - lp, hash(cell + 6.3) * 3.14159);
        if (h > 0.62) {
            if (h > 0.95) {
                // a cone: a brown oval with scales
                let e = length(lq / vec2<f32>(2.6, 1.6));
                fl = mix(fl, vec3<f32>(0.32, 0.2, 0.1) * (0.8 + 0.3 * step(0.5, fract(lq.x * 1.4))), smoothstep(1.05, 0.85, e));
            } else if (h > 0.8) {
                let e = length(lq / vec2<f32>(2.8, 1.3));
                fl = mix(fl, leaf_col(floor(h * 30.0) % 3.0, 0.4, false) * 0.75, smoothstep(1.0, 0.75, e) * 0.85);
            } else {
                // needles: thin pale strokes
                fl = mix(fl, vec3<f32>(0.5, 0.38, 0.2), smoothstep(0.5, 0.0, abs(lq.y)) * step(abs(lq.x), 3.0) * 0.6);
            }
        }
        fl *= 0.78 + 0.3 * smoothstep(480.0, gy, p.y);
        col = mix(col, fl, smoothstep(gy - 1.0, gy + 1.0, p.y));
    }
    // --- ferns on the floor, each looked at only near its own place
    let sway = tm * 0.9 + gust * 2.0;
    var fd = vec2<f32>(1e3, 0.0);
    var fs = 0.0;
    for (var i = 0; i < 9; i++) {
        let fi = f32(i);
        var fb: vec2<f32>;
        var sc = 1.0;
        switch i {
            case 0: { fb = vec2<f32>(30.0, 0.0); }
            case 1: { fb = vec2<f32>(132.0, 0.0); }
            case 2: { fb = vec2<f32>(214.0, 0.0); sc = 0.8; }
            case 3: { fb = vec2<f32>(318.0, 0.0); }
            case 4: { fb = vec2<f32>(448.0, 0.0); }
            case 5: { fb = vec2<f32>(130.0, 476.0); sc = 1.3; }
            case 6: { fb = vec2<f32>(196.0, 404.0); sc = 1.15; }
            case 7: { fb = vec2<f32>(292.0, 428.0); sc = 1.2; }
            default: { fb = vec2<f32>(354.0, 478.0); sc = 1.3; }
        }
        if (fb.y == 0.0) { fb.y = ground_at(fb.x) + 6.0; }
        let e = fern(p, fb, sc, fi, sway + fi);
        if (e.x < fd.x) { fd = e; fs = fi; }
    }
    if (fd.x < 1.0) {
        var fc = mix(vec3<f32>(0.1, 0.22, 0.05), vec3<f32>(0.36, 0.5, 0.12), fd.y);
        // two of them already turning
        if (fs > 2.5 && fs < 4.5) { fc = mix(fc, mix(vec3<f32>(0.6, 0.32, 0.08), vec3<f32>(0.78, 0.55, 0.14), fd.y), 0.7); }
        fc *= 0.8 + 0.3 * smoothstep(-2.0, 0.0, fd.x);
        col = mix(col, fc, smoothstep(0.8, -0.8, fd.x));
    }
    // --- ground mist between the trunks, drifting
    let mist = fbm3(vec2<f32>(p.x * 0.01 - tm * 0.025, p.y * 0.028));
    let band = smoothstep(190.0, 290.0, p.y) * smoothstep(380.0, 300.0, p.y);
    col = mix(col, vec3<f32>(0.7, 0.68, 0.54), band * (0.25 + 0.5 * mist) * 0.55);
    // --- shafts of sun through the canopy, breathing slowly; flecks of light on the floor and
    // the trunks, moving when the wind stirs the crowns
    let along = dot(p, vec2<f32>(LDIR.y, -LDIR.x));
    let shaft = smoothstep(0.6, 0.85, vnoise(vec2<f32>(along * 0.032, tm * 0.04))) * (0.5 + 0.5 * smoothstep(480.0, 0.0, p.y)) * (0.75 + 0.25 * sin(tm * 0.3 + along * 0.01));
    col += vec3<f32>(1.0, 0.76, 0.36) * shaft * (0.3 + 0.35 * band);
    let fleck = smoothstep(0.74, 0.86, vnoise(p * 0.07 + vec2<f32>(tm * 0.05 + gust * 0.6, gust * 0.4)));
    col += vec3<f32>(1.0, 0.82, 0.45) * fleck * shaft * 0.5;
    // dark by design: a soft knee near 40 %
    let lum = dot(col, vec3<f32>(0.299, 0.587, 0.114));
    if (lum > 0.36) { col *= (0.36 + (lum - 0.36) * 0.3) / lum; }
    col = clamp(col, vec3<f32>(0.0), vec3<f32>(1.0));
    c = mix(c, col, f.strength / 0.6 * 0.97 * (1.0 - keep_bg(c)));
    put(id.xy, c);
}
