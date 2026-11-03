<script lang="ts">
  let {
    checked,
    label,
    disabled = false,
    onchange,
  }: { checked: boolean; label: string; disabled?: boolean; onchange: (value: boolean) => void } =
    $props();
</script>

<button
  class="switch"
  role="switch"
  aria-checked={checked}
  aria-label={label}
  {disabled}
  onclick={() => onchange(!checked)}
>
  <span class="knob"></span>
</button>

<style>
  .switch {
    position: relative;
    width: 40px;
    height: 22px;
    padding: 0;
    border: 0;
    border-radius: 11px;
    background: var(--csr-mid);
    cursor: pointer;
    transition: background 0.15s;
  }
  .switch[aria-checked="true"] {
    background: var(--accent);
  }
  .knob {
    position: absolute;
    top: 3px;
    left: 3px;
    width: 16px;
    height: 16px;
    border-radius: 50%;
    background: #fff;
    transition: transform 0.15s;
  }
  .switch[aria-checked="true"] .knob {
    transform: translateX(18px);
  }
  .switch:disabled {
    cursor: default;
    opacity: 0.5;
  }
</style>
