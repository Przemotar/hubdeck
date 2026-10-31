// Hollow Hill: a Halloween night painted in layers. A harvest moon behind a dead tree (top left),
// a crooked manor with lit windows on its knoll (centre), a graveyard with an iron fence on the
// right slope, a patch of carved jack-o'-lanterns glowing in the ground fog (bottom), bats round
// the moon and a spider on its thread. A press lets a flock of bats burst out of the pressed key
// and a wave of candle light runs out through the fog, flaring every lantern it passes.
// World-square coordinates: landscape sees the tree trunk (left) and the graves (right); upright
// sees the sky with the treetop (top) and the big lanterns (bottom).

const PI: f32 = 3.14159265;

fn lum(c: vec3<f32>) -> f32 {
    return dot(c, vec3<f32>(0.299, 0.587, 0.114));
}

fn knee(c: vec3<f32>, lim: f32) -> vec3<f32> {
    let l = lum(c);
    let k = lim * 0.75;
    if (l <= k) { return c; }
    let nl = k + (lim - k) * (1.0 - exp(-(l - k) / (lim - k)));
    return c * (nl / l);
}

fn fill(d: f32) -> f32 {
    return 1.0 - smoothstep(-0.7, 0.7, d);
}

fn fbm(p: vec2<f32>) -> f32 {
    var acc = 0.0;
    var amp = 0.5;
    var pp = p;
    for (var i = 0; i < 4; i++) {
        acc += amp * vnoise(pp);
        pp = pp * 2.07 + vec2<f32>(13.7, 7.1);
        amp *= 0.5;
    }
    return acc / 0.9375;
}

fn rot(v: vec2<f32>, a: f32) -> vec2<f32> {
    let ca = cos(a);
    let sa = sin(a);
    return vec2<f32>(ca * v.x - sa * v.y, sa * v.x + ca * v.y);
}

fn sd_box(p: vec2<f32>, c: vec2<f32>, hs: vec2<f32>) -> f32 {
    let d = abs(p - c) - hs;
    return length(max(d, vec2<f32>(0.0))) + min(max(d.x, d.y), 0.0);
}

fn sd_cap(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>, ra: f32, rb: f32) -> f32 {
    let pa = p - a;
    let ba = b - a;
    let h = clamp(dot(pa, ba) / dot(ba, ba), 0.0, 1.0);
    return length(pa - ba * h) - mix(ra, rb, h);
}

fn sd_tri(p: vec2<f32>, p0: vec2<f32>, p1: vec2<f32>, p2: vec2<f32>) -> f32 {
    let e0 = p1 - p0;
    let e1 = p2 - p1;
    let e2 = p0 - p2;
    let v0 = p - p0;
    let v1 = p - p1;
    let v2 = p - p2;
    let q0 = v0 - e0 * clamp(dot(v0, e0) / dot(e0, e0), 0.0, 1.0);
    let q1 = v1 - e1 * clamp(dot(v1, e1) / dot(e1, e1), 0.0, 1.0);
    let q2 = v2 - e2 * clamp(dot(v2, e2) / dot(e2, e2), 0.0, 1.0);
    let sg = sign(e0.x * e2.y - e0.y * e2.x);
    let a0 = vec2<f32>(dot(q0, q0), sg * (v0.x * e0.y - v0.y * e0.x));
    let a1 = vec2<f32>(dot(q1, q1), sg * (v1.x * e1.y - v1.y * e1.x));
    let a2 = vec2<f32>(dot(q2, q2), sg * (v2.x * e2.y - v2.y * e2.x));
    let m = min(min(a0, a1), a2);
    return -sqrt(m.x) * sign(m.y);
}

// ---- the path from the manor's door down through the patch ----
fn path_x(y: f32) -> f32 {
    let pt = clamp((y - 266.0) / 210.0, 0.0, 1.0);
    return 242.0 + 62.0 * sin(pt * 3.2 - 0.25) * pt + 5.0 * (vnoise(vec2<f32>(y * 0.05, 1.0)) - 0.5);
}
fn path_a(w: vec2<f32>) -> f32 {
    if (w.y < 266.0) { return 0.0; }
    let pt = clamp((w.y - 266.0) / 210.0, 0.0, 1.0);
    let pw = 3.5 + 38.0 * pt * pt + 7.0 * pt;
    let pe = abs(w.x - path_x(w.y)) - pw * (0.80 + 0.30 * vnoise(w * 0.08) + 0.12 * vnoise(w * 0.5));
    return 1.0 - smoothstep(-2.0, 1.0, pe);
}
fn path_c(w: vec2<f32>, warm: f32, n: f32) -> vec3<f32> {
    let pt = clamp((w.y - 266.0) / 210.0, 0.0, 1.0);
    let pw = 3.5 + 38.0 * pt * pt + 7.0 * pt;
    let mid = 1.0 - clamp(abs(w.x - path_x(w.y)) / pw, 0.0, 1.0);
    var c = vec3<f32>(0.17, 0.14, 0.18) * (0.7 + 0.5 * n + 0.35 * mid) + vec3<f32>(0.95, 0.42, 0.08) * min(warm, 1.6) * 0.34;
    // pebbles: small stones, a lit top and a dark foot
    let pc = floor(w / 7.0);
    if (hash(pc + 8.8) > 0.84) {
        let pp = (pc + vec2<f32>(0.3 + 0.4 * hash(pc + 1.7), 0.3 + 0.4 * hash(pc + 2.6))) * 7.0;
        let psz = 0.6 + 0.9 * hash(pc + 3.9);
        let pq = (w - pp) / (vec2<f32>(1.9, 1.3) * psz);
        let pd = length(pq);
        let lit = clamp(-pq.y, 0.0, 1.0);
        c = mix(c, c * (0.6 + 0.9 * lit) + vec3<f32>(0.06, 0.05, 0.08) * lit, 1.0 - smoothstep(0.8, 1.1, pd));
    }
    return c;
}

fn vine_y(x: f32) -> f32 {
    return 318.0 + 6.0 * sin(x * 0.045) + 3.0 * sin(x * 0.12 + 1.0);
}
fn ridge_y(x: f32) -> f32 {
    return 232.0 + 11.0 * sin(x * 0.011 + 0.5) + 7.0 * sin(x * 0.029 + 2.0) + 7.0 * (fbm(vec2<f32>(x * 0.03, 3.0)) - 0.5);
}

// ---- the moon -------------------------------------------------------------------------------
const MOON: vec2<f32> = vec2<f32>(146.0, 138.0);
const MOON_R: f32 = 40.0;

// ---- the manor --------------------------------------------------------------------------------
fn sd_house(p: vec2<f32>) -> f32 {
    var d = sd_box(p, vec2<f32>(242.0, 242.0), vec2<f32>(34.0, 28.0));
    d = min(d, sd_tri(p, vec2<f32>(200.0, 216.0), vec2<f32>(240.0, 128.0), vec2<f32>(284.0, 216.0)));
    d = min(d, sd_box(p, vec2<f32>(264.0, 160.0), vec2<f32>(4.0, 18.0)));
    d = min(d, sd_box(p, vec2<f32>(264.0, 143.0), vec2<f32>(6.0, 2.0)));
    // a dormer on the left slope
    d = min(d, sd_box(p, vec2<f32>(222.0, 184.0), vec2<f32>(6.0, 9.0)));
    d = min(d, sd_tri(p, vec2<f32>(213.0, 177.0), vec2<f32>(222.0, 164.0), vec2<f32>(231.0, 177.0)));
    // the tower, leaning a little to the right
    let tp = vec2<f32>(p.x - (270.0 - p.y) * 0.035, p.y);
    d = min(d, sd_box(tp, vec2<f32>(318.0, 212.0), vec2<f32>(13.0, 58.0)));
    d = min(d, sd_box(tp, vec2<f32>(318.0, 155.0), vec2<f32>(16.0, 2.5)));
    d = min(d, sd_tri(tp, vec2<f32>(301.0, 154.0), vec2<f32>(318.0, 94.0), vec2<f32>(335.0, 154.0)));
    d = min(d, sd_cap(tp, vec2<f32>(318.0, 96.0), vec2<f32>(318.0, 84.0), 0.9, 0.6));
    // the wing that joins the tower
    d = min(d, sd_box(p, vec2<f32>(292.0, 250.0), vec2<f32>(16.0, 20.0)));
    d = min(d, sd_tri(p, vec2<f32>(274.0, 231.0), vec2<f32>(292.0, 214.0), vec2<f32>(310.0, 231.0)));
    // a porch roof and its posts
    d = min(d, sd_tri(p, vec2<f32>(226.0, 252.0), vec2<f32>(242.0, 242.0), vec2<f32>(258.0, 252.0)));
    return d;
}

// lit windows: x = distance (negative inside), y = which light (0..5)
fn house_win(p: vec2<f32>) -> vec4<f32> {
    var best = vec4<f32>(1e4, 0.0, 0.0, 0.0);
    var wins = array<vec4<f32>, 9>(
        vec4<f32>(222.0, 226.0, 5.0, 7.5),
        vec4<f32>(262.0, 226.0, 5.0, 7.5),
        vec4<f32>(222.0, 254.0, 5.0, 7.5),
        vec4<f32>(242.0, 196.0, 5.0, 5.0),
        vec4<f32>(222.0, 184.0, 3.0, 4.0),
        vec4<f32>(292.0, 252.0, 4.0, 6.0),
        vec4<f32>(318.0, 182.0, 3.5, 8.0),
        vec4<f32>(318.0, 222.0, 3.5, 8.0),
        vec4<f32>(242.0, 258.0, 6.0, 10.0)
    );
    for (var i = 0; i < 9; i++) {
        let wv = wins[i];
        var pp = p;
        if (i == 6 || i == 7) { pp = vec2<f32>(p.x - (270.0 - p.y) * 0.035, p.y); }
        var wd = 0.0;
        if (i == 3) {
            wd = length(pp - wv.xy) - wv.z;
        } else {
            // arched top
            let bx = sd_box(pp, wv.xy + vec2<f32>(0.0, wv.z * 0.5), vec2<f32>(wv.z, wv.w - wv.z * 0.5));
            let ar = length(pp - (wv.xy - vec2<f32>(0.0, wv.w - wv.z * 1.5))) - wv.z;
            wd = min(bx, ar);
        }
        if (wd < best.x) { best = vec4<f32>(wd, f32(i), (pp - wv.xy) / wv.zw); }
    }
    return best;
}

// ---- the graveyard ----------------------------------------------------------------------------
fn sd_graves(p: vec2<f32>) -> f32 {
    var st = array<vec4<f32>, 9>(
        vec4<f32>(348.0, 248.0, 8.0, 0.0),
        vec4<f32>(370.0, 236.0, 5.0, 1.0),
        vec4<f32>(394.0, 252.0, 9.0, 0.0),
        vec4<f32>(418.0, 226.0, 5.5, 2.0),
        vec4<f32>(442.0, 246.0, 10.0, 0.0),
        vec4<f32>(466.0, 238.0, 5.0, 1.0),
        vec4<f32>(160.0, 262.0, 7.0, 0.0),
        vec4<f32>(116.0, 262.0, 4.0, 1.0),
        vec4<f32>(136.0, 272.0, 3.5, 1.0)
    );
    var tilt = array<f32, 9>(-0.08, 0.13, 0.05, -0.03, 0.11, -0.16, 0.09, 0.22, -0.14);
    var d = 1e4;
    for (var i = 0; i < 9; i++) {
        let s = st[i];
        let lp = rot(p - vec2<f32>(s.x, 300.0), tilt[i]);
        let top = s.y - 300.0;
        var gd = 1e4;
        if (s.w < 0.5) {
            let r = s.z;
            gd = min(sd_box(lp, vec2<f32>(0.0, (top + r) * 0.5), vec2<f32>(r, -(top + r) * 0.5)), length(lp - vec2<f32>(0.0, top + r)) - r);
        } else if (s.w < 1.5) {
            gd = min(sd_box(lp, vec2<f32>(0.0, top * 0.5), vec2<f32>(1.9, -top * 0.5)), sd_box(lp, vec2<f32>(0.0, top + 7.0), vec2<f32>(s.z + 1.5, 1.7)));
            gd = min(gd, sd_box(lp, vec2<f32>(0.0, -6.0), vec2<f32>(6.0, 6.0)));
        } else {
            let hw = mix(s.z, s.z * 0.62, clamp(lp.y / top, 0.0, 1.0));
            gd = max(abs(lp.x) - hw, max(top - lp.y, lp.y));
            gd = min(gd, sd_tri(lp, vec2<f32>(-s.z * 0.62, top), vec2<f32>(0.0, top - 8.0), vec2<f32>(s.z * 0.62, top)));
            gd = min(gd, sd_box(lp, vec2<f32>(0.0, -4.0), vec2<f32>(s.z + 3.0, 4.0)));
        }
        d = min(d, gd);
    }
    return d;
}

fn sd_fence(p: vec2<f32>) -> f32 {
    if (p.x < 330.0) { return 1e4; }
    let topy = 254.0 + (p.x - 330.0) * 0.04;
    let bx = p.x - 330.0;
    let cell = bx - floor(bx / 7.0) * 7.0 - 3.5;
    let bar = max(abs(cell) - 0.55, max(topy - p.y, p.y - 300.0));
    let tip = sd_tri(vec2<f32>(cell, p.y), vec2<f32>(-1.6, topy + 0.5), vec2<f32>(0.0, topy - 4.5), vec2<f32>(1.6, topy + 0.5));
    let r1 = abs(p.y - (topy + 5.0)) - 0.7;
    let r2 = abs(p.y - (topy + 22.0)) - 0.7;
    return min(min(bar, tip), min(r1, r2));
}

// ---- the dead tree ----------------------------------------------------------------------------
fn sd_tree(p: vec2<f32>) -> f32 {
    var sg = array<vec4<f32>, 26>(
        vec4<f32>(38.0, 320.0, 48.0, 240.0),
        vec4<f32>(48.0, 240.0, 62.0, 186.0),
        vec4<f32>(62.0, 186.0, 100.0, 150.0),
        vec4<f32>(100.0, 150.0, 142.0, 126.0),
        vec4<f32>(142.0, 126.0, 180.0, 120.0),
        vec4<f32>(142.0, 126.0, 154.0, 92.0),
        vec4<f32>(100.0, 150.0, 114.0, 176.0),
        vec4<f32>(62.0, 186.0, 54.0, 120.0),
        vec4<f32>(54.0, 120.0, 80.0, 80.0),
        vec4<f32>(80.0, 80.0, 124.0, 54.0),
        vec4<f32>(80.0, 80.0, 84.0, 36.0),
        vec4<f32>(54.0, 120.0, 22.0, 94.0),
        vec4<f32>(48.0, 240.0, 12.0, 212.0),
        vec4<f32>(60.0, 206.0, 118.0, 206.0),
        vec4<f32>(118.0, 206.0, 176.0, 198.0),
        vec4<f32>(124.0, 54.0, 160.0, 46.0),
        vec4<f32>(114.0, 140.0, 128.0, 108.0),
        vec4<f32>(124.0, 54.0, 128.0, 18.0),
        vec4<f32>(180.0, 120.0, 195.0, 109.0),
        vec4<f32>(154.0, 92.0, 149.0, 72.0),
        vec4<f32>(152.0, 98.0, 170.0, 86.0),
        vec4<f32>(176.0, 198.0, 191.0, 187.0),
        vec4<f32>(160.0, 46.0, 177.0, 33.0),
        vec4<f32>(128.0, 18.0, 121.0, 3.0),
        vec4<f32>(22.0, 94.0, 7.0, 70.0),
        vec4<f32>(12.0, 212.0, 2.0, 189.0)
    );
    var rr = array<vec2<f32>, 26>(
        vec2<f32>(10.0, 8.0), vec2<f32>(8.0, 6.0), vec2<f32>(5.0, 3.2), vec2<f32>(3.2, 1.8),
        vec2<f32>(1.8, 0.6), vec2<f32>(1.5, 0.5), vec2<f32>(1.6, 0.5), vec2<f32>(4.4, 2.7),
        vec2<f32>(2.7, 1.5), vec2<f32>(1.5, 0.8), vec2<f32>(1.2, 0.5), vec2<f32>(2.0, 0.6),
        vec2<f32>(4.0, 1.2), vec2<f32>(3.4, 1.8), vec2<f32>(1.8, 0.6), vec2<f32>(0.8, 0.45),
        vec2<f32>(1.2, 0.5), vec2<f32>(0.8, 0.4),
        vec2<f32>(0.6, 0.3), vec2<f32>(0.55, 0.3), vec2<f32>(0.5, 0.25), vec2<f32>(0.6, 0.3),
        vec2<f32>(0.45, 0.25), vec2<f32>(0.4, 0.25), vec2<f32>(0.6, 0.3), vec2<f32>(1.2, 0.5)
    );
    // gnarl: the wood wanders a little
    let gp = p + vec2<f32>(vnoise(p * 0.09) - 0.5, vnoise(p * 0.09 + 31.0) - 0.5) * 3.0;
    var d = 1e4;
    for (var i = 0; i < 26; i++) {
        let s = sg[i];
        d = min(d, sd_cap(gp, s.xy, s.zw, rr[i].x, rr[i].y));
    }
    return d;
}

// ---- a bat: c its centre, s half its span, ph the wing beat ----------------------------------
fn sd_bat(p: vec2<f32>, c: vec2<f32>, s: f32, ph: f32) -> f32 {
    let q = (p - c) / s;
    let ax = abs(q.x);
    let fl = sin(ph);
    var d = (length(q / vec2<f32>(0.15, 0.28)) - 1.0) * 0.15;
    d = min(d, length(q - vec2<f32>(0.0, -0.28)) - 0.11);
    d = min(d, sd_tri(vec2<f32>(ax, q.y), vec2<f32>(0.02, -0.33), vec2<f32>(0.11, -0.33), vec2<f32>(0.09, -0.50)));
    let wx = clamp((ax - 0.06) / 0.94, 0.0, 1.0);
    let ytop = -0.10 - fl * 0.55 * wx * wx - 0.12 * wx;
    let thick = 0.40 * (1.0 - wx * wx) + 0.05;
    let scal = 0.11 * abs(sin(wx * 3.0 * PI)) * (1.0 - wx * 0.3);
    let ybot = ytop + thick - scal;
    let wd = max(max(ytop - q.y, q.y - ybot), ax - 1.0);
    d = min(d, wd * 0.7);
    return d * s;
}

// ---- the spider on its thread -----------------------------------------------------------------
fn sd_spider(p: vec2<f32>, c: vec2<f32>, kick: f32) -> f32 {
    let q = p - c;
    var d = (length((q - vec2<f32>(0.0, -3.0)) / vec2<f32>(5.5, 6.5)) - 1.0) * 5.5;
    d = min(d, length(q - vec2<f32>(0.0, 5.5)) - 3.4);
    let qs = vec2<f32>(abs(q.x), q.y);
    for (var j = 0; j < 4; j++) {
        let fj = f32(j);
        let root = vec2<f32>(1.8, 3.6 + fj * 1.3);
        let kn = vec2<f32>(8.5 + fj * 0.6, -4.0 + fj * 3.2 + kick * (fj - 1.5));
        let ft = vec2<f32>(12.5 + fj * 0.4, 3.0 + fj * 5.2);
        d = min(d, sd_cap(qs, root, kn, 0.95, 0.75));
        d = min(d, sd_cap(qs, kn, ft, 0.75, 0.4));
    }
    return d;
}

// ---- a jack-o'-lantern ------------------------------------------------------------------------
// carved holes in the unit face (q: -1..1 across), negative inside
fn face_sd(q: vec2<f32>, kind: f32) -> f32 {
    let mq = vec2<f32>(-abs(q.x), q.y);
    if (kind < 0.5) {
        let eye = sd_tri(mq, vec2<f32>(-0.54, -0.06), vec2<f32>(-0.36, -0.46), vec2<f32>(-0.14, -0.06));
        let nose = sd_tri(q, vec2<f32>(-0.09, 0.12), vec2<f32>(0.0, -0.08), vec2<f32>(0.09, 0.12));
        let tt = q.x / 0.66;
        let kk = max(1.0 - tt * tt, 0.0);
        let yu = 0.12 + 0.15 * kk;
        let yl = 0.16 + 0.44 * kk;
        var md = max(max(yu - q.y, q.y - yl), abs(q.x) - 0.66);
        md = max(md, -max(abs(q.x) - 0.07, q.y - (yu + 0.13)));
        md = max(md, -max(abs(abs(q.x) - 0.31) - 0.07, (yl - 0.13) - q.y));
        return min(min(eye, nose), md);
    } else if (kind < 1.5) {
        let eye = sd_tri(mq, vec2<f32>(-0.60, -0.40), vec2<f32>(-0.13, -0.14), vec2<f32>(-0.40, 0.02));
        let nose = sd_tri(q, vec2<f32>(-0.08, 0.06), vec2<f32>(0.08, 0.06), vec2<f32>(0.0, 0.18));
        let tt = q.x / 0.68;
        let kk = max(1.0 - tt * tt, 0.0);
        let saw = abs(fract(q.x * 3.6 + 0.5) - 0.5) * 2.0;
        let yu = 0.20 + 0.10 * kk + 0.12 * saw * kk;
        let yl = 0.30 + 0.30 * kk - 0.12 * (1.0 - saw) * kk;
        let md = max(max(yu - q.y, q.y - yl), abs(q.x) - 0.68);
        return min(min(eye, nose), md);
    } else if (kind < 2.5) {
        let eo = (length((mq - vec2<f32>(-0.32, -0.20)) / vec2<f32>(0.16, 0.19)) - 1.0) * 0.16;
        let pupil = length(mq - vec2<f32>(-0.29, -0.16)) - 0.055;
        let eye = max(eo, -pupil);
        let mo = (length((q - vec2<f32>(0.0, 0.34)) / vec2<f32>(0.20, 0.22)) - 1.0) * 0.2;
        return min(eye, mo);
    }
    return 1.0;
}

// one lobe of the skin; returns rgb and coverage
fn lobe(q: vec2<f32>, lc: vec2<f32>, lr: vec2<f32>, px_r: f32, fire: f32, hole_glow: f32) -> vec4<f32> {
    let lq = (q - lc) / lr;
    let ld = length(lq);
    let a = fill((ld - 1.0) * lr.x * px_r);
    if (a <= 0.0) { return vec4<f32>(0.0); }
    let nz = sqrt(max(1.0 - ld * ld, 0.0));
    let n = normalize(vec3<f32>(lq.x * 0.9, lq.y * 0.8, nz + 0.05));
    let lm = normalize(vec3<f32>(-0.55, -0.65, 0.55));
    let cool = max(dot(n, lm), 0.0);
    let warm_b = max(n.y, 0.0) * fire;
    let occ = pow(nz, 0.45);
    let albedo = vec3<f32>(0.92, 0.38, 0.06);
    var c = albedo * (0.10 + 0.36 * cool * vec3<f32>(0.75, 0.80, 1.05) + 0.42 * warm_b * vec3<f32>(1.0, 0.6, 0.3)) * (0.35 + 0.65 * occ);
    // moon sheen on the lobe's upper left
    let hv = normalize(lm + vec3<f32>(0.0, 0.0, 1.0));
    c += vec3<f32>(0.55, 0.55, 0.70) * pow(max(dot(n, hv), 0.0), 28.0) * 0.35;
    c += vec3<f32>(1.0, 0.42, 0.05) * hole_glow;
    return vec4<f32>(c, a);
}

fn pumpkin(p: vec2<f32>, c: vec2<f32>, rr: vec2<f32>, kind: f32, fire: f32) -> vec4<f32> {
    let q = (p - c) / rr;
    var out = vec4<f32>(0.0);
    // the stem first: the lobes overlap its foot
    let sa = c + vec2<f32>(0.0, -0.80 * rr.y);
    let sm = c + vec2<f32>(0.05 * rr.x, -1.05 * rr.y);
    let sb = c + vec2<f32>(0.20 * rr.x, -1.18 * rr.y);
    let sdv = min(sd_cap(p, sa, sm, 0.10 * rr.x, 0.075 * rr.x), sd_cap(p, sm, sb, 0.075 * rr.x, 0.05 * rr.x));
    let sta = fill(sdv);
    if (sta > 0.0) {
        let sx = clamp((p.x - mix(sa.x, sb.x, 0.5)) / (0.1 * rr.x), -1.0, 1.0);
        let sc = mix(vec3<f32>(0.10, 0.11, 0.04), vec3<f32>(0.30, 0.28, 0.12), 0.5 - 0.5 * sx) * (0.8 + 0.2 * sin(sx * 6.0));
        out = vec4<f32>(sc, sta);
    }
    // holes and their glow on the skin
    var hd = 1.0;
    if (kind < 2.9) { hd = face_sd(q, kind) * rr.x; }
    let hglow = select(exp(-max(hd, 0.0) / (0.10 * rr.x)) * 0.55 * fire, 0.0, kind > 2.9);
    // outer, side and centre lobes, back to front
    var lobes = array<vec4<f32>, 5>(
        vec4<f32>(-0.60, 0.04, 0.40, 0.84),
        vec4<f32>(0.60, 0.04, 0.40, 0.84),
        vec4<f32>(-0.32, 0.0, 0.42, 0.95),
        vec4<f32>(0.32, 0.0, 0.42, 0.95),
        vec4<f32>(0.0, -0.02, 0.40, 1.0)
    );
    for (var i = 0; i < 5; i++) {
        let lv = lobes[i];
        let lo = lobe(q, lv.xy, lv.zw, rr.x, fire, hglow);
        out = vec4<f32>(mix(out.rgb, lo.rgb, lo.a), max(out.a, lo.a));
    }
    if (kind < 2.9 && out.a > 0.0) {
        let hole = fill(hd);
        if (hole > 0.0) {
            let deep = clamp(-hd / (0.09 * rr.x), 0.0, 1.0);
            var inner = mix(vec3<f32>(1.0, 0.42, 0.04), vec3<f32>(1.0, 0.80, 0.36), deep) * fire;
            // the cut flesh on the lower edge, lit by the candle
            let below = face_sd(q + vec2<f32>(0.0, 0.07), kind) * rr.x;
            let wall = 1.0 - fill(below);
            inner = mix(inner, vec3<f32>(1.0, 0.86, 0.50) * fire, wall * 0.8);
            out = vec4<f32>(mix(out.rgb, inner, hole), out.a);
        }
    }
    return out;
}

@compute @workgroup_size(16, 16)
fn under(@builtin(global_invocation_id) gid: vec3<u32>) {
    if (gid.x >= f.size.x || gid.y >= f.size.y) { return; }
    let ix = i32(gid.x);
    let iy = i32(gid.y);
    let w = world(vec2<f32>(f32(gid.x) + 0.5, f32(gid.y) + 0.5));
    let base = px(ix, iy).rgb;
    let cv = cover(ix, iy);
    let t = f.time;

    // the press: a wave of candle light and a burst of bats
    var wave = 0.0;
    var page = -1.0;
    var pc = vec2<f32>(-999.0);
    let pr = f.aux[0];
    if (pr.w > 0.5) {
        let age0 = t - pr.z;
        if (age0 >= 0.0 && age0 < 3.0) {
            page = age0;
            pc = world(pr.xy);
            let dist = length(w - pc);
            wave = exp(-pow((dist - age0 * 240.0) / 46.0, 2.0)) * exp(-age0 * 1.1) * smoothstep(0.0, 0.08, age0);
        }
    }

    // ---- sky ----
    let dm = length(w - MOON);
    let sy = clamp(w.y / 290.0, 0.0, 1.0);
    var col = mix(vec3<f32>(0.10, 0.12, 0.30), vec3<f32>(0.21, 0.17, 0.43), smoothstep(0.0, 0.75, sy));
    col = mix(col, vec3<f32>(0.44, 0.23, 0.42), smoothstep(0.62, 1.0, sy));
    col += vec3<f32>(0.20, 0.17, 0.26) * exp(-dm / 130.0) * 0.8;
    // stars, slow to twinkle
    let cell = floor(w / 12.0);
    let sh = hash(cell + 7.3);
    if (sh > 0.80 && dm > MOON_R + 14.0) {
        let sp = (cell + vec2<f32>(0.25 + 0.5 * hash(cell + 1.1), 0.25 + 0.5 * hash(cell + 2.9))) * 12.0;
        let sz = 0.55 + 0.9 * hash(cell + 5.5);
        let tw = 0.65 + 0.35 * sin(t * (0.7 + 1.3 * hash(cell + 3.3)) + sh * 40.0);
        let sv = exp(-pow(length(w - sp) / sz, 2.0)) * tw * smoothstep(250.0, 140.0, w.y);
        let stc = mix(vec3<f32>(0.75, 0.82, 1.0), vec3<f32>(1.0, 0.85, 0.65), hash(cell + 9.1));
        col += stc * sv * 0.9;
    }
    // the moon
    let md = dm - MOON_R;
    col += vec3<f32>(0.62, 0.46, 0.32) * exp(-max(md, 0.0) / 20.0) * 0.32;
    let ma = fill(md);
    if (ma > 0.0) {
        let mq = (w - MOON) / MOON_R;
        let mr2 = dot(mq, mq);
        let mar = fbm(mq * 1.3 + vec2<f32>(3.0, 1.0));
        var mc = vec3<f32>(0.80, 0.65, 0.41) * (0.80 + 0.20 * sqrt(max(1.0 - mr2, 0.0)));
        mc *= 1.0 - 0.17 * smoothstep(0.42, 0.66, mar);
        // a few craters: a darker floor and a lit upper-left rim
        var crs = array<vec3<f32>, 5>(
            vec3<f32>(-0.38, -0.30, 0.11), vec3<f32>(0.30, 0.40, 0.16), vec3<f32>(0.50, -0.20, 0.07),
            vec3<f32>(-0.12, 0.52, 0.08), vec3<f32>(0.05, -0.05, 0.06)
        );
        for (var i = 0; i < 3; i++) {
            let cq = mq - crs[i].xy;
            let cr = crs[i].z;
            let cl = length(cq) * (1.0 + 0.25 * (vnoise(cq * 30.0 + f32(i) * 7.0) - 0.5));
            mc *= 1.0 - 0.07 * (1.0 - smoothstep(cr * 0.5, cr * 1.05, cl));
            let rimc = exp(-pow((cl - cr) / (cr * 0.18), 2.0)) * clamp(-(cq.x + cq.y) / (cl + 0.001), 0.0, 1.0);
            mc += vec3<f32>(0.035, 0.03, 0.02) * rimc;
        }
        mc = mix(mc, vec3<f32>(0.95, 0.66, 0.36), smoothstep(0.72, 1.0, sqrt(mr2)) * 0.45);
        col = mix(col, mc, ma);
    }
    // thin clouds drifting across, silver where the moon lights them
    let cn = fbm(vec2<f32>(w.x * 0.0055 - t * 0.008, w.y * 0.020));
    let cband = smoothstep(70.0, 120.0, w.y) * smoothstep(225.0, 175.0, w.y);
    let ca = smoothstep(0.47, 0.68, cn) * cband * 0.85;
    let lining = exp(-max(md, 0.0) / 40.0) * (1.0 - smoothstep(0.52, 0.72, cn));
    let cloud_c = vec3<f32>(0.16, 0.13, 0.29) + vec3<f32>(0.85, 0.70, 0.52) * lining;
    col = mix(col, cloud_c, ca);
    let cn2 = fbm(vec2<f32>(w.x * 0.004 + t * 0.005 + 40.0, w.y * 0.03));
    let ca2 = smoothstep(0.52, 0.72, cn2) * smoothstep(10.0, 40.0, w.y) * smoothstep(95.0, 55.0, w.y) * 0.6;
    col = mix(col, vec3<f32>(0.19, 0.17, 0.36), ca2);

    // ambient bats round the moon and the manor
    var batd = 1e4;
    for (var i = 0; i < 7; i++) {
        let fi = f32(i);
        let h1 = hash(vec2<f32>(fi, 3.0));
        let h2 = hash(vec2<f32>(fi, 7.0));
        let bp = vec2<f32>(250.0 + 135.0 * sin(t * (0.10 + 0.05 * h1) + fi * 2.9), 104.0 + 52.0 * sin(t * (0.17 + 0.07 * h2) + fi * 2.3) + 10.0 * sin(t * 0.9 + fi));
        let bs = 6.0 + 4.0 * h1;
        if (abs(w.x - bp.x) < bs + 2.0 && abs(w.y - bp.y) < bs + 2.0) {
            batd = min(batd, sd_bat(w, bp, bs, t * (6.5 + 2.0 * h2) + fi * 1.7));
        }
    }
    col = mix(col, vec3<f32>(0.05, 0.035, 0.07), fill(batd));

    // ---- far hills, hazy ----
    let ridge = ridge_y(w.x);
    let fh = fill(ridge - w.y);
    let hill_c = mix(vec3<f32>(0.24, 0.16, 0.33), vec3<f32>(0.14, 0.10, 0.22), smoothstep(ridge, ridge + 50.0, w.y));
    col = mix(col, hill_c + vec3<f32>(0.18, 0.16, 0.22) * exp(-(w.y - ridge) / 2.0) * step(ridge, w.y) * 0.6, fh);
    var rts = array<vec2<f32>, 4>(vec2<f32>(30.0, 17.0), vec2<f32>(128.0, 12.0), vec2<f32>(372.0, 14.0), vec2<f32>(440.0, 19.0));
    for (var k = 0; k < 4; k++) {
        let rx0 = rts[k].x;
        let rh = rts[k].y;
        if (abs(w.x - rx0) < rh * 0.7 && w.y > 180.0 && w.y < 262.0) {
            let rb = vec2<f32>(rx0, ridge_y(rx0) + 3.0);
            var rd = sd_cap(w, rb, rb + vec2<f32>(1.0, -rh), 1.2, 0.4);
            rd = min(rd, sd_cap(w, rb + vec2<f32>(0.4, -rh * 0.45), rb + vec2<f32>(-rh * 0.42, -rh * 0.85), 0.7, 0.3));
            rd = min(rd, sd_cap(w, rb + vec2<f32>(0.7, -rh * 0.62), rb + vec2<f32>(rh * 0.45, -rh * 0.98), 0.6, 0.3));
            rd = min(rd, sd_cap(w, rb + vec2<f32>(-rh * 0.25, -rh * 0.7), rb + vec2<f32>(-rh * 0.3, -rh * 1.05), 0.4, 0.25));
            col = mix(col, vec3<f32>(0.14, 0.10, 0.21), fill(rd) * 0.9);
        }
    }

    // ---- the manor on its knoll ----
    let hdist = sd_house(w);
    if (hdist < 30.0) {
        let lm2 = normalize(MOON - w);
        let rimd = sd_house(w + lm2 * 2.0);
        let rim = fill(hdist) * (1.0 - fill(rimd));
        // walls faintly moonlit with boards; roof slopes facing the moon lighter, with shingles
        let tpx = vec2<f32>(w.x - (270.0 - w.y) * 0.035, w.y);
        let roof_m = sd_tri(w, vec2<f32>(200.0, 216.0), vec2<f32>(240.0, 128.0), vec2<f32>(284.0, 216.0));
        let roof_t = sd_tri(tpx, vec2<f32>(301.0, 154.0), vec2<f32>(318.0, 94.0), vec2<f32>(335.0, 154.0));
        let roof_w = sd_tri(w, vec2<f32>(274.0, 231.0), vec2<f32>(292.0, 214.0), vec2<f32>(310.0, 231.0));
        let roof_d = sd_tri(w, vec2<f32>(213.0, 177.0), vec2<f32>(222.0, 164.0), vec2<f32>(231.0, 177.0));
        var hc = vec3<f32>(0.10, 0.085, 0.14) * (0.88 + 0.12 * step(0.5, fract(w.y / 3.5))) * (1.25 - 0.45 * clamp((w.x - 205.0) / 130.0, 0.0, 1.0)) * (1.0 - 0.4 * smoothstep(236.0, 270.0, w.y));
        var slope = 0.0;
        if (roof_m < 0.0) { slope = select(0.35, 1.0, w.x < 240.0 + (w.y - 128.0) * 0.05); }
        if (roof_t < 0.0) { slope = select(0.35, 1.0, tpx.x < 318.0); }
        if (roof_w < 0.0) { slope = select(0.35, 1.0, w.x < 292.0); }
        if (roof_d < 0.0) { slope = select(0.35, 1.0, w.x < 222.0); }
        if (slope > 0.0) {
            let row = floor(w.y / 3.6);
            let sxc = w.x / 5.0 + row * 0.5;
            let sgl = (0.72 + 0.28 * smoothstep(0.0, 0.3, fract(w.y / 3.6))) * (0.8 + 0.2 * smoothstep(0.0, 0.15, fract(sxc))) * (0.9 + 0.2 * hash(vec2<f32>(floor(sxc), row)));
            hc = mix(vec3<f32>(0.10, 0.085, 0.15), vec3<f32>(0.20, 0.17, 0.28), slope) * sgl;
        }
        hc += vec3<f32>(0.62, 0.56, 0.72) * rim * 0.45;
        let wv = house_win(w);
        let wn = i32(wv.y);
        let breathe = 0.85 + 0.15 * sin(t * (0.6 + 0.13 * wv.y) + wv.y * 2.3);
        var lit = 1.0;
        if (wn == 2 || wn == 4 || wn == 8) { lit = 0.0; }
        // a figure walks past the attic window now and then
        var ghost = 1.0;
        if (wn == 3) {
            let gx = 242.0 + 14.0 * sin(t * 0.35);
            ghost = 1.0 - 0.85 * (1.0 - smoothstep(2.0, 3.5, abs(w.x - gx))) * step(w.y, 199.5);
            ghost = min(ghost, 1.0 - 0.85 * (1.0 - smoothstep(2.6, 3.4, length(w - vec2<f32>(gx, 192.5)))));
        }
        let flare = 1.0 + 1.5 * wave;
        // the room's light: bright and warm low, deeper at the top; mullions in a cross
        let wgrad = clamp(0.5 + 0.5 * wv.w, 0.0, 1.0);
        let wl = mix(vec3<f32>(0.95, 0.44, 0.12), vec3<f32>(1.0, 0.76, 0.36), wgrad) * breathe * lit * ghost * flare;
        var mull = 1.0;
        if (wn != 3 && wn != 4) {
            let mx = 1.0 - smoothstep(0.10, 0.22, abs(wv.z));
            let my = 1.0 - smoothstep(0.06, 0.14, abs(wv.w - 0.05));
            mull = 1.0 - 0.85 * max(mx, my);
        }
        let wa = fill(wv.x);
        var win_c = select(wl, vec3<f32>(0.07, 0.055, 0.08), lit < 0.5) * mull;
        if (wn == 2) {
            // boarded up: two pale planks across dark glass
            let pl1 = abs(wv.w * 0.75 + wv.z * 0.55 + 0.1) - 0.17;
            let pl2 = abs(wv.w * 0.75 - wv.z * 0.55 - 0.35) - 0.15;
            win_c = mix(vec3<f32>(0.07, 0.055, 0.08), vec3<f32>(0.24, 0.19, 0.21) * (0.85 + 0.15 * wv.z), 1.0 - smoothstep(-0.02, 0.06, min(pl1, pl2)));
        }
        if (wn == 8) {
            // the door, ajar: dark boards and a line of warm light down one side
            let crack = 1.0 - smoothstep(0.06, 0.20, abs(wv.z - 0.55));
            win_c = mix(vec3<f32>(0.13, 0.08, 0.07) * (0.85 + 0.15 * step(0.5, fract(wv.z * 2.0 + 0.5))), vec3<f32>(1.0, 0.70, 0.30) * breathe * flare, crack * step(-0.95, wv.w));
        }
        hc = mix(hc, win_c, wa * fill(hdist + 0.5));
        // the window light spills on the wall
        hc += vec3<f32>(0.55, 0.30, 0.10) * exp(-max(wv.x, 0.0) / 4.0) * 0.35 * lit * (1.0 - wa);
        col = mix(col, hc, fill(hdist));
        // glow round lit windows in the air
        col += vec3<f32>(0.45, 0.25, 0.08) * exp(-max(wv.x, 0.0) / 7.0) * 0.18 * lit * (1.0 - fill(hdist));
    }
    let gbx = floor(w.x / 2.2);
    let gblade = (2.0 + 3.5 * hash(vec2<f32>(gbx, 5.0))) * (1.0 - abs(fract(w.x / 2.2) - 0.5) * 2.0);
    let knoll = 266.0 + 0.0017 * (w.x - 245.0) * (w.x - 245.0) + 6.0 * (fbm(vec2<f32>(w.x * 0.02, 9.0)) - 0.5) - gblade * smoothstep(0.35, 0.6, vnoise(vec2<f32>(w.x * 0.08, 3.0)));
    let kn_a = fill(knoll - w.y);
    let kn_n = fbm(w * vec2<f32>(0.04, 0.09));
    var kn_c = vec3<f32>(0.07, 0.055, 0.10) * (0.8 + 0.4 * kn_n);
    kn_c += vec3<f32>(0.34, 0.31, 0.46) * (1.0 - fill(knoll + 1.8 - w.y)) * 0.40 * smoothstep(0.3, 0.6, vnoise(vec2<f32>(w.x * 0.05, 1.0)));
    col = mix(col, kn_c, kn_a);
    col = mix(col, path_c(w, 0.0, kn_n), path_a(w) * kn_a);

    // ---- graveyard: fence behind, stones in front, moonlit edges ----
    if (w.x > 104.0 && w.y > 200.0 && w.y < 310.0) {
        let fd = sd_fence(w);
        col = mix(col, vec3<f32>(0.05, 0.04, 0.07), fill(fd) * 0.95);
        let gd = sd_graves(w);
        if (gd < 4.0) {
            let lm3 = normalize(MOON - w);
            let gr = fill(gd) * (1.0 - fill(sd_graves(w + lm3 * 1.8)));
            let stone = vec3<f32>(0.13, 0.12, 0.17) * (0.8 + 0.4 * vnoise(w * 0.4)) + vec3<f32>(0.62, 0.60, 0.75) * gr * 0.6;
            col = mix(col, stone, fill(gd));
        }
    }

    // ---- low fog among the graves and the manor's foot ----
    let fn1 = fbm(vec2<f32>(w.x * 0.010 - t * 0.020, w.y * 0.045 + t * 0.006));
    let fa1 = smoothstep(0.38, 0.80, fn1) * smoothstep(232.0, 262.0, w.y) * smoothstep(312.0, 286.0, w.y) * 0.55;
    col = mix(col, vec3<f32>(0.40, 0.36, 0.56) + vec3<f32>(0.35, 0.18, 0.05) * wave, fa1);

    // ---- the dead tree, black against the moon ----
    if (w.x < 200.0 && w.y < 330.0) {
        let td = sd_tree(w);
        let tl_dir = normalize(MOON - w + vec2<f32>(0.0, 0.001));
        let trim = fill(td) * (1.0 - fill(sd_tree(w + tl_dir * 1.6))) * smoothstep(-1.0, 2.0, -td + 2.0);
        let wood = vec3<f32>(0.035, 0.03, 0.05) * (0.8 + 0.4 * vnoise(w * vec2<f32>(0.5, 0.08))) + vec3<f32>(0.40, 0.34, 0.36) * trim * 0.40;
        col = mix(col, wood, fill(td));
        // the spider on its thread from the low branch
        let spy = 236.0 + 9.0 * sin(t * 0.55);
        let spx = 168.0 + 1.5 * sin(t * 0.8);
        let thread = abs(w.x - mix(168.0, spx, clamp((w.y - 200.0) / (spy - 200.0), 0.0, 1.0))) - 0.35;
        let th_a = (1.0 - smoothstep(0.0, 0.8, thread)) * step(200.0, w.y) * step(w.y, spy - 6.0);
        col = mix(col, vec3<f32>(0.75, 0.75, 0.85), th_a * 0.55);
        let spd = sd_spider(w, vec2<f32>(spx, spy), 1.2 * sin(t * 1.7));
        col = mix(col, vec3<f32>(0.03, 0.02, 0.04), fill(spd));
        // a moon glint on its back and two red eyes
        col += vec3<f32>(0.6, 0.6, 0.75) * exp(-pow(length(w - vec2<f32>(spx - 2.0, spy - 6.0)) / 1.2, 2.0)) * 0.6;
        let ey = vec2<f32>(abs(w.x - spx), w.y - (spy + 6.5));
        col += vec3<f32>(1.0, 0.12, 0.05) * exp(-pow(length(ey - vec2<f32>(1.3, 0.0)) / 0.8, 2.0)) * 0.9;
    }

    // ---- the pumpkin patch ----
    var pks = array<vec4<f32>, 8>(
        vec4<f32>(430.0, 344.0, 30.0, 24.0),
        vec4<f32>(50.0, 348.0, 30.0, 24.0),
        vec4<f32>(330.0, 350.0, 22.0, 18.0),
        vec4<f32>(150.0, 352.0, 21.0, 17.0),
        vec4<f32>(224.0, 366.0, 13.0, 11.0),
        vec4<f32>(240.0, 440.0, 46.0, 36.0),
        vec4<f32>(342.0, 448.0, 30.0, 23.0),
        vec4<f32>(140.0, 452.0, 26.0, 20.0)
    );
    var kinds = array<f32, 8>(1.0, 0.0, 2.0, 1.0, 3.0, 0.0, 1.0, 2.0);
    // the warm light each lantern throws on the ground and the fog
    var warm = 0.0;
    var fires = array<f32, 8>(0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
    for (var i = 0; i < 8; i++) {
        let pk = pks[i];
        let fi = f32(i);
        var bl = 0.0;
        if (page >= 0.0) {
            bl = exp(-pow((length(pk.xy - pc) - page * 240.0) / 60.0, 2.0)) * exp(-page * 0.9);
        }
        let fr = (0.86 + 0.08 * sin(t * (0.9 + 0.2 * fi) + fi * 1.9) + 0.05 * sin(t * 1.7 + fi)) * (1.0 + 1.3 * bl);
        fires[i] = fr;
        if (kinds[i] < 2.9) {
            let gq = (w - pk.xy - vec2<f32>(0.0, pk.w * 0.4)) * vec2<f32>(1.0, 1.7);
            warm += fr * exp(-length(gq) / (pk.z * 1.25));
            let fq = (w - vec2<f32>(pk.x, pk.y + pk.w * 1.2)) / vec2<f32>(pk.z * 1.5, pk.w * 0.45);
            warm += fr * 0.9 * exp(-dot(fq, fq));
        }
    }

    // the foreground ground
    let gy = 298.0 + 5.0 * sin(w.x * 0.021 + 0.7);
    if (w.y > gy - 2.0) {
        let soil_n = fbm(w * vec2<f32>(0.05, 0.12));
        var gc = vec3<f32>(0.08, 0.06, 0.09) * (0.7 + 0.6 * soil_n);
        gc += vec3<f32>(0.95, 0.40, 0.07) * min(warm, 1.6) * (0.20 + 0.40 * soil_n * soil_n);
        gc += vec3<f32>(0.10, 0.10, 0.16) * smoothstep(gy, gy + 30.0, w.y) * 0.22;
        col = mix(col, gc, fill(gy - w.y));
        col = mix(col, path_c(w, warm, soil_n), path_a(w) * fill(gy - w.y));
        // the vine behind the lanterns: in pieces, tapering, curled tendrils, notched leaves
        let vy = vine_y(w.x);
        let vth = 1.4 * smoothstep(0.32, 0.58, vnoise(vec2<f32>(w.x * 0.02, 4.0)));
        let vc = vec3<f32>(0.10, 0.16, 0.05) + vec3<f32>(0.55, 0.32, 0.05) * warm * 0.5;
        if (vth > 0.05) {
            col = mix(col, vc, fill(abs(w.y - vy) - vth));
        }
        let tx = floor(w.x / 27.0) * 27.0 + 13.0;
        if (vnoise(vec2<f32>(tx * 0.02, 4.0)) > 0.5) {
            let tq = w - vec2<f32>(tx, vine_y(tx) + 5.0);
            let tr = length(tq);
            let tang = atan2(tq.y, tq.x);
            let sr = 1.0 + 3.6 * fract((tang + PI) / (2.0 * PI) + 0.25);
            col = mix(col, vc, fill(abs(tr - sr) - 0.4) * step(tr, 5.2));
        }
        let lx = floor(w.x / 33.0) * 33.0 + 16.0;
        if (vnoise(vec2<f32>(lx * 0.02, 4.0)) > 0.4) {
            let side = select(-1.0, 1.0, hash(vec2<f32>(lx, 2.0)) > 0.5);
            let lsz = 5.0 + 3.5 * hash(vec2<f32>(lx, 6.0));
            let lp = rot(w - vec2<f32>(lx, vine_y(lx) - lsz * 0.8 * side), 0.5 * side + 0.3 * (hash(vec2<f32>(lx, 8.0)) - 0.5));
            let lang = atan2(lp.y, lp.x);
            let leaf = (length(lp / vec2<f32>(lsz, lsz * 0.62)) - 1.0 + 0.10 * abs(sin(lang * 2.5))) * lsz * 0.62;
            let vein = 1.0 - smoothstep(0.2, 0.7, abs(lp.y));
            let leaf_c = vec3<f32>(0.08, 0.16, 0.05) * (0.75 + 0.45 * smoothstep(-lsz, lsz, -lp.y) + 0.25 * vein) + vec3<f32>(0.5, 0.3, 0.05) * warm * 0.4;
            col = mix(col, leaf_c, fill(leaf));
        }
        // fallen leaves on the soil, seen where the lanterns light them
        let lc = floor(w / vec2<f32>(9.0, 7.0));
        if (hash(lc + 0.71) > 0.45 && vnoise(lc * 0.35 + 3.0) > 0.52) {
            let lpos = (lc + vec2<f32>(0.3 + 0.4 * hash(lc + 2.2), 0.3 + 0.4 * hash(lc + 3.3))) * vec2<f32>(9.0, 7.0);
            let lq2 = rot(w - lpos, hash(lc + 4.4) * 6.283);
            let lfd = (length(lq2 / vec2<f32>(3.0, 1.6)) - 1.0) * 1.6;
            let lh = hash(lc + 5.5);
            var lcol = mix(vec3<f32>(0.62, 0.20, 0.04), vec3<f32>(0.70, 0.45, 0.08), smoothstep(0.3, 0.7, lh));
            lcol = mix(lcol, vec3<f32>(0.40, 0.09, 0.05), step(0.8, lh));
            col = mix(col, lcol * (0.03 + 0.85 * min(warm, 1.4)), fill(lfd) * 0.9 * smoothstep(0.08, 0.35, warm));
        }
        // grass tufts, dark against the lit soil: denser and taller in the near ground
        if (w.y > gy + 10.0) {
            let tsz = vec2<f32>(15.0, 11.0);
            let tc = floor(w / tsz);
            let near = smoothstep(380.0, 470.0, w.y);
            let tb = tc * tsz + vec2<f32>(3.0 + 9.0 * hash(tc + 4.1), 10.5);
            var tnear = 0.0;
            for (var k = 0; k < 8; k++) {
                let kq = (tb - pks[k].xy - vec2<f32>(0.0, pks[k].w * 0.8)) / (pks[k].zw * vec2<f32>(1.6, 1.0));
                tnear = max(tnear, exp(-dot(kq, kq)));
            }
            if (hash(tc + 0.37) < 0.06 + 0.5 * near + 0.75 * tnear && path_a(tb) < 0.3) {
                var tdd = 1e4;
                for (var j = 0; j < 6; j++) {
                    let fj = f32(j);
                    let ta = (fj - 2.5) * 0.26 + (hash(tc + fj) - 0.5) * 0.35 + 0.07 * sin(t * 0.7 + tc.x * 0.7);
                    let tl = (7.0 + 8.0 * hash(tc + fj * 3.7)) * (0.8 + 0.6 * near);
                    let tm = tb + vec2<f32>(sin(ta), -cos(ta)) * tl * 0.55;
                    let te = tm + vec2<f32>(sin(ta * 1.6), -cos(ta * 1.6)) * tl * 0.45;
                    tdd = min(tdd, min(sd_cap(w, tb, tm, 0.9, 0.5), sd_cap(w, tm, te, 0.5, 0.1)));
                }
                col = mix(col, vec3<f32>(0.035, 0.04, 0.035) + vec3<f32>(0.22, 0.09, 0.02) * min(warm, 1.5), fill(tdd) * 0.92);
            }
        }
        // the dark contact shadows and the lanterns
        for (var i = 0; i < 8; i++) {
            let pk = pks[i];
            let sq = (w - vec2<f32>(pk.x, pk.y + pk.w * 0.92)) / vec2<f32>(pk.z * 0.95, pk.w * 0.22);
            col *= 1.0 - 0.6 * exp(-dot(sq, sq));
        }
        for (var i = 0; i < 8; i++) {
            let pk = pks[i];
            if (abs(w.x - pk.x) < pk.z + 3.0 && w.y > pk.y - pk.w * 1.25 && w.y < pk.y + pk.w + 3.0) {
                let pkc = pumpkin(w, pk.xy, pk.zw, kinds[i], fires[i]);
                col = mix(col, pkc.rgb, pkc.a);
            }
        }
    }

    // ---- ground fog in front, lit by the lanterns ----
    let fwarp = vnoise(w * 0.012 + vec2<f32>(t * 0.03, 0.0));
    let fn2 = fbm(vec2<f32>(w.x * 0.009 + t * 0.016 + 1.2 * fwarp, w.y * 0.045 - t * 0.004));
    let fa2 = smoothstep(0.48, 0.78, fn2) * smoothstep(320.0, 400.0, w.y) * 0.46;
    let fog_c = vec3<f32>(0.32, 0.30, 0.46) + vec3<f32>(0.95, 0.45, 0.10) * min(warm, 1.5) * 0.45;
    col = mix(col, fog_c, fa2);
    // the lantern light hanging in the air, and a halo round each lit lantern
    col += vec3<f32>(0.50, 0.20, 0.03) * min(warm, 2.0) * 0.08;
    for (var i = 0; i < 8; i++) {
        if (kinds[i] < 2.9) {
            let hq = (w - pks[i].xy - vec2<f32>(0.0, pks[i].w * 0.1)) / pks[i].zw;
            let hl = length(hq);
            col += vec3<f32>(1.0, 0.46, 0.08) * fires[i] * 0.16 * exp(-max(hl - 0.55, 0.0) * 3.2) * (0.4 + 0.6 * smoothstep(0.7, 1.05, hl));
        }
    }

    // ---- the press: the light wave through the fog, and the bats it frees ----
    if (page >= 0.0) {
        col += vec3<f32>(1.0, 0.52, 0.16) * wave * (0.26 + 0.7 * (fa1 + fa2));
        col = mix(col, col * vec3<f32>(1.25, 1.05, 0.85), wave);
        var pbd = 1e4;
        for (var i = 0; i < 12; i++) {
            let fi = f32(i);
            let ang = fi * 0.5236 + hash(vec2<f32>(fi, 11.0)) * 0.5;
            let spd = 120.0 + 80.0 * hash(vec2<f32>(fi, 13.0));
            let dir = vec2<f32>(cos(ang), sin(ang));
            let bp = pc + dir * spd * page * (1.0 - 0.12 * page) + vec2<f32>(0.0, -30.0 * page * page);
            let bs = 5.0 + 6.0 * clamp(page * 2.0, 0.0, 1.0) * (0.7 + 0.3 * hash(vec2<f32>(fi, 17.0)));
            if (abs(w.x - bp.x) < bs + 2.0 && abs(w.y - bp.y) < bs + 2.0) {
                pbd = min(pbd, sd_bat(w, bp, bs, page * 22.0 + fi * 1.3));
            }
        }
        let pfade = 1.0 - smoothstep(1.6, 2.4, page);
        col = mix(col, vec3<f32>(0.04, 0.03, 0.06), fill(pbd) * pfade);
    }

    // ---- the icons: a soft shade round them so they read on the moon and the lights ----
    let shd = (cover(ix + 3, iy + 3) + cover(ix - 3, iy + 3) + cover(ix + 3, iy - 3) + cover(ix - 3, iy - 3)
        + cover(ix + 5, iy) + cover(ix - 5, iy) + cover(ix, iy + 5) + cover(ix, iy - 5)
        + cover(ix + 6, iy + 6) + cover(ix - 6, iy + 6) + cover(ix + 6, iy - 6) + cover(ix - 6, iy - 6)) / 12.0;
    col *= 1.0 - 0.72 * smoothstep(0.0, 0.6, shd) * (1.0 - cv);
    col *= 1.0 - 0.2 * cv;
    col = knee(col, mix(0.85, 0.45, cv));
    col = mix(col, base, keep_bg(base));
    put(gid.xy, col);
}
