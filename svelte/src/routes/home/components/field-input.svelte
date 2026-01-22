<script lang="ts">
  import { Field, HelperText, Input, Stack } from "@immich/ui";

  interface Props {
    label: string;
    description?: string;
    required?: boolean | "indicator";
    placeholder?: string;
    value?: string;
    helperText?: string;
  }

  let { label, description, required, placeholder, value, helperText }: Props =
    $props();
</script>

<Stack gap={4}>
  <Field {label} {description} {required}>
    <Input {placeholder} {value} />
    {#if helperText}
      <HelperText>{helperText}</HelperText>
    {/if}
  </Field>
</Stack>
