// Halloween: eyes in the dark. Pairs of glowing eyes open in the keys' empty corners (more often on
// keys the owner rarely presses), blink, and look at the key just pressed; they close and move on.
// The engine runs them (`aux[i]`, i < 4: the pair's centre x, y, how open 0..1, size; `aux[4]`: the
// place they look at x, y and since when, 1 when there is one).
fn eye(q: vec2<f32>, size: f32, open: f32, look: vec2<f32>) -> vec4<f32> {
    // an almond: two arcs; its height follows the lid
    let half_h = size * 0.42 * open;
    let x = q.x / size;
    let edge = (1.0 - x * x * 4.0) * half_h;
    let inside = edge - abs(q.y);
    let a = smoothstep(-0.6, 0.6, inside) * step(abs(x), 0.5);
    // a slit pupil looking toward `look` (at most a third of the eye)
    let pq = q - look * size * 0.18;
    let pupil = smoothstep(0.7, -0.2, abs(pq.x) * 3.2 - size * 0.12 + abs(pq.y) * 0.05);
    // the iris glows hotter round the pupil
    let iris = mix(vec3<f32>(0.95, 0.85, 0.15), vec3<f32>(0.75, 1.0, 0.25), smoothstep(0.0, size * 0.5, length(q)));
    let col = mix(iris, vec3<f32>(0.02, 0.02, 0.0), pupil);
    return vec4<f32>(col, a);
}
@compute @workgroup_size(16, 16)
fn under(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= f.size.x || id.y >= f.size.y) { return; }
    let p = vec2<f32>(f32(id.x), f32(id.y)) + 0.5;
    var c = px(i32(id.x), i32(id.y)).rgb;
    let look_at = f.aux[4];
    for (var i = 0; i < 4; i++) {
        let e = f.aux[i];
        if (e.z < 0.01) { continue; }
        let size = e.w;
        let mid = e.xy;
        let q = p - mid;
        if (abs(q.x) > size * 2.5 || abs(q.y) > size * 1.5) { continue; }
        // where they look: the pressed key for a few seconds, else a slow drift of their own
        var look = vec2<f32>(sin(f.time * 0.7 + f32(i) * 2.0), sin(f.time * 0.5 + f32(i)) * 0.4);
        if (look_at.w > 0.5) {
            look = clamp((look_at.xy - mid) / 60.0, vec2<f32>(-1.0), vec2<f32>(1.0));
        }
        // a glow round the pair in the dark
        let glow = exp(-(q.x * q.x / (size * size * 3.0) + q.y * q.y / (size * size * 0.8))) * e.z;
        c += vec3<f32>(0.55, 0.65, 0.05) * glow * 0.18 * f.strength;
        let gap = size * 0.75;
        let l = eye(q + vec2<f32>(gap, 0.0), size, e.z, look);
        let r = eye(q - vec2<f32>(gap, 0.0), size, e.z, look);
        c = mix(c, l.rgb, l.a * f.strength);
        c = mix(c, r.rgb, r.a * f.strength);
    }
    put(id.xy, c);
}
