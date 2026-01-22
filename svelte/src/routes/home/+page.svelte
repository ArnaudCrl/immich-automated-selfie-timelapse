<script lang="ts">
  import { FieldInput, FieldNumberInput, SettingsLayout } from "$lib/components";
  import { mdiAccountOutline } from "@mdi/js";

  const settings = [
    {
      fields: [
        {
          component: FieldInput,
          props: {
            label: "Text field",
          },
        },
        {
          component: FieldNumberInput,
          props: {
            label: "Number field",
          },
        },
      ],
      title: "Accordion title",
      subtitle: "Accordion subtitle",
      key: "accordion-key",
      icon: mdiAccountOutline,
    },
  ];
</script>

<SettingsLayout settings={settings} />
