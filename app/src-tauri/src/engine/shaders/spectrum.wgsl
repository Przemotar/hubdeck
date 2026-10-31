// The music studio, for musicians: the spectrum of what plays behind the icons, one analyser through
// the fifteen windows. Sixteen bars from the bass on the left to the highs on the right rise from
// the bottom in lit segments (green, amber at the top, red at the peak) over a dark studio wall,
// their reflection on a glossy floor below them, the wall glowing softly on each beat. With no
// music the bars breathe low. A press: a wave of sound runs out from the key over the others, the
// bars it crosses lighting up (`aux[0..3]`: x, y, start, 1 when on; `aux[4..8]`: the sixteen
// bands 0..1; `aux[8]`: 1 while music is heard, seconds since its last beat).
fn band(i: i32) -> f32 {
    return f.aux[4 + i / 4][i % 4];
}
@compute @workgroup_size(16, 16)
fn under(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= f.size.x || id.y >= f.size.y) { return; }
    let p = vec2<f32>(f32(id.x), f32(id.y)) + 0.5;
    var c = px(i32(id.x), i32(id.y)).rgb;
    let t = f.time;
    let size = vec2<f32>(f.size);
    let heard = f.aux[8].x > 0.5;
    let since = f.aux[8].y;
    // the wall: deep blue-violet, a soft glow on each beat from below
    let beat = select(0.0, exp(-since * 5.0), heard);
    var col = mix(vec3<f32>(0.03, 0.025, 0.06), vec3<f32>(0.08, 0.05, 0.12), p.y / size.y);
    col += vec3<f32>(0.25, 0.08, 0.3) * beat * 0.5 * exp(-(size.y - p.y) / 90.0);
    // the bars: sixteen across, a little gap between, standing on the floor line
    let floor_y = size.y - 30.0;
    let bw = size.x / 16.0;
    let i = i32(clamp(floor(p.x / bw), 0.0, 15.0));
    let local = p.x - f32(i) * bw;
    var level: f32;
    if (heard) {
        level = band(i);
    } else {
        // no music: the bars breathe low, a slow wave along them
        level = 0.12 + 0.08 * sin(t * 1.3 - f32(i) * 0.5) + 0.04 * vnoise(vec2<f32>(f32(i) * 1.7, t * 0.8));
    }
    // a press's wave of sound: the bars it crosses jump
    var wave = 0.0;
    for (var k = 0; k < 3; k++) {
        let b = f.aux[k];
        if (b.w < 0.5) { continue; }
        let age = t - b.z;
        if (age < 0.0 || age > 2.0) { continue; }
        let r = age * 240.0;
        let d = length(p - b.xy);
        // the wavefront: a few rings of a sound wave, fading out
        let ring = cos((d - r) * 0.35) * exp(-abs(d - r) / 14.0) * (1.0 - age / 2.0);
        wave = max(wave, ring);
        let barc = vec2<f32>((f32(i) + 0.5) * bw, floor_y);
        level = max(level, exp(-abs(length(barc - b.xy) - r) / 40.0) * (1.0 - age / 2.0) * 0.9);
    }
    let top = floor_y - level * (floor_y - 8.0);
    let seg_h = 6.0;
    let inbar = abs(local - bw * 0.5) < bw * 0.5 - 2.5;
    if (inbar && p.y < floor_y && p.y > top) {
        let seg = fract((floor_y - p.y) / seg_h);
        if (seg > 0.18) {
            let h = (floor_y - p.y) / (floor_y - 8.0);
            var lit = mix(vec3<f32>(0.1, 0.85, 0.45), vec3<f32>(1.0, 0.72, 0.15), smoothstep(0.45, 0.75, h));
            lit = mix(lit, vec3<f32>(1.0, 0.18, 0.15), smoothstep(0.8, 0.92, h));
            // the segment's own glass: brighter in the middle
            lit *= 0.7 + 0.3 * smoothstep(0.5, 0.0, abs(seg - 0.59) * 2.0);
            col = lit * 0.85;
        }
    } else if (inbar && p.y < floor_y && p.y > 8.0) {
        // unlit segments, faint, so the analyser reads even when quiet
        let seg = fract((floor_y - p.y) / seg_h);
        if (seg > 0.18) { col += vec3<f32>(0.05, 0.06, 0.08); }
    }
    // the floor: the bars mirrored, dim and soft
    if (p.y > floor_y + 2.0 && inbar) {
        let m = floor_y - (p.y - floor_y - 2.0);
        if (m > top) {
            let h = (floor_y - m) / (floor_y - 8.0);
            let lit = mix(vec3<f32>(0.1, 0.85, 0.45), vec3<f32>(1.0, 0.72, 0.15), smoothstep(0.45, 0.75, h));
            col += lit * 0.22 * exp(-(p.y - floor_y) / 12.0);
        }
    }
    // the wave of sound over everything, a little
    col += vec3<f32>(0.4, 0.6, 1.0) * max(wave, 0.0) * 0.18;
    let lum = dot(col, vec3<f32>(0.299, 0.587, 0.114));
    if (lum > 0.45) { col *= (0.45 + (lum - 0.45) * 0.4) / lum; }
    col = clamp(col, vec3<f32>(0.0), vec3<f32>(1.0));
    c = mix(c, col, f.strength / 0.6 * 0.97 * (1.0 - keep_bg(c)));
    put(id.xy, c);
}
