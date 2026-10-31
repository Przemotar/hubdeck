// B8 trial: an aurora behind the icons: curtains of light that fold and drift across the deck,
// fine vertical rays inside them, green at the foot turning violet at the top, over a dark sky with
// a few stars.
fn fbm(p: vec2<f32>) -> f32 {
    var v = 0.0;
    var a = 0.5;
    var q = p;
    for (var i = 0; i < 4; i++) {
        v += a * vnoise(q);
        q = q * 2.03 + vec2<f32>(1.7, 9.2);
        a *= 0.5;
    }
    return v;
}
@compute @workgroup_size(16, 16)
fn under(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= f.size.x || id.y >= f.size.y) { return; }
    let size = vec2<f32>(f.size);
    let p = vec2<f32>(f32(id.x), f32(id.y)) + 0.5;
    let uv = p / size;
    let base = px(i32(id.x), i32(id.y)).rgb;
    let t = f.time * 0.6;
    var c = mix(vec3<f32>(0.05, 0.07, 0.13), vec3<f32>(0.10, 0.10, 0.14), uv.y);
    // stars, still
    let cell = floor(p / 8.0);
    if (hash(cell) > 0.93) {
        let at = cell * 8.0 + vec2<f32>(hash(cell + 1.3), hash(cell + 2.9)) * 6.0 + 1.0;
        c += vec3<f32>(0.8, 0.85, 1.0) * smoothstep(1.1, 0.0, distance(p, at)) * (0.5 + 0.5 * hash(cell + 4.4)) * (1.0 - uv.y * 0.6);
    }
    // three curtains: each a wavy line across the deck; light hangs above it, fading up
    var glow = vec3<f32>(0.0);
    for (var i = 0; i < 3; i++) {
        let fi = f32(i);
        let x = p.x / size.x;
        let line = 0.62 - fi * 0.12 + 0.13 * sin(x * (3.0 + fi) + t * (0.35 + 0.1 * fi) + fi * 2.0) + 0.08 * (fbm(vec2<f32>(x * 3.0 + fi * 5.0, t * 0.25)) - 0.5) * 2.0;
        let above = line - uv.y;
        // the foot sharp, the light rising softly
        let foot = smoothstep(-0.015, 0.012, above);
        let rise = exp(-max(above, 0.0) * (5.5 + fi));
        let curtain = foot * rise;
        // rays: fine vertical streaks that shimmer slowly
        let rays = 0.55 + 0.45 * fbm(vec2<f32>(p.x / 3.2 + fi * 13.0, t * 0.9 + uv.y * 0.6));
        let col = mix(vec3<f32>(0.25, 1.0, 0.55), mix(vec3<f32>(0.55, 0.35, 1.0), f.tint.rgb, 0.4), smoothstep(0.0, 0.35, above));
        glow += col * curtain * rays * (0.75 - fi * 0.18);
    }
    c += glow * (0.7 + 0.5 * f.strength);
    put(id.xy, mix(base, c, (0.88 * f.strength + 0.12) * (1.0 - keep_bg(base))));
}
