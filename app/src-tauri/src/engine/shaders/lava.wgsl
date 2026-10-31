// Trial: a lava lamp behind the icons: soft blobs of warm wax rise, stretch, part and sink in
// a glowing liquid, crossing from block to block; lit from below, with a rim where wax meets liquid.
@compute @workgroup_size(16, 16)
fn under(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= f.size.x || id.y >= f.size.y) { return; }
    let size = vec2<f32>(f.size);
    let p = vec2<f32>(f32(id.x), f32(id.y)) + 0.5;
    let uv = p / size;
    let base = px(i32(id.x), i32(id.y)).rgb;
    let t = f.time * 0.35;
    // the field of 9 blobs, each rising and sinking on its own slow cycle
    var field = 0.0;
    var grad = vec2<f32>(0.0);
    for (var i = 0; i < 9; i++) {
        let fi = f32(i);
        let cyc = t * (0.25 + 0.2 * hash(vec2<f32>(fi, 1.0))) + hash(vec2<f32>(fi, 2.0)) * 6.28;
        let x = size.x * (0.08 + 0.84 * hash(vec2<f32>(fi, 3.0))) + sin(cyc * 1.3 + fi) * 30.0;
        let y = size.y * (0.5 - 0.62 * sin(cyc)) ;
        let r = 26.0 + 22.0 * hash(vec2<f32>(fi, 4.0));
        // stretched upright while moving fast
        let stretch = 1.0 + 0.35 * abs(cos(cyc));
        let d = (p - vec2<f32>(x, y)) * vec2<f32>(1.0, 1.0 / stretch);
        // a soft gaussian blob: bounded slope, so no dark pinch at its middle
        let v = 1.7 * exp(-dot(d, d) / (r * r));
        field += v;
        grad += -2.0 * d / (r * r) * v;
    }
    let liquid = mix(vec3<f32>(0.55, 0.14, 0.24), vec3<f32>(0.32, 0.07, 0.20), uv.y * 0.6 + 0.2) * 0.8 + f.tint.rgb * 0.08;
    let waxc = mix(vec3<f32>(1.0, 0.62, 0.15), vec3<f32>(1.0, 0.35, 0.12), uv.y);
    let edge = smoothstep(0.92, 1.08, field);
    let n = normalize(vec3<f32>(grad * 14.0, 1.0));
    let lit = 0.7 + 0.35 * dot(n, normalize(vec3<f32>(0.2, 0.7, 0.7)));
    var c = mix(liquid, waxc * lit, edge);
    // the rim and the glow round the wax
    c += vec3<f32>(1.0, 0.55, 0.25) * exp(-abs(field - 1.0) * 6.0) * 0.18;
    // lit from below
    c *= 0.75 + 0.45 * uv.y;
    put(id.xy, mix(base, c, (0.88 * f.strength + 0.12) * (1.0 - keep_bg(base))));
}
