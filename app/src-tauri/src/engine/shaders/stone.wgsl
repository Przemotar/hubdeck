// Halloween, spooky keys: each key set in an old stone frame: a band of grey stone round the
// window with a rough, cracked surface and moss in the cracks, lit from above, dark in the
// corners; the icon inside stays as it is.
fn fbm(p: vec2<f32>) -> f32 {
    var v = 0.0;
    var a = 0.5;
    var q = p;
    for (var i = 0; i < 4; i++) {
        v += a * vnoise(q);
        q = q * 2.03 + vec2<f32>(1.7, 9.2);
        a *= 0.5;
    }
    return v;
}
@compute @workgroup_size(16, 16)
fn over(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= f.size.x || id.y >= f.size.y) { return; }
    let p = vec2<f32>(f32(id.x), f32(id.y)) + 0.5;
    var c = px(i32(id.x), i32(id.y)).rgb;
    let k = key_at(p);
    if (k < 0) { put(id.xy, c); return; }
    let r = keys[k];
    let inner = key_inside(p, r);
    let band = 6.5 + 1.5 * (vnoise(vec2<f32>((p.x + p.y) * 0.2, f32(k))) - 0.5);
    if (inner > band + 1.0) { put(id.xy, c); return; }
    // the stone: grey, rough, with cracks (thin dark lines where the noise crosses its middle)
    let n = fbm(p / 3.5 + vec2<f32>(f32(k) * 5.0, 0.0));
    var stone = mix(vec3<f32>(0.22, 0.21, 0.24), vec3<f32>(0.42, 0.40, 0.44), n);
    let crack = 1.0 - smoothstep(0.0, 0.04, abs(fbm(p / 6.0 + vec2<f32>(9.0, f32(k))) - 0.5));
    stone = mix(stone, vec3<f32>(0.05, 0.05, 0.06), crack * 0.8);
    stone = mix(stone, vec3<f32>(0.18, 0.32, 0.12), crack * smoothstep(0.5, 0.8, n) * 0.6);
    // lit from above: the top band lighter, the lower darker, the inner edge in shadow
    let top = smoothstep(r.y + r.w * 0.5, r.y, p.y);
    stone *= 0.75 + 0.45 * top;
    stone *= 0.55 + 0.45 * smoothstep(band, band - 2.0, inner);
    let a = smoothstep(band + 0.8, band - 0.4, inner) * f.strength;
    c = mix(c, stone, a);
    // the window's inner edge throws a little shadow on the screen inside
    c *= 1.0 - 0.35 * smoothstep(band + 4.0, band, inner) * (1.0 - a) * f.strength;
    put(id.xy, c);
}
