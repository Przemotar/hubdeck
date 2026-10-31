// K6: each key a glass jar of liquid behind its icon. Key k's level is `aux_at(k)` (0..1), the
// tilt of its surface `aux_at(16 + k)` and how much it sloshes `aux_at(32 + k)` (the engine moves
// them on springs; a press sloshes the key and its neighbours). The liquid is the tint, deeper
// toward the bottom, with a bright meniscus, light through it and bubbles rising.
@compute @workgroup_size(16, 16)
fn under(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= f.size.x || id.y >= f.size.y) { return; }
    let p = vec2<f32>(f32(id.x), f32(id.y)) + 0.5;
    var c = px(i32(id.x), i32(id.y)).rgb;
    let k = key_at(p);
    if (k < 0) { put(id.xy, c); return; }
    let r = keys[k];
    let level = clamp(aux_at(k), 0.0, 1.0);
    let tilt = aux_at(16 + k);
    let slosh = aux_at(32 + k);
    let mid = r.x + r.z * 0.5;
    let fk = f32(k);
    let wave = sin((p.x - r.x) * 0.21 - f.time * 3.1 + fk) * (0.6 + 2.2 * slosh) + sin((p.x - r.x) * 0.47 + f.time * 2.3 + fk * 2.0) * (0.3 + 0.9 * slosh);
    let surface = r.y + r.w * (1.0 - level) + tilt * (p.x - mid) + wave;
    let depth = p.y - surface;
    if (depth > -1.0) {
        let t = clamp(depth / max(r.y + r.w - surface, 1.0), 0.0, 1.0);
        var liquid = mix(f.tint.rgb * 0.95, f.tint.rgb * 0.38, t);
        // light through it, swaying
        liquid += f.tint.rgb * 0.18 * pow(0.5 + 0.5 * sin((p.x - r.x) * 0.15 + depth * 0.08 - f.time * 0.9 + fk), 4.0);
        // bubbles rising in each jar
        for (var i = 0; i < 4; i++) {
            let fi = f32(i);
            let life = 2.5 + 1.5 * hash(vec2<f32>(fk, fi));
            let age = fract(f.time / life + hash(vec2<f32>(fi, fk + 7.0)));
            let bx = r.x + 10.0 + (r.z - 20.0) * hash(vec2<f32>(fk + 3.0, fi)) + sin(age * 12.0 + fi) * 1.5;
            let by = r.y + r.w - 3.0 - age * (r.y + r.w - surface - 4.0);
            let bd = distance(p, vec2<f32>(bx, by));
            let rr = 0.9 + 0.8 * hash(vec2<f32>(fi, fk + 2.0));
            liquid += vec3<f32>(0.9) * smoothstep(rr + 0.6, rr, bd) * (0.3 + 0.7 * smoothstep(rr - 1.0, rr, bd)) * 0.45;
        }
        let a = smoothstep(-1.0, 0.6, depth) * (0.55 + 0.4 * f.strength);
        c = mix(c, liquid, a);
        // the meniscus: a bright line where the surface meets the air
        c += vec3<f32>(1.0) * exp(-depth * depth * 1.4) * 0.35;
    }
    // the jar's glass: faint light down its inner sides
    let side = min(p.x - r.x, r.x + r.z - p.x);
    c += vec3<f32>(0.85, 0.95, 1.0) * exp(-pow(side - 3.0, 2.0) * 0.6) * 0.08 * f.strength;
    put(id.xy, c);
}
