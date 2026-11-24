use sosie_core::{
    rasterize, DisplayList, Fonts, Item, LimitError, Limits, Radii, Rect, RenderError, Rgba, Shape,
};

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

fn filled(width: f32, height: f32, rect: Rect, radius: f32) -> Vec<u8> {
    let mut list = DisplayList::new(width, height, Rgba::WHITE);
    list.push(Item::Fill {
        shape: Shape {
            rect,
            radii: Radii::all(radius),
        },
        color: Rgba::hex(0x0969da),
    });
    rasterize(&Fonts::new(), &list, 2.0, &Limits::default())
        .unwrap_or_else(|e| panic!("{e}"))
        .rgba
}

#[test]
fn a_fill_past_the_canvas_edge_matches_the_same_fill_inside_a_larger_canvas() {
    let rect = |o: f32| Rect {
        x: o - 30.0,
        y: o - 20.0,
        w: 90.0,
        h: 70.0,
    };
    let clipped = filled(50.0, 40.0, rect(0.0), 24.0);
    let full = filled(150.0, 140.0, rect(50.0), 24.0);
    let crop: Vec<u8> = (0..80)
        .flat_map(|y| {
            let row = ((y + 100) * 300 + 100) * 4;
            full[row..row + 100 * 4].to_vec()
        })
        .collect();
    assert_eq!(clipped, crop);
}

#[test]
fn a_fill_far_larger_than_the_canvas_covers_it() {
    let huge = Rect {
        x: -1.0e7,
        y: -1.0e7,
        w: 2.0e7,
        h: 2.0e7,
    };
    let px = filled(20.0, 20.0, huge, 8.0);
    assert!(px
        .as_chunks::<4>()
        .0
        .iter()
        .all(|p| *p == [0x09, 0x69, 0xda, 0xff]));
}
