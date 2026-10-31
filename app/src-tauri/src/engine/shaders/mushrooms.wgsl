// Poland's golden autumn: mushrooms growing in the meadow of the bottom row (the forest floor of
// `goldlight`'s view; on the rows above they stood in the air in front of the trees), behind the
// icons and labels, each key its own kind, as found on a Polish mushroom hunt: a cep (prawdziwek:
// a fat cushion of a chestnut cap with a pale margin, a club of a pale stem netted near the top), a
// bay bolete (podgrzybek: a dark velvet cap, a slender streaked stem, lemon pores), a fly agaric
// (muchomor: a red cap with white flecks, white gills, a skirt and a bulb) and chanterelles (kurki:
// apricot trumpets with wavy, rolled rims and forked ridges running down), most with a smaller one
// beside them, standing in a tuft of moss and grass blades with a fallen leaf, lit from the low sun
// as `goldlight` draws it and held dark by its knee. A press: that key's mushroom bounces and puffs
// a cloud of glinting spores.
// The engine places them (`aux_at(2k)`, `aux_at(2k + 1)`: key k's spot on its floor, 0 = none;
// `aux_at(32 + k)`: seconds since key k was pressed), as for the jack-o'-lanterns.
const SUN = vec2<f32>(150.0, 118.0);
fn ellipse(q: vec2<f32>, r: vec2<f32>) -> f32 {
    return (length(q / r) - 1.0) * min(r.x, r.y);
}
fn smin(a: f32, b: f32, k: f32) -> f32 {
    let h = clamp(0.5 + 0.5 * (b - a) / k, 0.0, 1.0);
    return mix(b, a, h) - k * h * (1.0 - h);
}
// a stem from the ground (y 0) up to -h: half-width w0 at the foot, w1 at the top, a belly of
// `belly` at a third of the way up, a rounded foot
fn stem(q: vec2<f32>, h: f32, w0: f32, w1: f32, belly: f32) -> f32 {
    let t = clamp(-q.y / h, 0.0, 1.0);
    let w = mix(w0, w1, t) + belly * sin(3.14159 * clamp(t * 1.4, 0.0, 1.0));
    let body = max(abs(q.x) - w, -q.y - h);
    let foot = ellipse(q + vec2<f32>(0.0, w0 * 0.35), vec2<f32>(w0 * 1.02, w0 * 0.55));
    return min(max(body, q.y + w0 * 0.3), foot);
}
// a cap whose underside sits at y = base: a dome above, an underside bulging a little
fn cap(q: vec2<f32>, base: f32, cw: f32, ch: f32, under: f32) -> f32 {
    let dome = ellipse(vec2<f32>(q.x, q.y - base), vec2<f32>(cw, ch));
    let x = clamp(q.x / cw, -1.0, 1.0);
    let bottom = q.y - (base + under * (1.0 - x * x));
    return max(dome, bottom);
}
struct Hit {
    d: f32,
    col: vec3<f32>,
}
// a cap's light: round towards the sun, a soft sheen on that shoulder, the far side and the rim
// in shade
fn cap_light(q: vec2<f32>, base: f32, cw: f32, ch: f32, sun: vec2<f32>, sheen: f32) -> f32 {
    let u = vec2<f32>(q.x / cw, (q.y - base) / ch);
    let z = sqrt(max(1.0 - dot(u, u), 0.0));
    let n = normalize(vec3<f32>(u.x, u.y, z * 1.4));
    let l = normalize(vec3<f32>(sun.x, sun.y - 0.6, 0.9));
    let diff = clamp(dot(n, l), 0.0, 1.0);
    let spec = pow(clamp(dot(reflect(-l, n), vec3<f32>(0.0, 0.0, 1.0)), 0.0, 1.0), 10.0);
    return 0.45 + 0.75 * diff + sheen * spec;
}
fn bolete(q: vec2<f32>, s: f32, dark: bool, seed: f32, sun: vec2<f32>) -> Hit {
    var out = Hit(1e3, vec3<f32>(0.0));
    // a cep: a short fat club under a fat cushion; a bay bolete: taller, slender, a flatter cap
    let h = s * select(0.62, 0.74, dark);
    let w0 = s * select(0.27, 0.13, dark);
    let w1 = s * select(0.17, 0.1, dark);
    let st = stem(q, h, w0, w1, s * select(0.06, 0.01, dark));
    let base = -h + s * select(0.12, 0.08, dark);
    let cw = s * select(0.66, 0.6, dark);
    let ch = s * select(0.5, 0.36, dark);
    let cp = cap(q, base, cw, ch, s * 0.05);
    if (st < cp && st < 1.0) {
        var c = select(vec3<f32>(0.84, 0.77, 0.62), vec3<f32>(0.72, 0.58, 0.36), dark);
        // round: lit on the sun's side, in the cap's shadow at the top, earthy at the foot
        let side = clamp(q.x / (w0 + 0.01) * sun.x * 0.55 + 0.5, 0.0, 1.0);
        c *= 0.5 + 0.55 * side;
        c *= 1.0 - 0.5 * smoothstep(base + s * 0.22, base + s * 0.02, q.y);
        c = mix(c, c * vec3<f32>(0.75, 0.62, 0.45), smoothstep(-s * 0.12, s * 0.05, q.y));
        if (!dark) {
            // the white net on a cep's stem, below the cap
            let g = vec2<f32>(q.x * 0.9 + q.y * 0.5, q.x * 0.9 - q.y * 0.5) / (s * 0.07);
            let net = min(abs(fract(g.x) - 0.5), abs(fract(g.y) - 0.5));
            c = mix(c, vec3<f32>(0.93, 0.9, 0.82) * (0.6 + 0.4 * side), 0.35 * smoothstep(0.14, 0.04, net) * smoothstep(base + s * 0.42, base + s * 0.1, q.y));
        } else {
            // brown streaks running down a bay bolete's stem
            c *= 1.0 - 0.22 * smoothstep(0.55, 0.85, vnoise(vec2<f32>(q.x / s * 9.0, q.y / s * 1.2) + seed));
        }
        out = Hit(st, c);
    }
    if (cp < out.d && cp < 1.0) {
        let u = vec2<f32>(q.x / cw, (q.y - base) / ch);
        // chestnut (cep) or dark velvet (bay), darker in the middle, a paler margin at the rim
        var c = select(vec3<f32>(0.46, 0.25, 0.1), vec3<f32>(0.27, 0.15, 0.07), dark);
        c = mix(c, select(vec3<f32>(0.6, 0.4, 0.2), vec3<f32>(0.4, 0.25, 0.12), dark), smoothstep(0.55, 0.95, length(u)) * 0.6);
        // a cep's thin white margin where the cap meets the pores
        if (!dark) {
            c = mix(c, vec3<f32>(0.86, 0.8, 0.68), smoothstep(base - s * 0.07, base - s * 0.02, q.y) * smoothstep(0.75, 0.95, abs(u.x)) * 0.8);
        }
        c *= cap_light(q, base, cw, ch, sun, select(0.5, 0.12, dark));
        // the pores under the cap: cream to olive (cep), lemon (bay), in the cap's shade
        let pores = smoothstep(base - 0.4, base + 0.8, q.y);
        c = mix(c, select(vec3<f32>(0.7, 0.66, 0.42), vec3<f32>(0.72, 0.64, 0.22), dark) * 0.62, pores);
        // a cap's skin is never flat colour: a fine mottle, the velvet's fine grain
        c *= 1.0 - select(0.12, 0.18, dark) * (vnoise(q * (6.0 / s) + seed) - 0.3);
        out = Hit(cp, c);
    }
    return out;
}
fn agaric(q: vec2<f32>, s: f32, seed: f32, sun: vec2<f32>) -> Hit {
    var out = Hit(1e3, vec3<f32>(0.0));
    let h = s * 0.86;
    let st = stem(q, h, s * 0.13, s * 0.09, s * 0.0);
    // the bulb at the foot with its rings of the veil
    let bulb = ellipse(q + vec2<f32>(0.0, s * 0.1), vec2<f32>(s * 0.19, s * 0.14));
    // the skirt: hanging from below the cap, wider at its hem
    let ring_y = -h * 0.72;
    let rt = clamp((q.y - ring_y) / (s * 0.12), 0.0, 1.0);
    let ring = max(abs(q.x) - mix(s * 0.1, s * 0.18, rt), abs(q.y - ring_y - s * 0.06) - s * 0.06);
    let base = -h + s * 0.08;
    let cw = s * 0.64;
    let ch = s * 0.42;
    let cp = cap(q, base, cw, ch, s * 0.03);
    let stem_d = min(min(st, ring), bulb);
    if (stem_d < cp && stem_d < 1.0) {
        var c = vec3<f32>(0.9, 0.88, 0.82);
        c *= 0.52 + 0.5 * clamp(q.x / (s * 0.13) * sun.x * 0.55 + 0.5, 0.0, 1.0);
        c *= 1.0 - 0.5 * smoothstep(base + s * 0.2, base + s * 0.02, q.y);
        // the skirt's hem a shade darker; the bulb's rings
        c *= 1.0 - 0.2 * smoothstep(-0.5, 0.5, -ring) * smoothstep(0.5, 1.0, rt);
        c *= 1.0 - 0.18 * smoothstep(0.3, 0.0, abs(fract(q.y / (s * 0.055)) - 0.5)) * smoothstep(-s * 0.26, -s * 0.04, q.y);
        out = Hit(stem_d, c);
    }
    if (cp < out.d && cp < 1.0) {
        let u = vec2<f32>(q.x / cw, (q.y - base) / ch);
        // deep red at the top, orange-red towards the rim, a faint striated rim
        var c = mix(vec3<f32>(0.66, 0.05, 0.03), vec3<f32>(0.86, 0.26, 0.06), smoothstep(0.45, 1.0, length(u)));
        c *= 1.0 - 0.15 * smoothstep(0.85, 1.0, length(u)) * smoothstep(0.3, 0.0, abs(fract(atan2(u.y, u.x) * 9.0) - 0.5));
        c *= cap_light(q, base, cw, ch, sun, 0.7);
        // the white flecks of the veil, raised: many small ones, fewer at the rim
        for (var i = 0; i < 16; i++) {
            let fi = f32(i);
            let wu = vec2<f32>(hash(vec2<f32>(seed, fi)) * 1.7 - 0.85, -0.08 - hash(vec2<f32>(fi, seed)) * 0.84);
            if (length(wu) > 0.92) { continue; }
            let wr = 0.045 + 0.04 * hash(vec2<f32>(fi, seed + 3.0)) * (1.0 - length(wu));
            let wd = length((u - wu) * vec2<f32>(1.0, ch / cw * 1.5)) - wr;
            let lit = 0.75 + 0.25 * smoothstep(wr, -wr, u.y - wu.y);
            c = mix(c, vec3<f32>(0.92, 0.88, 0.8) * lit * cap_light(q, base, cw, ch, sun, 0.0) * 0.9, smoothstep(0.012, -0.012, wd));
        }
        // the white gills under the cap, in shade
        let gills = smoothstep(base - 0.4, base + 0.8, q.y);
        c = mix(c, vec3<f32>(0.8, 0.77, 0.7) * (0.5 + 0.15 * smoothstep(0.3, 0.0, abs(fract(q.x / 1.8) - 0.5))), gills);
        out = Hit(cp, c);
    }
    return out;
}
// chanterelles: apricot trumpets growing in a little group, each a stem flaring into a funnel with
// a wavy rim rolled down, seen a little from above (the funnel's hollow shows), forked ridges
// running down its underside
fn chanterelles(q: vec2<f32>, s: f32, seed: f32, sun: vec2<f32>) -> Hit {
    var out = Hit(1e3, vec3<f32>(0.0));
    for (var i = 0; i < 3; i++) {
        let fi = f32(i);
        let o = vec2<f32>((fi - 1.0) * s * 0.5 + (hash(vec2<f32>(seed, fi)) - 0.5) * s * 0.12, -s * 0.04 * f32(i % 2));
        let sz = s * mix(0.55, 0.85, hash(vec2<f32>(fi, seed + 2.0)));
        let lean = (fi - 1.0) * 0.18 + (hash(vec2<f32>(fi, seed + 5.0)) - 0.5) * 0.15;
        var p = q - o;
        p.x -= lean * -p.y;
        let h = sz * 0.72;
        let t = clamp(-p.y / h, 0.0, 1.0);
        let w = sz * (0.07 + 0.38 * pow(t, 2.3));
        let wave = sz * 0.045 * sin(p.x / sz * 11.0 + seed + fi * 2.0);
        var d = max(abs(p.x) - w, max(p.y, -p.y - h - wave));
        // the rim seen from a little above: a flat ellipse, its hollow darker inside
        let rim_c = vec2<f32>(p.x, p.y + h + wave * 0.6);
        let top = ellipse(rim_c, vec2<f32>(sz * 0.45, sz * 0.12));
        d = smin(d, top, sz * 0.05);
        if (d < out.d && d < 1.0) {
            var c = mix(vec3<f32>(0.62, 0.36, 0.06), vec3<f32>(0.86, 0.56, 0.12), smoothstep(0.0, 0.9, t));
            // the forked ridges running down from the rim, fading towards the stem
            let rx = p.x / max(w, 0.5);
            let ridge = smoothstep(0.3, 0.0, abs(fract(rx * 2.6 + 0.5 * floor(t * 3.0) * 0.5) - 0.5)) * smoothstep(0.15, 0.6, t) * smoothstep(0.5, -0.5, top);
            c *= 1.0 - 0.22 * ridge;
            // the funnel's hollow and its rolled lip
            let hollow = ellipse(rim_c + vec2<f32>(0.0, sz * 0.015), vec2<f32>(sz * 0.33, sz * 0.07));
            c = mix(c, vec3<f32>(0.92, 0.66, 0.2), smoothstep(0.5, -0.5, top) * 0.55);
            c = mix(c, vec3<f32>(0.55, 0.32, 0.06), smoothstep(0.5, -0.5, hollow) * 0.6);
            c *= 0.55 + 0.5 * clamp(p.x / (w + 0.01) * sun.x * 0.55 + 0.5, 0.0, 1.0);
            out = Hit(d, c);
        }
    }
    return out;
}
fn mushroom(q: vec2<f32>, kind: i32, s: f32, seed: f32, sun: vec2<f32>) -> Hit {
    if (kind == 0) { return bolete(q, s, false, seed, sun); }
    if (kind == 1) { return bolete(q, s, true, seed, sun); }
    if (kind == 2) { return agaric(q, s, seed, sun); }
    return chanterelles(q, s, seed, sun);
}
// the knee `goldlight` holds its picture with, a little higher for the mushrooms (objects close by)
fn knee(c: vec3<f32>) -> vec3<f32> {
    let lum = dot(c, vec3<f32>(0.299, 0.587, 0.114));
    if (lum > 0.42) { return c * (0.42 + (lum - 0.42) * 0.35) / lum; }
    return c;
}
@compute @workgroup_size(16, 16)
fn under(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= f.size.x || id.y >= f.size.y) { return; }
    let p = vec2<f32>(f32(id.x), f32(id.y)) + 0.5;
    var c = px(i32(id.x), i32(id.y)).rgb;
    let k = key_at(p);
    // the forest floor is the bottom row; the rows above look at the trees and the sky
    if (k < 10) { put(id.xy, c); return; }
    let at = vec2<f32>(aux_at(2 * k), aux_at(2 * k + 1));
    if (at.x <= 0.0) { put(id.xy, c); return; }
    let fk = f32(k);
    // the five keys of the bottom row each a different kind in turn (by chance three of four were
    // chanterelles)
    let kind = (k + 2) % 4;
    let s = 22.0 + 4.0 * hash(vec2<f32>(fk, 29.0));
    let ground = at + vec2<f32>(-2.0, 8.0);
    let sun = normalize(SUN - ground);
    let since = aux_at(32 + k);
    let strength = min(f.strength / 0.6, 1.3);
    // the press: a bounce, squashing and stretching
    let bounce = select(0.0, 0.24 * sin(since * 17.0) * exp(-since * 4.5), since >= 0.0 && since < 1.6);
    let d0 = p - ground;
    let q = vec2<f32>(d0.x * sqrt(1.0 + bounce), d0.y / (1.0 + bounce));
    // the floor round it: a soft contact shadow cast away from the sun, a low tuft of moss
    let away = vec2<f32>(-sun.x, 0.0) * s * 0.25;
    c *= 1.0 - 0.5 * smoothstep(1.0, 0.0, length((d0 - away) / vec2<f32>(s * 0.95, s * 0.17))) * strength;
    let moss_n = vnoise(vec2<f32>(d0.x / s * 7.0, fk * 3.0)) - 0.5;
    let moss = ellipse(d0 - vec2<f32>(-s * 0.05, s * 0.03), vec2<f32>(s * 0.7, s * 0.12)) + s * 0.07 * moss_n;
    if (moss < 1.0) {
        var m = mix(vec3<f32>(0.08, 0.15, 0.05), vec3<f32>(0.24, 0.32, 0.09), vnoise(d0 * 0.8 + fk * 3.0));
        m *= 0.7 + 0.45 * smoothstep(s * 0.1, -s * 0.12, d0.y);
        c = mix(c, m, smoothstep(0.8, -0.8, moss) * strength);
    }
    // a fallen leaf by the foot
    let lq = d0 - vec2<f32>(-s * 0.6, -s * 0.01);
    let la = 0.4 + fk;
    let lr = vec2<f32>(lq.x * cos(la) + lq.y * sin(la), -lq.x * sin(la) + lq.y * cos(la));
    let leaf = max(ellipse(lr, vec2<f32>(s * 0.24, s * 0.09)), -lr.y - s * 0.2);
    if (leaf < 1.0) {
        let lc = select(vec3<f32>(0.62, 0.2, 0.04), vec3<f32>(0.7, 0.48, 0.08), hash(vec2<f32>(fk, 41.0)) > 0.5);
        c = mix(c, lc * (0.75 + 0.25 * smoothstep(0.6, 0.0, abs(lr.y))) * (0.9 - 0.2 * smoothstep(0.4, 0.0, abs(lr.x / s - 0.0) * 4.0) * 0.0), smoothstep(0.8, -0.8, leaf) * strength);
    }
    // a smaller one beside it, a little behind (not with chanterelles, which grow in a group)
    if (kind != 3) {
        let q2 = q + vec2<f32>(s * 0.66, s * 0.03);
        let small = mushroom(q2 / 0.6, kind, s, fk + 7.0, sun);
        if (small.d * 0.6 < 0.9) {
            c = mix(c, knee(small.col * 0.9), smoothstep(0.8, -0.8, small.d * 0.6) * strength);
        }
    }
    let m = mushroom(q, kind, s, fk, sun);
    if (m.d < 0.9) {
        c = mix(c, knee(m.col), smoothstep(0.8, -0.8, m.d) * strength);
    }
    // grass blades in front of the foot, swaying a little
    for (var i = 0; i < 7; i++) {
        let fi = f32(i);
        let bx = (hash(vec2<f32>(fk, fi + 50.0)) - 0.5) * s * 1.5;
        let bh = s * (0.18 + 0.2 * hash(vec2<f32>(fi, fk + 51.0)));
        let bend = (hash(vec2<f32>(fi, fk + 52.0)) - 0.5) * 0.8 + 0.12 * sin(f.time * 1.3 + fi);
        let g = d0 - vec2<f32>(bx, s * 0.06);
        let tt = clamp(-g.y / bh, 0.0, 1.0);
        let gx = g.x - bend * tt * tt * bh;
        let bd = max(abs(gx) - s * 0.035 * (1.0 - tt), max(g.y, -g.y - bh));
        if (bd < 0.8) {
            let gc = mix(vec3<f32>(0.18, 0.2, 0.06), vec3<f32>(0.48, 0.4, 0.12), tt);
            c = mix(c, gc, smoothstep(0.6, -0.6, bd) * strength);
        }
    }
    // spores puffed from the cap after a press, glinting gold in the sun as they drift up and fade
    if (since >= 0.0 && since < 2.2) {
        for (var i = 0; i < 16; i++) {
            let fi = f32(i);
            let a = hash(vec2<f32>(fi, fk)) * 6.283;
            let sp = 12.0 + 22.0 * hash(vec2<f32>(fk, fi));
            let rise = vec2<f32>(cos(a) * sp * (1.0 - exp(-since * 2.5)) / 2.5 + sin(since * 3.0 + fi) * 2.0, -s * 0.9 - 18.0 * since - sin(a) * sp * 0.3 * since);
            let dd = length(d0 - rise);
            let fade = (1.0 - since / 2.2) * (0.6 + 0.4 * sin(since * 9.0 + fi));
            c += vec3<f32>(1.0, 0.86, 0.5) * smoothstep(1.4, 0.0, dd) * fade * 0.7;
        }
    }
    put(id.xy, c);
}
