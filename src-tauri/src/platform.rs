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

/// PulseAudio client property that keeps WirePlumber from restoring a saved
/// stream volume.
const NO_RESTORE_PROPS: &str = "state.restore-props=false";

/// `PULSE_PROP` for the web processes, or `None` to leave it alone.
///
/// WebKitGTK plays media through `pulsesink`, which turns the HTML volume
/// and mute into the stream's own volume. WirePlumber stores that volume per
/// `application.name`, which every FastZap stream shares, and restores it on
/// every new stream: once a page plays something at volume 0 (or the stream is
/// lowered in a mixer), every later audio of every account plays silent while
/// its progress bar still moves. The page drives the volume, so nothing must
/// be restored. A `state.restore-props` already set by the user always wins.
pub fn pulse_props(current: Option<&str>) -> Option<String> {
    match current.map(str::trim).filter(|p| !p.is_empty()) {
        None => Some(NO_RESTORE_PROPS.to_owned()),
        Some(p) if p.contains("state.restore-props") => None,
        Some(p) => Some(format!("{p} {NO_RESTORE_PROPS}")),
    }
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

    #[test]
    fn pulse_props_turn_off_stream_volume_restore() {
        assert_eq!(
            pulse_props(None).as_deref(),
            Some("state.restore-props=false")
        );
        assert_eq!(
            pulse_props(Some(" ")).as_deref(),
            Some("state.restore-props=false")
        );
        assert_eq!(
            pulse_props(Some("media.role=phone")).as_deref(),
            Some("media.role=phone state.restore-props=false")
        );
        assert_eq!(pulse_props(Some("state.restore-props=true")), None);
    }
}
