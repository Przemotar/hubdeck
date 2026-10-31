// K5: a ball of light hopping from block to block. The engine plans the hops (`aux[0]`: from x, y,
// to x, y; `aux[1]`: start, seconds, height, squash at landing); a press calls it to that key.
// It glows in the tint, lights the icons' edges that face it, lays a soft pool of light on the
// keys under it and a shadow where it will land.
fn ball() -> vec3<f32> {
    let a = f.aux[0];
    let b = f.aux[1];
    let s = clamp((f.time - b.x) / max(b.y, 0.001), 0.0, 1.0);
    let e = s;
    let pos = mix(a.xy, a.zw, e) - vec2<f32>(0.0, b.z * 4.0 * e * (1.0 - e));
    return vec3<f32>(pos, s);
}
@compute @workgroup_size(16, 16)
fn over(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= f.size.x || id.y >= f.size.y) { return; }
    let p = vec2<f32>(f32(id.x), f32(id.y)) + 0.5;
    var c = px(i32(id.x), i32(id.y)).rgb;
    let b = ball();
    let pos = b.xy;
    // squashed for a moment as it lands and leaves
    let land = f.aux[1].w;
    let squash = 1.0 + 0.35 * land;
    let q = (p - pos) * vec2<f32>(1.0 / squash, squash);
    let d = length(q);
    let k = f.strength;
    let tint = f.tint.rgb;
    // the light it throws: a pool on the backgrounds, the content's edges facing it lit
    let pool = exp(-d * d / (2.0 * 34.0 * 34.0));
    let m = cover(i32(id.x), i32(id.y));
    let to = normalize(pos - p + vec2<f32>(0.001));
    let g = vec2<f32>(cover(i32(id.x) + 1, i32(id.y)) - cover(i32(id.x) - 1, i32(id.y)), cover(i32(id.x), i32(id.y) + 1) - cover(i32(id.x), i32(id.y) - 1));
    let facing = clamp(dot(-g, to), 0.0, 1.0);
    c += tint * pool * 0.22 * k * (1.0 - m * 0.5);
    c += tint * facing * exp(-d / 40.0) * 0.9 * k;
    // its shadow on the keys below, under where it will land
    let ground = mix(f.aux[0].xy, f.aux[0].zw, b.z) + vec2<f32>(0.0, 2.0);
    let sd = length((p - ground) * vec2<f32>(1.0, 2.2));
    let lift = clamp(distance(pos, ground) / 30.0, 0.0, 1.0);
    c *= 1.0 - 0.28 * k * exp(-sd * sd / (2.0 * pow(4.0 + 5.0 * lift, 2.0))) * (1.0 - lift * 0.6);
    // the ball: a white-hot core in a coloured glow
    let core = smoothstep(4.2, 2.6, d);
    let glow = exp(-d * d / (2.0 * 7.5 * 7.5));
    c = mix(c, vec3<f32>(1.0, 0.98, 0.94), core);
    c += tint * glow * 0.85 * (1.0 - core);
    put(id.xy, c);
}
