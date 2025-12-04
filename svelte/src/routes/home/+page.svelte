<script lang="ts">
  import SettingAccordion from "$lib/components/setting-accordion.svelte";
  import {
    HStack,
    ThemeSwitcher,
    AppShell,
    AppShellHeader,
    Heading,
    Stack,
  } from "@immich/ui";
  import type { Component } from "svelte";

  const settings: Array<{
    component: Component;
    title: string;
    subtitle: string;
    key: string;
    icon: string;
  }> = [
    {
      component: null as unknown as Component,
      title: "Title",
      subtitle: "Subtitle",
      key: "key",
      icon: "icon",
    },
  ];
</script>

<Stack>
  <div class="border">
    <AppShell class="h-[400px]">
      <AppShellHeader>
        <nav class="flex items-center justify-between md:gap-2 p-2">
          <div class="flex gap-2 place-items-center">
            <a href="/" class="flex gap-2 text-4xl">
                <img src="/logo.gif" class="h-12" alt="Timelapse selfie logo" />
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
            {#each settings as { component: Component, title, subtitle, key, icon } (key)}
              <SettingAccordion {title} {subtitle} {key} {icon}>
                {#if Component}
                  <svelte:component this={Component} />
                {/if}
              </SettingAccordion>
            {/each}
          </section>
        </section>
      </div>
    </AppShell>
  </div>
</Stack>
