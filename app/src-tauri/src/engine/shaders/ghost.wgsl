// Halloween: a ghost drifting behind the windows now and then (one crossing every 24 s of the
// layer's clock): a soft white body with a waving hem, dark eyes and mouth, a cold glow; it passes
// behind each frame as if behind a wall, and its light catches the icons' edges as it goes by.
fn ghost_at(t: f32, size: vec2<f32>) -> vec4<f32> {
    // its place and how visible it is (fades in and out at the ends of a crossing)
    let period = 24.0;
    let n = floor(t / period);
    let age = t - n * period;
    let dur = 13.0;
    if (age > dur) { return vec4<f32>(0.0); }
    let s = age / dur;
    let ltr = hash(vec2<f32>(n, 1.0)) > 0.5;
    let x = mix(-30.0, size.x + 30.0, select(1.0 - s, s, ltr));
    let y = size.y * (0.25 + 0.4 * hash(vec2<f32>(n, 2.0))) + sin(age * 1.3) * 10.0;
    let vis = smoothstep(0.0, 0.08, s) * smoothstep(1.0, 0.9, s);
    return vec4<f32>(x, y, vis, select(-1.0, 1.0, ltr));
}
fn shape(q: vec2<f32>, t: f32) -> f32 {
    // a round head on a body that widens and ends in a waving hem
    let r = 13.0;
    let head = length(q - vec2<f32>(0.0, -6.0)) - r;
    let w = r + max(q.y + 6.0, 0.0) * 0.18;
    let body = max(abs(q.x) - w, q.y - 16.0 - 3.0 * sin(q.x * 0.55 + t * 5.0));
    return min(head, max(body, -(q.y + 6.0)));
}
@compute @workgroup_size(16, 16)
fn under(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= f.size.x || id.y >= f.size.y) { return; }
    let size = vec2<f32>(f.size);
    let p = vec2<f32>(f32(id.x), f32(id.y)) + 0.5;
    var c = px(i32(id.x), i32(id.y)).rgb;
    let g = ghost_at(f.time, size);
    if (g.z <= 0.0) { put(id.xy, c); return; }
    var q = p - g.xy;
    q.x *= g.w;
    // a slight lean in the direction it drifts
    q.x += q.y * 0.15;
    let d = shape(q, f.time);
    let glow = exp(-max(d, 0.0) / 9.0);
    c += vec3<f32>(0.55, 0.75, 1.0) * glow * 0.22 * g.z * f.strength;
    if (d < 1.0) {
        let a = smoothstep(1.0, -2.0, d) * 0.72 * g.z * f.strength;
        var body = mix(vec3<f32>(0.92, 0.96, 1.0), vec3<f32>(0.70, 0.80, 0.95), smoothstep(-10.0, 18.0, q.y));
        // eyes and mouth: dark ovals
        let eyes = min(length((q - vec2<f32>(-4.5, -8.0)) / vec2<f32>(2.2, 3.2)), length((q - vec2<f32>(4.5, -8.0)) / vec2<f32>(2.2, 3.2)));
        let mouth = length((q - vec2<f32>(0.0, -1.0)) / vec2<f32>(2.6, 3.4 + sin(f.time * 2.0)));
        body = mix(body, vec3<f32>(0.08, 0.08, 0.14), smoothstep(1.1, 0.9, min(eyes, mouth)));
        c = mix(c, body, a);
    }
    put(id.xy, c);
}
@compute @workgroup_size(16, 16)
fn over(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= f.size.x || id.y >= f.size.y) { return; }
    let size = vec2<f32>(f.size);
    let p = vec2<f32>(f32(id.x), f32(id.y)) + 0.5;
    var c = px(i32(id.x), i32(id.y)).rgb;
    let g = ghost_at(f.time, size);
    let m = cover(i32(id.x), i32(id.y));
    if (g.z <= 0.0 || m < 0.02) { put(id.xy, c); return; }
    // the icons' edges facing the ghost catch its cold light
    let to = normalize(g.xy - p + vec2<f32>(0.001));
    let grad = vec2<f32>(cover(i32(id.x) + 1, i32(id.y)) - cover(i32(id.x) - 1, i32(id.y)), cover(i32(id.x), i32(id.y) + 1) - cover(i32(id.x), i32(id.y) - 1));
    let facing = clamp(dot(-grad, to), 0.0, 1.0);
    c += vec3<f32>(0.6, 0.8, 1.0) * facing * exp(-distance(p, g.xy) / 35.0) * 0.8 * g.z * f.strength;
    put(id.xy, c);
}
