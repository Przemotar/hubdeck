//! The living deck's shader layers (`gpu.rs` has their inputs). Each is WGSL after the prelude:
//! `step` over its state, `under` and `over` over the panel's pixels. Rules of this screen (camera,
//! 2026-10-26): it crushes tones under ~#24 and shows each key through a small window; a pixel an
//! effect does not change is written back exactly (no dither there), so a layer at rest costs no
//! bytes.

/// Water: a wave field at the panel's full size (the CPU ripple's is half), 24 small steps a
/// frame; the rings bend the picture under them (smoothly, between pixels), gather light where
/// the surface curves (caustics, brightest on the icons, like a pool's floor), and glint where
/// they face the light. A press drops a stone.
pub const WATER: &str = r#"
fn h(x: i32, y: i32) -> f32 {
    let cx = clamp(x, 0, i32(f.size.x) - 1);
    let cy = clamp(y, 0, i32(f.size.y) - 1);
    return now[u32(cy) * f.size.x + u32(cx)];
}
fn smooth_px(p: vec2<f32>) -> vec3<f32> {
    let a = floor(p);
    let t = p - a;
    let x = i32(a.x);
    let y = i32(a.y);
    return mix(mix(px(x, y).rgb, px(x + 1, y).rgb, t.x), mix(px(x, y + 1).rgb, px(x + 1, y + 1).rgb, t.x), t.y);
}
@compute @workgroup_size(16, 16)
fn step(@builtin(global_invocation_id) id: vec3<u32>) {
    let w = f.size.x;
    if (id.x >= w || id.y >= f.size.y) { return; }
    let i = id.y * w + id.x;
    var v = 0.0;
    if (id.x > 0u && id.y > 0u && id.x < w - 1u && id.y < f.size.y - 1u) {
        v = ((now[i - 1u] + now[i + 1u] + now[i - w] + now[i + w]) * 0.5 - next[i]) * 0.9955;
    }
    for (var k = 0u; k < f.presses; k++) {
        let d = distance(vec2<f32>(f32(id.x), f32(id.y)), press[k].xy) / 15.0;
        if (d < 1.0) {
            v -= 7.0 * (0.4 + 0.6 * f.strength) * (0.5 + 0.5 * cos(d * 3.14159));
        }
    }
    next[i] = v;
}
@compute @workgroup_size(16, 16)
fn over(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= f.size.x || id.y >= f.size.y) { return; }
    let x = i32(id.x);
    let y = i32(id.y);
    let gx = h(x + 1, y) - h(x - 1, y);
    let gy = h(x, y + 1) - h(x, y - 1);
    let lap = h(x + 1, y) + h(x - 1, y) + h(x, y + 1) + h(x, y - 1) - 4.0 * h(x, y);
    let g = length(vec2<f32>(gx, gy));
    if (g < 0.004 && abs(lap) < 0.004) {
        dst[id.y * f.size.x + id.x] = src[id.y * f.size.x + id.x];
        return;
    }
    let k = 0.5 + 0.5 * f.strength;
    var c = smooth_px(vec2<f32>(f32(x), f32(y)) + vec2<f32>(gx, gy) * 3.2);
    let caustic = clamp(-lap * 1.6, 0.0, 1.0) * k;
    c += f.tint.rgb * caustic * (0.45 + 0.55 * cover(x, y));
    let n = normalize(vec3<f32>(-gx, -gy, 0.5));
    let l = normalize(vec3<f32>(-0.45, -0.7, 0.9));
    let spec = pow(max(dot(reflect(-l, n), vec3<f32>(0.0, 0.0, 1.0)), 0.0), 28.0) * min(g * 3.0, 1.0);
    c += vec3<f32>(spec * 0.9 * k);
    c += f.tint.rgb * min(g * 0.22, 0.45) * k;
    put(id.xy, c);
}
"#;

/// Light: a lamp that drifts round the deck, and the icons and labels as objects under it: the
/// keys' backgrounds lit in a soft pool round it, each icon's edge facing it lit in its colour and
/// the far edge darker, a shadow cast away from it onto the key behind. The lamp's colour is the
/// tint, its drift the speed, where it hangs the wind (the angle round the deck).
pub const LIGHT: &str = r#"
fn soft(x: i32, y: i32) -> f32 {
    var s = 0.0;
    for (var dy = -2; dy <= 2; dy++) {
        for (var dx = -2; dx <= 2; dx++) {
            s += cover(x + dx, y + dy);
        }
    }
    return s / 25.0;
}
// the content grown by a pixel: thin icon strokes cast a shadow that reads
fn grown(x: i32, y: i32) -> f32 {
    var m = 0.0;
    for (var dy = -1; dy <= 1; dy++) {
        for (var dx = -1; dx <= 1; dx++) {
            m = max(m, cover(x + dx, y + dy));
        }
    }
    return m;
}
@compute @workgroup_size(16, 16)
fn over(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= f.size.x || id.y >= f.size.y) { return; }
    let x = i32(id.x);
    let y = i32(id.y);
    let p = vec2<f32>(f32(x), f32(y));
    let size = vec2<f32>(f.size);
    // the lamp: an ellipse round the deck's middle, slowly
    // still where its angle puts it, unless it wanders (`aux[14].x`)
    let a = select(0.0, f.time * 0.22, f.aux[14].x > 0.5) + f.wind * 3.14159;
    let lamp = size * 0.5 + vec2<f32>(cos(a) * size.x * 0.42, sin(a * 1.3) * size.y * 0.36);
    let to = lamp - p;
    let ld = normalize(to + vec2<f32>(0.001));
    let near = 1.0 / (1.0 + dot(to, to) / (110.0 * 110.0));
    let k = f.strength;
    let m = cover(x, y);
    let c0 = px(x, y).rgb;
    var c = c0;
    // the pool of light on the keys' backgrounds
    c += f.tint.rgb * 0.22 * k * near * (1.0 - m);
    // the content's edges: lit on the lamp's side, darker on the far side
    let g = vec2<f32>(soft(x + 1, y) - soft(x - 1, y), soft(x, y + 1) - soft(x, y - 1));
    let edge = clamp(length(g) * 7.0, 0.0, 1.0);
    if (edge > 0.01 && m > 0.02) {
        let facing = dot(-normalize(g), ld);
        let rim = clamp(facing, 0.0, 1.0) * edge;
        let dark = clamp(-facing, 0.0, 1.0) * edge;
        c = c * (1.0 - 0.45 * dark * k) + f.tint.rgb * rim * (0.7 + 0.8 * near) * k;
    }
    // the shadow: the content moved away from the lamp, on what is behind it, read between
    // pixels so it glides with the lamp instead of stepping a pixel at a time (session 33)
    let off = p - ld * (3.0 + 4.0 * near);
    let o0 = floor(off);
    let ft = off - o0;
    let ox = i32(o0.x);
    let oy = i32(o0.y);
    let shadow = mix(mix(grown(ox, oy), grown(ox + 1, oy), ft.x), mix(grown(ox, oy + 1), grown(ox + 1, oy + 1), ft.x), ft.y) * (1.0 - m);
    c = c * (1.0 - 0.6 * shadow * k);
    put(id.xy, c);
}
"#;

/// Scene: a night sky behind the icons, across all keys as one picture: a deep gradient in the
/// tint, stars that stand still and breathe slowly (amount = density), a few brighter ones, a moon
/// where the content leaves the most room (`aux[0]`: x, y, radius, found by the engine from the
/// content mask), and now and then a shooting star over the top row. Owner, 2026-10-26: the moon
/// was hidden behind an icon and the stars seemed to move (their places took the frame's seed).
pub const SCENE: &str = r#"
// the distance from p to the segment a-b, and where along it (0..1)
fn seg(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>) -> vec2<f32> {
    let ab = b - a;
    let t = clamp(dot(p - a, ab) / max(dot(ab, ab), 0.0001), 0.0, 1.0);
    return vec2<f32>(distance(p, a + ab * t), t);
}
@compute @workgroup_size(16, 16)
fn under(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= f.size.x || id.y >= f.size.y) { return; }
    let p = vec2<f32>(f32(id.x), f32(id.y)) + 0.5;
    let size = vec2<f32>(f.size);
    let uv = p / size;
    let base = px(i32(id.x), i32(id.y)).rgb;
    // never darker than the screen can show (~#24), deepest at the top, a faint haze of the
    // Milky Way across the deck
    var sky = mix(f.tint.rgb * 0.80 + vec3<f32>(0.09), f.tint.rgb * 0.38 + vec3<f32>(0.12, 0.11, 0.15), uv.y);
    let band = exp(-pow((uv.y - 0.25 - uv.x * 0.45) * 4.5, 2.0));
    sky += vec3<f32>(0.13, 0.12, 0.20) * band * (0.45 + 0.55 * vnoise(p / 7.0)) * (0.6 + 0.6 * f.strength);
    // stars on a 9 px grid, each with its own place, size, colour and slow breath
    let cell = floor(p / 9.0);
    let r = hash(cell);
    if (r > 1.0 - 0.09 * f.density) {
        let at = cell * 9.0 + vec2<f32>(1.5) + vec2<f32>(hash(cell + 1.7), hash(cell + 3.1)) * 6.0;
        let bright = hash(cell + 9.3);
        let size = 0.95 + 1.5 * bright * bright;
        let d = distance(p, at);
        let breath = 0.72 + 0.28 * sin(f.time * (0.35 + 0.9 * hash(cell + 5.5)) + r * 40.0);
        let warm = hash(cell + 7.7);
        let col = mix(vec3<f32>(0.78, 0.86, 1.0), vec3<f32>(1.0, 0.92, 0.78), warm);
        sky += col * (smoothstep(size, 0.0, d) * (0.85 + 0.7 * bright) + exp(-d * d / (size * 6.0)) * 0.22 * bright) * breath;
    }
    // the moon: a soft-edged disc with smooth maria, lit from its upper left, and a halo
    let m = f.aux[0];
    if (m.z > 0.0) {
        let q = p - m.xy;
        let md = length(q);
        let disc = smoothstep(m.z + 0.8, m.z - 0.8, md);
        let maria = vnoise(q / (m.z * 0.33) + vec2<f32>(3.1, 7.2)) * 0.6 + vnoise(q / (m.z * 0.15) + vec2<f32>(1.3, 2.4)) * 0.4;
        let n = vec3<f32>(q / m.z, sqrt(max(0.0, 1.0 - dot(q / m.z, q / m.z))));
        let lit = clamp(dot(n, normalize(vec3<f32>(-0.45, -0.55, 0.7))) * 1.15, 0.0, 1.0);
        let surface = vec3<f32>(0.97, 0.94, 0.84) * (0.30 + 0.70 * lit) * (1.0 - 0.18 * smoothstep(0.45, 0.75, maria));
        sky = mix(sky, surface, disc);
        sky += vec3<f32>(0.9, 0.86, 0.72) * exp(-max(md - m.z, 0.0) / (m.z * 0.9)) * 0.22 * (1.0 - disc) * f.strength;
    }
    // a shooting star now and then (one in each 11 s of the layer's clock, at a place of its own)
    let period = 11.0;
    let k = floor(f.time / period);
    let age = f.time - k * period - 2.0;
    if (age > 0.0 && age < 0.9) {
        let leftward = hash(vec2<f32>(k, 4.0)) > 0.5;
        let a0 = vec2<f32>(size.x * select(0.08 + 0.45 * hash(vec2<f32>(k, 1.0)), 0.47 + 0.45 * hash(vec2<f32>(k, 1.0)), leftward), size.y * (0.04 + 0.22 * hash(vec2<f32>(k, 2.0))));
        let dir = normalize(vec2<f32>(select(1.0, -1.0, leftward), 0.32 + 0.3 * hash(vec2<f32>(k, 3.0))));
        let head = a0 + dir * age * 190.0;
        let tail = head - dir * min(age * 190.0, 46.0);
        let sd = seg(p, tail, head);
        let fade = smoothstep(0.0, 0.12, age) * smoothstep(0.9, 0.55, age);
        sky += vec3<f32>(1.0, 0.97, 0.9) * smoothstep(1.3, 0.0, sd.x) * pow(sd.y, 1.6) * fade;
        sky += vec3<f32>(0.75, 0.8, 1.0) * exp(-distance(p, head) * 0.55) * 0.5 * fade;
    }
    put(id.xy, mix(base, sky, (0.85 * f.strength + 0.15) * (1.0 - keep_bg(base))));
}
"#;

/// Finish: each key as a lit tile (a vignette inside its window), the shadows graded toward the
/// tint, and film grain only when asked for (amount above its middle; grain changes every frame
/// and makes the picture much heavier to send).
pub const FINISH: &str = r#"
@compute @workgroup_size(16, 16)
fn over(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= f.size.x || id.y >= f.size.y) { return; }
    let p = vec2<f32>(f32(id.x), f32(id.y)) + 0.5;
    var c = px(i32(id.x), i32(id.y)).rgb;
    for (var k = 0u; k < 15u; k++) {
        let r = keys[k];
        if (p.x >= r.x && p.y >= r.y && p.x < r.x + r.z && p.y < r.y + r.w) {
            let d = length((p - (r.xy + r.zw * 0.5)) / (r.zw * 0.5));
            c *= 1.0 - f.strength * 0.5 * smoothstep(0.45, 1.25, d);
        }
    }
    let luma = dot(c, vec3<f32>(0.299, 0.587, 0.114));
    c += f.tint.rgb * (1.0 - luma) * 0.10 * f.strength;
    let grain = max(f.density - 1.0, 0.0) * 0.035;
    if (grain > 0.0) {
        c += vec3<f32>((hash(p + floor(f.time * 24.0) * 13.1) - 0.5) * grain);
    }
    put(id.xy, c);
}
"#;
