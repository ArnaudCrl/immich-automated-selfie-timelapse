<script lang="ts">
  import {
    Alert,
    Button,
    Field,
    Input,
    PasswordInput,
  } from "@immich/ui";

  export let data;
  
  let baseUrl: string = data.IMMICH_BASE_URL;
  let apiKey: string = data.IMMICH_API_KEY;
  let isLoggedIn: boolean = data.isLoggedIn;
  let error: boolean = false;
  let errorMessage: string = "";
  let errorField: string = "";

  const onSubmit = async (event: Event) => {
    event.preventDefault();
    const form = event.target as HTMLFormElement;
    const formData = new FormData(form);
    const response = await fetch("http://localhost:5000/login", {
      method: "POST",
      body: formData,
    });
    if (response.ok) {
      // TODO: continue
    } else {
      // read response to determine which field is invalid
      const result = await response.json();
      error = true;
      errorMessage = result.message;
      errorField = result.field;
    }
  };
</script>

<form onsubmit={onSubmit} method="post" class="flex flex-col gap-4">
  {#if error}
    <Alert color="danger" title={errorMessage} closable />
  {/if}
  <Field
    label="Immich base URL"
    required
    invalid={error && errorField == "baseUrl"}
  >
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

  <Button class="mt-4" type="submit" size="giant" shape="round" fullWidth>
    {isLoggedIn ? "Update" : "Login"}
  </Button>
</form>
