<script lang="ts">
  import { Field, HelperText, NumberInput, Stack } from "@immich/ui";

  interface Props {
    label: string;
    description?: string;
    required?: boolean | "indicator";
    value?: number;
    helperText?: string;
  }

  let { label, description, required, value, helperText }: Props =
    $props();

</script>

<Stack gap={4}>
  <Field {label} {description} {required}>
    <NumberInput bind:value />
    {#if helperText}
      <HelperText>{helperText}</HelperText>
    {/if}
  </Field>
</Stack>
