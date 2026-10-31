// Halloween, lanterns: each key's border glows like the cut edge of a carved pumpkin with a candle
// inside: a thin hot line just inside the window, a softer orange bloom inward, uneven where the
// knife went (a jagged edge), flickering with the key's own candle.
fn candle(k: f32, t: f32) -> f32 {
    return 0.70 + 0.22 * vnoise(vec2<f32>(t * 6.5, k * 17.0)) + 0.08 * vnoise(vec2<f32>(t * 19.0, k * 3.0));
}
@compute @workgroup_size(16, 16)
fn over(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= f.size.x || id.y >= f.size.y) { return; }
    let p = vec2<f32>(f32(id.x), f32(id.y)) + 0.5;
    var c = px(i32(id.x), i32(id.y)).rgb;
    let k = key_at(p);
    if (k < 0) { put(id.xy, c); return; }
    let r = keys[k];
    let fk = f32(k);
    let light = candle(fk, f.time);
    // the carved edge: its distance from the window's border, made jagged along the border
    let local = p - r.xy;
    let along = local.x + local.y * 1.7;
    let jag = (vnoise(vec2<f32>(along * 0.35, fk * 7.0)) - 0.5) * 2.6;
    let e = key_inside(p, r) - 2.6 - jag;
    let line = exp(-e * e * 0.8);
    let bloom = exp(-max(e, 0.0) / 6.0) * smoothstep(-1.0, 1.0, e);
    let hot = mix(vec3<f32>(1.0, 0.5, 0.08), vec3<f32>(1.0, 0.86, 0.5), line);
    c = mix(c, c * 0.35, smoothstep(0.5, -0.5, e) * f.strength);
    c += hot * (line * 1.1 + bloom * 0.35) * light * f.strength;
    put(id.xy, c);
}
