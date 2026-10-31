// Japan: an ukiyo-e night behind the icons, one woodblock print through the fifteen windows: a deep
// Prussian blue sky fading lighter towards the hills (bokashi), a big pale moon, thin bands of
// cloud with dark key lines drifting across it, flat layered hills along the bottom, the grain of
// washi paper over all of it.
fn band(p: vec2<f32>, y: f32, thick: f32, drift: f32, seed: f32) -> f32 {
    // a long cloud band: wavy top and bottom edges, rounded ends; negative inside
    let x = p.x - drift;
    let wob = 2.5 * sin(x * 0.045 + seed) + 1.5 * sin(x * 0.11 + seed * 2.0);
    let half = thick * (0.6 + 0.4 * sin(x * 0.02 + seed * 3.0));
    let d = abs(p.y - y - wob) - half;
    // the band is broken into pieces along x
    let piece = fract(x / 230.0 + seed * 0.1);
    let gap = 0.5 - abs(piece - 0.5);
    return max(d, (0.06 - gap) * 230.0 * 0.25);
}
@compute @workgroup_size(16, 16)
fn under(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= f.size.x || id.y >= f.size.y) { return; }
    let size = vec2<f32>(f.size);
    let p = vec2<f32>(f32(id.x), f32(id.y)) + 0.5;
    var c = px(i32(id.x), i32(id.y)).rgb;
    let v = p.y / size.y;
    // bokashi: the woodblock's graded blue, dark at the top
    var col = mix(vec3<f32>(0.04, 0.09, 0.22), vec3<f32>(0.32, 0.43, 0.58), smoothstep(0.05, 0.85, v));
    // the moon, its soft halo, a faint rim of the paper's cream
    let moon = vec2<f32>(size.x * 0.74, 78.0);
    let md = length(p - moon) - 46.0;
    col += vec3<f32>(0.85, 0.82, 0.62) * exp(-max(md, 0.0) / 30.0) * 0.22;
    let disc = smoothstep(0.8, -0.8, md);
    let moon_col = vec3<f32>(0.97, 0.93, 0.78) * (0.95 + 0.05 * vnoise(p * 0.12));
    col = mix(col, moon_col, disc);
    // cloud bands drifting slowly left, grey-blue with a darker key line, passing over the moon
    for (var i = 0; i < 3; i++) {
        let fi = f32(i);
        let y = 60.0 + fi * 34.0 + 6.0 * sin(fi * 2.1);
        let d = band(p, y, 4.5 + fi * 1.5, -f.time * (3.0 + fi * 1.4), fi * 5.3);
        let fill = mix(vec3<f32>(0.5, 0.56, 0.66), vec3<f32>(0.78, 0.8, 0.82), disc * 0.6);
        col = mix(col, fill, smoothstep(0.6, -0.6, d) * 0.9);
        col = mix(col, vec3<f32>(0.08, 0.1, 0.18), smoothstep(1.0, 0.2, abs(d)) * 0.7);
    }
    // the hills: three flat layers, darker as they come near, with a key line along their tops
    for (var i = 0; i < 3; i++) {
        let fi = f32(i);
        let top = size.y - 70.0 + fi * 22.0 - 16.0 * sin(p.x * (0.011 + fi * 0.004) + fi * 1.7) - 6.0 * sin(p.x * 0.037 + fi);
        let fill = mix(vec3<f32>(0.2, 0.28, 0.42), vec3<f32>(0.03, 0.05, 0.12), fi / 2.0);
        let d = top - p.y;
        col = mix(col, fill, smoothstep(0.6, -0.6, d));
        col = mix(col, vec3<f32>(0.02, 0.03, 0.08), smoothstep(1.0, 0.2, abs(d)) * 0.6);
    }
    // washi: fibres and grain
    col *= 0.94 + 0.06 * hash(floor(p * 0.9)) + 0.03 * vnoise(vec2<f32>(p.x * 0.6, p.y * 0.08));
    c = mix(c, col, f.strength / 0.6 * 0.95 * (1.0 - keep_bg(c)));
    put(id.xy, clamp(c, vec3<f32>(0.0), vec3<f32>(1.0)));
}
