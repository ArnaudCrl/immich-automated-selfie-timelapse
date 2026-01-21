<!-- Inspired from https://github.com/immich-app/immich/blob/main/web/src/lib/components/layouts/AuthPageLayout.svelte -->

<script lang="ts">
  import {
	Alert,
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

  let baseUrl: string = "";
  let apiKey: string = "";
  let error: boolean = false;
  let errorMessage: string = "";
  let errorField: string = "";

  const onSubmit = async (event: Event) => {
	event.preventDefault();
	const form = event.target as HTMLFormElement;
	const formData = new FormData(form);
	const response = await fetch("/login", {
	  method: "POST",
	  body: formData,
	});
	if (response.ok) {
	  window.location.href = "/";
	} else {
	  // read response to determine which field is invalid
	  const result = await response.json();
	  error = true;
	  errorMessage = result.message;
	  errorField = result.field;
	}
  };
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
		  src="/_app/logo.gif"
		  class="h-24 aspect-square"
		/>
		<Heading size="large" class="font-semibold" color="primary" tag="h1">
		  Immich Selfie Timelapse Tool</Heading
		>
	  </VStack>
	</CardHeader>
	<CardBody class="p-8">
	  <form onsubmit={onSubmit} method="post" class="flex flex-col gap-4">
		{#if error}
		    <Alert color="danger" title={errorMessage} closable />
		{/if}
		<Field label="Immich base URL" required invalid={error && errorField == "baseUrl"}>
		  <Input
			bind:value={baseUrl}
			name="baseUrl"
			type="url"
			autocomplete="url"
			placeholder="http://192.168.1.94:2283/api"
		  />
		</Field>

		<Field label="API Key" required invalid={error && errorField == "apiKey"}>
			<PasswordInput
			  bind:value={apiKey}
			  name="apiKey"
			  autocomplete="new-password"
			  placeholder="abcdefghijklmnopqrstuvwxyz"
			/>
		</Field>

		<Button
		  class="mt-4"
		  type="submit"
		  size="giant"
		  shape="round"
		  fullWidth
		>
			Log in
		</Button>
	  </form>
	</CardBody>
  </Card>
</section>
