// FastZap: disable CSS animations and transitions (web/disable_animations).
(() => {
  const css =
    '*, *::before, *::after { animation-duration: 0s !important; animation-delay: 0s !important;' +
    ' transition-duration: 0s !important; transition-delay: 0s !important; }';
  const add = () => {
    const style = document.createElement('style');
    style.dataset.fastzapNoAnimations = '1';
    style.textContent = css;
    (document.head || document.documentElement).appendChild(style);
  };
  if (document.documentElement) add();
  else document.addEventListener('DOMContentLoaded', add, { once: true });
})();
