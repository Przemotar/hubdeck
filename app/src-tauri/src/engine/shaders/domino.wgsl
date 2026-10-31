// A press that travels: the keys turn over like cards in a wave out from the pressed key, each a
// full turn about its upright middle (its face, its back darker, its face again), in perspective,
// the nearest first. Up to three waves (`aux[i]`: the key's centre x, y, start, 1 when on).
@compute @workgroup_size(16, 16)
fn over(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= f.size.x || id.y >= f.size.y) { return; }
    let p = vec2<f32>(f32(id.x), f32(id.y)) + 0.5;
    var c = px(i32(id.x), i32(id.y)).rgb;
    let k = key_at(p);
    if (k < 0) { put(id.xy, c); return; }
    let r = keys[k];
    let mid = r.xy + r.zw * 0.5;
    // the turn of this key: the sum of the waves passing it (one at a time in practice)
    var angle = 0.0;
    for (var i = 0; i < 3; i++) {
        let w = f.aux[i];
        if (w.w < 0.5) { continue; }
        let delay = distance(mid, w.xy) / 420.0;
        let s = clamp((f.time - w.z - delay) / 0.55, 0.0, 1.0);
        angle += s * s * (3.0 - 2.0 * s) * 6.2832;
    }
    if (angle <= 0.0 || angle >= 6.2832) { put(id.xy, c); return; }
    let cw = cos(angle);
    let half = r.zw * 0.5;
    let u = (p.x - mid.x) / (half.x * max(abs(cw), 0.0001));
    let tall = 1.0 + 0.22 * sin(angle) * u * sign(cw);
    let v = (p.y - mid.y) / (half.y * tall);
    if (abs(u) > 1.0 || abs(v) > 1.0) { put(id.xy, vec3<f32>(0.035, 0.035, 0.045)); return; }
    // the back shows the face mirrored and in shade
    let x = select(-u, u, cw > 0.0);
    var face = px_smooth(mid + vec2<f32>(x, v) * half);
    if (cw < 0.0) { face = face * 0.45 + vec3<f32>(0.02, 0.02, 0.03); }
    face *= 0.5 + 0.5 * abs(cw);
    face += vec3<f32>(1.0) * pow(1.0 - abs(cw), 8.0) * 0.25 * f.strength;
    put(id.xy, face);
}
