// A press that travels: a colour bloom. The pressed key's own colour (read from its picture) floods
// out key by key, each key's background lighting up in it as the wave reaches it, softly, from its
// edges in, and fading; the icons stay as they are. Up to three (`aux[i]`: the key's
// centre x, y, start, 1 when on).
fn key_colour(at: vec2<f32>) -> vec3<f32> {
    // the brightest, most colourful of a few samples round the pressed key's middle
    var best = f.tint.rgb;
    var score = -1.0;
    for (var j = 0; j < 12; j++) {
        let a = f32(j) * 0.5236;
        let q = at + vec2<f32>(cos(a), sin(a)) * (6.0 + f32(j % 3) * 7.0);
        let c = px(i32(q.x), i32(q.y)).rgb;
        let mx = max(c.r, max(c.g, c.b));
        let mn = min(c.r, min(c.g, c.b));
        let s = (mx - mn) * 2.0 + mx;
        if (s > score) { score = s; best = c; }
    }
    return best / max(max(best.r, max(best.g, best.b)), 0.2);
}
@compute @workgroup_size(16, 16)
fn over(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= f.size.x || id.y >= f.size.y) { return; }
    let p = vec2<f32>(f32(id.x), f32(id.y)) + 0.5;
    var c = px(i32(id.x), i32(id.y)).rgb;
    let k = key_at(p);
    if (k < 0) { put(id.xy, c); return; }
    let r = keys[k];
    let mid = r.xy + r.zw * 0.5;
    for (var i = 0; i < 3; i++) {
        let w = f.aux[i];
        if (w.w < 0.5) { continue; }
        let age = f.time - w.z - distance(mid, w.xy) / 300.0;
        if (age < 0.0 || age > 1.3) { continue; }
        let col = key_colour(w.xy);
        // lit from the edges in, then fading
        let edge = key_inside(p, r);
        let fill = smoothstep(0.0, 0.25, age) * (1.0 - smoothstep(0.5, 1.3, age));
        let from_edge = exp(-edge / (4.0 + 30.0 * age));
        let behind = 1.0 - cover(i32(id.x), i32(id.y));
        c = mix(c, c * 0.5 + col * 0.6, fill * (0.35 + 0.65 * from_edge) * 0.7 * f.strength * behind);
    }
    put(id.xy, c);
}
