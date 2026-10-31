// The upright copy of `synthwave.wgsl` (owner, 2026-10-29: the deck may stand upright): drawn in
// the seen part's own coordinates (`to_view`); upright the sky takes the top three rows, the sun
// and the mountains sit on the third, the grid runs over the last two. Landscape it is the original.
// Gaming / neon, for gamers and streamers: a synthwave night through the fifteen windows. The top
// row a deep violet sky with stars and a big sun banded by stripes, sinking behind the horizon; the
// middle row a range of wireframe mountains glowing magenta and cyan; the bottom row a neon grid
// running to the horizon and rushing towards you, faint scanlines over it all. A press: a hit
// marker flashes on the key, a shock of light runs along the grid from it and a glitch wave tears
// the neighbouring keys into shifted, colour-split slices (`aux[0..3]`: x, y, start, 1 when on).
fn upright() -> bool {
    return view_size().x < view_size().y;
}
fn horizon() -> f32 {
    return select(180.0, 300.0, upright());
}
fn sun_r() -> f32 {
    return select(52.0, 56.0, upright());
}
fn sun_c() -> vec2<f32> {
    return vec2<f32>(view_size().x * 0.5, horizon() - 34.0);
}
// how the floor's depth is squeezed: upright it runs over two rows, not one, and would rush past
// twice as slow and wide
fn floor_k() -> f32 {
    return select(1.0, 0.62, upright());
}
fn ridge(x: f32) -> f32 {
    let HORIZON = horizon();
    // the mountains' top: a few peaks, sharper near the middle
    let m = 0.55 * abs(sin(x * 0.021 + 1.3)) + 0.35 * abs(sin(x * 0.047 + 0.4)) + 0.2 * abs(sin(x * 0.093 + 2.0));
    let k = view_size().x / 480.0;
    let valley = smoothstep(70.0 * k, 150.0 * k, abs(x - view_size().x * 0.5));
    return HORIZON - (8.0 + 38.0 * m) * (0.35 + 0.65 * valley);
}
fn scene(p: vec2<f32>, t: f32) -> vec3<f32> {
    let HORIZON = horizon();
    let SUN_C = sun_c();
    let SUN_R = sun_r();
    var col: vec3<f32>;
    if (p.y < HORIZON) {
        // the sky: deep indigo to magenta at the horizon, stars
        let v = p.y / HORIZON;
        col = mix(vec3<f32>(0.03, 0.01, 0.1), vec3<f32>(0.42, 0.04, 0.36), pow(v, 1.6));
        let sc = floor(p / 3.0);
        let st = hash(sc + 4.2);
        if (st > 0.985) { col += vec3<f32>(0.8, 0.8, 1.0) * (0.4 + 0.6 * sin(t * (1.0 + st * 5.0) + st * 40.0)) * (1.0 - v) * smoothstep(1.6, 0.4, length(p - (sc + 0.5) * 3.0)); }
        // the sun: yellow to hot pink, bands cut out of its lower half, moving down
        let ds = length(p - SUN_C);
        if (ds < SUN_R + 1.0) {
            let sv = (p.y - (SUN_C.y - SUN_R)) / (2.0 * SUN_R);
            var sun = mix(vec3<f32>(1.0, 0.86, 0.3), vec3<f32>(1.0, 0.2, 0.55), smoothstep(0.1, 0.9, sv));
            let band = fract((p.y - t * 4.0) / 9.0);
            let cut = smoothstep(0.45, 1.0, sv) * 0.75;
            let gap = step(1.0 - cut, band);
            col = mix(col, sun, smoothstep(SUN_R + 0.8, SUN_R - 0.8, ds) * (1.0 - gap));
        }
        col += vec3<f32>(1.0, 0.3, 0.6) * exp(-max(length(p - SUN_C) - SUN_R, 0.0) / 26.0) * 0.35;
        // the mountains: dark, a wire mesh glowing on them, cyan rim
        let top = ridge(p.x);
        if (p.y > top) {
            col = vec3<f32>(0.04, 0.0, 0.08);
            let u = vec2<f32>(p.x / 14.0, (p.y - top) / 7.0 + p.x * 0.02);
            let wire = min(abs(fract(u.x) - 0.5), abs(fract(u.y) - 0.5));
            col += vec3<f32>(0.85, 0.15, 0.9) * smoothstep(0.08, 0.0, wire) * 0.6 * smoothstep(HORIZON, top, p.y);
            col += vec3<f32>(0.2, 0.95, 1.0) * smoothstep(1.4, 0.0, p.y - top);
        }
    } else {
        // the grid floor in perspective, rushing towards you
        let fy = (p.y - HORIZON) * floor_k();
        let z = 1.0 / max(fy, 0.5);
        let gx = (p.x - view_size().x * 0.5) * z * 3.0;
        let gz = 40.0 * z + t * 1.6;
        let lx = abs(fract(gx) - 0.5) / max(fwidth_x(p.x, z), 0.001);
        let lz = abs(fract(gz) - 0.5) / max(40.0 * z * z * floor_k(), 0.001);
        // near the horizon the lines come closer than a pixel: they fade into their average
        // glow there instead of beating into a moiré
        let across_px = 40.0 * z * z * floor_k();
        let side_px = z * 3.0;
        // the grid's rows only once they are 6 px apart or more: closer, they beat with the 2 px
        // scanlines into arcs of moiré
        let fade_z = smoothstep(0.167, 0.083, across_px);
        let fade_x = smoothstep(0.2, 0.07, side_px);
        let line = max(smoothstep(1.4, 0.2, lx * 0.5) * fade_x, smoothstep(1.4, 0.2, lz * 0.5) * fade_z) + 0.16 * (1.0 - fade_z);
        let far = exp(-fy / 50.0);
        col = vec3<f32>(0.05, 0.0, 0.1) + vec3<f32>(0.25, 0.0, 0.3) * far;
        col += mix(vec3<f32>(0.1, 0.9, 1.0), vec3<f32>(1.0, 0.2, 0.8), far) * line * (0.55 + 0.45 * (1.0 - far));
        // the horizon's glow
        // the horizon's glow, thin: a wide band lay bright under the bottom row's top (session 35)
        col += vec3<f32>(1.0, 0.25, 0.7) * exp(-(p.y - HORIZON) / 2.2) * 0.32;
    }
    return col;
}
// how many grid cells one pixel across spans at depth z (for an even line width)
fn fwidth_x(x: f32, z: f32) -> f32 {
    return z * 3.0;
}
@compute @workgroup_size(16, 16)
fn under(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= f.size.x || id.y >= f.size.y) { return; }
    let p = to_view(vec2<f32>(f32(id.x), f32(id.y)) + 0.5);
    var c = px(i32(id.x), i32(id.y)).rgb;
    let t = f.time * f.speed;
    var col = scene(p, t);
    // a press: a shock of light along the grid from the key
    for (var i = 0; i < 3; i++) {
        let b = f.aux[i];
        if (b.w < 0.5) { continue; }
        let age = f.time - b.z;
        if (age < 0.0 || age > 1.4) { continue; }
        let ring = abs(length((p - to_view(b.xy)) * vec2<f32>(1.0, 1.8)) - age * 320.0);
        col += vec3<f32>(0.3, 1.0, 1.0) * exp(-ring / 5.0) * (1.0 - age / 1.4) * 0.8;
    }
    // scanlines, faint on the floor: there they beat with the grid's rays into arcs of moiré
    let scan = select(0.14, 0.04, p.y > horizon());
    col *= 1.0 - scan + scan * step(0.5, fract(p.y * 0.5));
    let lum = dot(col, vec3<f32>(0.299, 0.587, 0.114));
    if (lum > 0.42) { col *= (0.42 + (lum - 0.42) * 0.4) / lum; }
    col = clamp(col, vec3<f32>(0.0), vec3<f32>(1.0));
    c = mix(c, col, f.strength / 0.6 * 0.97 * (1.0 - keep_bg(c)));
    put(id.xy, c);
}
@compute @workgroup_size(16, 16)
fn over(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= f.size.x || id.y >= f.size.y) { return; }
    let pp = vec2<f32>(f32(id.x), f32(id.y)) + 0.5;
    let p = to_view(pp);
    var c = px(i32(id.x), i32(id.y)).rgb;
    for (var i = 0; i < 3; i++) {
        let b = f.aux[i];
        if (b.w < 0.5) { continue; }
        let age = f.time - b.z;
        if (age < 0.0 || age > 1.2) { continue; }
        let k = key_at(b.xy);
        if (k < 0) { continue; }
        let r = keys[k];
        // the hit marker: four short slanted strokes round the key's middle, snapping in
        let q = p - to_view(b.xy);
        let snap = 1.0 - exp(-age * 18.0);
        let fade = 1.0 - smoothstep(0.25, 0.6, age);
        let a = abs(q);
        let diag = abs(a.x - a.y) / 1.414;
        let along = (a.x + a.y) / 1.414;
        let mark = smoothstep(1.6, 0.6, diag) * step(9.0 * snap, along) * step(along, 9.0 * snap + 9.0);
        c = mix(c, vec3<f32>(1.0, 1.0, 1.0), mark * fade);
        // the glitch wave: slices of the neighbouring keys shifted sideways, colours split
        let dist = length(p - to_view(b.xy));
        let front = age * 300.0;
        let near = exp(-abs(dist - front) / 30.0) * (1.0 - smoothstep(0.5, 1.2, age));
        if (near > 0.02 && key_inside(pp, r) <= 0.0) {
            let slice = floor(p.y / 4.0);
            let shift = (hash(vec2<f32>(slice, floor(f.time * 30.0))) - 0.5) * 16.0 * near * step(0.55, hash(vec2<f32>(slice, 7.0 + floor(f.time * 15.0))));
            let s = p + vec2<f32>(shift, 0.0);
            c = vec3<f32>(px_smooth(from_view(s + vec2<f32>(3.0 * near, 0.0))).r, px_smooth(from_view(s)).g, px_smooth(from_view(s - vec2<f32>(3.0 * near, 0.0))).b);
        }
    }
    put(id.xy, c);
}
