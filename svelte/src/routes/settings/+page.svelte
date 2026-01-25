<script lang="ts">
  import SettingAccordion from "$lib/components/setting-accordion.svelte";
  import { mdiAccountOutline } from "@mdi/js";
  import {
    type SelectOption,
    DatePicker,
    Field,
    Input,
    NumberInput,
    Select,
    Stack,
    Switch,
  } from "@immich/ui";
  import { DateTime } from "luxon";

  const frameworks: SelectOption[] = [
    { label: "Angular", value: "angular" },
    { label: "React", value: "react" },
    { label: "Svelte", value: "svelte" },
    { label: "Vue", value: "vue" },
  ];

  export let data;

  let textInput: string = data.textInput;
  let dateValue: DateTime = DateTime.fromISO(data.dateValue);
  let switchValue: boolean = data.switchValue;
  let selectValue: SelectOption =
    frameworks.find((f) => f.value === data.selectValue) || "";
  let numberValue: number = data.numberValue;
</script>

<SettingAccordion
  title="Accordion title"
  key="accordion1"
  icon={mdiAccountOutline}
>
  <Stack gap={4}>
    <Field label="Text input">
      <Input placeholder="placeholder" bind:value={textInput} />
    </Field>
    <Field label="Date picker">
      <DatePicker bind:value={dateValue} />
    </Field>
    <Field label="Switch">
      <Switch bind:checked={switchValue} />
    </Field>
    <Field label="Select">
      <Select bind:value={selectValue} data={frameworks} />
    </Field>
    <Field label="Number input">
      <NumberInput bind:value={numberValue} />
    </Field>
  </Stack>
</SettingAccordion>
