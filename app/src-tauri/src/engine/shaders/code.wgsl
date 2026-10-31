// Matrix B, for developers working with AI agents: a dark code editor behind the icons, one screen
// through the fifteen windows. Lines of code with syntax colours (keywords violet, functions gold,
// strings amber, names light blue, comments grey-green), indentation and a gutter of line numbers,
// scrolling up slowly as an agent writes new ones at the bottom with a blinking cursor, the
// current line lit. A press selects the lines under the key (a blue selection that fades) and
// sends a wave out from it that re-types the characters it passes (`aux[0..3]`: x, y, start, on).
const CW = 6.0;
const CH = 10.0;
fn sd_seg(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>) -> f32 {
    let pa = p - a;
    let ba = b - a;
    let h = clamp(dot(pa, ba) / dot(ba, ba), 0.0, 1.0);
    return length(pa - ba * h);
}
// a character `g` (0..1) of strokes on a 3x4 grid in a 4x6 box, top left at 0; distance in px
fn glyph(q: vec2<f32>, g: f32) -> f32 {
    var d = 1e3;
    var n = 0;
    for (var i = 0; i < 17; i++) {
        let fi = f32(i);
        if (hash(vec2<f32>(g * 97.0, fi)) > 0.34 && !(i == 9 && n < 2)) { continue; }
        n += 1;
        var a = vec2<f32>(0.0);
        var b = vec2<f32>(0.0);
        if (i < 8) {
            let r = f32(i / 2);
            a = vec2<f32>(f32(i % 2) * 2.0, r * 2.0);
            b = a + vec2<f32>(2.0, 0.0);
        } else {
            let j = i - 8;
            a = vec2<f32>(f32(j % 3) * 2.0, f32(j / 3) * 2.0);
            b = a + vec2<f32>(0.0, 2.0);
        }
        d = min(d, sd_seg(q, a, b));
    }
    return d;
}
struct Line {
    indent: f32,
    len: f32,
    comment: bool,
}
fn line_of(n: f32) -> Line {
    let h = hash(vec2<f32>(n, 3.0));
    // blocks: indentation rises and falls like real code
    let indent = floor(2.0 + 1.6 * sin(n * 0.45) + 1.2 * sin(n * 0.13 + 1.0)) * 2.0;
    // real code: blank lines between blocks, short lines (a brace, a return) and long ones
    var len = floor(18.0 + 52.0 * hash(vec2<f32>(n, 5.0)));
    if (hash(vec2<f32>(n, 8.0)) < 0.3) { len = floor(3.0 + 10.0 * hash(vec2<f32>(n, 6.0))); }
    if (h < 0.2) { len = 0.0; }
    return Line(max(indent, 0.0), len, h > 0.86);
}
@compute @workgroup_size(16, 16)
fn under(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= f.size.x || id.y >= f.size.y) { return; }
    let p = vec2<f32>(f32(id.x), f32(id.y)) + 0.5;
    var c = px(i32(id.x), i32(id.y)).rgb;
    let rate = 0.55 * f.speed;
    // the scroll: a new line every 1 / rate seconds, sliding up smoothly
    let written = f.time * rate;
    let scroll = written * CH;
    let yy = p.y + scroll;
    let n = floor(yy / CH);
    let last = floor(written) + floor(272.0 / CH);
    var col = vec3<f32>(0.075, 0.08, 0.1);
    // the current line (the one being written) lit
    if (n == last) { col = vec3<f32>(0.1, 0.11, 0.14); }
    // the gutter: line numbers, dim
    let gutter = 30.0;
    if (p.x < gutter - 4.0) {
        let digits = 3.0;
        let k = floor((p.x - 3.0) / CW);
        if (k >= 0.0 && k < digits) {
            let q = vec2<f32>(p.x - 3.0 - k * CW, yy - n * CH - 2.0);
            let num = floor(n / pow(10.0, digits - 1.0 - k)) % 10.0;
            let d = glyph(q, (num + 1.0) * 0.0731);
            col = mix(col, vec3<f32>(0.32, 0.34, 0.4), smoothstep(0.9, 0.3, d));
        }
    } else {
        let ln = line_of(n);
        let k = floor((p.x - gutter) / CW);
        // how much of the bottom line is typed so far
        var typed = ln.len;
        if (n == last) { typed = floor(fract(written) * (ln.len + 6.0)); }
        if (k >= ln.indent && k < ln.indent + min(ln.len, typed)) {
            // words: a space now and then, each word its own colour
            let w = floor((k - ln.indent) / 5.0 + hash(vec2<f32>(n, 9.0)) * 3.0);
            let gap = hash(vec2<f32>(n * 3.1 + k, 2.0)) < 0.16;
            if (!gap) {
                let q = vec2<f32>(p.x - gutter - k * CW, yy - n * CH - 2.0);
                let g = hash(vec2<f32>(n * 7.0 + k, 1.0));
                let d = glyph(q, g);
                var ink = vec3<f32>(0.62, 0.8, 0.98);
                let kind = hash(vec2<f32>(n, w));
                if (ln.comment) { ink = vec3<f32>(0.42, 0.55, 0.4); }
                else if (kind < 0.2) { ink = vec3<f32>(0.78, 0.5, 0.92); }
                else if (kind < 0.35) { ink = vec3<f32>(0.9, 0.84, 0.5); }
                else if (kind < 0.5) { ink = vec3<f32>(0.86, 0.58, 0.42); }
                else if (kind < 0.62) { ink = vec3<f32>(0.6, 0.62, 0.66); }
                // a press's wave re-types what it passes: a bright flash, then the colour again
                var flash = 0.0;
                for (var i = 0; i < 3; i++) {
                    let b = f.aux[i];
                    if (b.w < 0.5) { continue; }
                    let age = f.time - b.z;
                    let ring = abs(length(p - b.xy) - age * 260.0);
                    flash = max(flash, exp(-ring / 10.0) * (1.0 - smoothstep(0.8, 1.6, age)));
                }
                ink = mix(ink, vec3<f32>(0.9, 1.0, 0.95), flash * 0.8);
                col = mix(col, ink, smoothstep(0.9, 0.3, d) * select(0.68, 0.95, n == last) * (1.0 - 0.6 * flash * step(0.5, hash(vec2<f32>(k + floor(f.time * 20.0), n)))));
            }
        }
        // the cursor blinking after the last typed character
        if (n == last && k == ln.indent + min(ln.len, typed)) {
            let cur = abs(p.x - (gutter + k * CW + 0.5)) < 0.8 && fract(f.time * 1.6) < 0.6;
            if (cur) { col = vec3<f32>(0.85, 0.9, 1.0); }
        }
    }
    // a press selects the lines under the key: a blue selection fading
    for (var i = 0; i < 3; i++) {
        let b = f.aux[i];
        if (b.w < 0.5) { continue; }
        let age = f.time - b.z;
        let k = key_at(b.xy);
        if (k < 0) { continue; }
        let r = keys[k];
        let sel = step(r.y, p.y) * step(p.y, r.y + r.w) * (1.0 - smoothstep(1.2, 2.6, age)) * smoothstep(0.0, 0.12, age);
        col = mix(col, col + vec3<f32>(0.05, 0.12, 0.3), sel * 0.9);
    }
    col = clamp(col, vec3<f32>(0.0), vec3<f32>(1.0));
    c = mix(c, col, f.strength / 0.6 * 0.97 * (1.0 - keep_bg(c)));
    put(id.xy, c);
}
