// A press that travels: a magnet. Every other key's picture leans toward the pressed key (the
// nearest most and first) and springs back with a little wobble, as if pulled for a moment. Up to
// three (`aux[i]`: the key's centre x, y, start, 1 when on).
@compute @workgroup_size(16, 16)
fn over(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= f.size.x || id.y >= f.size.y) { return; }
    let p = vec2<f32>(f32(id.x), f32(id.y)) + 0.5;
    var c = px(i32(id.x), i32(id.y)).rgb;
    let k = key_at(p);
    if (k < 0) { put(id.xy, c); return; }
    let r = keys[k];
    let mid = r.xy + r.zw * 0.5;
    var off = vec2<f32>(0.0);
    var squeeze = 0.0;
    for (var i = 0; i < 3; i++) {
        let w = f.aux[i];
        if (w.w < 0.5) { continue; }
        let to = w.xy - mid;
        let d = length(to);
        if (d < 8.0) { continue; }
        let age = f.time - w.z - d / 600.0;
        if (age < 0.0 || age > 1.4) { continue; }
        // a damped spring: pulled, overshooting back, settling
        let swing = sin(age * 13.0) * exp(-age * 4.2);
        let pull = 9.0 * (1.0 - smoothstep(60.0, 420.0, d) * 0.6) * f.strength;
        off += normalize(to) * pull * swing;
        squeeze += abs(swing) * 0.06 * f.strength;
    }
    if (dot(off, off) < 0.0004) { put(id.xy, c); return; }
    // the picture slid and a little stretched toward the pull, kept inside its window
    let dir = normalize(off);
    let rel = p - mid - off;
    let along = dot(rel, dir);
    let q = mid + rel - dir * along * squeeze;
    if (key_inside(q, r) > 0.5) {
        c = px_smooth(q);
    } else {
        c = px_smooth(clamp(q, r.xy + 1.0, r.xy + r.zw - 1.0)) * 0.6;
    }
    put(id.xy, c);
}
