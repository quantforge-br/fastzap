//! Tray icon artwork ported from ZapZap's `tray_icon.py`: the SVG templates are
//! rasterized at 128 px with an optional red unread counter.

use std::sync::{Arc, OnceLock};

use resvg::{tiny_skia, usvg};

use crate::settings::TrayTheme;

const DEFAULT_SVG: &str = include_str!("default.svg");
const SYMBOLIC_SVG: &str = include_str!("symbolic.svg");
const BADGE_SVG: &str = include_str!("badge.svg");
pub const SIZE: u32 = 128;

/// The counter shows at most three digits.
pub fn display_count(count: u32) -> u32 {
    count.min(999)
}

pub fn svg(theme: TrayTheme, count: u32) -> String {
    let count = display_count(count);
    let badge = if count == 0 {
        String::new()
    } else {
        let (width, x) = match count {
            0..=9 => ("100.1", "152.6"),
            10..=99 => ("180.3", "72.5"),
            _ => ("249.428", "3.286"),
        };
        BADGE_SVG
            .replace("WIDTH", width)
            .replace("XPOS", x)
            .replace("NUMBER", &count.to_string())
    };
    let base = match theme {
        TrayTheme::Default => DEFAULT_SVG.to_owned(),
        TrayTheme::SymbolicLight => SYMBOLIC_SVG.replace("COLOR", "#ffffff"),
        TrayTheme::SymbolicDark => SYMBOLIC_SVG.replace("COLOR", "#241f31"),
    };
    base.replace("<!--NOTIFY-->", &badge)
}

fn fonts() -> Arc<usvg::fontdb::Database> {
    static FONTS: OnceLock<Arc<usvg::fontdb::Database>> = OnceLock::new();
    FONTS
        .get_or_init(|| {
            let mut db = usvg::fontdb::Database::new();
            db.load_system_fonts();
            Arc::new(db)
        })
        .clone()
}

/// Straight (non-premultiplied) RGBA pixels of a `SIZE`×`SIZE` icon.
pub fn render(theme: TrayTheme, count: u32) -> anyhow::Result<Vec<u8>> {
    let options = usvg::Options {
        fontdb: fonts(),
        ..Default::default()
    };
    let tree = usvg::Tree::from_str(&svg(theme, count), &options)?;
    let mut pixmap = tiny_skia::Pixmap::new(SIZE, SIZE).ok_or_else(|| anyhow::anyhow!("pixmap"))?;
    let scale = SIZE as f32 / tree.size().width();
    resvg::render(
        &tree,
        tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    Ok(pixmap
        .pixels()
        .iter()
        .flat_map(|p| {
            let c = p.demultiply();
            [c.red(), c.green(), c.blue(), c.alpha()]
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn badge_width_follows_digit_count() {
        assert!(!svg(TrayTheme::Default, 0).contains("<rect"));
        assert!(svg(TrayTheme::Default, 7).contains(r#"width="100.1""#));
        assert!(svg(TrayTheme::Default, 42).contains(r#"width="180.3""#));
        let big = svg(TrayTheme::SymbolicDark, 5000);
        assert!(big.contains(">999<") && big.contains("#241f31"));
    }

    #[test]
    fn renders_all_themes() {
        for theme in [
            TrayTheme::Default,
            TrayTheme::SymbolicLight,
            TrayTheme::SymbolicDark,
        ] {
            let rgba = render(theme, 12).unwrap();
            assert_eq!(rgba.len(), (SIZE * SIZE * 4) as usize);
            assert!(rgba.chunks(4).any(|p| p[3] > 0));
        }
    }
}
