mod common;

use common::{decode_webp, pixmap};
use sosie_core::{encode, encode_png, EncodeError, Format, Pixmap};

fn gray(width: u32, height: u32) -> Pixmap {
    pixmap(width, height, |x, y| {
        [(x % 251) as u8, (y % 241) as u8, 90, 255]
    })
}

#[test]
fn webp_accepts_16383_px_per_side() {
    for p in [gray(16_383, 1), gray(1, 16_383)] {
        let bytes = encode(&p, Format::Webp).expect("16,383 px side");
        assert_eq!(decode_webp(&bytes), (p.width, p.height, p.rgba));
    }
}

#[test]
fn webp_rejects_16384_px_per_side() {
    for p in [gray(16_384, 1), gray(1, 16_384)] {
        assert_eq!(
            encode(&p, Format::Webp),
            Err(EncodeError::TooLarge {
                width: p.width,
                height: p.height,
                max: 16_383
            })
        );
    }
}

#[test]
fn png_has_no_webp_side_limit() {
    let p = gray(16_384, 1);
    assert_eq!(encode(&p, Format::Png), Ok(encode_png(&p)));
}

#[test]
fn an_empty_pixmap_is_rejected() {
    for format in [Format::Webp, Format::Png] {
        for (width, height) in [(0, 4), (4, 0)] {
            let p = Pixmap {
                width,
                height,
                rgba: Vec::new(),
            };
            assert_eq!(encode(&p, format), Err(EncodeError::Empty));
        }
    }
}

#[test]
fn a_buffer_that_does_not_match_the_size_is_rejected() {
    let short = Pixmap {
        rgba: vec![0; 15],
        ..gray(2, 2)
    };
    let huge = Pixmap {
        width: u32::MAX,
        height: u32::MAX,
        rgba: vec![0; 16],
    };
    for format in [Format::Webp, Format::Png] {
        assert_eq!(
            encode(&short, format),
            Err(EncodeError::Buffer {
                expected: Some(16),
                actual: 15
            })
        );
        assert_eq!(
            encode(&huge, format),
            Err(EncodeError::Buffer {
                expected: None,
                actual: 16
            })
        );
    }
}
