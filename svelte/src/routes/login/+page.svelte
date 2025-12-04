<!-- Inspired from https://github.com/immich-app/immich/blob/main/web/src/lib/components/layouts/AuthPageLayout.svelte -->

<script lang="ts">
  import {
	Button,
	Field,
	Input,
	PasswordInput,
	Card,
	CardBody,
	CardHeader,
	Heading,
	immichLogo,
	VStack,
  } from "@immich/ui";

  const onSubmit = async (event: Event) => {
	event.preventDefault();
	// Add form submission logic here
  };

  let baseUrl = "";
  let apiKey = "";

  $: valid = baseUrl.length > 0 && apiKey.length > 0;
</script>

<section class="min-w-dvw flex min-h-dvh items-center justify-center relative">
  <div class="absolute -z-10 w-full h-full flex place-items-center place-content-center">
	<img
	  src={immichLogo}
	  class="max-w-(--breakpoint-md) mx-auto h-full mb-2 antialiased overflow-hidden"
	  alt="Immich logo"
	/>
	<div
	  class="w-full h-[99%] absolute start-0 top-0 backdrop-blur-[200px] bg-transparent dark:bg-immich-dark-bg/20"
	></div>
  </div>

  <Card color="secondary" class="w-full max-w-lg border m-2">
	<CardHeader class="mt-6">
	  <VStack>
		<img
		  alt="Timelapse selfie logo"
		  src="/logo.gif"
		  class="h-24 aspect-square"
		/>
		<Heading size="large" class="font-semibold" color="primary" tag="h1">
		  Immich Selfie Timelapse Tool</Heading
		>
	  </VStack>
	</CardHeader>
	<CardBody class="p-8">
	  <form onsubmit={onSubmit} method="post" class="flex flex-col gap-4">
		<Field label="Immich base URL" required>
		  <Input
			bind:value={baseUrl}
			type="url"
			autocomplete="url"
			placeholder="http://192.168.1.10:2283"
		  />
		</Field>

		<Field label="API Key" required>
			<PasswordInput bind:value={apiKey} autocomplete="new-password" />
		</Field>

		<Button
		  class="mt-4"
		  type="submit"
		  size="giant"
		  shape="round"
		  fullWidth
		  disabled={!valid}>Log in</Button
		>
	  </form>
	</CardBody>
  </Card>
</section>
