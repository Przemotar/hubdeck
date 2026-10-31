// Halloween, spooky keys: the icons turn spectral: their light goes ghostly green, a soft green
// halo breathes round them, now and then they flicker as if something passed, and a thin green
// mist hangs low in each key.
@compute @workgroup_size(16, 16)
fn over(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= f.size.x || id.y >= f.size.y) { return; }
    let p = vec2<f32>(f32(id.x), f32(id.y)) + 0.5;
    var c = px(i32(id.x), i32(id.y)).rgb;
    let k = key_at(p);
    if (k < 0) { put(id.xy, c); return; }
    let r = keys[k];
    let fk = f32(k);
    let m = cover(i32(id.x), i32(id.y));
    let ghost = vec3<f32>(0.45, 1.0, 0.6);
    // the halo: the content's cover softened, breathing
    var halo = 0.0;
    for (var i = 0; i < 8; i++) {
        let a = f32(i) * 0.785;
        halo += cover(i32(p.x + cos(a) * 3.0), i32(p.y + sin(a) * 3.0));
    }
    halo = halo / 8.0;
    let breath = 0.6 + 0.4 * sin(f.time * 1.3 + fk * 1.7);
    // a flicker now and then, key by key
    let flick = select(1.0, 0.35, hash(vec2<f32>(fk, floor(f.time * 10.0))) > 0.985);
    let luma = dot(c, vec3<f32>(0.299, 0.587, 0.114));
    c = mix(c, ghost * (0.35 + 0.9 * luma), m * 0.75 * f.strength) * mix(1.0, flick, m);
    c += ghost * halo * (1.0 - m) * 0.35 * breath * f.strength;
    // the low mist in each key
    let low = smoothstep(r.y + r.w * 0.55, r.y + r.w, p.y);
    let mist = low * (0.5 + 0.5 * vnoise(vec2<f32>(p.x / 9.0 + f.time * 0.4, p.y / 5.0 + fk)));
    c = mix(c, ghost * 0.35 + vec3<f32>(0.05), mist * 0.35 * (1.0 - m) * f.strength);
    put(id.xy, c);
}
