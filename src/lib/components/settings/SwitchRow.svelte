<script lang="ts">
  import Row from "./Row.svelte";
  import Switch from "./Switch.svelte";

  let {
    title,
    description = "",
    checked,
    disabled = false,
    child = false,
    onchange,
  }: {
    title: string;
    description?: string;
    checked: boolean;
    disabled?: boolean;
    child?: boolean;
    onchange: (value: boolean) => void;
  } = $props();
</script>

<Row {title} {description} {disabled} {child}>
  {#snippet control()}
    <Switch {checked} label={title} {disabled} {onchange} />
  {/snippet}
</Row>
