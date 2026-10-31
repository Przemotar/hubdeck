// Halloween: cobwebs in the keys' corners, over the icons but faint: spokes fanning from the
// corner, a spiral of sagging threads, a drop of dew at a few crossings that catches the deck's
// light. The engine picks free corners (`aux[i]`, i < 8: the corner's x, y, which way it opens
// (x and y signs packed as 0..3), size; size 0 = none).
fn web(q: vec2<f32>, size: f32) -> vec2<f32> {
    // q: from the corner, both coordinates growing into the key
    let r = length(q);
    if (r > size || q.x < -1.0 || q.y < -1.0) { return vec2<f32>(0.0); }
    let a = atan2(q.y, q.x);
    // six spokes across the quarter
    let spoke_a = (round(a / (1.5708 / 5.0))) * (1.5708 / 5.0);
    let spoke = abs(sin(a - spoke_a)) * r;
    // the spiral: rings that sag between the spokes
    let between = fract(a / (1.5708 / 5.0));
    let sag = 1.0 + 0.9 * sin(between * 3.14159);
    let ring = abs(fract((r + sag) / 4.2) - 0.5) * 4.2;
    let line = min(smoothstep(0.55, 0.0, spoke), 1.0) + smoothstep(0.5, 0.0, ring) * step(4.0, r);
    // dew: a drop where a ring meets a spoke, at a few of them
    let dew = smoothstep(1.1, 0.3, length(vec2<f32>(spoke, ring))) * step(0.72, hash(vec2<f32>(floor((r + sag) / 4.2), spoke_a * 10.0)));
    let fade = 1.0 - smoothstep(size * 0.7, size, r);
    return vec2<f32>(clamp(line, 0.0, 1.0) * fade, dew * fade);
}
@compute @workgroup_size(16, 16)
fn over(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= f.size.x || id.y >= f.size.y) { return; }
    let p = vec2<f32>(f32(id.x), f32(id.y)) + 0.5;
    var c = px(i32(id.x), i32(id.y)).rgb;
    var lines = 0.0;
    var dew = 0.0;
    for (var i = 0; i < 8; i++) {
        let w = f.aux[i];
        if (w.w <= 0.0) { continue; }
        let code = i32(w.z + 0.5);
        let dir = vec2<f32>(select(1.0, -1.0, (code & 1) == 1), select(1.0, -1.0, (code & 2) == 2));
        let v = web((p - w.xy) * dir, w.w);
        lines = max(lines, v.x);
        dew = max(dew, v.y);
    }
    if (lines + dew <= 0.0) { put(id.xy, c); return; }
    // the dew twinkles as the light moves
    let glint = 0.5 + 0.5 * sin(f.time * 1.7 + p.x * 0.3 + p.y * 0.2);
    c = mix(c, vec3<f32>(0.88, 0.88, 0.95), lines * 0.42 * f.strength);
    c += vec3<f32>(0.95, 0.97, 1.0) * dew * (0.35 + 0.65 * glint) * 0.8 * f.strength;
    put(id.xy, c);
}
