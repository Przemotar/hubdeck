// The music studio B, a turntable: a vinyl record spinning behind the icons, one record through the
// fifteen windows. Black grooves catching the room's light in two soft spokes that stay still as
// the record turns (the way light lies on a real record), a red label in the middle with its
// spindle, the track gaps as darker rings; the music's spectrum glows in the grooves, the bass
// rings near the label, the highs at the rim, so the record lights up with what plays; the
// platter's strobe dots at the edge. A press: the needle's scratch, the record jolts and a ring of
// light runs out through the grooves (`aux[0..3]`: x, y, start, 1 when on; `aux[4..8]`: the
// sixteen bands; `aux[8]`: 1 while music is heard, seconds since its last beat).
const C = vec2<f32>(240.0, 136.0);
const R = 200.0;
fn band(i: i32) -> f32 {
    return f.aux[4 + i / 4][i % 4];
}
@compute @workgroup_size(16, 16)
fn under(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= f.size.x || id.y >= f.size.y) { return; }
    let p = vec2<f32>(f32(id.x), f32(id.y)) + 0.5;
    var c = px(i32(id.x), i32(id.y)).rgb;
    let t = f.time;
    let heard = f.aux[8].x > 0.5;
    let q = p - C;
    let r = length(q);
    let a = atan2(q.y, q.x);
    // the platter round the record: brushed metal, strobe dots at its rim
    var col = vec3<f32>(0.05, 0.05, 0.06) * (0.9 + 0.2 * vnoise(vec2<f32>(a * 40.0, r * 0.05)));
    // a scratch jolts the record back a little
    var jolt = 0.0;
    var ring_light = 0.0;
    for (var i = 0; i < 3; i++) {
        let b = f.aux[i];
        if (b.w < 0.5) { continue; }
        let age = t - b.z;
        if (age < 0.0 || age > 1.6) { continue; }
        jolt += -0.5 * sin(age * 18.0) * exp(-age * 5.0);
        ring_light = max(ring_light, exp(-abs(r - age * 220.0) / 8.0) * (1.0 - age / 1.6));
    }
    // 33 1/3 turns a minute
    let spin = t * 3.4907 + jolt;
    if (r < R) {
        // grooves: fine rings, their sheen from two light spokes fixed in the room
        let groove = 0.5 + 0.5 * sin(r * 2.6);
        let spoke = pow(abs(cos(a - 0.7)), 18.0) + 0.6 * pow(abs(cos(a + 0.9)), 30.0);
        var rec = vec3<f32>(0.035, 0.035, 0.04) * (0.85 + 0.3 * groove);
        rec += vec3<f32>(0.36, 0.36, 0.4) * spoke * (0.6 + 0.4 * groove) * smoothstep(46.0, 70.0, r);
        // the track gaps: smoother, darker rings
        let gaps = smoothstep(1.2, 0.0, abs(fract(r / 37.0 + 0.3) - 0.5) * 37.0 - 0.0);
        rec *= 1.0 - 0.5 * gaps;
        // the music glowing in the grooves: the bass near the label, the highs at the rim
        let bi = i32(clamp((r - 48.0) / (R - 48.0) * 16.0, 0.0, 15.0));
        var level = 0.0;
        if (heard) { level = band(bi); } else { level = 0.08 + 0.05 * sin(t * 1.4 - f32(bi) * 0.6); }
        let hue = mix(vec3<f32>(0.9, 0.2, 0.6), vec3<f32>(0.2, 0.7, 1.0), f32(bi) / 15.0);
        rec += hue * level * level * (0.35 + 0.65 * groove) * 0.55 * smoothstep(46.0, 54.0, r);
        rec += vec3<f32>(0.5, 0.8, 1.0) * ring_light * 0.5;
        // the label: red paper, a ring printed on it, the spindle; it turns with the record
        if (r < 46.0) {
            let la = a - spin;
            var lab = vec3<f32>(0.4, 0.05, 0.06) * (0.9 + 0.1 * vnoise(q * 0.1));
            lab = mix(lab, vec3<f32>(0.7, 0.62, 0.45), smoothstep(1.0, 0.0, abs(r - 40.0) - 0.6) * 0.7);
            // a printed mark that shows the turning
            lab = mix(lab, vec3<f32>(0.95, 0.88, 0.7), smoothstep(0.08, 0.0, abs(sin(la)) * r / 20.0) * step(12.0, r) * step(r, 32.0) * step(0.0, cos(la)) * 0.7);
            lab = mix(lab, vec3<f32>(0.75, 0.75, 0.78), smoothstep(5.0, 4.0, r));
            rec = mix(rec, lab, smoothstep(46.0, 45.0, r));
        }
        col = mix(col, rec, smoothstep(R + 0.8, R - 0.8, r));
    }
    // the strobe dots on the platter's edge, standing still at the right speed
    let sa = (a + 3.14159) / 6.283 * 90.0;
    let strobe = smoothstep(0.35, 0.15, abs(fract(sa) - 0.5)) * smoothstep(2.0, 0.5, abs(r - (R + 8.0)));
    col += vec3<f32>(0.55, 0.55, 0.6) * strobe * 0.5;
    let lum = dot(col, vec3<f32>(0.299, 0.587, 0.114));
    if (lum > 0.42) { col *= (0.42 + (lum - 0.42) * 0.4) / lum; }
    col = clamp(col, vec3<f32>(0.0), vec3<f32>(1.0));
    c = mix(c, col, f.strength / 0.6 * 0.97 * (1.0 - keep_bg(c)));
    put(id.xy, c);
}
