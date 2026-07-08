pub fn spectral_color(hue: f32, intensity: f32) -> [f32; 4] {
    let hue = hue.fract();
    let saturation = 0.72;
    let value = intensity.clamp(0.2, 1.25);
    let sector = hue * 6.0;
    let chroma = value * saturation;
    let x = chroma * (1.0 - ((sector % 2.0) - 1.0).abs());
    let m = value - chroma;

    let (r, g, b) = match sector as i32 {
        0 => (chroma, x, 0.0),
        1 => (x, chroma, 0.0),
        2 => (0.0, chroma, x),
        3 => (0.0, x, chroma),
        4 => (x, 0.0, chroma),
        _ => (chroma, 0.0, x),
    };

    [r + m, g + m, b + m, 1.0]
}

pub fn lerp_color(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
    let t = t.clamp(0.0, 1.0);
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
        a[3] + (b[3] - a[3]) * t,
    ]
}
