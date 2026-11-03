<script lang="ts" generics="T extends string | number">
  import Row from "./Row.svelte";

  let {
    title,
    description = "",
    value,
    options,
    disabled = false,
    child = false,
    onchange,
  }: {
    title: string;
    description?: string;
    value: T;
    options: { value: T; label: string }[];
    disabled?: boolean;
    child?: boolean;
    onchange: (value: T) => void;
  } = $props();
</script>

<Row {title} {description} {disabled} {child}>
  {#snippet control()}
    <select
      aria-label={title}
      {disabled}
      onchange={(e) => {
        const index = e.currentTarget.selectedIndex;
        onchange(options[index].value);
      }}
    >
      {#each options as option (option.value)}
        <option value={option.value} selected={option.value === value}>{option.label}</option>
      {/each}
    </select>
  {/snippet}
</Row>
