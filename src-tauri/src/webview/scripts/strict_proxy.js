// Strict proxy: WebRTC media and data channels do not go through HTTP or
// SOCKS proxies, so the API is removed from every frame before page scripts
// run. On Linux WebKitGTK's WebRTC switch is also turned off natively.
(function () {
  const names = [
    "RTCPeerConnection",
    "webkitRTCPeerConnection",
    "RTCDataChannel",
    "RTCIceCandidate",
    "RTCSessionDescription",
    "RTCRtpSender",
    "RTCRtpReceiver",
    "RTCRtpTransceiver",
  ];
  for (const name of names) {
    try {
      Object.defineProperty(window, name, {
        value: undefined,
        writable: false,
        configurable: false,
      });
    } catch (_error) {
      // Already locked by an earlier injection.
    }
  }
})();
