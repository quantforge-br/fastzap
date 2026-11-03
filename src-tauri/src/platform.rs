//! Runtime environment facts.

/// Running inside a Flatpak sandbox.
pub fn is_flatpak() -> bool {
    std::path::Path::new("/.flatpak-info").exists()
}

/// Whether WebKitGTK's DMA-BUF renderer must be turned off before GTK starts.
///
/// Inside Flatpak on the NVIDIA driver it makes the Wayland compositor drop
/// the connection ("Error 71 (protocol error)") and the app exits. An
/// explicit `WEBKIT_DISABLE_DMABUF_RENDERER` from the user always wins.
pub fn disable_dmabuf_renderer(flatpak: bool, nvidia: bool, current: Option<&str>) -> bool {
    current.is_none() && flatpak && nvidia
}

/// The NVIDIA proprietary driver is loaded.
pub fn has_nvidia_driver() -> bool {
    std::path::Path::new("/proc/driver/nvidia").exists()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dmabuf_renderer_is_disabled_only_for_flatpak_on_nvidia() {
        assert!(disable_dmabuf_renderer(true, true, None));
        assert!(!disable_dmabuf_renderer(true, false, None));
        assert!(!disable_dmabuf_renderer(false, true, None));
        assert!(!disable_dmabuf_renderer(true, true, Some("0")));
    }
}
