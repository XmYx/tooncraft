use proptest::prelude::*;
use tooncraft_raster::RgbaImage;

use super::*;

/// A 32×32 image with an opaque black 8×8 square in the middle.
fn square() -> RgbaImage {
    let mut img = RgbaImage::transparent(32, 32);
    for y in 12..20 {
        for x in 12..20 {
            img.set_pixel(x, y, [0, 0, 0, 255]);
        }
    }
    img
}

fn premultiplied(img: &RgbaImage) -> bool {
    img.data.as_chunks::<4>().0.iter().all(|p| p[0] <= p[3] && p[1] <= p[3] && p[2] <= p[3])
}

fn run(kind: EffectKind, img: &mut RgbaImage) {
    apply_stack(img, &[Effect::new(kind)], 0, None);
}

#[test]
fn blur_spreads_and_keeps_the_total() {
    let mut img = square();
    let before: u64 = img.data.as_chunks::<4>().0.iter().map(|p| p[3] as u64).sum();
    run(EffectKind::Blur { radius: 2.0 }, &mut img);
    let after: u64 = img.data.as_chunks::<4>().0.iter().map(|p| p[3] as u64).sum();
    assert!(img.pixel(10, 16)[3] > 0, "alpha spreads outside the square");
    assert!(img.pixel(16, 16)[3] > 200, "the middle stays nearly opaque");
    assert!(before.abs_diff(after) * 50 < before, "blur keeps the coverage: {before} vs {after}");
    assert!(premultiplied(&img));
}

#[test]
fn directional_and_radial_blur_streak() {
    let mut img = square();
    run(EffectKind::DirectionalBlur { length: 10.0, angle: 0.0 }, &mut img);
    assert!(img.pixel(8, 16)[3] > 0 && img.pixel(16, 8)[3] == 0, "streaks only horizontally");
    let mut img = square();
    run(EffectKind::RadialBlur { amount: 0.5, centre_x: 0.0, centre_y: 0.0 }, &mut img);
    assert!(img.pixel(22, 22)[3] > 0, "zoom blur streaks away from the centre");
    assert!(img.pixel(9, 9)[3] == 0, "and not towards it");
}

#[test]
fn shadow_outline_glow() {
    let mut img = square();
    run(EffectKind::DropShadow { dx: 6.0, dy: 6.0, blur: 0.0, colour: [255, 0, 0], opacity: 1.0 }, &mut img);
    assert_eq!(img.pixel(24, 24), [255, 0, 0, 255], "the shadow is offset");
    assert_eq!(img.pixel(16, 16), [0, 0, 0, 255], "the drawing stays on top");
    let mut img = square();
    run(EffectKind::Outline { width: 2.0, colour: [255, 255, 255] }, &mut img);
    assert_eq!(img.pixel(10, 16), [255, 255, 255, 255]);
    assert_eq!(img.pixel(8, 16)[3], 0, "no wider than asked");
    assert_eq!(img.pixel(16, 16), [0, 0, 0, 255]);
    let mut img = RgbaImage::transparent(32, 32);
    img.set_pixel(16, 16, [200, 200, 200, 255]);
    run(EffectKind::Glow { radius: 3.0, intensity: 2.0, colour: [255, 255, 255] }, &mut img);
    assert!(img.pixel(19, 16)[0] > 0, "light spreads");
    assert!(premultiplied(&img));
}

#[test]
fn thicken_and_thin_keep_colours() {
    let mut img = RgbaImage::transparent(32, 32);
    for y in 4..28 {
        img.set_pixel(16, y, [0, 0, 200, 255]);
    }
    let mut thick = img.clone();
    run(EffectKind::Thicken { amount: 2.0 }, &mut thick);
    assert_eq!(thick.pixel(18, 16), [0, 0, 200, 255], "grown with the line's colour");
    assert_eq!(thick.pixel(20, 16)[3], 0);
    let mut thin = thick.clone();
    run(EffectKind::Thicken { amount: -2.0 }, &mut thin);
    assert_eq!(thin.pixel(18, 16)[3], 0);
    assert_eq!(thin.pixel(16, 16), [0, 0, 200, 255]);
}

#[test]
fn colour_effects() {
    let mut img = RgbaImage::filled(4, 4, [200, 100, 50, 255]);
    run(EffectKind::Levels { in_black: 0.0, in_white: 1.0, gamma: 1.0, out_black: 1.0, out_white: 0.0 }, &mut img);
    assert_eq!(img.pixel(0, 0), [55, 155, 205, 255]);
    let mut img = RgbaImage::filled(4, 4, [255, 0, 0, 255]);
    run(EffectKind::HueSaturation { hue: 120.0, saturation: 0.0, lightness: 0.0 }, &mut img);
    assert_eq!(img.pixel(0, 0), [0, 255, 0, 255]);
    run(EffectKind::Tint { colour: [0, 0, 255], amount: 1.0 }, &mut img);
    assert_eq!(img.pixel(0, 0), [0, 0, 255, 255]);
    let mut img = RgbaImage::filled(8, 8, [128, 128, 128, 255]);
    run(EffectKind::Grain { amount: 0.5, size: 1.0, seed: 3, animated: false }, &mut img);
    let distinct: std::collections::BTreeSet<u8> = img.data.as_chunks::<4>().0.iter().map(|p| p[0]).collect();
    assert!(distinct.len() > 4, "grain varies per pixel");
    let mut a = RgbaImage::filled(8, 8, [128, 128, 128, 255]);
    let mut b = a.clone();
    apply(&mut a, &EffectKind::Grain { amount: 0.5, size: 1.0, seed: 3, animated: true }, 1, None);
    apply(&mut b, &EffectKind::Grain { amount: 0.5, size: 1.0, seed: 3, animated: true }, 2, None);
    assert_ne!(a, b, "animated grain changes every frame");
    let mut img = RgbaImage::transparent(2, 1);
    img.set_pixel(0, 0, [100, 50, 0, 200]);
    img.set_pixel(1, 0, [10, 5, 0, 20]);
    run(EffectKind::AlphaThreshold { threshold: 0.5 }, &mut img);
    assert_eq!(img.pixel(0, 0)[3], 255);
    assert_eq!(img.pixel(1, 0), [0, 0, 0, 0]);
}

#[test]
fn feathered_mask() {
    let mut img = RgbaImage::filled(100, 10, [0, 0, 0, 255]);
    run(
        EffectKind::Mask { shape: MaskShape::Rectangle, x: 25.0, y: 5.0, width: 50.0, height: 100.0, feather: 20.0, invert: false, points: Vec::new() },
        &mut img,
    );
    assert_eq!(img.pixel(10, 5)[3], 255);
    assert_eq!(img.pixel(80, 5)[3], 0);
    let mid = img.pixel(50, 5)[3];
    assert!((100..160).contains(&mid), "half way through the feather: {mid}");
    let mut inv = RgbaImage::filled(100, 10, [0, 0, 0, 255]);
    run(
        EffectKind::Mask { shape: MaskShape::Rectangle, x: 25.0, y: 5.0, width: 50.0, height: 100.0, feather: 20.0, invert: true, points: Vec::new() },
        &mut inv,
    );
    assert_eq!((inv.pixel(10, 5)[3], inv.pixel(80, 5)[3]), (0, 255));
}

#[test]
fn light_wrap_needs_the_layers_below() {
    let below = RgbaImage::filled(32, 32, [255, 255, 255, 255]);
    let mut img = square();
    let plain = img.clone();
    run(EffectKind::LightWrap { radius: 3.0, intensity: 1.0 }, &mut img);
    assert_eq!(img, plain, "without the layers below nothing changes");
    apply_stack(&mut img, &[Effect::new(EffectKind::LightWrap { radius: 3.0, intensity: 1.0 })], 0, Some(&below));
    assert!(img.pixel(12, 16)[0] > 30, "the edge picks up the white below");
    assert!(img.pixel(16, 16)[0] < img.pixel(12, 16)[0], "the middle less than the edge");
}

#[test]
fn colour_key_and_effect_blending() {
    let mut img = RgbaImage::filled(4, 1, [255, 255, 255, 255]);
    img.set_pixel(1, 0, [0, 0, 0, 255]);
    img.set_pixel(2, 0, [240, 240, 240, 255]);
    run(EffectKind::ColourKey { colour: [255, 255, 255], tolerance: 0.02, softness: 0.1 }, &mut img);
    assert_eq!(img.pixel(0, 0)[3], 0, "the key colour goes");
    assert_eq!(img.pixel(1, 0), [0, 0, 0, 255], "far colours stay");
    let a = img.pixel(2, 0)[3];
    assert!(a > 0 && a < 255, "near colours fade: {a}");
    // half the effect, and a blend mode for its result
    let base = RgbaImage::filled(2, 1, [200, 100, 50, 255]);
    let mut half = base.clone();
    let mut e = Effect::new(EffectKind::Tint { colour: [0, 0, 0], amount: 1.0 });
    e.mix = 0.5;
    apply_stack(&mut half, std::slice::from_ref(&e), 0, None);
    assert_eq!(half.pixel(0, 0), [100, 50, 25, 255]);
    let mut screen = base.clone();
    e.mix = 1.0;
    e.blend = BlendMode::Screen;
    apply_stack(&mut screen, &[e], 0, None);
    assert_eq!(screen.pixel(0, 0), base.pixel(0, 0), "screening black changes nothing");
    let json = serde_json::to_string(&Effect::new(EffectKind::Blur { radius: 1.0 })).unwrap();
    assert!(!json.contains("blend") && !json.contains("mix"), "defaults stay out of the file: {json}");
}

#[test]
fn keys_interpolate_and_hold() {
    let mut e = Effect::new(EffectKind::Blur { radius: 1.0 });
    assert_eq!(e.at(50), EffectKind::Blur { radius: 1.0 });
    assert!(e.set_key(10, EffectKind::Blur { radius: 0.0 }));
    assert!(e.set_key(20, EffectKind::Blur { radius: 10.0 }));
    assert!(!e.set_key(5, EffectKind::Thicken { amount: 1.0 }), "a key of another kind is refused");
    assert_eq!(e.at(0), EffectKind::Blur { radius: 0.0 });
    assert_eq!(e.at(15), EffectKind::Blur { radius: 5.0 });
    assert_eq!(e.at(99), EffectKind::Blur { radius: 10.0 });
    assert!(e.edit(15, EffectKind::Blur { radius: 2.0 }), "with keys an edit sets a key");
    assert_eq!(e.keys.len(), 3);
    assert!(e.remove_key(15) && !e.remove_key(15));
    let c = EffectKind::Curves { points: vec![[0.0, 0.0], [1.0, 1.0]] }.lerp(&EffectKind::Curves { points: vec![[0.0, 0.5], [1.0, 1.0]] }, 0.5);
    assert_eq!(c, EffectKind::Curves { points: vec![[0.0, 0.25], [1.0, 1.0]] });
}

#[test]
fn json_round_trip_and_hostile_values() {
    let stack: Vec<Effect> = EffectKind::catalogue(1920, 1080).into_iter().map(Effect::new).collect();
    let json = serde_json::to_string(&stack).unwrap();
    let back: Vec<Effect> = serde_json::from_str(&json).unwrap();
    assert_eq!(back, stack);
    let hostile: Effect = serde_json::from_str(
        r#"{"kind":{"type":"blur","radius":1e30},"keys":[{"frame":3,"value":{"type":"glow","radius":1,"intensity":1,"colour":[1,2,3]}},{"frame":1,"value":{"type":"blur","radius":-5}}]}"#,
    )
    .unwrap();
    let s = hostile.sanitized();
    assert_eq!(s.kind, EffectKind::Blur { radius: MAX_RADIUS });
    assert_eq!(s.keys, vec![EffectKey { frame: 1, value: EffectKind::Blur { radius: 0.0 } }]);
    assert!(s.enabled, "enabled by default");
    let nan = EffectKind::Levels { in_black: f32::NAN, in_white: f32::NAN, gamma: f32::INFINITY, out_black: 2.0, out_white: -1.0 }.sanitized();
    assert_eq!(nan, EffectKind::Levels { in_black: 0.0, in_white: 1.0, gamma: 1.0, out_black: 1.0, out_white: 0.0 });
}

#[test]
fn degenerate_images_are_left_alone() {
    let mut empty = RgbaImage { width: 0, height: 0, data: Vec::new() };
    let mut wrong = RgbaImage { width: 5, height: 5, data: vec![0; 7] };
    for k in EffectKind::catalogue(5, 5) {
        apply(&mut empty, &k, 0, None);
        apply(&mut wrong, &k, 0, None);
    }
    assert_eq!(wrong.data.len(), 7);
}

fn any_kind() -> impl Strategy<Value = EffectKind> {
    let f = || prop_oneof![Just(f32::NAN), Just(f32::INFINITY), -1e6f32..1e6, -5.0f32..60.0];
    let col = || any::<[u8; 3]>();
    prop_oneof![
        f().prop_map(|radius| EffectKind::Blur { radius }),
        (f(), f()).prop_map(|(length, angle)| EffectKind::DirectionalBlur { length, angle }),
        (f(), f(), f()).prop_map(|(amount, centre_x, centre_y)| EffectKind::RadialBlur { amount, centre_x, centre_y }),
        (f(), f(), col()).prop_map(|(radius, intensity, colour)| EffectKind::Glow { radius, intensity, colour }),
        (f(), f(), f(), col(), f()).prop_map(|(dx, dy, blur, colour, opacity)| EffectKind::DropShadow { dx, dy, blur, colour, opacity }),
        (f(), col()).prop_map(|(width, colour)| EffectKind::Outline { width, colour }),
        f().prop_map(|amount| EffectKind::Thicken { amount }),
        (f(), f(), f(), f(), f()).prop_map(|(a, b, c, d, e)| EffectKind::Levels { in_black: a, in_white: b, gamma: c, out_black: d, out_white: e }),
        proptest::collection::vec((f(), f()), 0..40).prop_map(|p| EffectKind::Curves { points: p.into_iter().map(|(x, y)| [x, y]).collect() }),
        (f(), f(), f()).prop_map(|(hue, saturation, lightness)| EffectKind::HueSaturation { hue, saturation, lightness }),
        (col(), f()).prop_map(|(colour, amount)| EffectKind::Tint { colour, amount }),
        (f(), f(), any::<u32>(), any::<bool>()).prop_map(|(amount, size, seed, animated)| EffectKind::Grain { amount, size, seed, animated }),
        (f(), f()).prop_map(|(radius, intensity)| EffectKind::LightWrap { radius, intensity }),
        f().prop_map(|threshold| EffectKind::AlphaThreshold { threshold }),
        (col(), f(), f()).prop_map(|(colour, tolerance, softness)| EffectKind::ColourKey { colour, tolerance, softness }),
        (0u8..3, f(), f(), f(), f(), f(), any::<bool>(), proptest::collection::vec((f(), f()), 0..12)).prop_map(
            |(e, x, y, width, height, feather, invert, pts)| EffectKind::Mask {
                shape: [MaskShape::Rectangle, MaskShape::Ellipse, MaskShape::Drawn][e as usize % 3],
                x,
                y,
                width,
                height,
                feather,
                invert,
                points: pts.into_iter().map(|(a, b)| [a, b]).collect(),
            }
        ),
        (f(), f(), f(), f(), f(), f(), col(), any::<u32>(), any::<u32>()).prop_map(|(x, y, through_x, through_y, size, intensity, colour, rays, ghosts)| {
            EffectKind::LensFlare { x, y, through_x, through_y, size, intensity, colour, rays, ghosts }
        }),
        (any::<u64>(), f(), f(), any::<bool>()).prop_map(|(layer, amount_x, amount_y, brightness)| EffectKind::Displace {
            layer,
            amount_x,
            amount_y,
            brightness
        }),
        (f(), any::<u32>()).prop_map(|(shutter, samples)| EffectKind::MotionBlur { shutter, samples }),
        (any::<u32>(), any::<u32>(), f()).prop_map(|(count, step, fade)| EffectKind::Trails { count, step, fade }),
        f().prop_map(|amount| EffectKind::FrameBlend { amount }),
        (f(), f()).prop_map(|(speed, offset)| EffectKind::TimeRemap { speed, offset }),
        (0u8..4, 0u32..20, any::<u32>(), any::<bool>(), f(), f(), f(), f()).prop_map(|(st, start, length, out, angle, x, y, size)| EffectKind::Transition {
            style: [TransitionStyle::Dissolve, TransitionStyle::Wipe, TransitionStyle::Iris, TransitionStyle::Push][st as usize % 4],
            start,
            length,
            out,
            angle,
            x,
            y,
            size
        }),
        (col(), col(), f()).prop_map(|(dark, light, amount)| EffectKind::GradientMap { dark, light, amount }),
        any::<u32>().prop_map(|levels| EffectKind::Posterize { levels }),
        f().prop_map(|level| EffectKind::Threshold { level }),
        (f(), f(), f(), f(), f()).prop_map(|(x, y, radius, softness, amount)| EffectKind::Vignette { x, y, radius, softness, amount }),
        f().prop_map(|amount| EffectKind::ChromaticAberration { amount }),
        (f(), f(), f(), f(), f()).prop_map(|(x, y, amplitude, wavelength, speed)| EffectKind::Ripple { x, y, amplitude, wavelength, speed }),
        (f(), f(), f(), f()).prop_map(|(amplitude, wavelength, angle, speed)| EffectKind::Wave { amplitude, wavelength, angle, speed }),
        (f(), f(), any::<u32>(), any::<bool>()).prop_map(|(amount, scale, seed, animated)| EffectKind::Turbulence { amount, scale, seed, animated }),
        f().prop_map(|radius| EffectKind::Smooth { radius }),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(48))]
    /// Running a stack on the drawn area plus its reach gives the same pixels as the whole canvas.
    #[test]
    fn a_crop_with_the_reach_equals_the_whole_canvas(kinds in proptest::collection::vec(any_kind(), 1..4), x0 in 0u32..40, y0 in 0u32..40) {
        let (w, h) = (96u32, 80u32);
        let mut full = RgbaImage::transparent(w, h);
        for y in y0..y0 + 12 {
            for x in x0..x0 + 9 {
                full.set_pixel(x, y, [30, 60, 90, 200]);
            }
        }
        let below = RgbaImage::filled(w, h, [200, 100, 50, 255]);
        let stack: Vec<Effect> = kinds.into_iter().map(Effect::new).collect();
        let Some(r) = stack_reach(&stack, 0) else { return Ok(()) };
        let (cx, cy) = (x0.saturating_sub(r), y0.saturating_sub(r));
        let (cw, ch) = ((x0 + 9 + r).min(w) - cx, (y0 + 12 + r).min(h) - cy);
        let mut part = full.crop(cx, cy, cw, ch);
        let below_part = below.crop(cx, cy, cw, ch);
        apply_stack_at(&mut part, (cx, cy), &stack, 3, Some(&below_part));
        apply_stack(&mut full, &stack, 3, Some(&below));
        prop_assert_eq!(part, full.crop(cx, cy, cw, ch));
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(160))]
    /// Any parameters, any image: no panic, same size, still premultiplied.
    #[test]
    fn hostile_parameters_keep_images_valid(kinds in proptest::collection::vec(any_kind(), 1..4), w in 1u32..40, h in 1u32..40, seed in any::<u64>()) {
        let mut img = RgbaImage::transparent(w, h);
        let mut s = seed;
        for p in img.data.as_chunks_mut::<4>().0.iter_mut() {
            s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            let a = (s >> 56) as u8;
            let c = ((s >> 40) as u8).min(a);
            p.copy_from_slice(&[c, c / 2, a / 3, a]);
        }
        let below = RgbaImage::filled(w, h, [10, 200, 30, 255]);
        let modes = [BlendMode::Normal, BlendMode::Multiply, BlendMode::Screen, BlendMode::Overlay, BlendMode::Add];
        let stack: Vec<Effect> = kinds.into_iter().enumerate().map(|(i, k)| {
            let mut e = Effect::new(k);
            e.blend = modes[(seed as usize + i) % modes.len()];
            e.mix = ((seed >> (i * 3)) % 5) as f32 / 4.0;
            e
        }).collect();
        apply_stack(&mut img, &stack, 7, Some(&below));
        prop_assert_eq!((img.width, img.height, img.data.len()), (w, h, (w * h * 4) as usize));
        prop_assert!(premultiplied(&img));
    }

    /// Interpolated parameters stay within the keys' range.
    #[test]
    fn interpolation_stays_between_keys(a in 0.0f32..100.0, b in 0.0f32..100.0, f in 0u32..40) {
        let mut e = Effect::new(EffectKind::Blur { radius: 0.0 });
        e.set_key(10, EffectKind::Blur { radius: a });
        e.set_key(30, EffectKind::Blur { radius: b });
        let EffectKind::Blur { radius } = e.at(f) else { return Err(TestCaseError::fail("kind changed")) };
        prop_assert!(radius >= a.min(b) - 1e-3 && radius <= a.max(b) + 1e-3);
    }
}

#[test]
fn presets_and_saved_stacks() {
    for (id, stack) in crate::presets(640, 360) {
        assert!(!stack.is_empty() && stack.len() <= MAX_EFFECTS, "{id}");
        let json = crate::stack_to_json(&stack).unwrap();
        assert_eq!(crate::stack_from_json(&json).unwrap(), stack.into_iter().map(Effect::sanitized).collect::<Vec<_>>());
    }
    assert!(crate::stack_from_json("not json").is_err());
    let blur = |radius| Effect { kind: EffectKind::Blur { radius }, ..Effect::new(EffectKind::Blur { radius: 1.0 }) };
    let too_many = crate::stack_to_json(&vec![blur(1.0); MAX_EFFECTS + 1]).unwrap();
    assert!(crate::stack_from_json(&too_many).is_err());
    // hostile numbers come back clamped
    let hostile = crate::stack_to_json(&[blur(1e30)]).unwrap();
    assert_eq!(crate::stack_from_json(&hostile).unwrap()[0].kind, EffectKind::Blur { radius: MAX_RADIUS });
}

/// A drawn mask shape seen through a part of the canvas equals the whole canvas, and a shape
/// with fixed points (from [`resample_closed`]) blends between keys.
#[test]
fn drawn_masks_and_their_keys() {
    let tri = vec![[10.0, 10.0], [80.0, 12.0], [30.0, 70.0]];
    let mask =
        |points: Vec<[f32; 2]>| EffectKind::Mask { shape: MaskShape::Drawn, x: 0.0, y: 0.0, width: 0.0, height: 0.0, feather: 12.0, invert: false, points };
    let mut full = RgbaImage::filled(96, 80, [40, 40, 40, 255]);
    let mut part = full.crop(20, 16, 40, 30);
    apply_stack(&mut full, &[Effect::new(mask(tri.clone()))], 0, None);
    apply_stack_at(&mut part, (20, 16), &[Effect::new(mask(tri.clone()))], 0, None);
    assert_eq!(part, full.crop(20, 16, 40, 30));
    assert_eq!(full.pixel(40, 30)[3], 255, "inside the triangle");
    assert_eq!(full.pixel(90, 75)[3], 0, "outside it");
    let a = resample_closed(&tri, MASK_SHAPE_POINTS);
    let b: Vec<[f32; 2]> = a.iter().map(|p| [p[0] + 10.0, p[1]]).collect();
    let mut e = Effect::new(mask(a.clone()));
    e.set_key(0, mask(a.clone()));
    e.set_key(10, mask(b));
    let EffectKind::Mask { points, .. } = e.at(5) else { panic!("kind") };
    assert_eq!(points.len(), MASK_SHAPE_POINTS);
    assert!((points[0][0] - (a[0][0] + 5.0)).abs() < 1e-3, "half way between the keys");
}

/// A lens flare lights a layer with no paint of its own; displacement needs its map.
#[test]
fn flares_generate_and_displacement_reads_its_layer() {
    let flare = Effect::new(EffectKind::LensFlare {
        x: 20.0,
        y: 20.0,
        through_x: 48.0,
        through_y: 40.0,
        size: 6.0,
        intensity: 1.0,
        colour: [255, 255, 255],
        rays: 4,
        ghosts: 2,
    });
    assert!(generates(std::slice::from_ref(&flare)));
    assert_eq!(stack_reach(std::slice::from_ref(&flare), 0), None, "a flare can light the whole picture");
    let mut img = RgbaImage::transparent(96, 80);
    apply_stack(&mut img, std::slice::from_ref(&flare), 0, None);
    assert!(img.pixel(20, 20)[3] > 200);
    let shift = Effect::new(EffectKind::Displace { layer: 7, amount_x: 5.0, amount_y: 0.0, brightness: false });
    assert_eq!(source_layers(&[shift.clone(), shift.clone()]), vec![7], "each layer once");
    let mut line = RgbaImage::transparent(40, 10);
    for y in 0..10 {
        line.set_pixel(10, y, [0, 0, 0, 255]);
    }
    let mut without = line.clone();
    apply_stack(&mut without, std::slice::from_ref(&shift), 0, None);
    assert_eq!(without, line, "no map given, nothing moves");
    let map = RgbaImage::filled(40, 10, [255, 128, 0, 255]);
    let layers = [(7u64, map)];
    apply_stack_with(&mut line, (0, 0), std::slice::from_ref(&shift), 0, Inputs { below: None, layers: &layers });
    assert!(line.pixel(15, 4)[3] > 200, "moved by the map: {:?}", line.pixel(15, 4));
}
