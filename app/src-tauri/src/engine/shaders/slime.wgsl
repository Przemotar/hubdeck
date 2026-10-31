// Halloween, spooky keys: glowing green slime oozes from the top edge of each key: a wavy band
// along the top with drips that slowly stretch down, swell at their tips, and now and then let a
// drop fall to the key's floor; glossy, lit from above, a faint glow round it.
@compute @workgroup_size(16, 16)
fn over(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= f.size.x || id.y >= f.size.y) { return; }
    let p = vec2<f32>(f32(id.x), f32(id.y)) + 0.5;
    var c = px(i32(id.x), i32(id.y)).rgb;
    let k = key_at(p);
    if (k < 0) { put(id.xy, c); return; }
    let r = keys[k];
    let fk = f32(k);
    let lx = p.x - r.x;
    let ly = p.y - r.y;
    // the band along the top
    var depth = 3.5 + 1.5 * sin(lx * 0.35 + fk) + 1.0 * sin(lx * 0.9 + fk * 3.0);
    // drips: four per key, each its own slow cycle of stretching and letting go
    var d = ly - depth;
    var drop = 1e3;
    for (var i = 0; i < 4; i++) {
        let fi = f32(i);
        let x0 = r.z * (0.15 + 0.23 * fi) + 4.0 * (hash(vec2<f32>(fk, fi)) - 0.5);
        let period = 5.0 + 4.0 * hash(vec2<f32>(fi, fk + 3.0));
        let ph = fract(f.time / period + hash(vec2<f32>(fk, fi + 9.0)));
        let len = 4.0 + 22.0 * smoothstep(0.0, 0.8, ph) * (0.5 + 0.5 * hash(vec2<f32>(fk + 1.0, fi)));
        let w = 1.6 + 1.2 * hash(vec2<f32>(fi + 4.0, fk));
        // the drip: a narrowing tongue with a round tip
        let dx = abs(lx - x0);
        let tongue = max(dx - w * (1.0 - 0.45 * clamp(ly / max(len, 1.0), 0.0, 1.0)), ly - len);
        let tip = length(vec2<f32>(lx - x0, ly - len)) - w * 1.25;
        d = min(d, min(tongue, tip));
        // a drop falling after the drip lets go
        if (ph > 0.8) {
            let fall = (ph - 0.8) / 0.2;
            let dy = len + fall * fall * (r.w - len);
            drop = min(drop, length(vec2<f32>(lx - x0, ly - dy)) - w);
        }
    }
    d = min(d, drop);
    let slime = smoothstep(0.7, -0.7, d);
    let green = vec3<f32>(0.35, 0.95, 0.25);
    // glossy: lighter near its upper surface, a highlight line
    let gloss = exp(-pow(d + 1.6, 2.0) * 0.8);
    let body = mix(green * 0.55, green * 1.05, smoothstep(-4.0, 0.0, d)) + vec3<f32>(0.8, 1.0, 0.8) * gloss * 0.45;
    c += green * exp(-max(d, 0.0) / 4.0) * 0.18 * f.strength;
    c = mix(c, body, slime * 0.92 * f.strength);
    put(id.xy, c);
}
