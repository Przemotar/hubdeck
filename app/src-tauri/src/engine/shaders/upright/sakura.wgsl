// The upright copy of `sakura.wgsl` (owner, 2026-10-29: the deck may stand upright): the petals live in
// the seen part's own coordinates (`to_view`), so they fall down the deck as it stands and settle on
// the tops of icons as they stand; landscape it is the original.
// Japan: cherry petals drifting down over the keys, fluttering and turning, settling on an icon's top
// or a key's floor, a press blowing them off in a gust out over the other keys. Each petal lives on
// the card (`now`/`next`, 8 cells): x, y, its speed x and y, its turn, how fast it turns, the
// seconds it still rests (< 0 while it falls), its seed (0 = unborn). As `leaves.wgsl`, lighter.
const N: u32 = 64u;
fn count() -> u32 {
    return min(N, u32(30.0 * f.density + 0.5));
}
// the icons' cover at a point of the view
fn cover_v(v: vec2<f32>) -> f32 {
    let at = from_view(v);
    return cover(i32(at.x), i32(at.y));
}
fn rnd(a: f32, b: f32) -> f32 {
    return hash(vec2<f32>(a * 1.731 + 0.37, b * 0.917 + 11.3));
}
@compute @workgroup_size(16, 16)
fn step(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.y != 0u || id.x >= N) { return; }
    let i = id.x;
    let b = i * 8u;
    let size = view_size();
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
        for (var k = 0u; k < 8u; k++) { next[b + k] = 0.0; }
        next[b + 1u] = -1000.0;
        return;
    }
    let born = seed != 0.0;
    let gone = y > size.y + 20.0 || x < -60.0 || x > size.x + 60.0 || (rest >= 0.0 && rest < 0.001 && born);
    if (!born || gone) {
        seed = 1.0 + fi + floor(f.time * 7.0) * 0.013;
        // petals come from the upper left, as from a branch beyond the deck, drifting right
        x = rnd(seed, 1.0) * (size.x + 120.0) - 100.0;
        y = select(-14.0 - rnd(seed, 2.0) * 50.0, rnd(seed, 2.0) * size.y - 14.0, !born);
        vx = 10.0;
        vy = 12.0;
        turn = rnd(seed, 3.0) * 6.283;
        spin = (2.0 + rnd(seed, 4.0) * 3.5) * select(-1.0, 1.0, rnd(seed, 5.0) > 0.5);
        rest = -1.0;
    }
    if (rest > 0.0) {
        rest = max(rest - dt, 0.0);
        turn = mix(turn, round(turn / 3.14159) * 3.14159, 1.0 - exp(-dt * 6.0));
    } else {
        // a light flutter: a quick sway, a slow fall, a breeze to the right
        let fall = 13.0 + rnd(seed, 6.0) * 10.0;
        let sway = sin(f.time * (1.3 + rnd(seed, 7.0) * 1.1) + seed * 3.1) * (16.0 + rnd(seed, 8.0) * 12.0);
        let want = vec2<f32>(12.0 + f.wind * 45.0 + sway, fall * (0.6 + 0.4 * abs(cos(turn))));
        vx += (want.x - vx) * (1.0 - exp(-dt * 1.6));
        vy += (want.y - vy) * (1.0 - exp(-dt * 1.3));
        turn += spin * dt * (1.0 + length(vec2<f32>(vx, vy)) / 100.0);
        x += vx * dt;
        y += vy * dt;
        let k = key_at(from_view(vec2<f32>(x, y)));
        if (k >= 0 && vy > 0.0 && abs(vx) < 60.0) {
            let r = view_rect(keys[k]);
            let on_icon = cover_v(vec2<f32>(x, y + 4.0)) > 0.5 && cover_v(vec2<f32>(x, y - 3.0)) < 0.2;
            let on_floor = y + 4.0 >= r.y + r.w - 2.0;
            if ((on_icon || on_floor) && rnd(seed, floor(y / 7.0) + 21.0) < 0.55) {
                rest = 6.0 + rnd(seed, 9.0) * 9.0;
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
            let sp = 240.0 * (0.55 + 0.6 * rnd(seed, 41.0)) * (0.7 + 0.5 * press[k].z);
            let at = to_view(press[k].xy);
            x = at.x + cos(a) * 6.0;
            y = at.y + sin(a) * 6.0;
            vx = cos(a) * sp;
            vy = sin(a) * sp - 240.0 * 0.35;
            spin = sign(spin + 0.001) * 12.0;
            rest = -1.0;
        }
    }
    // a press: a gust off the key, the petals near it out over the others, spinning
    for (var k = 0u; k < f.presses; k++) {
        let d = vec2<f32>(x, y) - to_view(press[k].xy);
        let dist = length(d);
        if (dist < 130.0) {
            let push = pow(1.0 - dist / 130.0, 1.1) * (300.0 + 240.0 * press[k].z);
            let dir = normalize(d + vec2<f32>(rnd(seed, 30.0) - 0.5, -0.45) * 14.0 + vec2<f32>(0.001, 0.0));
            vx += dir.x * push;
            vy += dir.y * push - 80.0 * (1.0 - dist / 130.0);
            spin *= 3.0;
            rest = -1.0;
        }
    }
    spin = clamp(spin, -18.0, 18.0);
    spin += (sign(spin) * (2.0 + rnd(seed, 4.0) * 3.5) - spin) * (1.0 - exp(-dt * 0.8));
    next[b] = x;
    next[b + 1u] = y;
    next[b + 2u] = vx;
    next[b + 3u] = vy;
    next[b + 4u] = turn;
    next[b + 5u] = spin;
    next[b + 6u] = rest;
    next[b + 7u] = seed;
}
// a cherry petal: a rounded oval narrowing to its base, with the small notch at its tip
fn petal(q: vec2<f32>, r: f32) -> f32 {
    // t: 0 at the base (+y), 1 at the tip (-y); widest two thirds of the way out, the tip rounded
    let t = clamp((r - q.y) / (2.0 * r), 0.0, 1.0);
    let w = r * 0.86 * pow(t, 0.5) * (1.0 - 0.12 * t) + 0.5;
    var d = max(abs(q.x) - w, abs(q.y) - r);
    d = max(d, length(q - vec2<f32>(0.0, -r * 0.12)) - r * 0.92);
    let notch = length(q - vec2<f32>(0.0, -r * 1.06)) - r * 0.24;
    return max(d, -notch);
}
@compute @workgroup_size(16, 16)
fn over(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= f.size.x || id.y >= f.size.y) { return; }
    let p = to_view(vec2<f32>(f32(id.x), f32(id.y)) + 0.5);
    var c = px(i32(id.x), i32(id.y)).rgb;
    let tint = f.tint.rgb;
    for (var i = 0u; i < count(); i++) {
        let b = i * 8u;
        let pos = vec2<f32>(now[b], now[b + 1u]);
        let seed = now[b + 7u];
        if (seed == 0.0) { continue; }
        let d0 = p - pos;
        if (abs(d0.x) > 11.0 || abs(d0.y) > 11.0) { continue; }
        let turn = now[b + 4u];
        let rest = now[b + 6u];
        let r = 3.6 + rnd(seed, 13.0) * 2.4;
        let a = seed * 5.0 + turn * 0.4;
        let cs = vec2<f32>(cos(a), sin(a));
        let face = cos(turn);
        let narrow = 0.42 + 0.58 * abs(face);
        let fade = select(1.0, clamp(rest / 1.2, 0.0, 1.0), rest > 0.0) * min(f.strength / 0.6, 1.4);
        var qs = d0 - vec2<f32>(1.0, 1.6);
        qs = vec2<f32>(qs.x * cs.x + qs.y * cs.y, -qs.x * cs.y + qs.y * cs.x);
        qs.x /= narrow;
        c *= 1.0 - 0.2 * smoothstep(1.5, -0.8, petal(qs, r) * narrow) * min(fade, 1.0);
        var q = vec2<f32>(d0.x * cs.x + d0.y * cs.y, -d0.x * cs.y + d0.y * cs.x);
        q.x /= narrow;
        let d = petal(q, r) * narrow;
        if (d > 1.0) { continue; }
        // pale pink, near white at the tip, deeper pink at the base; the back a touch greyer
        let along = clamp((q.y + r) / (2.0 * r), 0.0, 1.0);
        var col = mix(vec3<f32>(1.0, 0.97, 0.98), tint, 0.35 + 0.5 * along);
        col = mix(col, vec3<f32>(0.93, 0.45, 0.62), smoothstep(0.7, 1.0, along) * 0.55);
        if (face < 0.0) { col *= 0.9; }
        col *= 0.84 + 0.16 * abs(face);
        // fine lines along it, and a brighter rim where light passes through the edge
        col *= 1.0 - 0.05 * smoothstep(0.3, 0.0, abs(fract(q.x / (r * 0.4)) - 0.5));
        col += vec3<f32>(0.08, 0.05, 0.06) * smoothstep(-1.4, -0.2, d);
        c = mix(c, col, smoothstep(0.7, -0.7, d) * min(fade, 1.0) * 0.95);
    }
    put(id.xy, c);
}
