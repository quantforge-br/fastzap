// Strict proxy: WebKit has no "disable non-proxied UDP" policy, so force every
// RTCPeerConnection to relay-only ICE. Direct host/srflx paths are never used.
(function () {
  const Original = window.RTCPeerConnection || window.webkitRTCPeerConnection;
  if (!Original || Original.__fastzapStrict) return;
  const Strict = function (config = {}, ...rest) {
    return new Original({ ...config, iceTransportPolicy: "relay" }, ...rest);
  };
  Strict.prototype = Original.prototype;
  Strict.__fastzapStrict = true;
  Object.setPrototypeOf(Strict, Original);
  window.RTCPeerConnection = Strict;
  if (window.webkitRTCPeerConnection) window.webkitRTCPeerConnection = Strict;
})();
