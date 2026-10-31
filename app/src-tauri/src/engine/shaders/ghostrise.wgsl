// A press (Halloween): a ghost rises out of the pressed key, swells, floats up over the keys above
// with a wobble and fades. Up to three at once (`aux[i]`: the key's centre x, y, the start on the
// layer's clock, 1 when on).
fn shape(q: vec2<f32>, r: f32, t: f32) -> f32 {
    let head = length(q - vec2<f32>(0.0, -r * 0.45)) - r;
    let w = r + max(q.y + r * 0.45, 0.0) * 0.18;
    let body = max(abs(q.x) - w, q.y - r * 1.2 - r * 0.25 * sin(q.x * 6.0 / r + t * 6.0));
    return min(head, max(body, -(q.y + r * 0.45)));
}
@compute @workgroup_size(16, 16)
fn over(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= f.size.x || id.y >= f.size.y) { return; }
    let p = vec2<f32>(f32(id.x), f32(id.y)) + 0.5;
    var c = px(i32(id.x), i32(id.y)).rgb;
    for (var i = 0; i < 3; i++) {
        let g = f.aux[i];
        if (g.w < 0.5) { continue; }
        let age = f.time - g.z;
        if (age < 0.0 || age > 2.2) { continue; }
        let grow = 1.0 - exp(-age * 5.0);
        let r = mix(4.0, 16.0, grow);
        let pos = g.xy + vec2<f32>(sin(age * 3.0) * 8.0, -age * 55.0);
        let q = p - pos;
        let d = shape(q, r, age);
        let vis = smoothstep(0.0, 0.15, age) * (1.0 - smoothstep(1.5, 2.2, age));
        c += vec3<f32>(0.55, 0.75, 1.0) * exp(-max(d, 0.0) / (r * 0.6)) * 0.25 * vis * f.strength;
        if (d < 1.0) {
            var body = mix(vec3<f32>(0.94, 0.97, 1.0), vec3<f32>(0.72, 0.82, 0.96), smoothstep(-r, r * 1.3, q.y));
            let eyes = min(length((q - vec2<f32>(-r * 0.33, -r * 0.6)) / vec2<f32>(r * 0.16, r * 0.24)), length((q - vec2<f32>(r * 0.33, -r * 0.6)) / vec2<f32>(r * 0.16, r * 0.24)));
            let mouth = length((q - vec2<f32>(0.0, -r * 0.1)) / vec2<f32>(r * 0.2, r * 0.28));
            body = mix(body, vec3<f32>(0.08, 0.08, 0.14), smoothstep(1.1, 0.9, min(eyes, mouth)));
            c = mix(c, body, smoothstep(1.0, -2.0, d) * 0.8 * vis * f.strength);
        }
    }
    put(id.xy, c);
}
