// The upright copy of `mist.wgsl` (owner, 2026-10-29: the deck may stand upright): the banks drift
// across the deck as it stands (`to_view`), lit from its top; landscape it is the original.
// B7: fog made anew, with depth (owner, 2026-10-26: the first fog "wygląda słabo", keys jumped).
// Three banks at three depths drift with the wind at their own pace: the far bank lies behind the
// icons (`under`), the near one passes in front of them (`over`), thin, so the icons read through
// it. Large, smooth shapes only (no fine noise: it made the keys tremble), lit from the deck's
// light, never darker than the screen shows.
fn fbm3(p: vec2<f32>) -> f32 {
    var v = 0.0;
    var a = 0.5;
    var q = p;
    for (var i = 0; i < 3; i++) {
        v += a * vnoise(q);
        q = q * 2.02 + vec2<f32>(4.7, 1.3);
        a *= 0.5;
    }
    return v / 0.875;
}
fn bank(p: vec2<f32>, depth: f32) -> f32 {
    let t = f.time;
    let drift = vec2<f32>((0.35 + f.wind * 1.6) * (6.0 + 10.0 * depth), sin(t * 0.05 + depth * 4.0) * 3.0) * t;
    let q = (p + drift) / (70.0 - 30.0 * depth);
    let warp = vec2<f32>(fbm3(q * 0.6 + vec2<f32>(t * 0.02, depth * 7.0)), fbm3(q * 0.6 + vec2<f32>(3.0, t * 0.015))) - 0.5;
    let n = fbm3(q + warp * 1.4);
    // soft banks: thick in places, clear between them
    let amount = clamp(0.35 + 0.15 * f.density, 0.2, 0.75);
    return smoothstep(1.0 - amount - 0.12, 1.0 - amount + 0.28, n);
}
fn lit(p: vec2<f32>) -> vec3<f32> {
    var l = vec2<f32>(view_size().x * 0.5, -40.0);
    if (f.aux[15].w > 0.0) { l = to_view(f.aux[15].xy); }
    let near = 1.0 / (1.0 + dot(p - l, p - l) / (300.0 * 300.0));
    return mix(f.tint.rgb * 0.75 + vec3<f32>(0.10), vec3<f32>(0.92, 0.93, 0.96), 0.35) * (0.75 + 0.45 * near);
}
@compute @workgroup_size(16, 16)
fn under(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= f.size.x || id.y >= f.size.y) { return; }
    let p = to_view(vec2<f32>(f32(id.x), f32(id.y)) + 0.5);
    var c = px(i32(id.x), i32(id.y)).rgb;
    let far = bank(p, 0.0);
    let mid = bank(p + vec2<f32>(130.0, 40.0), 0.5);
    let fog = clamp(far * 0.55 + mid * 0.45, 0.0, 1.0) * (0.45 + 0.5 * f.strength);
    c = mix(c, max(lit(p) * 0.55, vec3<f32>(0.16)), fog * 0.85);
    put(id.xy, c);
}
@compute @workgroup_size(16, 16)
fn over(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= f.size.x || id.y >= f.size.y) { return; }
    let p = to_view(vec2<f32>(f32(id.x), f32(id.y)) + 0.5);
    var c = px(i32(id.x), i32(id.y)).rgb;
    // the near bank: thin wisps in front of the icons
    let near = bank(p + vec2<f32>(-60.0, 90.0), 1.0);
    let wisp = near * 0.32 * f.strength;
    c = mix(c, lit(p) * 0.8, wisp);
    put(id.xy, c);
}
