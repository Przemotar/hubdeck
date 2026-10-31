// Japan: red paper lanterns (chōchin) hanging behind the icons from a cord sagging across the top
// of the deck, every other one lower, swaying a little, glowing from inside with a candle's
// flicker: ribs across the paper, black caps, a warm halo on what is around them.
fn lantern(q: vec2<f32>, w: f32, h: f32) -> vec3<f32> {
    // .x: the paper's distance (negative inside), .y: the caps', .z: how far up the body (0..1)
    let body = length(q / vec2<f32>(w, h)) - 1.0;
    let caps = max(abs(q.x) - w * 0.55, abs(abs(q.y) - h * 0.98) - h * 0.1);
    return vec3<f32>(body * min(w, h), caps, clamp(0.5 - q.y / (2.0 * h), 0.0, 1.0));
}
@compute @workgroup_size(16, 16)
fn under(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= f.size.x || id.y >= f.size.y) { return; }
    let size = vec2<f32>(f.size);
    let p = vec2<f32>(f32(id.x), f32(id.y)) + 0.5;
    var c = px(i32(id.x), i32(id.y)).rgb;
    let k = f.strength / 0.6;
    // the cord: a shallow sag across the top
    let u = p.x / size.x;
    let cord_y = 14.0 + 16.0 * (1.0 - pow(2.0 * u - 1.0, 2.0));
    var glow = vec3<f32>(0.0);
    var col = c;
    let cord = smoothstep(1.2, 0.3, abs(p.y - cord_y));
    col = mix(col, vec3<f32>(0.06, 0.04, 0.03), cord * 0.85 * k * (1.0 - keep_bg(c)));
    let n = 6;
    for (var j = 0; j < n; j++) {
        let fj = f32(j);
        let ux = (fj + 0.5) / f32(n);
        let ax = ux * size.x;
        let ay = 14.0 + 16.0 * (1.0 - pow(2.0 * ux - 1.0, 2.0));
        let drop = select(18.0, 98.0, j % 2 == 1);
        // a slow pendulum, each its own, the wind leaning them
        let swing = 0.07 * sin(f.time * (1.1 + 0.13 * fj) + fj * 1.9) + f.wind * 0.15;
        let dir = vec2<f32>(sin(swing), cos(swing));
        let centre = vec2<f32>(ax, ay) + dir * (drop + 17.0);
        // the string down to it
        let along = clamp(dot(p - vec2<f32>(ax, ay), dir), 0.0, drop);
        let sd = length(p - vec2<f32>(ax, ay) - dir * along);
        col = mix(col, vec3<f32>(0.08, 0.05, 0.03), smoothstep(0.9, 0.2, sd) * k * (1.0 - keep_bg(c)));
        // the lantern in its own upright space
        let d0 = p - centre;
        let q = vec2<f32>(d0.x * dir.y - d0.y * dir.x, d0.x * dir.x + d0.y * dir.y);
        let flick = 0.82 + 0.18 * vnoise(vec2<f32>(f.time * 7.0 + fj * 13.0, fj));
        glow += vec3<f32>(1.0, 0.42, 0.14) * exp(-length(d0) / 26.0) * 0.5 * flick;
        let l = lantern(q, 13.0, 17.0);
        if (l.x < 1.0) {
            // the paper: brightest at the middle, the ribs darker, red with the candle behind
            let mid = 1.0 - length(q / vec2<f32>(13.0, 17.0));
            var paper = mix(vec3<f32>(0.62, 0.08, 0.04), vec3<f32>(1.0, 0.55, 0.22), smoothstep(0.0, 0.9, mid)) * flick;
            let rib = smoothstep(0.35, 0.0, abs(fract(q.y / 3.4) - 0.5) - 0.3);
            paper *= 1.0 - 0.28 * rib;
            // a darker band where a character would be painted, simplified to a stripe
            paper *= 1.0 - 0.18 * smoothstep(1.4, 0.4, abs(q.x)) * smoothstep(9.0, 6.0, abs(q.y));
            col = mix(col, paper, smoothstep(0.7, -0.7, l.x) * k);
        }
        col = mix(col, vec3<f32>(0.04, 0.03, 0.03), smoothstep(0.6, -0.6, l.y) * k);
    }
    col += glow * k * (1.0 - keep_bg(c) * 0.6);
    put(id.xy, clamp(col, vec3<f32>(0.0), vec3<f32>(1.0)));
}
