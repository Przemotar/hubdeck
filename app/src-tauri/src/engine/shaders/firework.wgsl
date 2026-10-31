// A press that travels: a firework. A rocket rises from the pressed key with a spark trail, bursts
// high over the deck into a ring of coloured sparks that fall and fade over the other keys. Up to
// three (`aux[i]`: the key's centre x, y, start, 1 when on).
fn hue(h: f32) -> vec3<f32> {
    return clamp(abs(fract(vec3<f32>(h) + vec3<f32>(0.0, 0.667, 0.333)) * 6.0 - 3.0) - 1.0, vec3<f32>(0.0), vec3<f32>(1.0));
}
@compute @workgroup_size(16, 16)
fn over(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= f.size.x || id.y >= f.size.y) { return; }
    let p = vec2<f32>(f32(id.x), f32(id.y)) + 0.5;
    var c = px(i32(id.x), i32(id.y)).rgb;
    for (var i = 0; i < 3; i++) {
        let w = f.aux[i];
        if (w.w < 0.5) { continue; }
        let age = f.time - w.z;
        if (age < 0.0 || age > 2.4) { continue; }
        let rise = 0.45;
        let top = vec2<f32>(w.x + (hash(vec2<f32>(w.z, 1.0)) - 0.5) * 40.0, max(30.0, w.y - 110.0));
        let base_hue = hash(vec2<f32>(w.z, 2.0));
        if (age < rise) {
            // the rocket and its trail
            let s = age / rise;
            let e = 1.0 - (1.0 - s) * (1.0 - s);
            let at = mix(w.xy, top, e);
            let d = distance(p, at);
            c += vec3<f32>(1.0, 0.9, 0.6) * smoothstep(2.2, 0.0, d) * f.strength;
            let tail = dot(p - at, normalize(w.xy - top));
            let off = length((p - at) - normalize(w.xy - top) * tail);
            if (tail > 0.0 && tail < 18.0) {
                c += vec3<f32>(1.0, 0.6, 0.2) * smoothstep(1.4, 0.0, off) * (1.0 - tail / 18.0) * 0.7 * f.strength;
            }
            continue;
        }
        // the burst: 36 sparks out from the top, slowing, falling, fading, twinkling at the end
        let b = age - rise;
        for (var j = 0; j < 36; j++) {
            let fj = f32(j);
            let ang = fj / 36.0 * 6.2832 + hash(vec2<f32>(fj, w.z)) * 0.2;
            let speed = 70.0 + 30.0 * hash(vec2<f32>(fj, w.z + 3.0));
            let dist = speed * (1.0 - exp(-b * 2.6)) / 2.6 * 2.0;
            let pos = top + vec2<f32>(cos(ang), sin(ang)) * dist + vec2<f32>(0.0, 28.0 * b * b);
            let d = distance(p, pos);
            if (d > 6.0) { continue; }
            let fade = 1.0 - smoothstep(1.1, 1.9, b);
            let tw = select(1.0, step(0.4, hash(vec2<f32>(fj, floor(f.time * 20.0)))), b > 1.2);
            let col = mix(hue(base_hue + fj * 0.01), f.tint.rgb, 0.2) * 1.2 + vec3<f32>(0.15);
            c += col * (smoothstep(1.6, 0.0, d) + exp(-d * d / 8.0) * 0.3) * fade * tw * f.strength;
        }
        // the flash of the burst lights the deck for a moment
        c += vec3<f32>(1.0, 0.9, 0.8) * exp(-distance(p, top) / 90.0) * exp(-b * 9.0) * 0.4 * f.strength;
    }
    put(id.xy, c);
}
