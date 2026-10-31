// K4: a page change as the fifteen keys turning like cards, in a cascade from the top left. Each
// card shows the page before (the engine put it in the state as packed RGBA) until it is edge-on,
// then the new page; in perspective (the near edge taller), shaded as it turns, dark behind.
// aux[0]: start (the layer's clock), seconds a card takes, delay a step of the cascade, on.
fn old_px(x: i32, y: i32) -> vec3<f32> {
    let cx = clamp(x, 0, i32(f.size.x) - 1);
    let cy = clamp(y, 0, i32(f.size.y) - 1);
    return unpack4x8unorm(bitcast<u32>(now[u32(cy) * f.size.x + u32(cx)])).rgb;
}
fn old_smooth(p: vec2<f32>) -> vec3<f32> {
    let a = floor(p - 0.5);
    let t = p - 0.5 - a;
    let x = i32(a.x);
    let y = i32(a.y);
    return mix(mix(old_px(x, y), old_px(x + 1, y), t.x), mix(old_px(x, y + 1), old_px(x + 1, y + 1), t.x), t.y);
}
@compute @workgroup_size(16, 16)
fn over(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= f.size.x || id.y >= f.size.y) { return; }
    let p = vec2<f32>(f32(id.x), f32(id.y)) + 0.5;
    let k = key_at(p);
    var c = px(i32(id.x), i32(id.y)).rgb;
    let a = f.aux[0];
    if (k < 0 || a.w < 0.5) { put(id.xy, c); return; }
    let col = f32(k % 5);
    let row = f32(k / 5);
    let s = clamp((f.time - a.x - (col + row) * a.z) / a.y, 0.0, 1.0);
    if (s >= 1.0) { put(id.xy, c); return; }
    // eased turn, 0 to 180 degrees about the card's upright middle
    let e = s * s * (3.0 - 2.0 * s);
    let angle = e * 3.14159;
    let w = cos(angle);
    let r = keys[k];
    let mid = r.xy + r.zw * 0.5;
    let half = r.zw * 0.5;
    let u = (p.x - mid.x) / (half.x * max(abs(w), 0.0001));
    // perspective: the edge turning toward the eye grows taller
    let tall = 1.0 + 0.22 * sin(angle) * u * sign(w);
    let v = (p.y - mid.y) / (half.y * tall);
    let behind = vec3<f32>(0.035, 0.035, 0.045);
    if (abs(u) > 1.0 || abs(v) > 1.0) { put(id.xy, behind); return; }
    let q = mid + vec2<f32>(u, v) * half;
    var face: vec3<f32>;
    if (w > 0.0) { face = old_smooth(q); } else { face = px_smooth(q); }
    // shaded as it turns away from the light, a glint as it passes edge-on
    face *= 0.45 + 0.55 * abs(w);
    face += vec3<f32>(1.0) * pow(1.0 - abs(w), 8.0) * 0.25 * f.strength;
    put(id.xy, face);
}
