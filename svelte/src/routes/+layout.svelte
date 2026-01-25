<script lang="ts">
	import favicon from '$lib/assets/favicon.svg';
	import '$lib/app.css';
	import { page } from '$app/stores';
	import {
		HStack,
		ThemeSwitcher,
		AppShell,
		AppShellHeader,
		Heading,
		Stack,
		AppShellSidebar,
		NavbarItem,
	} from '@immich/ui';
	import { mdiHome } from '@mdi/js';

	let { children } = $props();

	// Determine current route for active tab highlighting
	let currentPath = $derived($page.url.pathname);
</script>

<svelte:head>
	<link rel="icon" href={favicon} />
</svelte:head>

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
					<NavbarItem icon={mdiHome} title="Login" href="/login" active={currentPath === '/login'} />
					<NavbarItem icon={mdiHome} title="Settings" href="/settings" active={currentPath === '/settings'} />
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
