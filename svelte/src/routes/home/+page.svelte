<script lang="ts">
  import SettingAccordion from "./components/setting-accordion.svelte";
  import FieldInput from "./components/field-input.svelte";
  import FieldNumberInput from "./components/field-numberinput.svelte";
  import {
    HStack,
    ThemeSwitcher,
    AppShell,
    AppShellHeader,
    Heading,
    Stack,
  } from "@immich/ui";
  import { mdiAccountOutline } from "@mdi/js";
  import type { Component } from "svelte";
  import { fade } from "svelte/transition";
  

  const settings: Array<{
    fields: Array<{
      component: Component<any>;
      props: Object;
    }>;
    title: string;
    subtitle: string;
    key: string;
    icon: string;
  }> = [
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

<Stack>
  <div class="border">
    <AppShell>
      <AppShellHeader>
        <nav class="flex items-center justify-between md:gap-2 p-2">
          <div class="flex gap-2 place-items-center">
            <a href="/" class="flex gap-2 text-4xl">
              <img src="/_app/logo.gif" class="h-12" alt="Timelapse selfie logo" />
              <div class="p-4">
                <Heading size="tiny">Immich Selfie Timelapse Tool</Heading>
              </div>
            </a>
          </div>
          <HStack gap={1}>
            <ThemeSwitcher />
          </HStack>
        </nav>
      </AppShellHeader>

      <div class="p-4">
        <section id="setting-content" class="flex place-content-center sm:mx-4">
          <section class="w-full pb-28 sm:w-5/6 md:w-[896px]">
            {#each settings as { fields, title, subtitle, key, icon } (key)}
              <SettingAccordion {title} {subtitle} {key} {icon}>
                <div in:fade={{ duration: 500 }}>
                  <div class="ms-4 mt-4 flex flex-col gap-4">
                    {#if fields}
                      {#each fields as { component, props }}
                        <svelte:component this={component} {...props} />
                      {/each}
                    {/if}
                  </div>
                </div>
              </SettingAccordion>
            {/each}
          </section>
        </section>
      </div>
    </AppShell>
  </div>
</Stack>
