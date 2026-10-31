// Poland's golden autumn: leaves of three Polish trees (Norway maple, pedunculate oak, birch)
// tumbling down over the keys, turning in 3D as they fall, nearer ones bigger, sharper and quicker,
// farther ones smaller, softer and hazed by the golden air; they settle on an icon's top or a key's
// floor for a while; a press throws a handful out of the key and blows the near ones away. Each leaf lives on the card (`now`/`next`, 8 cells): x, y, its speed x and y, its
// turn, how fast it turns, the seconds it still rests (< 0 while it falls), its seed (0 = unborn).
const N: u32 = 64u;
fn count() -> u32 {
    return min(N, u32(22.0 * f.density + 0.5));
}
fn rnd(a: f32, b: f32) -> f32 {
    return hash(vec2<f32>(a * 1.731 + 0.37, b * 0.917 + 11.3));
}
@compute @workgroup_size(16, 16)
fn step(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.y != 0u || id.x >= N) { return; }
    let i = id.x;
    let b = i * 8u;
    let size = vec2<f32>(f.size);
    let fi = f32(i);
    var x = now[b];
    var y = now[b + 1u];
    var vx = now[b + 2u];
    var vy = now[b + 3u];
    var turn = now[b + 4u];
    var spin = now[b + 5u];
    var rest = now[b + 6u];
    var seed = now[b + 7u];
    let dt = clamp(f.dt, 0.0, 0.1);
    if (i >= count()) {
        // not on the deck now (fewer leaves asked for)
        for (var k = 0u; k < 8u; k++) { next[b + k] = 0.0; }
        next[b + 1u] = -1000.0;
        return;
    }
    let born = seed != 0.0;
    let gone = y > size.y + 24.0 || x < -60.0 || x > size.x + 60.0 || (rest >= 0.0 && rest < 0.001 && born);
    if (!born || gone) {
        // a new leaf above the deck; the first ones spread over it so it is not empty at the start
        seed = 1.0 + fi + floor(f.time * 7.0) * 0.013;
        x = rnd(seed, 1.0) * (size.x + 80.0) - 40.0;
        y = select(-18.0 - rnd(seed, 2.0) * 60.0, rnd(seed, 2.0) * size.y - 20.0, !born);
        vx = 0.0;
        vy = 18.0;
        turn = rnd(seed, 3.0) * 6.283;
        spin = (1.4 + rnd(seed, 4.0) * 2.6) * select(-1.0, 1.0, rnd(seed, 5.0) > 0.5);
        rest = -1.0;
    }
    if (rest > 0.0) {
        // resting where it landed, lying flat; then it goes (faded in `over`)
        rest = max(rest - dt, 0.0);
        turn = mix(turn, round(turn / 3.14159) * 3.14159, 1.0 - exp(-dt * 6.0));
    } else {
        // falling: a slow sway with the wind, the fall speed of a leaf, its tumble
        let z = rnd(seed, 15.0);
        let fall = (20.0 + rnd(seed, 6.0) * 16.0) * mix(0.65, 1.3, z);
        let sway = sin(f.time * (0.7 + rnd(seed, 7.0) * 0.6) + seed * 3.1) * (14.0 + rnd(seed, 8.0) * 14.0) * mix(0.7, 1.2, z);
        let want = vec2<f32>(f.wind * 45.0 + sway, fall * (0.75 + 0.25 * abs(cos(turn))));
        vx += (want.x - vx) * (1.0 - exp(-dt * 1.3));
        vy += (want.y - vy) * (1.0 - exp(-dt * 1.1));
        turn += spin * dt * (1.0 + length(vec2<f32>(vx, vy)) / 120.0);
        x += vx * dt;
        y += vy * dt;
        // landing: on the top of an icon or on the floor of the key it is in, when it comes down slowly
        let k = key_at(vec2<f32>(x, y));
        if (k >= 0 && vy > 0.0 && abs(vx) < 70.0) {
            let r = keys[k];
            let on_icon = cover(i32(x), i32(y + 6.0)) > 0.5 && cover(i32(x), i32(y - 3.0)) < 0.2;
            let on_floor = y + 6.0 >= r.y + r.w - 2.0;
            if ((on_icon || on_floor) && rnd(seed, floor(y / 9.0) + 21.0) < 0.6) {
                rest = 5.0 + rnd(seed, 9.0) * 8.0;
                vx = 0.0;
                vy = 0.0;
            }
        }
    }
    // a press: a handful thrown out of the pressed key itself (a quarter of them), whatever is near
    // blown away
    for (var k = 0u; k < f.presses; k++) {
        if (rnd(seed, f.time * 3.7 + f32(k)) < 0.28) {
            let a = rnd(seed, 40.0 + f.time) * 6.283;
            let sp = 150.0 * (0.55 + 0.6 * rnd(seed, 41.0)) * (0.7 + 0.5 * press[k].z);
            x = press[k].x + cos(a) * 6.0;
            y = press[k].y + sin(a) * 6.0;
            vx = cos(a) * sp;
            vy = sin(a) * sp - 150.0 * 0.35;
            spin = sign(spin + 0.001) * 12.0;
            rest = -1.0;
        }
    }
    // presses blow the leaves near them out over the other keys, the resting ones lifted
    for (var k = 0u; k < f.presses; k++) {
        let d = vec2<f32>(x, y) - press[k].xy;
        let dist = length(d);
        if (dist < 120.0) {
            let push = pow(1.0 - dist / 120.0, 1.2) * (170.0 + 140.0 * press[k].z);
            let dir = normalize(d + vec2<f32>(rnd(seed, 30.0) - 0.5, -0.35) * 12.0 + vec2<f32>(0.001, 0.0));
            vx += dir.x * push;
            vy += dir.y * push - 70.0 * (1.0 - dist / 120.0);
            spin *= 2.5;
            rest = -1.0;
        }
    }
    // the air: a leaf is light, so a fast one slows hard and soon flutters again, staying on the deck
    // (owner, 2026-10-30: the press's leaves "mega szybko rozpraszają się i wystrzeliwują na boki tak
    // że ledwo je widać"); the slow fall is not touched
    let speed = length(vec2<f32>(vx, vy));
    if (speed > 40.0) {
        let slow = exp(-dt * speed / 90.0);
        vx *= slow;
        vy *= slow;
    }
    spin = clamp(spin, -14.0, 14.0);
    spin += (sign(spin) * (1.4 + rnd(seed, 4.0) * 2.6) - spin) * (1.0 - exp(-dt * 0.8));
    next[b] = x;
    next[b + 1u] = y;
    next[b + 2u] = vx;
    next[b + 3u] = vy;
    next[b + 4u] = turn;
    next[b + 5u] = spin;
    next[b + 6u] = rest;
    next[b + 7u] = seed;
}
// --- the leaves' shapes, in the leaf's own space: the tip up (-y), the stalk down (+y), R its size;
// negative inside
fn smin(a: f32, b: f32, k: f32) -> f32 {
    let h = clamp(0.5 + 0.5 * (b - a) / k, 0.0, 1.0);
    return mix(b, a, h) - k * h * (1.0 - h);
}
fn segment(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>) -> f32 {
    let ab = b - a;
    let t = clamp(dot(p - a, ab) / dot(ab, ab), 0.0, 1.0);
    return length(p - a - ab * t);
}
fn dir_of(ang: f32) -> vec2<f32> {
    return vec2<f32>(sin(ang), -cos(ang));
}
// one lobe of a maple leaf from the centre out along `ang` (0 = up): broad in its middle, drawn to a
// point, with two sharp teeth on each side as the Norway maple has
fn lobe(q: vec2<f32>, ang: f32, len: f32, wid: f32) -> f32 {
    let d = dir_of(ang);
    let u = dot(q, d);
    let v = abs(q.x * d.y - q.y * d.x);
    let t = clamp(u / len, 0.0, 1.0);
    var w = wid * pow(sin(3.14159 * min(t * 0.94 + 0.06, 1.0)), 0.62) * (1.0 - 0.28 * t);
    w += wid * 0.42 * max(0.0, 1.0 - abs(t - 0.5) / 0.08);
    w += wid * 0.3 * max(0.0, 1.0 - abs(t - 0.74) / 0.06);
    return max(v - w, max(-u, u - len));
}
const LOBES = array<vec3<f32>, 5>(
    vec3<f32>(0.0, 1.0, 0.36),
    vec3<f32>(1.1, 0.9, 0.33),
    vec3<f32>(-1.1, 0.9, 0.33),
    vec3<f32>(2.25, 0.5, 0.22),
    vec3<f32>(-2.25, 0.5, 0.22),
);
fn maple(q: vec2<f32>, r: f32) -> f32 {
    var d = length(q) - r * 0.22;
    for (var i = 0; i < 5; i++) {
        let l = LOBES[i];
        d = smin(d, lobe(q, l.x, r * l.y, r * l.z), r * 0.07);
    }
    let stalk = segment(q, vec2<f32>(0.0, r * 0.15), vec2<f32>(r * 0.14, r * 1.05)) - r * 0.045;
    return min(d, stalk);
}
fn maple_veins(q: vec2<f32>, r: f32) -> f32 {
    var v = segment(q, vec2<f32>(0.0, r * 0.15), vec2<f32>(r * 0.14, r * 1.05));
    for (var i = 0; i < 5; i++) {
        let l = LOBES[i];
        v = min(v, segment(q, vec2<f32>(0.0, r * 0.12), dir_of(l.x) * r * l.y * 0.92));
    }
    return v;
}
// the oak: obovate, widest beyond its middle, rounded lobes with deep narrow bays, a short stalk
fn oak_width(t: f32, r: f32, lobes: f32, phase: f32) -> f32 {
    let env = r * 0.52 * pow(sin(3.14159 * pow(t, 0.78)), 0.75);
    return env * (0.46 + 0.54 * pow(abs(sin(3.14159 * (t * lobes + phase))), 0.5));
}
var<private> OAK: vec3<f32>;
fn oak(q: vec2<f32>, r: f32) -> f32 {
    let t = clamp((r - q.y) / (2.0 * r), 0.0, 1.0);
    let phase = select(OAK.y, OAK.z, q.x > 0.0);
    let d = max(abs(q.x) - oak_width(t, r, OAK.x, phase), abs(q.y) - r);
    let stalk = segment(q, vec2<f32>(0.0, r * 0.9), vec2<f32>(r * 0.04, r * 1.22)) - r * 0.05;
    return min(d, stalk);
}
fn oak_veins(q: vec2<f32>, r: f32) -> f32 {
    var v = segment(q, vec2<f32>(0.0, r * 1.2), vec2<f32>(0.0, -r * 0.85));
    for (var k = 0; k < 5; k++) {
        let t = (f32(k) + 0.5 - OAK.y) / OAK.x;
        if (t > 0.95) { break; }
        let y = r - t * 2.0 * r;
        let phase = select(OAK.y, OAK.z, q.x > 0.0);
        let w = oak_width(t, r, OAK.x, phase) * 0.8;
        v = min(v, segment(vec2<f32>(abs(q.x), q.y), vec2<f32>(0.0, y + r * 0.18), vec2<f32>(w, y)));
    }
    return v;
}
// the birch: a small ovate leaf drawn to a point, finely toothed
fn birch_width(t: f32, r: f32) -> f32 {
    return r * 0.62 * pow(sin(3.14159 * pow(t, 0.62)), 1.1) + r * 0.035 * abs(sin(t * 46.0));
}
fn birch(q: vec2<f32>, r: f32) -> f32 {
    let t = clamp((r - q.y) / (2.0 * r), 0.0, 1.0);
    let d = max(abs(q.x) - birch_width(t, r), abs(q.y) - r);
    let stalk = segment(q, vec2<f32>(0.0, r * 0.95), vec2<f32>(-r * 0.06, r * 1.35)) - r * 0.04;
    return min(d, stalk);
}
fn birch_veins(q: vec2<f32>, r: f32) -> f32 {
    var v = segment(q, vec2<f32>(0.0, r * 1.3), vec2<f32>(0.0, -r * 0.85));
    for (var k = 0; k < 6; k++) {
        let t = (f32(k) + 0.6) / 7.0;
        let y = r - t * 2.0 * r;
        v = min(v, segment(vec2<f32>(abs(q.x), q.y), vec2<f32>(0.0, y + r * 0.1), vec2<f32>(birch_width(t, r) * 0.85, y - r * 0.12)));
    }
    return v;
}
// which tree: 0 maple, 1 oak, 2 birch
fn species(seed: f32) -> i32 {
    let s = rnd(seed, 12.0);
    if (s < 0.5) { return 0; }
    if (s < 0.8) { return 1; }
    return 2;
}
fn size_of(sp: i32, z: f32) -> f32 {
    let r = mix(8.5, 17.5, z);
    if (sp == 1) { return r * 1.05; }
    if (sp == 2) { return r * 0.72; }
    return r;
}
fn shape(q: vec2<f32>, r: f32, sp: i32) -> f32 {
    if (sp == 0) { return maple(q, r); }
    if (sp == 1) { return oak(q, r); }
    return birch(q, r);
}
fn vein(q: vec2<f32>, r: f32, sp: i32) -> f32 {
    if (sp == 0) { return maple_veins(q, r); }
    if (sp == 1) { return oak_veins(q, r); }
    return birch_veins(q, r);
}
// the leaf's colour near its veins and at its edge, by tree and by leaf
fn colours(sp: i32, s: f32) -> array<vec3<f32>, 2> {
    if (sp == 1) { return array<vec3<f32>, 2>(vec3<f32>(0.74, 0.46, 0.12), vec3<f32>(0.42, 0.18, 0.05)); }
    if (sp == 2) { return array<vec3<f32>, 2>(vec3<f32>(1.0, 0.86, 0.22), vec3<f32>(0.93, 0.62, 0.08)); }
    if (s < 0.3) { return array<vec3<f32>, 2>(vec3<f32>(1.0, 0.8, 0.14), vec3<f32>(1.0, 0.48, 0.04)); }
    if (s < 0.7) { return array<vec3<f32>, 2>(vec3<f32>(1.0, 0.55, 0.05), vec3<f32>(0.8, 0.1, 0.03)); }
    return array<vec3<f32>, 2>(vec3<f32>(0.9, 0.22, 0.05), vec3<f32>(0.48, 0.03, 0.05));
}
@compute @workgroup_size(16, 16)
fn over(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= f.size.x || id.y >= f.size.y) { return; }
    let p = vec2<f32>(f32(id.x), f32(id.y)) + 0.5;
    var c = px(i32(id.x), i32(id.y)).rgb;
    let k = min(f.strength / 0.6, 1.4);
    // the sun is low beyond the upper left (as `goldlight` draws it)
    let sun = normalize(vec2<f32>(-0.75, -0.6));
    let haze = vec3<f32>(0.98, 0.74, 0.44);
    // the far leaves first, then the near ones over them
    for (var layer = 0; layer < 2; layer++) {
        for (var i = 0u; i < count(); i++) {
            let b = i * 8u;
            let seed = now[b + 7u];
            if (seed == 0.0) { continue; }
            let z = rnd(seed, 15.0);
            if ((layer == 0) != (z < 0.5)) { continue; }
            let pos = vec2<f32>(now[b], now[b + 1u]);
            let d0 = p - pos;
            if (abs(d0.x) > 32.0 || abs(d0.y) > 32.0) { continue; }
            let turn = now[b + 4u];
            let rest = now[b + 6u];
            let sp = species(seed);
            let r = size_of(sp, z);
            OAK = vec3<f32>(4.0 + floor(rnd(seed, 16.0) * 2.0), rnd(seed, 17.0) * 0.3, rnd(seed, 18.0) * 0.3);
            // its angle in the screen, and the turn about its long axis (3D: it narrows, shows its back)
            let a = seed * 5.0 + turn * 0.35;
            let cs = vec2<f32>(cos(a), sin(a));
            let face = cos(turn);
            let narrow = 0.3 + 0.7 * abs(face);
            let fade = select(1.0, clamp(rest / 1.2, 0.0, 1.0), rest > 0.0) * k;
            let aa = mix(1.3, 0.65, z);
            // its shadow on what is under it: farther above the keys for a nearer leaf, softer
            let off = mix(1.5, 5.0, z);
            var qs = d0 - vec2<f32>(off * 0.7, off);
            qs = vec2<f32>(qs.x * cs.x + qs.y * cs.y, -qs.x * cs.y + qs.y * cs.x);
            qs.x /= narrow;
            let ds = shape(qs, r, sp) * narrow;
            c *= 1.0 - mix(0.14, 0.34, z) * smoothstep(off * 0.7 + 0.5, -off * 0.5, ds) * min(fade, 1.0);
            var q = vec2<f32>(d0.x * cs.x + d0.y * cs.y, -d0.x * cs.y + d0.y * cs.x);
            q.x /= narrow;
            let d = shape(q, r, sp) * narrow;
            if (d > aa) { continue; }
            // colour: from the veins out to the edge, a few spots of autumn
            let pal = colours(sp, rnd(seed, 14.0));
            let v = vein(q, r, sp) * narrow;
            let out = clamp(min(v / (r * 0.45), 1.0) * 0.75 + 0.25 * smoothstep(-r * 0.5, 0.0, d), 0.0, 1.0);
            var col = mix(pal[0], pal[1], out);
            col *= 1.0 - 0.22 * smoothstep(0.66, 0.82, vnoise(q * (3.2 / r) + seed * 9.0));
            // the veins lighter, as on a turned leaf; the edge a little darker and drier
            col = mix(col, pal[0] * 1.12 + 0.04, smoothstep(r * 0.05 + 0.35, 0.0, v) * 0.5);
            col *= 0.66 + 0.34 * smoothstep(0.0, -1.3, d);
            // light: a cupped leaf lit from the sun, edge-on darker; its back paler
            let side = clamp(-dot(q / r, vec2<f32>(sun.x * sign(face + 0.0001), sun.y)) * 0.5 + 0.5, 0.0, 1.0);
            col *= (0.72 + 0.4 * side) * (0.8 + 0.2 * abs(face));
            if (face < 0.0) { col = mix(col, vec3<f32>(dot(col, vec3<f32>(0.3, 0.55, 0.15))), 0.2) * 1.05; }
            // the sun through the leaf: a turned leaf glows warm, its rim on the sun's side lit
            col += vec3<f32>(0.32, 0.14, 0.02) * smoothstep(0.1, -0.7, face) * (0.6 + 0.4 * z);
            let rim = smoothstep(-1.8, -0.2, d) * clamp(-dot(normalize(d0 + vec2<f32>(0.001)), sun), 0.0, 1.0);
            col += vec3<f32>(0.5, 0.3, 0.08) * rim * 0.6;
            // the farther ones hazed by the golden air
            col = mix(col, haze, (1.0 - z) * 0.18);
            c = mix(c, col, smoothstep(aa, -aa, d) * min(fade, 1.0) * mix(0.85, 1.0, z));
        }
    }
    put(id.xy, c);
}
