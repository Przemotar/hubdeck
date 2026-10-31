// Halloween, lanterns: every key lit from below by a candle that flickers on its own: a warm
// glow rising from the key's floor over its background, the icon's lower edges catching the
// orange light, the top of the key left in shadow. Over the icons, so they are lit too.
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
    // the candle stands at the floor's middle, a little off to a side per key
    let flame = vec2<f32>(r.x + r.z * (0.5 + 0.15 * (hash(vec2<f32>(fk, 2.0)) - 0.5)), r.y + r.w + 4.0);
    let d = distance(p, flame);
    let up = clamp((r.y + r.w - p.y) / r.w, 0.0, 1.0);
    let warm = mix(vec3<f32>(1.0, 0.45, 0.08), vec3<f32>(1.0, 0.75, 0.35), light - 0.7);
    // the glow on the background, strongest low, and the top left in shadow
    let glow = exp(-d / (34.0 + 10.0 * light)) * light;
    c = c * (1.0 - 0.45 * up * up * f.strength) + warm * glow * 0.55 * f.strength;
    // the icon's edges facing down catch the flame
    let m = cover(i32(id.x), i32(id.y));
    if (m > 0.02) {
        let below = cover(i32(id.x), i32(id.y) + 2);
        let rim = clamp(m - below, 0.0, 1.0);
        c += warm * rim * glow * 1.6 * f.strength;
        c = mix(c, c * vec3<f32>(1.15, 0.92, 0.75), m * glow * 0.6 * f.strength);
    }
    put(id.xy, c);
}
