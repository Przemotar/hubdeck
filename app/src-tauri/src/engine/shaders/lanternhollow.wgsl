// Lantern hollow: a misty hollow under a low amber moon. A pumpkin patch burns by a little
// graveyard, an owl keeps watch in the dead tree, a black cat sits in the fog, bats wheel round the
// moon, will-o'-the-wisps drift, leaves fall, and now and then a witch crosses the moon. A press is a
// gust: every candle gutters and roars back with smoke and sparks, the leaves blow, the owl stares.

fn aa(d: f32) -> f32 {
    return clamp(0.5 - d, 0.0, 1.0);
}
fn aa_w(d: f32, w: f32) -> f32 {
    return clamp(0.5 - d / max(w, 0.0001), 0.0, 1.0);
}
fn hex(v: u32) -> vec3<f32> {
    return vec3<f32>(f32((v >> 16u) & 255u), f32((v >> 8u) & 255u), f32(v & 255u)) / 255.0;
}
fn luma(c: vec3<f32>) -> f32 {
    return dot(c, vec3<f32>(0.299, 0.587, 0.114));
}
fn fbm_n(p: vec2<f32>, oct: i32) -> f32 {
    var v = 0.0;
    var a = 0.5;
    var q = p;
    let rot = mat2x2<f32>(0.8, -0.6, 0.6, 0.8);
    for (var i = 0; i < 7; i++) {
        if (i >= oct) { break; }
        v += a * vnoise(q);
        q = rot * q * 2.02 + vec2<f32>(17.3, 9.1);
        a *= 0.5;
    }
    return v;
}
fn sd_ell(q: vec2<f32>, r: vec2<f32>) -> f32 {
    let k0 = length(q / r);
    let k1 = length(q / (r * r));
    return k0 * (k0 - 1.0) / max(k1, 0.0001);
}
fn sd_triangle(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>, c: vec2<f32>) -> f32 {
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
    let mn = min(min(d0, d1), d2);
    return -sqrt(mn.x) * sign(mn.y);
}
fn sd_segment(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>) -> f32 {
    let pa = p - a;
    let ba = b - a;
    let h = clamp(dot(pa, ba) / dot(ba, ba), 0.0, 1.0);
    return length(pa - ba * h);
}
fn sd_taper(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>, ta: f32, tb: f32) -> f32 {
    let pa = p - a;
    let ba = b - a;
    let h = clamp(dot(pa, ba) / dot(ba, ba), 0.0, 1.0);
    return length(pa - ba * h) - mix(ta, tb, h);
}
fn sd_bez(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>, c: vec2<f32>) -> f32 {
    var d = 1e9;
    var prev = a;
    for (var i = 1; i <= 8; i++) {
        let s = f32(i) / 8.0;
        let pt = mix(mix(a, b, s), mix(b, c, s), s);
        d = min(d, sd_segment(p, prev, pt));
        prev = pt;
    }
    return d;
}
fn sd_box_s(p: vec2<f32>, h: vec2<f32>) -> f32 {
    let d = abs(p) - h;
    return length(max(d, vec2<f32>(0.0))) + min(max(d.x, d.y), 0.0);
}
fn glow(d: f32, r: f32) -> f32 {
    return exp(-max(d, 0.0) / max(r, 0.0001));
}
fn smin(a: f32, b: f32, k: f32) -> f32 {
    let h = clamp(0.5 + 0.5 * (b - a) / k, 0.0, 1.0);
    return mix(b, a, h) - k * h * (1.0 - h);
}
fn rot2(v: vec2<f32>, a: f32) -> vec2<f32> {
    let c = cos(a);
    let s = sin(a);
    return vec2<f32>(c * v.x - s * v.y, s * v.x + c * v.y);
}

// ---- pumpkins: a body of five lobes in the hero's unit frame (y down, about 136 x 114)
fn sd_pumpkin(u: vec2<f32>) -> f32 {
    var d = sd_ell(u - vec2<f32>(0.0, 2.0), vec2<f32>(36.0, 56.0));
    d = smin(d, sd_ell(u - vec2<f32>(-26.0, 3.0), vec2<f32>(36.0, 53.0)), 4.0);
    d = smin(d, sd_ell(u - vec2<f32>(26.0, 3.0), vec2<f32>(36.0, 53.0)), 4.0);
    d = smin(d, sd_ell(u - vec2<f32>(-44.0, 6.0), vec2<f32>(24.0, 46.0)), 4.0);
    d = smin(d, sd_ell(u - vec2<f32>(44.0, 6.0), vec2<f32>(24.0, 46.0)), 4.0);
    return d;
}
// a grin between two arcs: the crescent smile, corners up
fn sd_smile(u: vec2<f32>) -> f32 {
    let a = length(u - vec2<f32>(0.0, -6.0)) - 46.0;
    let b = length(u - vec2<f32>(0.0, -20.0)) - 50.0;
    return max(max(a, -b), abs(u.x) - 36.0);
}
// the carved face, five characters (negative inside the openings), same unit frame
fn sd_face(u: vec2<f32>, style: i32) -> f32 {
    let ax = abs(u.x);
    let um = vec2<f32>(ax, u.y);
    var d = 1e9;
    switch style {
        case 0: {
            // the classic: triangle eyes, a small nose, a wide grin with two square teeth
            let eye = sd_triangle(um, vec2<f32>(8.0, -20.0), vec2<f32>(31.0, -18.0), vec2<f32>(22.0, -41.0));
            let nose = sd_triangle(u, vec2<f32>(-5.5, 17.0), vec2<f32>(5.5, 17.0), vec2<f32>(0.0, 7.0));
            let top = 22.0 - 0.0045 * ax * ax;
            let bot = 46.0 - 0.011 * ax * ax;
            var m = max(max(top - u.y, u.y - bot), ax - 40.0);
            let t1 = sd_box_s(vec2<f32>(ax - 15.0, u.y - 21.0), vec2<f32>(4.5, 5.0));
            let t2 = sd_box_s(vec2<f32>(u.x, u.y - 44.0), vec2<f32>(5.0, 5.5));
            m = max(m, -t1);
            m = max(m, -t2);
            d = min(min(eye, nose), m);
        }
        case 1: {
            // the scared one: round eyes and an O of a mouth
            let eye = sd_ell(um - vec2<f32>(18.0, -24.0), vec2<f32>(10.0, 12.0));
            let nose = sd_triangle(u, vec2<f32>(-4.0, 6.0), vec2<f32>(4.0, 6.0), vec2<f32>(0.0, -1.0));
            let mo = sd_ell(u - vec2<f32>(0.0, 29.0), vec2<f32>(12.0, 16.0));
            d = min(min(eye, nose), mo);
        }
        case 2: {
            // the happy one: closed crescent eyes, a crescent grin with two teeth
            let eye = max(length(um - vec2<f32>(18.0, -20.0)) - 12.0, -(length(um - vec2<f32>(18.0, -12.0)) - 11.0));
            var m = sd_smile(u);
            let tt = sd_box_s(vec2<f32>(ax - 12.0, u.y - 26.0), vec2<f32>(4.0, 5.0));
            m = max(m, -tt);
            d = min(eye, m);
        }
        case 3: {
            // the angry one: slanted eyes, a jagged mouth of pointed teeth
            let eye = sd_triangle(um, vec2<f32>(6.0, -19.0), vec2<f32>(32.0, -36.0), vec2<f32>(28.0, -15.0));
            let zt = abs(fract(u.x / 11.0) - 0.5) * 2.0;
            let zb = abs(fract(u.x / 11.0 + 0.5) - 0.5) * 2.0;
            let top = 18.0 + 0.006 * ax * ax + 8.0 * zt;
            let bot = 42.0 - 0.004 * ax * ax - 8.0 * zb;
            let m = max(max(top - u.y, u.y - bot) / 1.6, ax - 35.0);
            d = min(eye, m);
        }
        default: {
            // the wink: one triangle eye, one shut in an arc, a crooked smile
            let el = sd_triangle(u, vec2<f32>(-8.0, -20.0), vec2<f32>(-31.0, -18.0), vec2<f32>(-22.0, -41.0));
            let er = max(abs(length(u - vec2<f32>(19.0, -15.0)) - 11.0) - 3.0, u.y + 18.0);
            let ur = rot2(u - vec2<f32>(5.0, 2.0), -0.16);
            var m = sd_smile(ur);
            let tt = sd_box_s(vec2<f32>(ur.x + 10.0, ur.y - 27.0), vec2<f32>(4.0, 5.0));
            m = max(m, -tt);
            d = min(min(el, er), m);
        }
    }
    return d;
}
fn sd_stem(u: vec2<f32>) -> f32 {
    return min(sd_taper(u, vec2<f32>(1.0, -47.0), vec2<f32>(3.0, -58.0), 6.0, 4.6), sd_taper(u, vec2<f32>(3.0, -58.0), vec2<f32>(9.0, -64.0), 4.6, 3.4));
}

// one lantern: c its centre, s its size (1 = the hero), asp its width, style its face, rind its
// skin (0 deep, 1 bright orange, 2 a pale ghost pumpkin), flk its flicker, gk its gust
fn draw_pumpkin(col_in: vec3<f32>, p: vec2<f32>, c: vec2<f32>, s: f32, asp: f32, style: i32, rind: i32, flk: f32, gk: f32, t: f32, tm: vec2<f32>, k: f32) -> vec3<f32> {
    let q = p - c;
    if (abs(q.x) > 76.0 * s * asp || q.y < -72.0 * s || q.y > 62.0 * s) { return col_in; }
    let warm = hex(0xFF8A1Eu);
    let core = hex(0xFFE9A8u);
    let deep = hex(0xB83A0Cu);
    let fogc = hex(0x6F5C93u);
    let sc = vec2<f32>(s * asp, s);
    let u = q / sc;
    let dk = s * min(asp, 1.0);
    let body = sd_pumpkin(u) * dk;
    var col = col_in;
    if (body < 1.0) {
        // the face is cut by hand: its edges wander a little
        let wob = (vec2<f32>(vnoise(u * 0.07 + vec2<f32>(k * 7.0, 0.0)), vnoise(u * 0.07 + vec2<f32>(31.0, k * 7.0))) - 0.5) * 1.6;
        let face = sd_face(u + wob, style) * dk;
        // ribs: lobes that bend with the body, round in the middle, dark in the grooves
        let bend = sqrt(max(1.0 - pow(clamp(abs(u.y - 4.0) / 60.0, 0.0, 1.0), 2.0), 0.15));
        let ribx = u.x / bend;
        let rib = ribx / 72.0;
        let lobe = 0.5 + 0.5 * cos(ribx * 6.28318 / 24.0);
        let groove = smoothstep(0.16, 0.0, lobe);
        let rnd = sqrt(max(1.0 - rib * rib, 0.0));
        var lo = hex(0x1E0A03u);
        var hi = hex(0x461A07u);
        var gc = hex(0x8A3A0Eu);
        if (rind == 1) {
            lo = hex(0x280C02u);
            hi = hex(0x5E2408u);
            gc = hex(0x9A420Eu);
        }
        if (rind == 2) {
            lo = hex(0x1C1922u);
            hi = hex(0x59536Au);
            gc = hex(0x8A5A30u);
        }
        var skin = mix(lo, hi, (0.35 + 0.65 * sqrt(lobe)) * (0.4 + 0.6 * rnd));
        skin *= 0.92 + 0.16 * fbm_n(u * vec2<f32>(0.12, 0.06) + c, 3);
        skin *= 0.84 + 0.16 * smoothstep(-56.0, -18.0, u.y);
        skin *= 1.0 - 0.3 * groove;
        // the dimple the stem sits in, and the shadowed shoulders round it
        skin *= 1.0 - 0.5 * exp(-length((u - vec2<f32>(2.0, -50.0)) * vec2<f32>(1.0, 1.8)) / 16.0);
        // light through the thin rind round the carving, strongest along the grooves
        skin += gc * 0.55 * flk * gk * glow(face, 7.0 * s) * (0.6 + 0.4 * lobe);
        skin += gc * 0.35 * flk * gk * groove * glow(face, 16.0 * s);
        // the candle's pool on the ground lights the lower lobes from below
        skin += warm * 0.2 * flk * gk * smoothstep(22.0, 58.0, u.y) * (0.45 + 0.55 * lobe) * rnd;
        // the moon's sheen on the crests, upper left, and its cold rim
        skin += fogc * 0.24 * pow(lobe, 5.0) * rnd * clamp(0.5 - (u.x + u.y) / 100.0, 0.0, 1.0) * smoothstep(40.0, -10.0, u.y);
        let rimm = clamp(sd_pumpkin((q + tm * 2.5) / sc) * dk - body, 0.0, 2.5) / 2.5;
        skin += fogc * 0.55 * rimm * smoothstep(-4.0 * s, 0.0, body);
        // inside the openings: the lit flesh, hottest near the candle low in the middle
        let candle = length(u - vec2<f32>(0.0, 30.0));
        let hot = exp(-candle / 48.0) * (0.75 + 0.3 * flk);
        let hot_h = max(hot, 0.5 * (0.75 + 0.3 * flk));
        var hole = mix(deep, warm * 1.05, smoothstep(0.0, 0.35, hot_h));
        hole = mix(hole, core, smoothstep(0.3, 0.95, hot_h));
        // the cut wall of the thick rind, seen on the side away from the middle
        let toc = q / max(length(q - vec2<f32>(0.0, 12.0 * s)), 0.001);
        let face_in = sd_face((q + toc * 4.0 * s) / sc + wob, style) * dk;
        let wall = aa(face) * (1.0 - aa(face_in));
        let wall_l = smoothstep(-4.0 * s, 0.0, face);
        let flesh = mix(hex(0xB04A10u), hex(0xEE9636u), wall_l) * (0.75 + 0.35 * hot);
        // depth: under the top edge the rind overhangs and the hollow is in shadow
        let face_up = sd_face(u + wob - vec2<f32>(0.0, 6.0), style) * dk;
        let overhang = smoothstep(-1.5 * s, 2.5 * s, face_up);
        hole *= 1.0 - 0.42 * overhang;
        hole = mix(hole * 0.92, flesh, wall);
        hole = mix(hole, hex(0xFFC870u), smoothstep(-1.0 * s, 0.0, face) * aa(face) * 0.45);
        // the candle's flame
        let sway = 1.2 * sin(t * 3.1 + k) * max(33.0 - u.y, 0.0) / 8.0;
        let fq = (u - vec2<f32>(sway, 33.0)) * vec2<f32>(1.0, 0.8);
        let flame = length(fq) - (4.5 - max(-fq.y, 0.0) * 0.45);
        hole = mix(hole, vec3<f32>(1.0, 0.97, 0.88), aa_w(flame * s, 1.5) * 0.9 * min(gk, 1.0));
        hole += core * 0.35 * glow(flame * s, 5.0 * s);
        hole *= (0.82 + 0.22 * flk) * gk;
        var pc = mix(skin, hole, aa(face));
        // the fog it stands in curls round its foot
        let ff = smoothstep(34.0, 60.0, u.y + 10.0 * (vnoise(vec2<f32>(u.x * 0.07 + t * 0.25, k * 3.0)) - 0.5));
        pc = mix(pc, hex(0x3B2E58u) + warm * 0.18 * flk * gk, ff * 0.55);
        col = mix(col, pc, aa(body));
    }
    // the stem: a short bent stalk, its moon side cold
    let st = sd_stem(u) * dk;
    if (st < 2.0) {
        let st_m = sd_stem((q + tm * 1.5) / sc) * dk;
        let stem = hex(0x171206u) * (0.75 + 0.5 * fbm_n(u * vec2<f32>(0.9, 0.25), 2)) + hex(0x4E5A36u) * 0.3 * clamp(st_m - st, 0.0, 1.0);
        col = mix(col, stem, aa(st));
    }
    return col;
}

// ---- the dead tree: a trunk and four levels of forking branches, crooked by hash
fn sd_tree(p: vec2<f32>, root: vec2<f32>, seed: f32) -> f32 {
    var pa = array<vec2<f32>, 32>();
    var dir = array<vec2<f32>, 32>();
    var len = array<f32, 32>();
    var th = array<f32, 32>();
    pa[0] = root;
    dir[0] = normalize(vec2<f32>(-0.1, -1.0));
    len[0] = 92.0;
    th[0] = 10.0;
    var d = 1e9;
    for (var i = 0; i < 31; i++) {
        let b = pa[i] + dir[i] * len[i];
        let bend = vec2<f32>(-dir[i].y, dir[i].x) * (hash(vec2<f32>(f32(i), seed)) - 0.5) * len[i] * 0.3;
        let mid = (pa[i] + b) * 0.5 + bend;
        d = min(d, min(sd_taper(p, pa[i], mid, th[i], th[i] * 0.82), sd_taper(p, mid, b, th[i] * 0.82, th[i] * 0.58)));
        let c0 = 2 * i + 1;
        if (c0 < 31) {
            let h1 = hash(vec2<f32>(f32(i) * 1.7, seed + 3.0));
            let h2 = hash(vec2<f32>(f32(i) * 2.3, seed + 7.0));
            let a1 = 0.35 + 0.4 * h1;
            let a2 = -(0.3 + 0.45 * h2);
            let cs1 = vec2<f32>(cos(a1), sin(a1));
            let cs2 = vec2<f32>(cos(a2), sin(a2));
            let dv = dir[i];
            pa[c0] = b;
            pa[c0 + 1] = mid * 0.25 + b * 0.75;
            dir[c0] = normalize(vec2<f32>(dv.x * cs1.x - dv.y * cs1.y, dv.x * cs1.y + dv.y * cs1.x) + vec2<f32>(0.0, -0.15));
            dir[c0 + 1] = normalize(vec2<f32>(dv.x * cs2.x - dv.y * cs2.y, dv.x * cs2.y + dv.y * cs2.x) + vec2<f32>(0.0, -0.1));
            len[c0] = len[i] * (0.62 + 0.15 * h2);
            len[c0 + 1] = len[i] * (0.55 + 0.15 * h1);
            th[c0] = th[i] * 0.6;
            th[c0 + 1] = th[i] * 0.52;
        }
    }
    // the owl's perch: a long low limb reaching left, with a twig up from its end
    d = smin(d, sd_taper(p, vec2<f32>(402.0, 196.0), vec2<f32>(360.0, 178.0), 4.2, 3.0), 3.0);
    d = min(d, sd_taper(p, vec2<f32>(360.0, 178.0), vec2<f32>(318.0, 172.0), 3.0, 1.2));
    d = min(d, sd_taper(p, vec2<f32>(326.0, 173.0), vec2<f32>(312.0, 158.0), 1.4, 0.6));
    return d;
}

// a bat, local b in units of half its wingspan, `fl` the wings' beat (-1 up .. 1 down)
fn sd_bat(b: vec2<f32>, fl: f32) -> f32 {
    let ax = abs(b.x);
    let tip = -0.45 * fl;
    let top = tip * pow(min(ax, 1.0), 1.4) - 0.06 + 0.1 * ax * (1.0 - ax);
    let bot = top + 0.1 + 0.32 * (1.0 - ax) - 0.11 * abs(sin(ax * 9.42));
    let wing = max(max(top - b.y, b.y - bot), ax - 1.0);
    let body = sd_ell(b - vec2<f32>(0.0, 0.08), vec2<f32>(0.1, 0.2));
    let head = length(b - vec2<f32>(0.0, -0.12)) - 0.08;
    let ears = min(length(b - vec2<f32>(0.05, -0.22)), length(b - vec2<f32>(-0.05, -0.22))) - 0.03;
    return min(min(wing, body), min(head, ears));
}

// the witch on her broom, local px (flying toward -x, y down), the seat at the origin
fn sd_witch(q: vec2<f32>, t: f32) -> f32 {
    // the broom: a thin crooked stick, a bundle of twigs tied at its back, ragged at the end
    let stick = min(sd_taper(q, vec2<f32>(-38.0, -1.5), vec2<f32>(-10.0, 3.0), 0.9, 1.3), sd_taper(q, vec2<f32>(-10.0, 3.0), vec2<f32>(17.0, 6.0), 1.3, 1.6));
    let bq = rot2(q - vec2<f32>(16.0, 6.0), -0.12);
    let spread = 1.8 + max(bq.x, 0.0) * 0.36;
    let rag = 2.2 * sin(bq.y * 1.9 + 1.0) + 1.4 * sin(bq.y * 3.7 + t * 7.0);
    let bundle = max(max(abs(bq.y) - spread, -bq.x), bq.x - (19.0 + rag));
    // the rider: torso, head with a hooked nose, a hat bent back by the wind
    let torso = sd_taper(q, vec2<f32>(1.0, -0.5), vec2<f32>(-4.5, -13.5), 4.6, 3.2);
    let head = length(q - vec2<f32>(-7.0, -18.0)) - 3.4;
    let nose = sd_triangle(q, vec2<f32>(-9.5, -19.5), vec2<f32>(-9.5, -16.5), vec2<f32>(-13.5, -15.5));
    let brim = sd_ell(rot2(q - vec2<f32>(-6.0, -21.2), 0.16), vec2<f32>(8.5, 1.3));
    let cone = min(sd_taper(q, vec2<f32>(-5.5, -21.5), vec2<f32>(-1.5, -30.0), 4.2, 2.0), sd_taper(q, vec2<f32>(-1.5, -30.0), vec2<f32>(7.0, -35.0 + 1.2 * sin(t * 5.0)), 2.0, 0.4));
    // the cape streams back from the shoulders, its hem rippling
    let cx = q.x + 4.0;
    let ctop = -15.0 + 0.3 * cx + 1.6 * sin(cx * 0.32 - t * 10.0);
    let cbot = ctop + max(11.0 - 0.34 * cx, 1.5) + 1.8 * sin(cx * 0.55 - t * 12.0);
    let cape = max(max(ctop - q.y, q.y - cbot) * 0.8, max(-cx, cx - (27.0 + 2.0 * sin(t * 9.0))));
    // arms to the stick, legs bent with a pointed boot
    let arm = sd_taper(q, vec2<f32>(-4.0, -11.5), vec2<f32>(-16.5, -0.5), 1.9, 1.3);
    let thigh = sd_taper(q, vec2<f32>(1.0, -1.0), vec2<f32>(-8.0, 2.0), 2.6, 2.0);
    let shin = sd_taper(q, vec2<f32>(-8.0, 2.0), vec2<f32>(-9.5, 9.0), 2.0, 1.5);
    let boot = sd_taper(q, vec2<f32>(-9.5, 9.0), vec2<f32>(-14.5, 7.5), 1.6, 0.6);
    let hair = min(sd_taper(q, vec2<f32>(-5.0, -19.0), vec2<f32>(5.0, -15.0 + 2.0 * sin(t * 7.0)), 1.8, 0.4), sd_taper(q, vec2<f32>(-5.0, -17.0), vec2<f32>(3.0, -10.5 + 2.0 * sin(t * 8.0 + 1.0)), 1.5, 0.4));
    var d = min(stick, bundle);
    d = min(d, smin(torso, head, 1.5));
    d = min(d, min(nose, min(brim, cone)));
    d = min(d, smin(cape, torso, 2.0));
    d = min(d, min(arm, min(thigh, min(shin, boot))));
    d = min(d, hair);
    return d;
}
fn witch_path(s: f32) -> vec2<f32> {
    let x = 560.0 - 640.0 * s;
    return vec2<f32>(x, 126.0 + 0.00055 * (x - 128.0) * (x - 128.0));
}

// the owl, local px from its body's centre, y down
fn sd_owl(q: vec2<f32>) -> f32 {
    let body = sd_ell(q - vec2<f32>(0.0, 4.0), vec2<f32>(9.5, 12.5));
    let head = sd_ell(q - vec2<f32>(0.0, -9.5), vec2<f32>(9.0, 7.5));
    var d = smin(body, head, 3.0);
    let qm = vec2<f32>(abs(q.x), q.y);
    let ear = sd_triangle(qm, vec2<f32>(3.0, -15.0), vec2<f32>(8.5, -13.0), vec2<f32>(8.0, -21.5));
    d = smin(d, ear, 1.0);
    let tail = sd_triangle(q, vec2<f32>(-4.0, 13.0), vec2<f32>(4.0, 13.0), vec2<f32>(1.5, 21.0));
    return min(d, tail);
}

// the cat sitting, local px from the middle of its base, y down; `sway` moves the tail's tip
fn sd_cat(q: vec2<f32>, sway: f32) -> f32 {
    let haunch = sd_ell(q - vec2<f32>(0.0, -8.0), vec2<f32>(14.0, 8.5));
    let body = sd_ell(q - vec2<f32>(0.0, -18.0), vec2<f32>(9.5, 14.0));
    var d = smin(haunch, body, 4.0);
    let head = sd_ell(q - vec2<f32>(0.0, -33.0), vec2<f32>(9.5, 8.5));
    d = smin(d, head, 3.0);
    let qm = vec2<f32>(abs(q.x), q.y);
    let ear = sd_triangle(qm, vec2<f32>(1.0, -38.5), vec2<f32>(9.5, -33.0), vec2<f32>(7.0, -46.0));
    d = smin(d, ear, 2.0);
    let paw = sd_ell(qm - vec2<f32>(4.5, -1.5), vec2<f32>(3.2, 2.0));
    d = min(d, paw);
    let tl = sd_bez(q, vec2<f32>(9.0, -2.0), vec2<f32>(31.0, 1.0), vec2<f32>(27.0 + sway, -20.0)) - 2.4;
    return smin(d, tl, 2.0);
}

// a gravestone from the middle of its base, y down: 0 round top, 1 a cross, 2 a pointed slab
fn sd_stone(q: vec2<f32>, w: f32, h: f32, kind: i32) -> f32 {
    if (kind == 1) {
        let v = sd_box_s(q - vec2<f32>(0.0, -h * 0.5), vec2<f32>(w * 0.32, h * 0.5));
        let hz = sd_box_s(q - vec2<f32>(0.0, -h * 0.68), vec2<f32>(w, w * 0.3));
        return min(v, hz);
    }
    if (kind == 2) {
        let bx = sd_box_s(q - vec2<f32>(0.0, -(h - w) * 0.5), vec2<f32>(w, (h - w) * 0.5));
        let tp = sd_triangle(q, vec2<f32>(-w, -(h - w) + 0.5), vec2<f32>(w, -(h - w) + 0.5), vec2<f32>(0.0, -h));
        return min(bx, tp);
    }
    let bx = sd_box_s(q - vec2<f32>(0.0, -(h - w) * 0.5), vec2<f32>(w, (h - w) * 0.5));
    let top = length(q - vec2<f32>(0.0, -(h - w))) - w;
    return min(bx, top);
}

// a pumpkin leaf, five round lobes
fn sd_pleaf(q: vec2<f32>, r: f32) -> f32 {
    let a = atan2(q.y, q.x);
    return length(q) - r * (0.7 + 0.3 * abs(cos(2.5 * a)));
}

// the far ridge (world y of the horizon) at x: a low hill on the left for the graveyard
fn ridge_y(x: f32) -> f32 {
    let hl = (x - 95.0) / 80.0;
    let hr = (x - 440.0) / 70.0;
    return 252.0 - 24.0 * exp(-hl * hl) - 12.0 * exp(-hr * hr) + 8.0 * (fbm_n(vec2<f32>(x * 0.018, 3.1), 3) - 0.5);
}

@compute @workgroup_size(16, 16)
fn under(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= f.size.x || id.y >= f.size.y) { return; }
    let pp = vec2<f32>(id.xy) + 0.5;
    let p = world(pp);
    let t = f.time * f.speed;

    let warm = hex(0xFF8A1Eu);
    let core = hex(0xFFE9A8u);
    let fogc = hex(0x6F5C93u);
    let night = hex(0x07040Fu);

    // the lanterns: centre, size, width; faces and skins below
    var pos = array<vec2<f32>, 6>(
        vec2<f32>(240.0, 196.0),
        vec2<f32>(42.0, 255.0),
        vec2<f32>(446.0, 262.0),
        vec2<f32>(146.0, 338.0),
        vec2<f32>(338.0, 346.0),
        vec2<f32>(240.0, 444.0));
    var siz = array<f32, 6>(1.15, 0.40, 0.36, 0.50, 0.44, 0.58);
    var asp = array<f32, 6>(1.0, 1.1, 0.9, 1.12, 0.95, 1.08);
    var sty = array<i32, 6>(0, 2, 1, 4, 3, 2);
    var rnd = array<i32, 6>(0, 1, 2, 0, 1, 1);

    // a press: where and how long ago
    let pr = f.aux[0];
    var age = 99.0;
    if (pr.w > 0.5) { age = f.time - pr.z; }
    let pw = world(pr.xy);

    // the gust runs out from the pressed key: every candle gutters almost out as it passes, then
    // roars back brighter than before and settles
    var gut = array<f32, 6>();
    var fl = array<f32, 6>();
    for (var i = 0; i < 6; i++) {
        let ga = age - length(pos[i] - pw) / 420.0;
        var g = 1.0;
        if (ga >= 0.0 && ga < 3.0) {
            let dim = smoothstep(0.0, 0.07, ga) * (1.0 - smoothstep(0.26, 0.6, ga));
            let roar = smoothstep(0.34, 0.62, ga) * exp(-max(ga - 0.62, 0.0) * 2.0);
            g = (1.0 - 0.8 * dim) * (1.0 + 0.55 * roar);
        }
        gut[i] = g;
        let fi = f32(i) * 13.7;
        fl[i] = 0.84 + 0.12 * (vnoise(vec2<f32>(t * 1.3, fi)) - 0.5) * 2.0 + 0.06 * (vnoise(vec2<f32>(t * 7.0, fi + 3.0)) - 0.5) * 2.0;
    }
    // the gust as one number for the scene (the owl, the leaves): up fast, down slow
    var gust = 0.0;
    if (age < 3.0) { gust = smoothstep(0.0, 0.12, age) * (1.0 - smoothstep(0.8, 2.6, age)); }

    // far: sky, lighter and warmer around the moon
    let mc = vec2<f32>(128.0, 138.0);
    let mr = 32.0;
    let md = length(p - mc) - mr;
    let sky_t = clamp((p.y - 20.0) / 250.0, 0.0, 1.0);
    var col = mix(hex(0x06030Du), hex(0x26123Cu), sky_t);
    col += fogc * 0.22 * glow(md, 75.0) + hex(0xC27A34u) * 0.35 * glow(md, 18.0);

    // the moon: limb darkened, a little warm at the edge, with soft grey seas
    let mq = (p - mc) / mr;
    let limb = sqrt(max(1.0 - dot(mq, mq), 0.0));
    var moon = mix(hex(0xD9A963u), hex(0xF6E0AAu), 0.35 + 0.65 * limb);
    let seas = fbm_n((p - mc) * 0.07 + vec2<f32>(3.0, 1.0), 4);
    moon = mix(moon, moon * vec3<f32>(0.78, 0.76, 0.8), smoothstep(0.48, 0.62, seas));
    moon *= 0.94 + 0.06 * fbm_n((p - mc) * 0.35, 2);
    col = mix(col, moon, aa(md));

    // stars, sparse, faint, gone near the moon and the horizon
    let scl = floor(p / 7.0);
    let sh = hash(scl + 11.0);
    if (sh > 0.95 - 0.02 * smoothstep(110.0, 20.0, p.y)) {
        let so = vec2<f32>(hash(scl + 3.1), hash(scl + 5.7)) * 5.0 + 1.0;
        let sdd = length(p - scl * 7.0 - so);
        let sb = max((sh - 0.95) / 0.05, 0.0);
        let tw = 0.6 + 0.4 * sin(t * (1.3 + 2.0 * hash(scl + 9.3)) + sh * 40.0);
        col += vec3<f32>(0.75, 0.72, 0.9) * (0.25 + 0.7 * sb) * tw * exp(-sdd * sdd * 1.6) * smoothstep(235.0, 150.0, p.y) * smoothstep(30.0, 80.0, md + 30.0);
    }

    // now and then a falling star across the high sky
    let sc_n = floor(t / 9.0);
    let sc_t = t - sc_n * 9.0 - 4.0;
    if (sc_t > 0.0 && sc_t < 0.9) {
        let s0 = vec2<f32>(200.0 + 200.0 * hash(vec2<f32>(sc_n, 1.0)), 20.0 + 110.0 * hash(vec2<f32>(sc_n, 2.0)));
        let sdir = normalize(vec2<f32>(-1.0, 0.42));
        let shd = s0 + sdir * sc_t * 240.0;
        let sl = 50.0 * smoothstep(0.0, 0.25, sc_t);
        let sw = p - shd;
        let along = clamp(-dot(sw, sdir) / max(sl, 1.0), 0.0, 1.0);
        let sdist = length(sw + sdir * along * sl);
        let sfade = smoothstep(0.0, 0.1, sc_t) * (1.0 - smoothstep(0.6, 0.9, sc_t));
        col += vec3<f32>(0.95, 0.9, 1.0) * exp(-sdist * sdist / 1.2) * (1.0 - along) * sfade * 0.9;
    }

    // thin cloud strands drifting across the moon, lit from behind at their edges
    let cl = fbm_n(vec2<f32>(p.x * 0.011 - t * 0.012, p.y * 0.06), 4);
    let strand = smoothstep(0.52, 0.7, cl) * smoothstep(60.0, 120.0, p.y) * smoothstep(205.0, 155.0, p.y);
    col = mix(col, hex(0x1A0F2Au) + hex(0xC27A34u) * 0.55 * glow(md, 22.0), strand * 0.72);

    // high up, a bank of cloud with silver edges where the moon catches it
    if (p.y < 115.0) {
        let hq = vec2<f32>(p.x * 0.008 - t * 0.008, p.y * 0.022);
        let hn = fbm_n(hq + vec2<f32>(fbm_n(hq * 2.0 + vec2<f32>(0.0, t * 0.01), 3) * 0.8, 0.0), 5);
        let hb = smoothstep(115.0, 70.0, p.y) * smoothstep(-10.0, 30.0, p.y);
        let hm = smoothstep(0.42, 0.62, hn) * hb;
        let hedge = smoothstep(0.42, 0.5, hn) * (1.0 - smoothstep(0.5, 0.62, hn));
        let hlit = glow(md, 120.0);
        let hc = hex(0x120A20u) + hex(0xC9A0D8u) * 0.22 * hedge * (0.4 + hlit) + hex(0xC27A34u) * 0.18 * hedge * hlit;
        col = mix(col, hc, hm * 0.85);
    }
    let to_moon = normalize(mc - vec2<f32>(240.0, 200.0));

    // the witch, now and then, across the moon from behind the tree, a trail of sparks behind her
    let wph = t + 1.8 - 22.0 * floor((t + 1.8) / 22.0);
    if (wph < 8.0) {
        let ws = wph / 8.0;
        var wp = witch_path(ws);
        wp.y += 2.5 * sin(t * 2.6);
        let wdx = wp.x - 128.0;
        let wf = normalize(vec2<f32>(-1.0, -0.0011 * wdx));
        let wn = vec2<f32>(wf.y, -wf.x);
        let ww = p - wp;
        if (length(ww - vec2<f32>(60.0, 0.0)) < 130.0) {
            if (length(ww) < 50.0) {
                let wq = vec2<f32>(-dot(ww, wf), dot(ww, wn)) / 1.1;
                let wd = sd_witch(wq, t) * 1.1;
                let wq2 = vec2<f32>(-dot(ww + to_moon * 1.5, wf), dot(ww + to_moon * 1.5, wn)) / 1.1;
                let wd2 = sd_witch(wq2, t) * 1.1;
                let wcol = hex(0x050309u) + fogc * 0.5 * clamp(wd2 - wd, 0.0, 1.0) * smoothstep(-1.6, 0.0, wd);
                col = mix(col, wcol, aa(wd));
            }
            // the spark trail from her broom: fine, scattered, falling a little, gone fast
            let tail0 = wp - wf * 38.0 + wn * 7.0;
            for (var j = 1; j < 15; j++) {
                let fj = f32(j);
                let jit = vec2<f32>(hash(vec2<f32>(fj, 1.0)), hash(vec2<f32>(fj + 5.0, 3.0))) - 0.5;
                let sp = witch_path(ws - fj * 0.006) - wf * 38.0 + wn * (7.0 + fj * 0.9) + jit * (3.0 + fj * 1.2);
                let sd2 = dot(p - sp, p - sp);
                let tw = pow(1.0 - fj / 15.0, 1.6) * (0.5 + 0.5 * sin(t * 19.0 + fj * 2.3));
                col += hex(0xFFD58Au) * exp(-sd2 / 0.9) * tw * 0.85;
            }
            col += hex(0xFFD58Au) * 0.18 * exp(-length(p - tail0) / 5.0);
        }
    }

    // the lanterns' light hanging in the air
    var air = 0.0;
    for (var i = 0; i < 6; i++) {
        let lq = (p - pos[i] - vec2<f32>(0.0, 22.0 * siz[i])) / siz[i];
        air += 0.55 * siz[i] * fl[i] * gut[i] * exp(-length(lq * vec2<f32>(0.8, 1.2)) / 55.0);
    }

    // middle: the ridge with its hill, the top caught by the moon
    let ry = ridge_y(p.x);
    let hill = mix(hex(0x1C1030u), hex(0x0E0818u), clamp((p.y - ry) / 40.0, 0.0, 1.0));
    col = mix(col, hill, aa(ry - p.y));
    col += fogc * 0.5 * aa(ry - p.y) * exp(-max(p.y - ry, 0.0) / 1.6) * smoothstep(320.0, 100.0, p.x);

    // the little graveyard on the hill: leaning stones and a cross, moonlit on one edge
    if (p.x < 170.0 && p.y > 195.0 && p.y < 260.0) {
        var gst = 1e9;
        var gst_m = 1e9;
        for (var g = 0; g < 5; g++) {
            var gx = 8.0;
            var gw = 5.0;
            var gh = 15.0;
            var gk = 0;
            var ga2 = 0.1;
            if (g == 1) { gx = 70.0; gw = 5.5; gh = 18.0; gk = 2; ga2 = -0.12; }
            if (g == 2) { gx = 109.0; gw = 5.5; gh = 16.0; gk = 0; ga2 = 0.14; }
            if (g == 3) { gx = 131.0; gw = 7.0; gh = 26.0; gk = 1; ga2 = -0.06; }
            if (g == 4) { gx = 153.0; gw = 5.0; gh = 14.0; gk = 0; ga2 = 0.2; }
            let gb = vec2<f32>(gx, ridge_y(gx) + 4.0);
            let gq = rot2(p - gb, ga2);
            gst = min(gst, sd_stone(gq, gw, gh, gk));
            gst_m = min(gst_m, sd_stone(rot2(p + to_moon * 1.5 - gb, ga2), gw, gh, gk));
        }
        let stc = hex(0x0C0714u) * (0.85 + 0.3 * fbm_n(p * 0.25, 2)) + fogc * 0.6 * clamp(gst_m - gst, 0.0, 1.0) * smoothstep(-1.8, 0.0, gst);
        col = mix(col, stc, aa(gst));
    }

    // the dead tree on the right, a cold edge on its moon side
    if (p.x > 290.0 && p.y < 300.0) {
        let tr = sd_tree(p, vec2<f32>(410.0, 282.0), 4.0);
        if (tr < 3.0) {
            let tr_m = sd_tree(p + to_moon * 1.5, vec2<f32>(410.0, 282.0), 4.0);
            let bark = 0.85 + 0.3 * fbm_n(p * vec2<f32>(0.5, 0.08), 2);
            col = mix(col, night * bark + fogc * 0.45 * clamp(tr_m - tr, 0.0, 1.0) * smoothstep(-2.2, 0.0, tr), aa(tr));
        }
    }

    // the owl on its limb: blinks now and then, stares at a press
    let oc = vec2<f32>(346.0, 160.0);
    let oq = p - oc;
    if (abs(oq.x) < 16.0 && abs(oq.y) < 26.0) {
        let od = sd_owl(oq);
        let od_m = sd_owl(oq + to_moon * 1.5);
        var ocol = hex(0x1A120Eu) * (0.8 + 0.25 * smoothstep(-0.3, 0.6, sin(oq.y * 1.4 + abs(oq.x) * 0.8)));
        ocol += fogc * 0.55 * clamp(od_m - od, 0.0, 1.0) * smoothstep(-1.8, 0.0, od);
        // the facial disc and the eyes
        let om = vec2<f32>(abs(oq.x), oq.y);
        let disc = length(om - vec2<f32>(4.0, -10.0)) - 4.8;
        ocol = mix(ocol, hex(0x3A2C24u), aa(disc) * 0.8);
        let blink = clamp(abs(fract(t * 0.19 + 0.4) - 0.5) / 0.02, 0.0, 1.0);
        let open = max(blink, gust);
        let er = 2.4 * (1.0 + 0.35 * gust);
        let eye = sd_ell(om - vec2<f32>(3.9, -10.5), vec2<f32>(er, er * max(open, 0.12)));
        var look = vec2<f32>(0.7 * sin(t * 0.37), 0.2);
        if (gust > 0.0) { look = mix(look, normalize(pw - oc + vec2<f32>(0.001, 0.0)) * 1.0, gust); }
        let pup = length(om - vec2<f32>(3.9, -10.5) - vec2<f32>(look.x * sign(oq.x), look.y)) - 1.1 * (1.0 - 0.3 * gust);
        var ec = mix(hex(0xFFC43Au), hex(0xFFE58Au), 0.5 + 0.5 * gust);
        ec = mix(ec, hex(0x0A0604u), aa(pup) * step(0.3, open));
        ocol = mix(ocol, ec, aa(eye));
        let beak = sd_triangle(oq, vec2<f32>(-1.3, -8.0), vec2<f32>(1.3, -8.0), vec2<f32>(0.0, -4.5));
        ocol = mix(ocol, hex(0x5A4630u), aa(beak));
        col = mix(col, ocol, aa(od));
        // the eyes' glow in the dark
        col += hex(0xFFB030u) * 0.25 * open * exp(-max(eye, 0.0) / 2.5) * (1.0 - aa(eye));
    }

    // bats wheeling over the hollow, uneven, their wings beating; two of them high up
    for (var k = 0; k < 7; k++) {
        let fk = f32(k);
        let w1 = 0.09 + 0.03 * fk;
        var bc = vec2<f32>(240.0 + 215.0 * sin(t * w1 + fk * 1.9), 140.0 + 70.0 * sin(t * w1 * 1.7 + fk * 2.7));
        if (k >= 5) { bc = vec2<f32>(240.0 + 130.0 * sin(t * w1 + fk * 1.9), 48.0 + 34.0 * sin(t * w1 * 2.1 + fk)); }
        bc += 7.0 * vec2<f32>(sin(t * 2.3 + fk * 5.0), cos(t * 3.1 + fk * 3.0));
        let bs = 11.0 + 5.0 * hash(vec2<f32>(fk, 9.0)) - select(0.0, 2.0, k >= 5);
        if (length(p - bc) < bs * 1.3) {
            let bf = sin(t * (15.0 + 2.0 * fk) + fk * 2.0);
            let bd = sd_bat((p - bc) / bs, bf) * bs;
            let bd_m = sd_bat((p - bc + to_moon * 1.2) / bs, bf) * bs;
            col = mix(col, night + fogc * 0.4 * clamp(bd_m - bd, 0.0, 1.0) * smoothstep(-1.5, 0.0, bd), aa(bd));
        }
    }

    // near: ground, warmed in front of each lantern
    let gy = 258.0 + 4.0 * sin(p.x * 0.017 + 0.4);
    let ground = mix(hex(0x150C1Eu), hex(0x08040Fu), clamp((p.y - gy) / 140.0, 0.0, 1.0));
    col = mix(col, ground, aa(gy - p.y));

    // fallen leaves on the ground, more toward the front
    if (p.y > gy + 6.0) {
        for (var lyr = 0; lyr < 2; lyr++) {
            let cell = 12.0;
            let off = f32(lyr) * 6.0;
            let gcl = floor((p + off) / cell);
            let gh = hash(gcl + 41.0 * f32(lyr) + 2.0);
            // drifts: the leaves lie in heaps where the noise says, thin elsewhere
            let drift = smoothstep(0.42, 0.66, fbm_n(gcl * cell * 0.018 + vec2<f32>(4.0, 1.0), 3));
            var heap = 0.0;
            for (var hi2 = 3; hi2 < 6; hi2++) {
                heap += exp(-length((p - pos[hi2] - vec2<f32>(0.0, 58.0 * siz[hi2])) * vec2<f32>(0.8, 2.2)) / (40.0 * siz[hi2] + 8.0));
            }
            let dens = min(0.08 + 0.6 * drift + 0.7 * heap, 0.85) * smoothstep(gy + 10.0, gy + 70.0, p.y);
            if (gh < dens) {
                let ctr = gcl * cell + cell * 0.5 - off + (vec2<f32>(hash(gcl + 7.0), hash(gcl + 9.0)) - 0.5) * 2.5;
                let lq = rot2(p - ctr, gh * 40.0);
                let dpt = clamp((p.y - 270.0) / 200.0, 0.0, 1.0);
                let lsz = (2.3 + 1.0 * hash(gcl + 13.0)) * (0.8 + 0.75 * dpt);
                let ld = sd_ell(lq, vec2<f32>(lsz, lsz * 0.5));
                let lh = hash(gcl + 17.0);
                var lc = mix(hex(0x3A1608u), hex(0x6B2A0Cu), lh);
                if (lh > 0.75) { lc = hex(0x5E4612u); }
                lc *= 0.75 + 0.25 * smoothstep(0.0, 0.9, abs(lq.y));
                col = mix(col, lc * (0.3 + 1.8 * air) + vec3<f32>(0.014, 0.01, 0.024), aa(ld) * 0.9);
            }
        }
    }

    // the vines winding over the ground, with leaves and curling tendrils
    // only round the near lanterns, short and thick, lying in the fog
    if (p.y > 320.0) {
        var vd = sd_bez(p, vec2<f32>(122.0, 360.0), vec2<f32>(84.0, 374.0), vec2<f32>(40.0, 354.0)) - 2.0;
        vd = min(vd, sd_bez(p, vec2<f32>(362.0, 366.0), vec2<f32>(402.0, 380.0), vec2<f32>(446.0, 358.0)) - 2.0);
        vd = min(vd, sd_bez(p, vec2<f32>(212.0, 470.0), vec2<f32>(170.0, 446.0), vec2<f32>(118.0, 452.0)) - 2.4);
        vd = min(vd, sd_bez(p, vec2<f32>(270.0, 470.0), vec2<f32>(314.0, 440.0), vec2<f32>(360.0, 456.0)) - 2.4);
        // tendrils: little spirals off the vines
        var td = 1e9;
        for (var s = 0; s < 4; s++) {
            var tc = vec2<f32>(66.0, 375.0);
            if (s == 1) { tc = vec2<f32>(420.0, 381.0); }
            if (s == 2) { tc = vec2<f32>(150.0, 437.0); }
            if (s == 3) { tc = vec2<f32>(330.0, 437.0); }
            let tq = p - tc;
            let tr2 = length(tq);
            if (tr2 < 9.0) {
                let ta = atan2(tq.y, tq.x) + 3.14159;
                let turns = tr2 / (1.3 * 6.28318) - ta / 6.28318;
                td = min(td, abs(fract(turns + 0.5) - 0.5) * 1.3 * 6.28318 - 0.75);
            }
        }
        var lfd = 1e9;
        var lfq = vec2<f32>(0.0);
        var lfr = 1.0;
        for (var s = 0; s < 4; s++) {
            var lc2 = vec2<f32>(56.0, 356.0);
            var lr = 8.5;
            if (s == 1) { lc2 = vec2<f32>(436.0, 360.0); lr = 8.0; }
            if (s == 2) { lc2 = vec2<f32>(128.0, 446.0); lr = 10.5; }
            if (s == 3) { lc2 = vec2<f32>(350.0, 450.0); lr = 10.0; }
            let ld2 = sd_pleaf(p - lc2, lr);
            if (ld2 < lfd) { lfd = ld2; lfq = p - lc2; lfr = lr; }
        }
        let vine = min(vd, td);
        let vcol = hex(0x16200Eu) * (0.5 + 1.5 * air) + hex(0x24301Cu) * 0.12;
        col = mix(col, vcol, aa(vine));
        // the leaf: veins along its lobes, a lighter edge toward the moon
        let la = atan2(lfq.y, lfq.x);
        let vein = 1.0 - 0.35 * (1.0 - smoothstep(0.4, 1.1, abs(sin(2.5 * la)) * length(lfq)));
        let lfc = hex(0x1C2612u) * (0.45 + 1.4 * air) * (0.85 + 0.3 * fbm_n(p * 0.3, 2)) * vein + fogc * 0.25 * smoothstep(-2.0, 0.0, lfd) * clamp(-dot(normalize(lfq + vec2<f32>(0.001, 0.0)), to_moon) * -1.0, 0.0, 1.0);
        col = mix(col, lfc, aa(lfd));
    }

    // each lantern stands in a pool of its own light, with a dark contact under it
    for (var i = 0; i < 6; i++) {
        let gq = (p - pos[i] - vec2<f32>(0.0, 56.0 * siz[i])) / siz[i];
        let pool = exp(-length(gq * vec2<f32>(1.0, 3.2)) / 38.0) * aa(gy - p.y + 2.0);
        col += warm * 0.35 * pool * fl[i] * gut[i];
        let contact = exp(-length(gq * vec2<f32>(1.0, 5.0)) / 22.0);
        col *= 1.0 - 0.6 * contact;
    }

    // the air glows warm round the lanterns
    col += warm * air * 0.45;

    // fog, far layer: a band along the foot of the ridge, drifting, moonlit on top
    let fa = fbm_n(vec2<f32>(p.x * 0.010 + t * 0.035, p.y * 0.05 - t * 0.01), 5);
    let band_a = smoothstep(222.0, 252.0, p.y) * smoothstep(292.0, 262.0, p.y);
    let dens_a = smoothstep(0.33, 0.72, fa) * band_a * 0.75;
    let fog_a = mix(hex(0x3E3060u), fogc * 1.1, smoothstep(275.0, 232.0, p.y)) + fogc * 0.35 * smoothstep(0.55, 0.8, fa) + warm * air * 1.2;
    col = mix(col, fog_a, dens_a);

    // the middle lanterns: the big one, and two on the ridge
    for (var i = 0; i < 3; i++) {
        col = draw_pumpkin(col, p, pos[i], siz[i], asp[i], sty[i], rnd[i], fl[i], gut[i], t, to_moon, f32(i));
    }

    // fog round the big lantern's feet
    let fb_q = vec2<f32>(p.x * 0.014 - t * 0.05, p.y * 0.03);
    let fb = fbm_n(fb_q + vec2<f32>(fbm_n(fb_q * 1.7 + vec2<f32>(t * 0.04, 0.0), 3) * 1.6, 0.0), 4);
    let band_m = smoothstep(244.0, 268.0, p.y) * smoothstep(320.0, 280.0, p.y);
    let dens_m = smoothstep(0.3, 0.72, fb) * band_m * 0.6;
    let fog_m = hex(0x3B2E58u) + fogc * 0.45 * smoothstep(0.45, 0.85, fb) + warm * air * 1.7;
    col = mix(col, fog_m, dens_m);

    // the near lanterns
    for (var i = 3; i < 6; i++) {
        col = draw_pumpkin(col, p, pos[i], siz[i], asp[i], sty[i], rnd[i], fl[i], gut[i], t, to_moon, f32(i));
    }

    // the black cat in front of the big lantern, its rim lit by it, eyes green
    let cb = vec2<f32>(240.0, 375.0);
    let cq = p - cb;
    col *= 1.0 - 0.55 * exp(-length(cq * vec2<f32>(1.0, 4.0) - vec2<f32>(4.0, 0.0)) / 14.0);
    if (cq.x > -20.0 && cq.x < 40.0 && cq.y > -52.0 && cq.y < 4.0) {
        let csw = 6.0 * sin(t * 1.1) + 3.0 * sin(t * 2.3 + 1.0);
        let cd = sd_cat(cq, csw);
        let cd_l = sd_cat(cq + vec2<f32>(0.0, -2.4), csw);
        var ccol = hex(0x07040Bu) + warm * 0.75 * clamp((cd_l - cd) / 2.4, 0.0, 1.0) * smoothstep(-2.6, 0.0, cd) * (0.7 + 0.3 * fl[0]) * gut[0];
        let cm = vec2<f32>(abs(cq.x), cq.y);
        let cblink = clamp(abs(fract(t * 0.13 + 0.15) - 0.5) / 0.018, 0.0, 1.0);
        let copen = max(cblink, gust);
        var clook = 0.6 * sin(t * 0.29);
        if (gust > 0.0) { clook = mix(clook, clamp((pw.x - cb.x) / 60.0, -1.0, 1.0), gust); }
        let ceye = sd_ell(cm - vec2<f32>(4.2, -34.0), vec2<f32>(2.7, 1.9 * max(copen, 0.1)));
        let cpup = sd_ell(cq - vec2<f32>(4.2 * sign(cq.x) + clook, -34.0), vec2<f32>(0.6 + 0.7 * gust, 1.7 * max(copen, 0.1)));
        var cec = mix(hex(0xC8F050u), hex(0x0A0604u), aa(cpup));
        ccol = mix(ccol, cec, aa(ceye));
        col = mix(col, ccol, aa(cd));
        col += hex(0xB8F048u) * 0.22 * copen * exp(-max(ceye, 0.0) / 2.5) * (1.0 - aa(ceye));
    }

    // fog, low: thin wisps hugging the ground in front, lit by the lanterns
    let fn_q = vec2<f32>(p.x * 0.012 + t * 0.03, p.y * 0.045);
    let fnn = fbm_n(fn_q + vec2<f32>(fbm_n(fn_q * 1.5 - vec2<f32>(t * 0.03, 0.0), 3) * 1.4, 0.0), 4);
    let rise = smoothstep(340.0, 440.0, p.y);
    let dens_b = smoothstep(0.4, 0.8, fnn) * rise * 0.42;
    let fog_b = hex(0x3B2E58u) + fogc * 0.4 * smoothstep(0.5, 0.85, fnn) + warm * air * 1.6;
    col = mix(col, fog_b, dens_b);

    // will-o'-the-wisps: cold green lights drifting low through the fog
    for (var k = 0; k < 3; k++) {
        let fk = f32(k);
        var wc = vec2<f32>(120.0 + 40.0 * sin(t * 0.21), 262.0 + 8.0 * sin(t * 0.9));
        if (k == 1) { wc = vec2<f32>(345.0 + 30.0 * sin(t * 0.17 + 2.0), 250.0 + 9.0 * sin(t * 1.1 + 1.0)); }
        if (k == 2) { wc = vec2<f32>(240.0 + 100.0 * sin(t * 0.11 + 4.0), 415.0 + 12.0 * sin(t * 0.8 + 3.0)); }
        let wl = length(p - wc);
        if (wl < 44.0) {
            let pulse = 0.65 + 0.35 * sin(t * (2.1 + fk) + fk * 4.0);
            // a small cold flame: round below, drawn up to a flickering point
            let wq = p - wc;
            let up = max(-wq.y, 0.0);
            let wfl = vec2<f32>(wq.x + 1.2 * sin(t * 7.0 + fk + up * 0.4) * up / 8.0, wq.y);
            let wd = length(wfl * vec2<f32>(1.0, select(1.0, 0.45, wq.y < 0.0))) - 3.2;
            let wcore = aa_w(wd, 2.0);
            // its trail: a faint smear behind it, fading
            let wv = normalize(vec2<f32>(cos(t * 0.21 + fk * 1.7), 0.15) + vec2<f32>(0.001, 0.0));
            let wt = clamp(dot(wq, wv) / 26.0, 0.0, 1.0);
            let wtr = exp(-length(wq - wv * wt * 26.0) / 3.0) * (1.0 - wt) * step(0.0, dot(wq, wv));
            let wcol = hex(0x5FE8C0u);
            col += wcol * (0.34 * exp(-wl / 12.0) + 0.18 * wtr) * pulse * (1.0 - 0.6 * gust);
            col = mix(col, mix(wcol, vec3<f32>(0.85, 1.0, 0.95), aa_w(wd + 1.6, 1.5)), wcore * 0.85 * pulse * (1.0 - 0.6 * gust));
        }
    }

    // leaves falling from the tree, tumbling, drifting left; a gust blows them away from the press
    for (var k = 0; k < 9; k++) {
        let fk = f32(k);
        let hk = hash(vec2<f32>(fk, 3.3));
        let per = 10.0 + 6.0 * hk;
        let lt = (t + fk * 3.7) / per;
        let cyc = floor(lt);
        let lu = lt - cyc;
        let x0 = 290.0 + 180.0 * hash(vec2<f32>(fk, cyc + 1.0));
        var lp = vec2<f32>(x0 - lu * 200.0 + 16.0 * sin(lu * 11.0 + fk), 100.0 + lu * 390.0);
        if (gust > 0.0) {
            let dv = lp - pw;
            lp += normalize(dv + vec2<f32>(0.001, 0.0)) * 46.0 * gust * exp(-length(dv) / 140.0);
        }
        let lw = p - lp;
        if (dot(lw, lw) < 64.0) {
            let lq = rot2(lw, t * 1.7 + fk * 2.0);
            let lwid = 0.35 + 0.65 * abs(cos(t * 2.6 + fk * 1.3));
            // a pointed leaf (two arcs), its width turning as it tumbles, a dark midrib
            let lW = 2.2 * lwid + 0.35;
            let lR = (21.0 + lW * lW) / (2.0 * lW);
            let ld = max(length(lq - vec2<f32>(0.0, lR - lW)), length(lq + vec2<f32>(0.0, lR - lW))) - lR;
            var lc = mix(hex(0x9A3A0Eu), hex(0xC06A1Au), hk);
            if (hk > 0.7) { lc = hex(0x7A1E10u); }
            lc *= 0.7 + 0.3 * smoothstep(0.0, 0.7, abs(lq.y));
            col = mix(col, lc * (0.4 + 1.4 * air) + fogc * 0.08, aa(ld));
        }
    }

    // the press, near: smoke curling up from the pressed key like a snuffed wick, then sparks
    // when the flames roar back
    if (age >= 0.0 && age < 3.0) {
        let w = p - pw;
        let top_y = 14.0 + min(age, 2.2) * 95.0;
        let hy = (-w.y + 6.0) / 190.0;
        if (-w.y > -8.0 && -w.y < top_y + 20.0 && abs(w.x) < 80.0) {
            let hc = clamp(hy, 0.0, 1.0);
            let curl = (9.0 * sin(hy * 16.0 - t * 2.2) + 5.0 * sin(hy * 29.0 + t * 1.3)) * smoothstep(0.0, 0.45, hy);
            let wid = 2.6 + 26.0 * hc * hc + 6.0 * hc;
            let dx = abs(w.x - curl);
            var m = smoothstep(wid, wid * 0.25, dx);
            let wn2 = fbm_n(vec2<f32>(w.x * 0.09, (p.y + t * 30.0) * 0.07) + vec2<f32>(pr.z * 7.0, 0.0), 3);
            m *= mix(1.0, smoothstep(0.25, 0.65, wn2), smoothstep(0.08, 0.4, hc));
            m *= smoothstep(top_y + 18.0, top_y - 10.0, -w.y) * smoothstep(-8.0, 4.0, -w.y);
            m *= (1.0 - smoothstep(1.4, 3.0, age)) * smoothstep(0.03, 0.15, age) * (1.0 - 0.55 * hc);
            let smoke = mix(hex(0xB9AED6u), hex(0x6F5C93u), hc);
            col = mix(col, smoke, clamp(m * 0.85, 0.0, 1.0));
        }
        for (var j = 0; j < 12; j++) {
            let fj = f32(j);
            let hj = hash(vec2<f32>(fj * 3.1, pr.z * 11.0 + 1.0));
            let hk2 = hash(vec2<f32>(fj * 5.7, pr.z * 13.0 + 2.0));
            let life = 0.7 + 0.9 * hk2;
            let ea = age - 0.42 - hj * 0.2;
            if (ea > 0.0 && ea < life) {
                let ep = pw + vec2<f32>((hj - 0.5) * 50.0 + 10.0 * sin(ea * (2.0 + 2.0 * hk2) + fj) * ea, -ea * (50.0 + 55.0 * hk2) + 10.0);
                let ed = length(p - ep);
                let fade = (1.0 - ea / life) * (0.75 + 0.25 * sin(ea * 23.0 + fj * 4.0));
                col += mix(warm, core, fade) * exp(-ed * ed / (1.6 + 2.0 * hk2)) * fade * 1.8;
            }
        }
    }

    // icons stay readable on the bright carving: a soft shadow round the keys' content, only where
    // the scene is bright (the dark mist needs none)
    let lum = luma(col);
    if (lum > 0.3) {
        let ix = i32(id.x);
        let iy = i32(id.y);
        var halo = 0.0;
        for (var k = 0; k < 8; k++) {
            let a = f32(k) * 0.7854;
            halo = max(halo, cover(ix + i32(round(cos(a) * 3.0)), iy + i32(round(sin(a) * 3.0))));
        }
        halo = max(halo, 0.6 * max(max(cover(ix + 5, iy), cover(ix - 5, iy)), max(cover(ix, iy + 5), cover(ix, iy - 5))));
        col *= 1.0 - 0.5 * halo * smoothstep(0.3, 0.65, lum);
    }

    let base = px(i32(id.x), i32(id.y)).rgb;
    put(id.xy, mix(col, base, keep_bg(base)));
}
