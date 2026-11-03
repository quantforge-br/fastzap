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
    rasterize(&svg(theme, count), SIZE)
}

pub const BADGE_SIZE: u32 = 32;

/// Taskbar overlay for Windows, which has no launcher badge: a red disc with
/// the unread count (`99+` above two digits).
pub fn badge_svg(count: u32) -> String {
    let (text, font_size) = match count {
        0..=9 => (count.to_string(), 20),
        10..=99 => (count.to_string(), 16),
        _ => ("99+".to_owned(), 12),
    };
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="32" height="32" viewBox="0 0 32 32"><circle cx="16" cy="16" r="16" fill="#e01b24"/><text x="16" y="16" dy="0.35em" text-anchor="middle" font-family="sans-serif" font-weight="bold" font-size="{font_size}" fill="#ffffff">{text}</text></svg>"##
    )
}

pub fn render_badge(count: u32) -> anyhow::Result<Vec<u8>> {
    rasterize(&badge_svg(count), BADGE_SIZE)
}

fn rasterize(svg: &str, size: u32) -> anyhow::Result<Vec<u8>> {
    let options = usvg::Options {
        fontdb: fonts(),
        ..Default::default()
    };
    let tree = usvg::Tree::from_str(svg, &options)?;
    let mut pixmap = tiny_skia::Pixmap::new(size, size).ok_or_else(|| anyhow::anyhow!("pixmap"))?;
    let scale = size as f32 / tree.size().width();
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
    fn renders_the_taskbar_badge() {
        assert!(badge_svg(150).contains(">99+<"));
        let rgba = render_badge(7).unwrap();
        assert_eq!(rgba.len(), (BADGE_SIZE * BADGE_SIZE * 4) as usize);
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
