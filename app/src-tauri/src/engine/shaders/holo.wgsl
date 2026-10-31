// A holographic foil: the icons and labels turn into foil that throws rainbow colours, sliding as
// the light's angle moves (slowly with time; later the dial's turn), with the fine diffraction
// lines of a hologram sticker and a white crease of light passing over; the keys' backgrounds
// keep only a faint shimmer, so the icons stand out as the foil.
fn spectrum(x: f32) -> vec3<f32> {
    // a rainbow, saturated but bright
    return clamp(abs(fract(vec3<f32>(x) + vec3<f32>(0.0, 0.667, 0.333)) * 6.0 - 3.0) - 1.0, vec3<f32>(0.0), vec3<f32>(1.0));
}
@compute @workgroup_size(16, 16)
fn over(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= f.size.x || id.y >= f.size.y) { return; }
    let size = vec2<f32>(f.size);
    let p = vec2<f32>(f32(id.x), f32(id.y)) + 0.5;
    var c = px(i32(id.x), i32(id.y)).rgb;
    let m = cover(i32(id.x), i32(id.y));
    let uv = p / size;
    let angle = f.time * 0.10 + f.wind;
    // diffraction: fine diagonal lines whose colour turns with the angle and the place
    let lines = 0.5 + 0.5 * sin((p.x * 0.9 + p.y * 1.4) * 1.3);
    let phase = uv.x * 1.3 + uv.y * 0.8 + angle + 0.18 * vnoise(p / 19.0) + lines * 0.05 * m;
    let foil = spectrum(phase);
    let luma = dot(c, vec3<f32>(0.299, 0.587, 0.114));
    // the content becomes foil (keeping its own light and dark); the background only shimmers
    // thin content (line icons, labels) turns to foil; a whole picture only takes a sheen, so a
    // logo keeps its own colours; the background keeps a faint, smooth shimmer (no fine lines: they
    // made every picture much heavier to send)
    let thin = 1.0 - smoothstep(0.55, 0.95, (cover(i32(id.x) - 3, i32(id.y)) + cover(i32(id.x) + 3, i32(id.y)) + cover(i32(id.x), i32(id.y) - 3) + cover(i32(id.x), i32(id.y) + 3)) * 0.25);
    let on_content = m * f.strength;
    // the foil keeps the icon's light and dark and shines in full rainbow colour
    c = mix(c, foil * (0.45 + 0.95 * luma) + vec3<f32>(0.12), on_content * mix(0.35, 0.95, thin));
    c += spectrum(uv.x * 0.9 + uv.y * 0.6 + angle) * (1.0 - m) * f.strength * 0.05;
    // the crease of light passing over
    let crease = exp(-pow((uv.x * 1.2 - uv.y * 0.5 - fract(angle * 0.4) * 2.2 + 0.5) * 7.0, 2.0));
    c += vec3<f32>(1.0, 0.98, 0.96) * crease * (0.12 + 0.6 * m) * f.strength;
    // sparkles of the foil's grain, where the crease passes
    let glint = step(0.985, hash(floor(p / 2.0) + floor(f.time * 6.0))) * m * (0.3 + crease);
    c += vec3<f32>(1.0) * glint * 0.8 * f.strength;
    put(id.xy, c);
}
