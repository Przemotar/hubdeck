// A press (Halloween): a flock of bats bursts out of the pressed key and scatters over the other
// keys, flapping, growing smaller as they fly off. Up to three bursts at once (`aux[i]`: the key's
// centre x, y, the start on the layer's clock, 1 when on).
fn bat(q: vec2<f32>, size: f32, flap: f32) -> f32 {
    // a body and two wings that rise and fall, each wing two scallops at its trailing edge
    let body = length(q / vec2<f32>(size * 0.22, size * 0.32)) - 1.0;
    let wq = vec2<f32>(abs(q.x), q.y + abs(q.x) * flap * 0.6);
    let wing = max(max(wq.y - size * 0.15, -wq.y - size * 0.35 + wq.x * 0.25), wq.x - size);
    let notch = length(vec2<f32>(fract(wq.x / (size * 0.45)) * size * 0.45 - size * 0.22, wq.y - size * 0.2)) - size * 0.2;
    return min(body * size * 0.3, max(wing, -notch));
}
@compute @workgroup_size(16, 16)
fn over(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= f.size.x || id.y >= f.size.y) { return; }
    let p = vec2<f32>(f32(id.x), f32(id.y)) + 0.5;
    var c = px(i32(id.x), i32(id.y)).rgb;
    for (var i = 0; i < 3; i++) {
        let b = f.aux[i];
        if (b.w < 0.5) { continue; }
        let age = f.time - b.z;
        if (age < 0.0 || age > 1.6) { continue; }
        for (var j = 0; j < 8; j++) {
            let fj = f32(j);
            let ang = fj / 8.0 * 6.2832 + hash(vec2<f32>(fj, b.z)) * 0.6 - 1.57;
            let speed = 120.0 + 70.0 * hash(vec2<f32>(fj, 3.0));
            // out fast, then a wavering glide
            let dist = speed * (1.0 - exp(-age * 2.2)) / 2.2 * 1.6;
            let pos = b.xy + vec2<f32>(cos(ang), sin(ang) * 0.7 - 0.3) * dist + vec2<f32>(0.0, sin(age * 9.0 + fj) * 3.0);
            let size = mix(9.0, 5.0, clamp(age / 1.6, 0.0, 1.0));
            let flap = sin(age * 28.0 + fj * 1.3);
            var q = p - pos;
            if (abs(q.x) > size * 1.4 || abs(q.y) > size * 1.4) { continue; }
            let d = bat(q, size, flap);
            let fade = 1.0 - smoothstep(1.1, 1.6, age);
            c = mix(c, vec3<f32>(0.03, 0.02, 0.05), smoothstep(0.6, -0.6, d) * fade * f.strength);
            // a violet rim so they read on dark keys
            c += vec3<f32>(0.35, 0.2, 0.6) * smoothstep(1.5, 0.0, abs(d)) * 0.35 * fade * f.strength;
        }
    }
    put(id.xy, c);
}
