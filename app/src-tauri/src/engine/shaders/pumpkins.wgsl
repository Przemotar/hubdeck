// Halloween: jack-o'-lanterns sitting on the keys' floors, behind the icons and labels, every one
// different (owner, 2026-10-26: the pumpkins were copies, they need more variety): its size, how
// squat or tall, three or five ribs, a lean, the stem's length and curve, its shade (deep orange,
// amber, red-orange, now and then a pale ghost pumpkin) and one of six carved faces, all from its
// key's number. A candle inside flickers on its own and lights the floor round it; a press makes
// that key's pumpkin flare like a candle in a draught. The engine places them (`aux_at(2k)`,
// `aux_at(2k + 1)`: key k's pumpkin, 0 = none; `aux_at(32 + k)`: seconds since key k was pressed).
fn candle(k: f32, t: f32) -> f32 {
    return 0.72 + 0.20 * vnoise(vec2<f32>(t * 7.0, k * 13.0)) + 0.08 * vnoise(vec2<f32>(t * 23.0, k * 5.0));
}
// the body: lobes side by side, its outline (in px) and how near a rib it is (0..1)
fn body(q: vec2<f32>, w: f32, h: f32, five: bool) -> vec2<f32> {
    let mid = length(q / vec2<f32>(w * 0.34, h * 0.5)) - 1.0;
    let off = select(0.22, 0.27, five);
    let l1 = length((q + vec2<f32>(w * off, h * 0.02)) / vec2<f32>(w * 0.30, h * 0.47)) - 1.0;
    let r1 = length((q - vec2<f32>(w * off, -h * 0.02)) / vec2<f32>(w * 0.30, h * 0.47)) - 1.0;
    var d = min(mid, min(l1, r1));
    var ridge = 1.0 - smoothstep(0.0, 0.3, abs(mid - min(l1, r1)));
    if (five) {
        let l2 = length((q + vec2<f32>(w * 0.12, 0.0)) / vec2<f32>(w * 0.26, h * 0.5)) - 1.0;
        let r2 = length((q - vec2<f32>(w * 0.12, 0.0)) / vec2<f32>(w * 0.26, h * 0.5)) - 1.0;
        ridge = max(ridge, (1.0 - smoothstep(0.0, 0.25, abs(l2 - r2))) * step(abs(q.x), w * 0.2));
        d = min(d, min(l2, r2));
    }
    return vec2<f32>(d * h * 0.5, ridge);
}
fn tri(u: vec2<f32>, c: vec2<f32>, s: f32, down: bool) -> f32 {
    // a small triangle, its point down or up
    let q = (u - c) / s;
    let y = select(q.y, -q.y, down);
    return max(abs(q.x) * 1.7 + y, -y - 0.6) * s;
}
// the carved face, one of six; negative inside the cut (u: body units, x -0.5..0.5)
fn face(u: vec2<f32>, kind: i32) -> f32 {
    var eyes = 1.0;
    var mouth = 1.0;
    let lx = vec2<f32>(-0.17, -0.07);
    let rx = vec2<f32>(0.17, -0.07);
    if (kind == 0) {
        // classic: triangles, a toothy smile
        eyes = min(tri(u, lx, 0.11, false), tri(u, rx, 0.11, false));
        let curve = 0.13 + 0.9 * u.x * u.x;
        let teeth = 0.04 * step(0.55, fract(u.x * 6.0 + 0.5)) * step(0.0, curve - u.y);
        mouth = max(abs(u.y - curve) - 0.055 + teeth, abs(u.x) - 0.27);
    } else if (kind == 1) {
        // surprised: round eyes, an O
        eyes = min(length(u - lx) - 0.075, length(u - rx) - 0.075);
        mouth = (length((u - vec2<f32>(0.0, 0.16)) / vec2<f32>(0.08, 0.1)) - 1.0) * 0.08;
    } else if (kind == 2) {
        // angry: slanted eyes, a jagged frown
        let el = max(tri(u, lx, 0.12, true), -(u.y + 0.02 + (u.x + 0.17) * 0.7));
        let er = max(tri(u, rx, 0.12, true), -(u.y + 0.02 - (u.x - 0.17) * 0.7));
        eyes = min(el, er);
        let curve = 0.2 - 0.7 * u.x * u.x;
        let zig = abs(fract(u.x * 8.0) - 0.5) * 0.06;
        mouth = max(abs(u.y - curve - zig) - 0.04, abs(u.x) - 0.26);
    } else if (kind == 3) {
        // happy: crescent eyes, a wide grin
        let cl = max(length(u - lx - vec2<f32>(0.0, 0.03)) - 0.09, -(length(u - lx - vec2<f32>(0.0, 0.07)) - 0.08));
        let cr = max(length(u - rx - vec2<f32>(0.0, 0.03)) - 0.09, -(length(u - rx - vec2<f32>(0.0, 0.07)) - 0.08));
        eyes = min(cl, cr);
        let curve = 0.1 + 1.1 * u.x * u.x;
        mouth = max(max(u.y - 0.26, curve - u.y), abs(u.x) - 0.3);
    } else if (kind == 4) {
        // a wink: one eye shut (a line), a smirk to one side
        let shut = max(abs(u.y + 0.06 - (u.x + 0.17) * (u.x + 0.17) * 3.0) - 0.022, abs(u.x + 0.17) - 0.1);
        eyes = min(shut, tri(u, rx, 0.11, false));
        let curve = 0.14 + 0.5 * (u.x - 0.08) * (u.x - 0.08) - 0.12 * u.x;
        mouth = max(abs(u.y - curve) - 0.045, abs(u.x - 0.04) - 0.22);
    } else {
        // diamonds and a zigzag
        eyes = min(abs(u.x - lx.x) + abs(u.y - lx.y) - 0.09, abs(u.x - rx.x) + abs(u.y - rx.y) - 0.09);
        let zig = abs(fract(u.x * 6.0) - 0.5) * 0.12;
        mouth = max(abs(u.y - 0.15 - zig) - 0.04, abs(u.x) - 0.27);
    }
    return min(eyes, mouth);
}
@compute @workgroup_size(16, 16)
fn under(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= f.size.x || id.y >= f.size.y) { return; }
    let p = vec2<f32>(f32(id.x), f32(id.y)) + 0.5;
    var c = px(i32(id.x), i32(id.y)).rgb;
    let k = key_at(p);
    if (k < 0) { put(id.xy, c); return; }
    let at = vec2<f32>(aux_at(2 * k), aux_at(2 * k + 1));
    if (at.x <= 0.0) { put(id.xy, c); return; }
    let fk = f32(k);
    // this pumpkin's own shape and face
    let h1 = hash(vec2<f32>(fk, 31.0));
    let h2 = hash(vec2<f32>(fk, 47.0));
    let h3 = hash(vec2<f32>(fk, 59.0));
    let w = 22.0 + 8.0 * h1;
    let h = w * (0.62 + 0.28 * h2);
    let five = h3 > 0.5;
    let lean = (hash(vec2<f32>(fk, 71.0)) - 0.5) * 0.24;
    let kind = i32(hash(vec2<f32>(fk, 83.0)) * 5.999);
    let pale = hash(vec2<f32>(fk, 97.0)) > 0.86;
    let since = aux_at(32 + k);
    let flare = exp(-since * 2.2) * step(0.0, since);
    let light = candle(fk, f.time) * (1.0 + 1.6 * flare);
    // sitting on the floor: its middle half its height above the floor
    let base = at + vec2<f32>(0.0, 5.0 - h * 0.5);
    let d0 = p - base;
    let q = vec2<f32>(d0.x * cos(lean) - d0.y * sin(lean), d0.x * sin(lean) + d0.y * cos(lean));
    let spill = exp(-dot(d0, d0) / (2.0 * 17.0 * 17.0)) * light;
    c += vec3<f32>(1.0, 0.45, 0.10) * spill * 0.45 * f.strength;
    let b = body(q, w, h, five);
    if (b.x < 1.0) {
        let u = q / w;
        var deep = vec3<f32>(0.95, 0.42, 0.05);
        if (h1 > 0.66) { deep = vec3<f32>(0.98, 0.55, 0.12); }
        if (h1 < 0.25) { deep = vec3<f32>(0.85, 0.28, 0.05); }
        if (pale) { deep = vec3<f32>(0.92, 0.88, 0.78); }
        var shell = mix(deep, deep * 0.5, clamp(u.y * 1.6 + 0.4, 0.0, 1.0));
        shell *= 1.0 - 0.45 * b.y;
        shell *= 0.5 + 0.5 * smoothstep(0.0, -3.0, b.x);
        let fd = face(vec2<f32>(u.x, u.y * w / h * 0.72), kind);
        let carved = smoothstep(0.02, -0.02, fd);
        let flame = mix(vec3<f32>(1.0, 0.55, 0.08), vec3<f32>(1.0, 0.95, 0.55), smoothstep(0.0, -0.05, fd)) * (0.6 + 0.8 * light);
        let inside = mix(shell * (0.55 + 0.25 * light), flame, carved);
        c = mix(c, inside, smoothstep(0.8, -0.6, b.x) * (0.75 + 0.25 * f.strength));
    }
    // the stem: curved, its own length
    let stem_len = 3.0 + 4.0 * hash(vec2<f32>(fk, 101.0));
    let bend = (hash(vec2<f32>(fk, 103.0)) - 0.5) * 0.9;
    let sq = q - vec2<f32>((q.y + h * 0.5) * bend, -h * 0.5 - stem_len * 0.5 + 1.0);
    let stem = max(abs(sq.x) - 1.2 - 0.3 * (sq.y / stem_len), abs(sq.y) - stem_len * 0.5);
    c = mix(c, vec3<f32>(0.25, 0.30, 0.10), smoothstep(0.6, -0.4, stem) * 0.95);
    put(id.xy, c);
}
