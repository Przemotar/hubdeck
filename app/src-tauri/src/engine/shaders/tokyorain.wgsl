// Tokyo rain through the glass: a night facade of neon boards, the wet street mirroring them,
// drops on the glass of every key. A press ignites a neon sign at the top of that key.

const YS: f32 = 290.0; // the street line, in the lower grille gap in both turns (owner, 2026-10-29: "wystarczy ten horyzont przenieść")
const DY: f32 = YS - 190.0; // how far the street line moved down from the first version; the boards stand on it

fn aa_w(d: f32, w: f32) -> f32 {
    return clamp(0.5 - d / max(w, 0.0001), 0.0, 1.0);
}
fn sd_box(p: vec2<f32>, h: vec2<f32>, r: f32) -> f32 {
    let d = abs(p) - h + vec2<f32>(r);
    return length(max(d, vec2<f32>(0.0))) + min(max(d.x, d.y), 0.0) - r;
}
fn sd_seg(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>) -> f32 {
    let pa = p - a;
    let ba = b - a;
    let h = clamp(dot(pa, ba) / dot(ba, ba), 0.0, 1.0);
    return length(pa - ba * h);
}
fn fbm3(p: vec2<f32>) -> f32 {
    var v = 0.0;
    var a = 0.5;
    var q = p;
    let rot = mat2x2<f32>(0.8, -0.6, 0.6, 0.8);
    for (var i = 0; i < 3; i++) {
        v += a * vnoise(q);
        q = rot * q * 2.02 + vec2<f32>(17.3, 9.1);
        a *= 0.5;
    }
    return v / 0.875;
}
fn hex(v: u32) -> vec3<f32> {
    return vec3<f32>(f32((v >> 16u) & 255u), f32((v >> 8u) & 255u), f32(v & 255u)) / 255.0;
}
fn luma(c: vec3<f32>) -> f32 {
    return dot(c, vec3<f32>(0.299, 0.587, 0.114));
}
fn soft_knee(c: vec3<f32>, knee: f32, slope: f32) -> vec3<f32> {
    let l = luma(c);
    if (l <= knee) { return c; }
    return c * (knee + (l - knee) * slope) / l;
}
fn neon_col(r: f32) -> vec3<f32> {
    if (r < 0.38) { return hex(0xFF2D7Au); }
    if (r < 0.70) { return hex(0x26E0FFu); }
    if (r < 0.87) { return hex(0xFFB040u); }
    return hex(0xFF4A3Du);
}

// the press: a sign lit at the top of the pressed key, set once per pixel in `under`
var<private> ps_on: f32 = 0.0;
var<private> ps_c: vec2<f32> = vec2<f32>(0.0);
var<private> ps_h: vec2<f32> = vec2<f32>(1.0);
var<private> ps_col: vec3<f32> = vec3<f32>(0.0);
var<private> ps_seed: f32 = 0.0;
var<private> ps_flash: f32 = 0.0;

struct Sign {
    c: vec2<f32>,
    h: vec2<f32>,
    col: vec3<f32>,
    on: f32,
    seed: f32,
    tw: f32,
}

// the board of column cell i in layer l (0 near, 1 far)
fn sign_of(i: f32, l: f32) -> Sign {
    var s: Sign;
    let cw = select(60.0, 38.0, l > 0.5);
    let r0 = hash(vec2<f32>(i, 3.1 + l * 17.0));
    let r1 = hash(vec2<f32>(i, 7.7 + l * 17.0));
    let r2 = hash(vec2<f32>(i, 11.3 + l * 17.0));
    let r3 = hash(vec2<f32>(i, 19.9 + l * 17.0));
    let x = (i + 0.5) * cw + (r1 - 0.5) * cw * 0.3;
    var hw = mix(8.0, 11.0, r2);
    var hh = mix(45.0, 110.0, r3);
    var bottom = mix(150.0, 182.0, r1) + DY;
    var on = mix(0.5, 0.85, r3) * select(1.0, 0.0, r2 > 0.88);
    if (r2 < 0.3 && l < 0.5) {
        // a horizontal board
        hw = mix(17.0, 22.0, r3);
        hh = mix(7.0, 9.0, r1);
        bottom = mix(110.0, 170.0, r3) + DY;
    }
    if (l > 0.5) { hw = hw * 0.62; hh = hh * 0.7; bottom = mix(95.0, 165.0, r1) + DY; on = on * 0.42; }
    s.c = vec2<f32>(x, bottom - hh);
    s.h = vec2<f32>(hw, hh);
    s.col = neon_col(r0);
    s.on = on;
    s.seed = r0 * 91.7 + r1 * 13.3 + l * 5.0;
    s.tw = 0.8;
    // a few tubes buzz now and then
    if (r3 > 0.8) {
        let burst = step(0.78, vnoise(vec2<f32>(f.time * 0.45 + r0 * 40.0, i)));
        s.on = s.on * mix(1.0, 0.25 + 0.75 * step(0.45, hash(vec2<f32>(floor(f.time * 14.0), i + l))), burst);
    }
    if (l < 0.5 && abs(x - 240.0) < 32.0) {
        s.c = vec2<f32>(240.0, 184.0 + DY - 128.0);
        s.h = vec2<f32>(15.0, 128.0);
        s.col = hex(0xFF2D7Au);
        s.on = 1.2;
    }
    return s;
}

// the glyph strokes of one cell, u in about -1..1
fn glyph(u: vec2<f32>, g: f32) -> f32 {
    var d = 1e5;
    let b0 = hash(vec2<f32>(g, 1.0));
    let b1 = hash(vec2<f32>(g, 2.0));
    let b2 = hash(vec2<f32>(g, 3.0));
    let b3 = hash(vec2<f32>(g, 4.0));
    let b4 = hash(vec2<f32>(g, 5.0));
    if (b0 < 0.6) { d = min(d, sd_seg(u, vec2<f32>(-0.7, -0.6), vec2<f32>(0.7, -0.6))); }
    if (b1 < 0.75) { d = min(d, sd_seg(u, vec2<f32>(-0.75, 0.05), vec2<f32>(0.75, 0.05))); }
    if (b2 < 0.6) { d = min(d, sd_seg(u, vec2<f32>(0.0, -0.8), vec2<f32>(0.0, 0.8))); }
    if (b3 < 0.45) { d = min(d, sd_seg(u, vec2<f32>(-0.6, -0.5), vec2<f32>(-0.6, 0.75))); }
    if (b3 > 0.55) { d = min(d, sd_seg(u, vec2<f32>(0.6, -0.5), vec2<f32>(0.6, 0.75))); }
    if (b4 < 0.5) { d = min(d, sd_seg(u, vec2<f32>(-0.7, 0.75), vec2<f32>(0.7, 0.75))); }
    if (b4 > 0.8) { d = min(d, sd_seg(u, vec2<f32>(-0.6, 0.7), vec2<f32>(0.1, 0.2))); }
    return d;
}

fn sign_light(p: vec2<f32>, s: Sign, blur: f32, cw: f32) -> vec3<f32> {
    let lp = p - s.c;
    let d = sd_box(lp, s.h, 3.0);
    // the tube frame, inset from the board's edge
    var dt = abs(d + 2.5) - s.tw;
    // the glyphs, stacked along the board's long side
    let vert = s.h.y > s.h.x;
    let side = min(s.h.x, s.h.y);
    let step = side * 2.0;
    var along = select(lp.x, lp.y, vert);
    let across = select(lp.y, lp.x, vert);
    let n = max(floor((max(s.h.x, s.h.y) - 2.0) / side), 1.0);
    let j = clamp(floor(along / step + n * 0.5), 0.0, n - 1.0);
    let cc = (j + 0.5 - n * 0.5) * step;
    var u = vec2<f32>(across, along - cc) / (side * 0.55);
    if (!vert) { u = vec2<f32>(along - cc, across) / (side * 0.55); }
    if (d < 0.0) {
        dt = min(dt, glyph(u, s.seed + j * 7.13) * side * 0.55 - s.tw);
    }
    let w = 1.0 + blur;
    let core = aa_w(dt, w);
    let tube = exp(-max(dt, 0.0) / (1.6 + blur * 0.8));
    let plate = aa_w(d, w) * select(0.16, 0.34, s.tw > 1.0);
    let halo = exp(-max(d, 0.0) / (8.0 + blur)) * 0.22;
    let wide = exp(-max(d, 0.0) / 34.0) * 0.12 * smoothstep(cw * 1.45, cw * 0.7, abs(lp.x));
    let hot = vec3<f32>(1.0, 0.94, 0.97);
    return s.on * (s.col * (plate + tube * 0.55 + halo + wide) + mix(s.col, hot, 0.65) * core * 0.9);
}

// the board's light in the wet street: mirrored on the street line, stretched down by the
// roughness of the asphalt, narrow across
fn sign_refl(p: vec2<f32>, s: Sign, depth: f32) -> vec3<f32> {
    let top = YS + (YS - (s.c.y + s.h.y));
    let bot = YS + (YS - (s.c.y - s.h.y));
    let dx = max(abs(p.x - s.c.x) - s.h.x * 0.75, 0.0);
    let wx = 2.5 + depth * 0.03;
    let across = exp(-dx / wx) * 0.8 + exp(-dx / (wx * 6.0)) * 0.2;
    let down = smoothstep(top - 8.0, top + 6.0, p.y) * (1.0 - smoothstep(bot, bot + 160.0, p.y) * 0.8);
    return s.col * s.on * across * down;
}

fn signs(p: vec2<f32>, blur: f32) -> vec3<f32> {
    var c = vec3<f32>(0.0);
    for (var l = 0; l < 2; l++) {
        let lf = f32(l);
        let cw = select(60.0, 38.0, l > 0);
        let ci = floor(p.x / cw);
        for (var k = -1; k <= 1; k++) {
            let s = sign_of(ci + f32(k), lf);
            c += sign_light(p, s, blur + lf * 2.0, cw);
        }
    }
    return c;
}

fn reflections(p: vec2<f32>, depth: f32) -> vec3<f32> {
    var c = vec3<f32>(0.0);
    for (var l = 0; l < 2; l++) {
        let lf = f32(l);
        let cw = select(60.0, 38.0, l > 0);
        let ci = floor(p.x / cw);
        for (var k = -1; k <= 1; k++) {
            let s = sign_of(ci + f32(k), lf);
            c += sign_refl(p, s, depth);
        }
    }
    return c;
}

fn press_sign() -> Sign {
    var s: Sign;
    s.c = ps_c;
    s.h = ps_h;
    s.col = ps_col;
    s.on = ps_on;
    s.seed = ps_seed;
    s.tw = 1.15;
    return s;
}

// the pressed sign's light where it hangs, and its streak down the wet ground under it
fn press_light(p: vec2<f32>, blur: f32) -> vec3<f32> {
    if (ps_on <= 0.001) { return vec3<f32>(0.0); }
    let s = press_sign();
    var c = sign_light(p, s, min(blur, 0.6), 100.0);
    let below = p.y - (s.c.y + s.h.y);
    if (below > 0.0) {
        let dx = max(abs(p.x - s.c.x) - s.h.x * 0.85, 0.0);
        let dash = 0.55 + 0.45 * smoothstep(0.2, 0.8, vnoise(vec2<f32>(p.x * 0.08, p.y * 0.3)));
        c += s.col * s.on * exp(-dx / 3.5) * smoothstep(0.0, 14.0, below) * exp(-below / 70.0) * 0.45 * dash;
    }
    return c;
}

// rain falling through the air beyond the glass: thin slanted streaks
fn rain(p: vec2<f32>) -> f32 {
    let t = f.time;
    let u = p.x + p.y * 0.13;
    let cw = 4.0;
    let cx = floor(u / cw);
    let h = hash(vec2<f32>(cx, 3.0));
    if (h > 0.45) { return 0.0; }
    let dx = (fract(u / cw) - 0.5 - (h - 0.22) * 0.8) * cw;
    let v = mix(520.0, 760.0, hash(vec2<f32>(cx, 9.0)));
    let period = 140.0 + h * 200.0;
    let s = fract((p.y - t * v) / period + hash(vec2<f32>(cx, 5.0)));
    let len = 26.0 / period;
    let along = smoothstep(0.0, len * 0.3, s) * (1.0 - smoothstep(len * 0.6, len, s));
    return exp(-dx * dx / 0.7) * along;
}

fn scene(p: vec2<f32>, blur: f32) -> vec3<f32> {
    let t = f.time;
    if (p.y < YS) {
        let wall = hex(0x0B0918u);
        let lit = signs(p, blur);
        let mist = hex(0x2A1645u) * smoothstep(100.0 + DY, YS, p.y) * 0.4;
        // the far building: dim warm windows high up, most of them dark
        let wc = floor(vec2<f32>(p.x / 11.0, p.y / 15.0));
        let wf = fract(vec2<f32>(p.x / 11.0, p.y / 15.0));
        let wh = hash(wc + 51.0);
        let wbox = smoothstep(0.5, 0.25, max(abs(wf.x - 0.5) * 1.35, abs(wf.y - 0.5) * 1.15));
        let win = select(0.0, 1.0, wh > 0.8) * wbox * (1.0 - smoothstep(60.0, 150.0, p.y)) * mix(0.03, 0.08, hash(wc + 7.0));
        let wcol = mix(hex(0xFFB040u), hex(0x26E0FFu), step(0.95, wh));
        let fog = fbm3(vec2<f32>(p.x * 0.011 - t * 0.025, p.y * 0.035 + t * 0.01));
        let fogb = smoothstep(60.0 + DY, YS, p.y) * smoothstep(0.35, 0.8, fog);
        let air = hex(0x2A1645u) * fogb * 0.35 + lit * fogb * 0.3;
        let pl = press_light(p, blur);
        return wall + mist + lit + air + pl + wcol * win + rain(p) * (0.04 + luma(lit + pl) * 0.9);
    }
    let depth = p.y - YS;
    // the ground seen in perspective: puddles are flat, wide, and grow toward the viewer
    let gy = 900.0 / (depth + 45.0);
    let gx = (p.x - 240.0) * gy / 140.0;
    let pud = smoothstep(0.46, 0.6, fbm3(vec2<f32>(gx * 0.55 + 7.0, gy * 1.6)));
    let jit = (vnoise(vec2<f32>(p.x * 0.08, p.y * 0.5 - t * 0.9)) - 0.5) * (0.6 + depth * 0.01) * (1.0 - pud * 0.6);
    let q = vec2<f32>(p.x + jit, p.y);
    let dash = smoothstep(0.2, 0.8, vnoise(vec2<f32>(q.x * 0.07, q.y * 0.32)));
    let streak = (0.45 + 0.55 * vnoise(vec2<f32>(q.x * 0.3, q.y * 0.01))) * mix(0.45, 1.0, dash);
    let fres = mix(0.8, 0.3, clamp(depth / 290.0, 0.0, 1.0));
    var refl = reflections(q, depth) * mix(streak, 0.8, pud) * fres;
    if (pud > 0.01) {
        let m = vec2<f32>(q.x, YS - depth * 1.02);
        refl = refl * (1.0 - pud * 0.55) + signs(m, blur + 1.5) * pud * fres * 0.6;
    }
    // a worn zebra crossing, its stripes running away toward the facade
    let ang = (p.x - 240.0) / (depth + 45.0);
    let zs = fract(ang * 2.2 + 0.25);
    let zmask = smoothstep(0.18, 0.24, zs) * (1.0 - smoothstep(0.62, 0.68, zs));
    let zband = smoothstep(60.0, 90.0, depth) * (1.0 - smoothstep(230.0, 260.0, depth));
    let worn = smoothstep(0.25, 0.6, fbm3(vec2<f32>(gx * 3.0, gy * 6.0)));
    let paint = zmask * zband * worn;
    refl = refl * (1.0 + paint * 0.6);
    // rain rings on the pools
    let rg = vec2<f32>(gx * 1.4, gy * 2.4);
    let rc = floor(rg);
    let rh = hash(rc + 19.0);
    let age = fract(t * 0.7 + rh * 7.0);
    let rcen = rc + vec2<f32>(0.25, 0.25) + vec2<f32>(hash(rc + 3.0), hash(rc + 8.0)) * 0.5;
    let rd = length(rg - rcen);
    let ring = exp(-abs(rd - age * 0.45) * 28.0) * (1.0 - age) * select(0.0, 1.0, rh < 0.6) * pud;
    refl = refl * (1.0 + ring * 1.6) + hex(0x2A1645u) * ring * 0.25;
    let grain = (vnoise(p * 0.9) - 0.5) * 0.012 * (1.0 - pud);
    let ground = hex(0x08070Fu) + hex(0x2A1645u) * exp(-depth / 34.0) * 0.4 + vec3<f32>(grain)
        + (hex(0x3A3550u) * 0.35 + refl * 0.35) * paint * (1.0 - pud * 0.6);
    let pl = press_light(p, blur);
    return ground + refl + pl + rain(p) * (0.03 + luma(refl + pl) * 0.7);
}

// --- the glass: drops on every key
struct Drop {
    n: vec2<f32>,   // the surface normal across, -1..1
    c: vec2<f32>,   // the drop's centre
    r: f32,         // its radius
    m: f32,         // how much of the pixel it covers
    trail: f32,     // a wiped trail (clearer glass)
    edge: f32,      // the bright film along a trail's edges
}

fn take(d: ptr<function, Drop>, p: vec2<f32>, c: vec2<f32>, r: f32, sy: f32) {
    let v = (p - c) * vec2<f32>(1.0, sy);
    let l = length(v);
    let m = clamp(r - l + 0.5, 0.0, 1.0);
    if (m > (*d).m) {
        (*d).m = m;
        (*d).n = v / max(r, 0.001);
        (*d).c = c;
        (*d).r = r;
    }
}

fn drops(p: vec2<f32>) -> Drop {
    let t = f.time;
    var d: Drop;
    d.m = 0.0;
    d.trail = 0.0;
    d.edge = 0.0;
    d.n = vec2<f32>(0.0);
    d.c = p;
    d.r = 1.0;
    // small beads, one per cell, slowly forming and drying
    let cs = 7.0;
    let ci = floor(p / cs);
    let dens = smoothstep(0.25, 0.75, vnoise(p * 0.018 + vec2<f32>(3.0, 7.0)));
    let h0 = hash(ci + 3.7);
    if (h0 < mix(0.18, 0.72, dens)) {
        let h1 = hash(ci + 9.1);
        let life = 0.5 + 0.5 * sin(t * 0.25 + h1 * 6.283);
        let r = mix(0.7, 2.2, fract(h0 * 13.1) * fract(h0 * 13.1)) * smoothstep(0.0, 0.4, life);
        let o = vec2<f32>(hash(ci + 1.3), hash(ci + 5.9)) * (cs - 2.0 * r - 1.0) + r + 0.5;
        take(&d, p, ci * cs + o, r, 1.0);
    }
    // larger drops
    let cb = 19.0;
    let bi = floor(p / cb);
    let g0 = hash(bi + 41.3);
    let dens2 = smoothstep(0.3, 0.8, vnoise((bi + 0.5) * cb * 0.018 + vec2<f32>(3.0, 7.0)));
    if (g0 < mix(0.08, 0.5, dens2)) {
        let r = mix(2.4, 4.8, hash(bi + 2.2));
        let o = vec2<f32>(hash(bi + 8.8), hash(bi + 4.4)) * (cb - 2.0 * r - 2.0) + r + 1.0;
        take(&d, p, bi * cb + o, r, 0.92);
    }
    // a drop that runs down now and then, wiping a trail of beads behind it
    let cwid = 30.0;
    let col = floor(p.x / cwid);
    let k0 = hash(vec2<f32>(col, 77.0));
    let k1 = hash(vec2<f32>(col, 13.0));
    if (k0 < 0.42) {
        let ph = t * mix(0.035, 0.065, k1) + k1 * 3.0;
        let fall = fract(ph);
        let head = fall * 700.0 - 110.0 + sin(fall * 40.0 + k1 * 9.0) * 4.0;
        let xc = (col + 0.5) * cwid + (k1 - 0.5) * 10.0;
        let wob = sin(p.y * 0.07 + k1 * 20.0) * 2.0;
        let rr = mix(4.4, 5.6, k1);
        take(&d, p, vec2<f32>(xc + sin(head * 0.07 + k1 * 20.0) * 2.0, head - rr * 0.2), rr, 0.82);
        let above = head - p.y;
        let tw = rr * 1.0;
        let along = smoothstep(0.0, 6.0, above) * (1.0 - smoothstep(70.0, 190.0, above));
        let ax = abs(p.x - xc - wob);
        d.trail = along * (1.0 - smoothstep(tw * 0.65, tw, ax));
        d.edge = along * exp(-abs(ax - tw * 0.82) * 1.6) * 0.7;
        if (along > 0.0) {
            let seg = floor(p.y / 9.0);
            let sh = hash(vec2<f32>(col, seg));
            let sr = mix(0.8, 1.7, sh) * (1.0 - smoothstep(70.0, 170.0, above));
            if (sh < 0.75 && above > 10.0) {
                take(&d, p, vec2<f32>(xc + sin((seg * 9.0 + 4.5) * 0.07 + k1 * 20.0) * 2.0 + (sh - 0.4) * 2.0, seg * 9.0 + 4.5), sr, 1.0);
            }
        }
    }
    return d;
}

fn glass(p: vec2<f32>) -> vec3<f32> {
    let d = drops(p);
    // fogged glass blurs the street; the wiped trail is clearer
    let blur = mix(2.8, 0.4, d.trail);
    var base = vec3<f32>(0.0);
    if (d.m < 0.99) {
        let sc = scene(p, blur);
        base = sc * mix(0.86, 1.08, d.trail) + hex(0x2A1645u) * 0.08 * (1.0 - d.trail) + sc * d.edge * 0.7 + vec3<f32>(0.025) * d.edge;
    }
    if (d.m <= 0.0) { return base; }
    let n = d.n;
    let nl = min(length(n), 1.0);
    let z = sqrt(max(1.0 - nl * nl, 0.0));
    // a drop is a small lens: the street through it is sharp, wide and upside down
    let q = d.c - n * (14.0 + d.r * 4.0) + vec2<f32>(0.0, 6.0);
    var lens = scene(q, 0.5) * (0.85 + 0.45 * z);
    // light gathers at the bottom, a white glint on top, a thin dark rim
    let rimd = smoothstep(0.7, 1.0, nl);
    lens = lens * (1.0 - rimd * 0.5) + lens * smoothstep(0.2, 0.9, n.y) * 0.4;
    let glint = exp(-dot(n - vec2<f32>(-0.32, -0.42), n - vec2<f32>(-0.32, -0.42)) * 14.0);
    lens += vec3<f32>(1.0, 0.95, 0.98) * glint * 0.55 * smoothstep(1.2, 3.0, d.r);
    return mix(base, lens, d.m);
}

@compute @workgroup_size(16, 16)
fn under(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= f.size.x || id.y >= f.size.y) { return; }
    let p = world(vec2<f32>(id.xy) + vec2<f32>(0.5));
    // the press: which key, its window in the world square, the tube's ignition
    let age = f.time - aux_at(2);
    if (aux_at(3) > 0.5 && age >= 0.0 && age < 7.0) {
        let k = key_at(vec2<f32>(aux_at(0), aux_at(1)));
        if (k >= 0) {
            let r = keys[k];
            let a = world(r.xy);
            let b = world(r.xy + r.zw);
            let lo = min(a, b);
            let hi = max(a, b);
            let w = hi.x - lo.x;
            ps_c = vec2<f32>((lo.x + hi.x) * 0.5, lo.y + 9.5);
            ps_h = vec2<f32>(min(w * 0.5 - 8.0, 32.0), 8.0);
            ps_col = neon_col(hash(vec2<f32>(f32(k), 4.2)));
            ps_seed = f32(k) * 13.7 + 2.0;
            // a tube strikes: two stutters, then steady; it dims after a few seconds
            var ign = 1.0;
            if (age < 0.08) { ign = 0.9; } else if (age < 0.16) { ign = 0.05; } else if (age < 0.24) { ign = 0.5; } else if (age < 0.36) { ign = 0.0; } else if (age < 0.5) { ign = smoothstep(0.36, 0.5, age); }
            ps_on = 1.3 * ign * (1.0 - smoothstep(4.0, 7.0, age));
            ps_flash = exp(-age * 5.0) * step(0.36, age);
        }
    }
    var col = glass(p);
    if (ps_on > 0.0) {
        // the light of the strike scatters in the fogged glass of that key
        let dd = length((p - ps_c) * vec2<f32>(0.6, 1.0));
        col += ps_col * exp(-dd / 30.0) * (0.12 * ps_on + 0.3 * ps_flash);
    }
    col = soft_knee(col, 0.72, 0.35);
    // a quiet dark halo around the keys' content, so the icons lead over the brightest tubes
    let ix = i32(id.x);
    let iy = i32(id.y);
    // a weighted average, not a max: a soft shadow, no stamped copies of the letters
    var acc = cover(ix, iy) * 2.0;
    acc += cover(ix + 2, iy) + cover(ix - 2, iy) + cover(ix, iy + 2) + cover(ix, iy - 2);
    acc += 0.8 * (cover(ix + 3, iy + 3) + cover(ix - 3, iy - 3) + cover(ix - 3, iy + 3) + cover(ix + 3, iy - 3));
    acc += 0.6 * (cover(ix + 5, iy + 1) + cover(ix - 5, iy - 1) + cover(ix - 1, iy + 5) + cover(ix + 1, iy - 5));
    let near = clamp(acc / 11.6 * 4.5, 0.0, 1.0);
    col = col * (1.0 - 0.6 * near);
    let bg = px(i32(id.x), i32(id.y)).rgb;
    col = mix(col, bg, keep_bg(bg));
    put(id.xy, col);
}
