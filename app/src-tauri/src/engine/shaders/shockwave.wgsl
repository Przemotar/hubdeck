// A press that travels: a shockwave. A ring of bent glass runs out from the pressed key over the
// whole deck, bending the icons as it passes (the picture pushed outward inside the ring, pulled in
// behind it), its edge splitting the colours, a faint bright rim. Up to three (`aux[i]`: the key's
// centre x, y, start, 1 when on).
@compute @workgroup_size(16, 16)
fn over(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= f.size.x || id.y >= f.size.y) { return; }
    let p = vec2<f32>(f32(id.x), f32(id.y)) + 0.5;
    var off = vec2<f32>(0.0);
    var rim = 0.0;
    var split = 0.0;
    for (var i = 0; i < 3; i++) {
        let w = f.aux[i];
        if (w.w < 0.5) { continue; }
        let age = f.time - w.z;
        if (age < 0.0 || age > 1.6) { continue; }
        let radius = age * 330.0;
        let to = p - w.xy;
        let d = length(to);
        let x = (d - radius) / 13.0;
        if (abs(x) > 1.6) { continue; }
        let fade = 1.0 - smoothstep(0.6, 1.6, age);
        // a wave shape: out in front of the ring, in behind it
        let push = -x * exp(-x * x * 2.2) * 7.0 * fade * f.strength;
        off += normalize(to + vec2<f32>(0.001)) * push;
        rim += exp(-x * x * 6.0) * fade;
        split += abs(push);
    }
    if (rim <= 0.0 && split <= 0.0) { put(id.xy, px(i32(id.x), i32(id.y)).rgb); return; }
    let dir = normalize(off + vec2<f32>(0.0001));
    var c = vec3<f32>(px_smooth(p - off - dir * split * 0.18).r, px_smooth(p - off).g, px_smooth(p - off + dir * split * 0.18).b);
    c += mix(vec3<f32>(0.85, 0.95, 1.0), f.tint.rgb, 0.3) * rim * 0.16 * f.strength;
    put(id.xy, c);
}
