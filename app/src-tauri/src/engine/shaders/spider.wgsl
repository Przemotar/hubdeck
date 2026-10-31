// Halloween: a spider lets itself down on a thread from the top of a key, hangs there swaying,
// legs moving, and climbs back up (fast when that key is pressed), then tries another key. The
// engine runs it (`aux[0]`: thread x, the window's top y, the thread's length, 1 while it is out;
// `aux[1]`: the sway, how fast its legs move).
fn seg(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>) -> f32 {
    let ab = b - a;
    let t = clamp(dot(p - a, ab) / max(dot(ab, ab), 0.0001), 0.0, 1.0);
    return distance(p, a + ab * t);
}
@compute @workgroup_size(16, 16)
fn over(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= f.size.x || id.y >= f.size.y) { return; }
    let p = vec2<f32>(f32(id.x), f32(id.y)) + 0.5;
    var c = px(i32(id.x), i32(id.y)).rgb;
    let s = f.aux[0];
    if (s.w < 0.5 || s.z < 1.0) { put(id.xy, c); return; }
    let sway = f.aux[1].x;
    let top = vec2<f32>(s.x, s.y);
    let body_at = vec2<f32>(s.x + sway, s.y + s.z);
    // drawn 2.2 times its first size (owner, 2026-10-26: it was too small to see)
    let S = 2.2;
    if (abs(p.x - body_at.x) > 16.0 * S && abs(p.x - s.x) > 3.0) { put(id.xy, c); return; }
    // the thread: thin, silvery, catching the light
    let thread = seg(p, top, body_at - vec2<f32>(0.0, 3.0 * S));
    c = mix(c, vec3<f32>(0.85, 0.85, 0.95), smoothstep(0.75, 0.0, thread) * 0.55 * f.strength);
    let q = (p - body_at) / S;
    // eight legs, four a side, each bent at a knee, moving a little out of step
    var legs = 99.0;
    let step_t = f.time * f.aux[1].y;
    for (var i = 0; i < 4; i++) {
        let fi = f32(i);
        let a = (fi - 1.5) * 0.55 + sin(step_t + fi * 1.7) * 0.12;
        for (var side = -1; side <= 1; side += 2) {
            let sd = f32(side);
            let hip = vec2<f32>(sd * 1.6, -1.0 + fi * 0.9);
            let knee = hip + vec2<f32>(sd * cos(a) * 5.0, -2.8 + sin(a) * 3.0);
            let foot = knee + vec2<f32>(sd * 3.2, 4.2 + fi * 0.6);
            legs = min(legs, min(seg(q, hip, knee), seg(q, knee, foot)));
        }
    }
    // the body: an abdomen and a head
    let abdomen = length((q - vec2<f32>(0.0, 2.6)) / vec2<f32>(3.2, 3.9)) - 1.0;
    let head = length((q - vec2<f32>(0.0, -2.2)) / vec2<f32>(2.1, 1.9)) - 1.0;
    let solid = min(abdomen * 3.0, head * 2.0);
    let dark = vec3<f32>(0.04, 0.03, 0.05);
    // a rim of moonlight so it reads on dark keys
    let rim = smoothstep(0.0, 1.2, -solid) * (1.0 - smoothstep(1.2, 2.4, -solid));
    c = mix(c, dark, smoothstep(0.7, 0.0, legs * S - 0.4) * 0.95 * f.strength);
    c = mix(c, dark + vec3<f32>(0.45, 0.38, 0.70) * rim, smoothstep(0.6, -0.6, solid * S) * f.strength);
    // two red eyes
    let ey = min(length(q - vec2<f32>(-0.8, -2.6)), length(q - vec2<f32>(0.8, -2.6)));
    c = mix(c, vec3<f32>(1.0, 0.15, 0.1), smoothstep(0.7, 0.2, ey * S - 0.6) * f.strength);
    put(id.xy, c);
}
