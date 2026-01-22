<script lang="ts">
  import type { Snippet } from "svelte";
  import {
    HStack,
    ThemeSwitcher,
    AppShell,
    AppShellHeader,
    Heading,
    Stack,
    AppShellSidebar,
    NavbarItem,
  } from "@immich/ui";
  import { mdiHome } from "@mdi/js";

  interface Props {
    children?: Snippet;
    currentTab?: string;
  }

  let { children, currentTab }: Props = $props();
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

      <AppShellSidebar class="pt-2" open>
        <Stack class="pe-4">
          <NavbarItem icon={mdiHome} title="Item 1" href="/" active={currentTab === "tab1"} />
          <NavbarItem icon={mdiHome} title="Item 2" href="#" active={currentTab === "tab2"} />
        </Stack>
      </AppShellSidebar>

      <div class="p-4">
        <section id="setting-content" class="flex place-content-center sm:mx-4">
          <section class="w-full pb-28 sm:w-5/6 md:w-[896px]">
            {@render children?.()}
          </section>
        </section>
      </div>
    </AppShell>
  </div>
</Stack>
