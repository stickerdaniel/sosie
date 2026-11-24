use sosie_core::{rasterize, DisplayList, Fonts, LimitError, Limits, RenderError, Rgba};

fn render(width: f32, height: f32, dpr: f32, background: Rgba) -> Result<(u32, u32), RenderError> {
    let list = DisplayList::new(width, height, background);
    rasterize(&Fonts::new(), &list, dpr, &Limits::default()).map(|p| (p.width, p.height))
}

#[test]
fn output_size_follows_css_size_and_dpr() {
    assert_eq!(render(400.0, 100.0, 2.0, Rgba::WHITE), Ok((800, 200)));
}

#[test]
fn dpr_outside_one_to_four_is_refused() {
    for dpr in [0.5, 4.5, f32::NAN] {
        assert!(
            matches!(
                render(400.0, 100.0, dpr, Rgba::WHITE),
                Err(RenderError::Limit(LimitError::Dpr { .. }))
            ),
            "dpr {dpr}"
        );
    }
}

#[test]
fn oversized_output_fails_before_allocation() {
    assert_eq!(
        render(5_000.0, 100.0, 2.0, Rgba::WHITE),
        Err(RenderError::Limit(LimitError::Width {
            px: 10_000,
            max: 8_192
        }))
    );
    assert_eq!(
        render(400.0, 20_000.0, 2.0, Rgba::WHITE),
        Err(RenderError::Limit(LimitError::Height {
            px: 40_000,
            max: 32_768
        }))
    );
    assert_eq!(
        render(4_000.0, 16_000.0, 2.0, Rgba::WHITE),
        Err(RenderError::Limit(LimitError::Pixels {
            pixels: 256_000_000,
            max: 64_000_000
        }))
    );
    assert_eq!(
        render(f32::INFINITY, 1e30, 4.0, Rgba::WHITE),
        Err(RenderError::Limit(LimitError::Size {
            width: f32::INFINITY,
            height: 1e30
        }))
    );
}

#[test]
fn translucent_background_is_refused() {
    assert_eq!(
        render(400.0, 100.0, 2.0, Rgba::TRANSPARENT),
        Err(RenderError::TranslucentBackground)
    );
}
