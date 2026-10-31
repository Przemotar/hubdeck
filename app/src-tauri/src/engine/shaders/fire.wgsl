// B6 trial: real fire behind the icons, rising from the deck's foot: tongues that lick upward,
// curl and break off, white-yellow at the heart, red at the tips, black-body colours; smoke above
// it drifting with the wind; the icons stand dark against it with a hot rim (in `over`).
fn fbm(p: vec2<f32>) -> f32 {
    var v = 0.0;
    var a = 0.5;
    var q = p;
    for (var i = 0; i < 5; i++) {
        v += a * vnoise(q);
        q = q * 2.02 + vec2<f32>(3.1, 1.7);
        a *= 0.5;
    }
    return v;
}
fn heat(p: vec2<f32>) -> f32 {
    let size = vec2<f32>(f.size);
    let t = f.time;
    let h = (size.y - p.y) / size.y;
    // the flame field: noise rising, warped by more noise
    let q = vec2<f32>(p.x / 34.0 + f.wind * h * 1.5, p.y / 30.0 + t * 1.6);
    let w = vec2<f32>(fbm(q + vec2<f32>(0.0, t * 0.4)), fbm(q + vec2<f32>(5.2, 1.3 + t * 0.3)));
    let n = fbm(q + w * 1.6);
    // tall in places, low in others, slowly
    let reach = 0.55 + 0.35 * fbm(vec2<f32>(p.x / 90.0, t * 0.15)) * f.density;
    return clamp(n * 1.25 - h / reach + 0.15, 0.0, 1.0);
}
fn ramp(x: f32) -> vec3<f32> {
    // black body: dark red, red, orange, yellow, near white
    let c1 = vec3<f32>(0.35, 0.03, 0.01);
    let c2 = vec3<f32>(0.95, 0.25, 0.04);
    let c3 = vec3<f32>(1.0, 0.62, 0.12);
    let c4 = vec3<f32>(1.0, 0.92, 0.62);
    if (x < 0.33) { return mix(vec3<f32>(0.0), c1, x / 0.33) + mix(vec3<f32>(0.0), c2 - c1, 0.0); }
    if (x < 0.6) { return mix(c1, c2, (x - 0.33) / 0.27); }
    if (x < 0.82) { return mix(c2, c3, (x - 0.6) / 0.22); }
    return mix(c3, c4, (x - 0.82) / 0.18);
}
@compute @workgroup_size(16, 16)
fn under(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= f.size.x || id.y >= f.size.y) { return; }
    let size = vec2<f32>(f.size);
    let p = vec2<f32>(f32(id.x), f32(id.y)) + 0.5;
    var c = px(i32(id.x), i32(id.y)).rgb;
    let h = heat(p);
    // smoke above the flames: grey, slow, drifting with the wind
    let up = (size.y - p.y) / size.y;
    let sq = vec2<f32>(p.x / 40.0 - f.time * (0.15 + f.wind * 0.4), p.y / 36.0 + f.time * 0.45);
    let smoke = smoothstep(0.45, 0.85, fbm(sq + vec2<f32>(fbm(sq * 0.7), 0.0))) * smoothstep(0.25, 0.75, up) * (1.0 - up * 0.5) * 0.45;
    c = mix(c, vec3<f32>(0.20, 0.19, 0.20), smoke * f.strength);
    // the fire, added like light, with a glow on what is behind it
    let fire = ramp(h) * (0.6 + 0.6 * f.strength);
    c = c * (1.0 - h * 0.7) + fire + vec3<f32>(0.25, 0.08, 0.02) * smoothstep(0.0, 0.5, heat(p + vec2<f32>(0.0, 10.0))) * 0.35;
    put(id.xy, c);
}
@compute @workgroup_size(16, 16)
fn over(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= f.size.x || id.y >= f.size.y) { return; }
    let p = vec2<f32>(f32(id.x), f32(id.y)) + 0.5;
    var c = px(i32(id.x), i32(id.y)).rgb;
    let m = cover(i32(id.x), i32(id.y));
    if (m < 0.02) { put(id.xy, c); return; }
    let h = heat(p + vec2<f32>(0.0, 5.0));
    // the edge that faces the fire (below) glows hot; the face is lit warm
    let rim = clamp(m - cover(i32(id.x), i32(id.y) + 2), 0.0, 1.0);
    c += vec3<f32>(1.0, 0.55, 0.18) * rim * h * 1.1 * f.strength;
    c = mix(c, c * vec3<f32>(1.10, 0.94, 0.82) + vec3<f32>(0.05, 0.015, 0.0), h * 0.7 * m * f.strength);
    put(id.xy, c);
}
