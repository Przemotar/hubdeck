// A heart for a "Like" (owner's "Lubię to", his second most pressed key): from the liked key a
// heart's outline swells across the deck and fades, small hearts float up from the key, and the
// key itself blushes. Up to four at once: `aux[i]` = key centre x, y, start (layer clock), on.
fn sd_heart(q0: vec2<f32>) -> f32 {
    // iq's heart, unit size, point down, centred
    var q = vec2<f32>(abs(q0.x), -q0.y + 0.55);
    if (q.y + q.x > 1.0) {
        return sqrt(dot(q - vec2<f32>(0.25, 0.75), q - vec2<f32>(0.25, 0.75))) - sqrt(2.0) / 4.0;
    }
    let a = q - vec2<f32>(0.0, 1.0);
    let b = q - 0.5 * max(q.x + q.y, 0.0);
    return sqrt(min(dot(a, a), dot(b, b))) * sign(q.x - q.y);
}
@compute @workgroup_size(16, 16)
fn over(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= f.size.x || id.y >= f.size.y) { return; }
    let p = vec2<f32>(f32(id.x), f32(id.y)) + 0.5;
    var c = px(i32(id.x), i32(id.y)).rgb;
    let tint = f.tint.rgb;
    for (var i = 0; i < 4; i++) {
        let h = f.aux[i];
        if (h.w < 0.5) { continue; }
        let age = f.time - h.z;
        if (age < 0.0 || age > 2.4) { continue; }
        // the outline swelling over the deck
        let size = 10.0 + age * 260.0;
        let d = sd_heart((p - h.xy) / size) * size;
        let fade = (1.0 - smoothstep(0.2, 1.4, age)) * smoothstep(0.0, 0.08, age);
        c += tint * exp(-d * d / (2.0 * pow(2.0 + age * 5.0, 2.0))) * fade * 0.9 * f.strength;
        // the liked key blushes
        let k = key_at(h.xy);
        if (k >= 0 && key_inside(p, keys[k]) > 0.0) {
            c = mix(c, c * 0.6 + tint * 0.55, (1.0 - smoothstep(0.0, 0.9, age)) * 0.6 * f.strength);
        }
        // small hearts rising from the key
        for (var j = 0; j < 6; j++) {
            let fj = f32(j);
            let rise = age - fj * 0.08;
            if (rise < 0.0) { continue; }
            let pos = h.xy + vec2<f32>((hash(vec2<f32>(fj, h.z)) - 0.5) * 50.0 + sin(rise * 5.0 + fj) * 6.0, -rise * (55.0 + 25.0 * hash(vec2<f32>(fj, 2.0))));
            let s = 5.0 + 3.0 * hash(vec2<f32>(fj, 3.0));
            let dh = sd_heart((p - pos) / s) * s;
            let f2 = 1.0 - smoothstep(0.6, 1.6, rise);
            c = mix(c, tint * 1.15 + vec3<f32>(0.15), smoothstep(0.6, -0.4, dh) * f2 * f.strength);
        }
    }
    put(id.xy, c);
}
