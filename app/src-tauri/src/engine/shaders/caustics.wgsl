// B8: caustics: the light of a pool's floor on the keys, a slow net of bright lines that bends and
// drifts as if water moved above the deck; on the backgrounds in the tint, and lightly on the
// icons too (they lie at the bottom of the same pool).
fn caustic(p: vec2<f32>, t: f32) -> f32 {
    // the edges of slowly warping cells: thin bright lines where the noise crosses its middle,
    // a second, finer net multiplied in lightly so the lines break up as real caustics do
    let q = p / 38.0 + vec2<f32>(t * 0.10, t * 0.07);
    let w = vec2<f32>(vnoise(q * 0.7 + vec2<f32>(0.0, t * 0.08)), vnoise(q * 0.7 + vec2<f32>(3.0, 5.0))) - 0.5;
    let n = vnoise(q + w * 1.6);
    let main = pow(1.0 - abs(n * 2.0 - 1.0), 6.0);
    let q2 = p / 21.0 - vec2<f32>(t * 0.06, t * 0.11);
    let n2 = vnoise(q2 + w);
    let fine = 0.55 + 0.45 * pow(1.0 - abs(n2 * 2.0 - 1.0), 3.0);
    return main * fine;
}
@compute @workgroup_size(16, 16)
fn under(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= f.size.x || id.y >= f.size.y) { return; }
    let p = vec2<f32>(f32(id.x), f32(id.y)) + 0.5;
    var c = px(i32(id.x), i32(id.y)).rgb;
    let l = caustic(p, f.time * 0.6);
    c = max(c, vec3<f32>(0.09, 0.12, 0.14)) + f.tint.rgb * (l * 0.85 + 0.04) * f.strength;
    put(id.xy, c);
}
@compute @workgroup_size(16, 16)
fn over(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= f.size.x || id.y >= f.size.y) { return; }
    let p = vec2<f32>(f32(id.x), f32(id.y)) + 0.5;
    var c = px(i32(id.x), i32(id.y)).rgb;
    let m = cover(i32(id.x), i32(id.y));
    if (m > 0.02) {
        c += f.tint.rgb * caustic(p, f.time * 0.6) * 0.35 * m * f.strength;
    }
    put(id.xy, c);
}
