// The painter's studio B, oil on canvas: a night painted in thick impasto behind the icons, one
// canvas through the fifteen windows. Short, loaded brushstrokes follow slow swirling currents
// (deep blues and teals, with ribbons of cadmium yellow and cream curling through them), each stroke
// a ridge of paint with a lit edge and a shadowed one under the studio lamp from the upper left,
// the linen's weave showing where the paint is thin; the currents turn very slowly. A press: a
// palette knife drags a smear of fresh paint through the key, pushing the strokes aside, then it
// settles (`aux[0..3]`: x, y, start, 1 when on).
fn fbm(p: vec2<f32>) -> f32 {
    var v = 0.0;
    var a = 0.5;
    var q = p;
    for (var i = 0; i < 4; i++) {
        v += a * vnoise(q);
        q = q * 2.03 + vec2<f32>(17.1, 9.3);
        a *= 0.5;
    }
    return v;
}
// the currents: a slowly turning swirl field (the curl of a noise), its direction at p; broad
// swirls, so the strokes curl in big arcs
fn flow(p: vec2<f32>, t: f32) -> vec2<f32> {
    let e = 2.0;
    let s = 0.0075;
    let o = vec2<f32>(t * 0.02, -t * 0.015);
    let n1 = fbm(p * s + vec2<f32>(0.0, e * s) + o);
    let n2 = fbm(p * s - vec2<f32>(0.0, e * s) + o);
    let n3 = fbm(p * s + vec2<f32>(e * s, 0.0) + o);
    let n4 = fbm(p * s - vec2<f32>(e * s, 0.0) + o);
    return normalize(vec2<f32>(n1 - n2, -(n3 - n4)) + vec2<f32>(0.0001));
}
fn palette(v: f32) -> vec3<f32> {
    // deep ultramarine, prussian, teal, cadmium yellow, cream
    if (v < 0.35) { return mix(vec3<f32>(0.05, 0.08, 0.26), vec3<f32>(0.08, 0.2, 0.42), v / 0.35); }
    if (v < 0.62) { return mix(vec3<f32>(0.08, 0.2, 0.42), vec3<f32>(0.12, 0.42, 0.48), (v - 0.35) / 0.27); }
    if (v < 0.82) { return mix(vec3<f32>(0.12, 0.42, 0.48), vec3<f32>(0.92, 0.72, 0.18), (v - 0.62) / 0.2); }
    return mix(vec3<f32>(0.92, 0.72, 0.18), vec3<f32>(0.96, 0.9, 0.7), (v - 0.82) / 0.18);
}
// two octaves of noise: a stroke's direction and colour vary slowly over the canvas, and a stroke is
// tested 36 times a pixel (the full four octaves cost 6.5 ms a picture)
fn soft(p: vec2<f32>) -> f32 {
    return 0.67 * vnoise(p) + 0.33 * vnoise(p * 2.03 + vec2<f32>(17.1, 9.3));
}
fn flow2(p: vec2<f32>, t: f32) -> vec2<f32> {
    let s = 0.0075;
    let o = vec2<f32>(t * 0.02, -t * 0.015);
    let e = 2.0 * s;
    let n1 = soft(p * s + vec2<f32>(0.0, e) + o);
    let n2 = soft(p * s - vec2<f32>(0.0, e) + o);
    let n3 = soft(p * s + vec2<f32>(e, 0.0) + o);
    let n4 = soft(p * s - vec2<f32>(e, 0.0) + o);
    return normalize(vec2<f32>(n1 - n2, -(n3 - n4)) + vec2<f32>(0.0001));
}
// one brushstroke from cell g of layer l: its middle, direction (along the currents), length,
// width and colour value; .x how far inside (0 out .. 1 on its ridge), .y across (-1..1), .z
// along (0..1), .w its colour value
fn stroke(p: vec2<f32>, g: vec2<f32>, l: f32, cell: f32, t: f32) -> vec4<f32> {
    let h = hash(g + vec2<f32>(l * 17.0, 3.0));
    let c = (g + vec2<f32>(hash(g + l + 1.3), hash(g + l + 2.7))) * cell;
    let dir = flow2(c, t);
    let nrm = vec2<f32>(-dir.y, dir.x);
    let len = cell * (1.7 + 0.9 * h);
    let wid = cell * (0.56 + 0.16 * hash(g + l + 4.1));
    let d = p - c;
    // a gentle curve along the stroke, as a wrist moves
    let a = dot(d, dir);
    let b = dot(d, nrm) - a * a * (hash(g + l + 5.9) - 0.5) * 0.02;
    let along = a / len + 0.5;
    if (along < 0.0 || along > 1.0) { return vec4<f32>(0.0); }
    // loaded at the start, thinning and splitting at the end
    let taper = smoothstep(0.0, 0.12, along) * (1.0 - 0.55 * smoothstep(0.55, 1.0, along));
    let across = b / (wid * taper + 0.001);
    let ragged = 0.12 * (vnoise(vec2<f32>(along * 9.0, l + g.x)) - 0.5);
    let inside = smoothstep(1.0 + ragged, 0.8 + ragged, abs(across));
    let v = soft(c * 0.007 + vec2<f32>(t * 0.01, 0.0)) * 0.7 + 0.45 * smoothstep(0.46, 0.66, soft(c * 0.01 + 7.0)) + (h - 0.5) * 0.18;
    return vec4<f32>(inside, clamp(across, -1.0, 1.0), along, v);
}
@compute @workgroup_size(16, 16)
fn under(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= f.size.x || id.y >= f.size.y) { return; }
    let p = vec2<f32>(f32(id.x), f32(id.y)) + 0.5;
    var c = px(i32(id.x), i32(id.y)).rgb;
    let t = f.time * f.speed;
    // the canvas under the paint: linen, dark umber ground
    var col = vec3<f32>(0.1, 0.09, 0.08) * (0.85 + 0.3 * vnoise(p * 0.7));
    // four layers of strokes, each on its own jittered grid, overlapping so no ground shows but
    // at a stroke's thin end; a later layer lies on top
    let cell = 15.0;
    for (var l = 0; l < 3; l++) {
        let fl = f32(l);
        let off = vec2<f32>(fl * 5.3, fl * 9.1);
        let g0 = floor((p + off) / cell);
        var best = vec4<f32>(0.0);
        var bh = -1.0;
        for (var j = -1; j <= 1; j++) {
            for (var i = -1; i <= 1; i++) {
                let g = g0 + vec2<f32>(f32(i), f32(j));
                let s = stroke(p + off, g, fl, cell, t);
                if (s.x > 0.02) {
                    let order = hash(g + vec2<f32>(fl, 11.0));
                    if (order > bh) { bh = order; best = s; }
                }
            }
        }
        if (best.x > 0.0) {
            var paint = palette(clamp(best.w, 0.0, 1.0));
            // the ridge: raised in the middle, lit on the side facing the lamp (upper left), a
            // shadow on the other, bristle grooves along it
            let ridge = sqrt(max(1.0 - best.y * best.y, 0.0));
            let bristle = 0.5 + 0.5 * sin(best.y * 9.0 + hash(vec2<f32>(fl, best.w * 40.0)) * 6.0);
            paint *= 0.55 + 0.5 * ridge - 0.28 * best.y + 0.14 * (bristle - 0.5);
            // the wet edge of the ridge catching the lamp, and the paint's own shadow beside it
            paint += vec3<f32>(0.2, 0.19, 0.15) * smoothstep(0.35, 0.8, -best.y) * smoothstep(1.0, 0.85, -best.y);
            paint *= 1.0 - 0.3 * smoothstep(0.6, 0.98, best.y);
            // the paint thins out at the stroke's end, the layer under it showing
            let thin = smoothstep(0.7, 1.0, best.z) * 0.4;
            col = mix(col, paint, best.x * (1.0 - thin));
        }
    }
    // a press: a palette knife drags a swoosh of fresh paint through the key, tapered at both ends,
    // its ridge lit along the leading edge, streaked along the stroke
    for (var i = 0; i < 3; i++) {
        let b = f.aux[i];
        if (b.w < 0.5) { continue; }
        let age = f.time - b.z;
        if (age < 0.0 || age > 3.2) { continue; }
        let k = key_at(b.xy);
        if (k < 0) { continue; }
        let r = keys[k];
        // the stroke's path: from the key's left to its right, a gentle arc
        let u = (p.x - (r.x - 6.0)) / (r.z + 12.0);
        let reach = smoothstep(0.0, 0.5, age);
        if (u < 0.0 || u > reach) { continue; }
        let mid = r.y + r.w * 0.52 - sin(u * 3.14159) * r.w * 0.16;
        // a knife's stroke: full almost at once, a rounded start, thinning a little where it lifts
        let lift = clamp(u / max(reach, 0.01), 0.0, 1.0);
        let half = r.w * 0.19 * smoothstep(0.0, 0.12, u) * (1.0 - 0.35 * smoothstep(0.75, 1.0, lift));
        let d = abs(p.y - mid) - half;
        if (d > 1.0) { continue; }
        let fresh = palette(0.68 + 0.3 * fract(b.z * 0.37));
        // streaks along the knife's way, and the paint scraped thin at the edges
        let along = vnoise(vec2<f32>(p.x * 0.04, (p.y - mid) * 1.1 + b.z));
        var paint = fresh * (0.72 + 0.4 * along);
        // the ridge on the upper edge catches the lamp, the lower edge sits in shade
        let edge = clamp((p.y - mid) / max(half, 0.5), -1.0, 1.0);
        paint *= 1.0 + 0.35 * smoothstep(-0.4, -0.95, edge) - 0.3 * smoothstep(0.4, 0.95, edge);
        let fade = 1.0 - smoothstep(2.2, 3.2, age);
        col = mix(col, paint, smoothstep(0.8, -0.8, d) * fade * 0.95);
    }
    let lum = dot(col, vec3<f32>(0.299, 0.587, 0.114));
    if (lum > 0.4) { col *= (0.4 + (lum - 0.4) * 0.35) / lum; }
    col = clamp(col, vec3<f32>(0.0), vec3<f32>(1.0));
    c = mix(c, col, f.strength / 0.6 * 0.97 * (1.0 - keep_bg(c)));
    put(id.xy, c);
}
