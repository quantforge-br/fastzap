// FastZap bridge injected into web.whatsapp.com before any page script.
// It may only call the `wa_*` commands allowed by capabilities/whatsapp-remote.json.
(() => {
  if (window.__FASTZAP__) return;
  if (location.hostname !== "web.whatsapp.com") return;

  const internals = () => window.__TAURI_INTERNALS__;
  const invoke = (cmd, args = {}) => {
    const tauri = internals();
    if (!tauri || typeof tauri.invoke !== "function") return Promise.resolve(null);
    return tauri.invoke(cmd, args).catch((error) => {
      console.warn("[FastZap]", cmd, error);
      return null;
    });
  };

  // ---- Unread count from the page title (same rule as ZapZap). ----
  let lastTitle = null;
  const reportTitle = () => {
    const title = document.title || "";
    if (title === lastTitle) return;
    lastTitle = title;
    invoke("wa_title_changed", { title });
  };
  const watchTitle = () => {
    reportTitle();
    const target = document.querySelector("head") || document.documentElement;
    new MutationObserver(reportTitle).observe(target, {
      subtree: true,
      childList: true,
      characterData: true,
    });
  };

  // ---- Notifications are routed to the native backend. ----
  const notifications = new Map();
  let nextNotificationId = 1;

  const toDataUrl = async (url) => {
    if (!url) return null;
    if (url.startsWith("data:")) return url;
    try {
      const response = await fetch(url);
      const blob = await response.blob();
      if (blob.size > 2 * 1024 * 1024) return null;
      return await new Promise((resolve) => {
        const reader = new FileReader();
        reader.onload = () => resolve(reader.result);
        reader.onerror = () => resolve(null);
        reader.readAsDataURL(blob);
      });
    } catch (_error) {
      return null;
    }
  };

  class FastZapNotification extends EventTarget {
    constructor(title, options = {}) {
      super();
      this.title = String(title ?? "");
      this.body = String(options.body ?? "");
      this.tag = String(options.tag ?? "");
      this.icon = options.icon ?? "";
      this.data = options.data;
      this.silent = Boolean(options.silent);
      this.onclick = null;
      this.onclose = null;
      this.onshow = null;
      this.onerror = null;
      this._id = nextNotificationId++;
      notifications.set(this._id, this);
      toDataUrl(this.icon).then((icon) =>
        invoke("wa_notify", {
          notification: {
            id: this._id,
            title: this.title,
            body: this.body,
            tag: this.tag,
            icon,
            silent: this.silent,
          },
        }),
      );
      queueMicrotask(() => this._fire("show"));
    }

    static get permission() {
      return "granted";
    }

    static requestPermission(callback) {
      if (typeof callback === "function") callback("granted");
      return Promise.resolve("granted");
    }

    static get maxActions() {
      return 0;
    }

    close() {
      if (!notifications.delete(this._id)) return;
      invoke("wa_notification_close", { id: this._id });
      this._fire("close");
    }

    _fire(type) {
      const event = new Event(type);
      const handler = this["on" + type];
      if (typeof handler === "function") handler.call(this, event);
      this.dispatchEvent(event);
    }
  }

  try {
    Object.defineProperty(window, "Notification", {
      configurable: true,
      writable: true,
      value: FastZapNotification,
    });
  } catch (_error) {
    window.Notification = FastZapNotification;
  }

  // ---- App shortcuts must work while the remote page has focus. ----
  const shortcutFor = (event) => {
    const ctrl = event.ctrlKey || event.metaKey;
    const key = event.key;
    if (event.key === "F5") return "reload";
    if (event.key === "F11") return "fullscreen";
    if (!ctrl) return null;
    if (event.altKey) {
      if (key === "1") return "theme_auto";
      if (key === "2") return "theme_light";
      if (key === "3") return "theme_dark";
      return null;
    }
    if (event.shiftKey) {
      if (key === "V" || key === "v") return "paste_plain_text";
      if (key === "?") return "show_shortcuts";
      return null;
    }
    switch (key.toLowerCase()) {
      case "q": return "quit";
      case "w": return "hide";
      case "p": return "settings";
      case "0": return "zoom_reset";
      case "+": case "=": return "zoom_in";
      case "-": return "zoom_out";
      case "g": return "toggle_grid";
      case "j": return "downloads";
      case "u": return "add_account";
      case "m": return "open_chat_by_number";
      case "n": return "new_chat";
      case "/": case "?": return "show_shortcuts";
      default:
        if (/^[1-9]$/.test(key)) return "account_" + key;
        return null;
    }
  };

  window.addEventListener(
    "keydown",
    (event) => {
      if (!event.isTrusted) return;
      const action = shortcutFor(event);
      if (!action) return;
      event.preventDefault();
      event.stopImmediatePropagation();
      invoke("wa_shortcut", { action });
    },
    true,
  );

  // ---- Plain-text paste (ported from ZapZap's WebView._plain_text_paste_script). ----
  const editableAncestor = (node) => {
    let element = node;
    if (element && element.nodeType !== Node.ELEMENT_NODE) element = element.parentElement;
    while (element && element !== document.documentElement) {
      if (element.isContentEditable) return element;
      element = element.parentElement;
    }
    return null;
  };

  const pastePlainText = (text) => {
    const active = document.activeElement;
    const selection = window.getSelection();
    const tag = active ? active.tagName : "";
    const inputType = String((active && active.type) || "text").toLowerCase();
    const textInput =
      active &&
      (tag === "TEXTAREA" ||
        (tag === "INPUT" &&
          !["button", "checkbox", "file", "hidden", "image", "radio", "reset", "submit"].includes(
            inputType,
          )));

    if (textInput) {
      try {
        active.focus({ preventScroll: true });
      } catch (_error) {
        active.focus();
      }
      const start = active.selectionStart ?? active.value.length;
      const end = active.selectionEnd ?? start;
      active.setRangeText(text, start, end, "end");
      active.dispatchEvent(new InputEvent("input", { bubbles: true, inputType: "insertText", data: text }));
      return true;
    }

    const editor =
      (active && active.isContentEditable ? active : null) ||
      editableAncestor(selection && selection.anchorNode) ||
      editableAncestor(selection && selection.focusNode);
    if (!editor) return false;

    let range = null;
    if (selection && selection.rangeCount > 0) {
      const candidate = selection.getRangeAt(0).cloneRange();
      const container = candidate.commonAncestorContainer;
      const containerElement =
        container.nodeType === Node.ELEMENT_NODE ? container : container.parentElement;
      if (containerElement && (containerElement === editor || editor.contains(containerElement))) {
        range = candidate;
      }
    }

    try {
      editor.focus({ preventScroll: true });
    } catch (_error) {
      editor.focus();
    }
    if (!selection) return false;
    selection.removeAllRanges();
    if (!range) {
      range = document.createRange();
      range.selectNodeContents(editor);
      range.collapse(false);
    }
    selection.addRange(range);

    // WhatsApp's composer is a Lexical editor: let it handle a synthetic paste first.
    try {
      if (editor.hasAttribute("data-lexical-editor")) {
        const pasteData = new DataTransfer();
        pasteData.setData("text/plain", text);
        const pasteEvent = new ClipboardEvent("paste", {
          clipboardData: pasteData,
          bubbles: true,
          cancelable: true,
        });
        editor.dispatchEvent(pasteEvent);
        if (pasteEvent.defaultPrevented) return true;
      }
    } catch (_error) {
      // Fall back to DOM insertion.
    }

    try {
      if (text.includes("\n")) {
        const lines = text.split("\n");
        let inserted = false;
        for (let index = 0; index < lines.length; index++) {
          if (index > 0) {
            if (!document.execCommand("insertLineBreak", false, null)) return inserted;
            inserted = true;
          }
          if (lines[index]) {
            if (!document.execCommand("insertText", false, lines[index])) return inserted;
            inserted = true;
          }
        }
        return true;
      }
      if (document.execCommand("insertText", false, text)) return true;
    } catch (_error) {
      // Fall back to direct range insertion below.
    }

    range = selection.getRangeAt(0);
    range.deleteContents();
    const node = document.createTextNode(text);
    range.insertNode(node);
    range.setStartAfter(node);
    range.collapse(true);
    selection.removeAllRanges();
    selection.addRange(range);
    editor.dispatchEvent(new InputEvent("input", { bubbles: true, inputType: "insertText", data: text }));
    return true;
  };

  // ---- Small actions requested by the native side. ----
  const dispatchKey = (init) => {
    document.dispatchEvent(new KeyboardEvent("keydown", { bubbles: true, cancelable: true, ...init }));
  };

  const openDeepLink = (href) => {
    const a = document.createElement("a");
    a.href = href;
    document.body.appendChild(a);
    a.click();
    a.remove();
  };

  const showToast = (message, duration = 1000) => {
    const toast = document.createElement("div");
    Object.assign(toast.style, {
      position: "fixed",
      bottom: "20px",
      left: "50%",
      transform: "translateX(-50%)",
      padding: "10px 20px",
      backgroundColor: "#333",
      color: "#fff",
      borderRadius: "5px",
      fontSize: "14px",
      zIndex: "9999",
      boxShadow: "0 2px 10px rgba(0, 0, 0, 0.1)",
    });
    toast.innerText = message;
    document.body.appendChild(toast);
    setTimeout(() => toast.remove(), duration);
  };

  const readProfilePhoto = () => {
    const header = document.querySelector('header[data-testid="chatlist-header"]');
    if (!header) return null;
    const image = Array.from(header.querySelectorAll("img"))
      .reverse()
      .find((node) => {
        const src = node.currentSrc || node.src || "";
        return src && src.startsWith("http") && node.naturalWidth > 16;
      });
    return image ? image.currentSrc || image.src : null;
  };

  // ---- QWebChannel shim so the ported theme controller keeps its contract. ----
  const zapZapBridge = {
    on_theme_controller_injection_success: () => invoke("wa_theme_event", { event: "ready" }),
    on_waweb_theme_changed: () => invoke("wa_theme_event", { event: "changed" }),
    on_theme_controller_failure: (message) =>
      invoke("wa_theme_event", { event: "failed", message: String(message || "") }),
    on_quick_accounts_controller_ready: () => invoke("wa_theme_event", { event: "selector_ready" }),
    open_recent_accounts: (x, y, width, height) =>
      invoke("wa_open_account_selector", { rect: [x, y, width, height] }),
  };
  window.qt = { webChannelTransport: {} };
  window.QWebChannel = function QWebChannel(_transport, callback) {
    callback({ objects: { zapZapBridge } });
  };

  window.__FASTZAP__ = {
    invoke,
    pastePlainText,
    openDeepLink,
    showToast,
    readProfilePhoto,
    closeConversation: () => dispatchKey({ key: "Escape" }),
    newChat: () => dispatchKey({ key: "n", code: "KeyN", ctrlKey: true, altKey: true }),
    lockApp: () => dispatchKey({ key: "l", code: "KeyL", ctrlKey: true, altKey: true }),
    notificationClicked(id) {
      const notification = notifications.get(id);
      if (!notification) return;
      window.focus();
      notification._fire("click");
    },
    notificationClosed(id) {
      const notification = notifications.get(id);
      if (!notification) return;
      notifications.delete(id);
      notification._fire("close");
    },
  };

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", watchTitle, { once: true });
  } else {
    watchTitle();
  }
})();
