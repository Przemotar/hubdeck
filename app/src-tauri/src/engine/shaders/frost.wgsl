// Frost on the blocks: ice grows in from each key's walls over half a minute, feathered like frost
// on a window, frosting what is under it, with a few sparkles; the icon in the middle stays clear.
// A press breathes on the key: the ice melts back from its middle and grows again
// (`aux_at(k)`: seconds since key k was pressed, large when never).
fn fbm(p: vec2<f32>) -> f32 {
    var v = 0.0;
    var a = 0.5;
    var q = p;
    for (var i = 0; i < 4; i++) {
        v += a * vnoise(q);
        q = q * 2.07 + vec2<f32>(5.3, 1.9);
        a *= 0.5;
    }
    return v;
}
@compute @workgroup_size(16, 16)
fn over(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= f.size.x || id.y >= f.size.y) { return; }
    let p = vec2<f32>(f32(id.x), f32(id.y)) + 0.5;
    var c = px(i32(id.x), i32(id.y)).rgb;
    let k = key_at(p);
    if (k < 0) { put(id.xy, c); return; }
    let r = keys[k];
    let edge = key_inside(p, r);
    // how far in the ice has come: half a minute to full, a breath melts it back
    let grown = smoothstep(0.0, 30.0, f.time) * (0.55 + 0.45 * f.density);
    let since = aux_at(k);
    let melt = 1.0 - smoothstep(0.0, 0.35, since) * (1.0 - smoothstep(2.0, 14.0, since));
    let reach = 26.0 * grown * melt;
    let n = fbm(p / 5.5 + vec2<f32>(f32(k) * 3.7, 0.0));
    let front = reach * (0.55 + 0.9 * n);
    var ice = smoothstep(front, front - 5.0, edge);
    // feathers: thin bright veins along the noise's ridges
    let ridge = 1.0 - abs(fbm(p / 3.0 + vec2<f32>(0.0, f32(k))) * 2.0 - 1.0);
    let feather = pow(ridge, 10.0) * smoothstep(front + 4.0, front - 8.0, edge);
    ice = clamp(ice + feather * 0.6, 0.0, 1.0) * f.strength;
    if (ice < 0.003) { put(id.xy, c); return; }
    // frosted: what is under it blurred and paled
    let soft = (px(i32(id.x) - 1, i32(id.y)).rgb + px(i32(id.x) + 1, i32(id.y)).rgb + px(i32(id.x), i32(id.y) - 1).rgb + px(i32(id.x), i32(id.y) + 1).rgb + c) / 5.0;
    let frosted = mix(soft, vec3<f32>(0.80, 0.90, 1.0), 0.45 + 0.25 * n);
    c = mix(c, frosted, ice * 0.85);
    c += vec3<f32>(0.9, 0.97, 1.0) * feather * 0.35 * f.strength;
    // sparkles that catch the light now and then
    let cell = floor(p / 4.0);
    let tw = pow(max(sin(f.time * (0.7 + hash(cell)) + hash(cell + 3.3) * 60.0), 0.0), 30.0);
    c += vec3<f32>(1.0) * tw * step(0.8, hash(cell + 1.1)) * ice * 0.7;
    put(id.xy, c);
}
