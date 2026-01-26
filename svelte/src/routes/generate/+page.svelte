<script lang="ts">
  import {
    Button,
    Field,
    HelperText,
    Input,
    ProgressBar,
    Stack,
    Text,
} from "@immich/ui";

  let personId: string = "";
  let isGenerating: boolean = false;

  const onSubmit = async (event: Event) => {
    event.preventDefault();
    const form = event.target as HTMLFormElement;
    const formData = new FormData(form);
    const response = await fetch("http://localhost:5000/generate", {
      method: "POST",
      body: formData,
    });
    if (response.ok) {
      // TODO: continue
    } else {
      // TODO: handle error
    }
  };
</script>

<form onsubmit={onSubmit} method="post" class="flex flex-col gap-4">
  <Stack gap={4}>
    <Field label="Person ID">
        <Input
        bind:value={personId}
        />
        <HelperText>
            Enter the UUID of the person from Immich to generate a timelapse video.
        </HelperText>
    </Field>

    <Button
        size="giant"
        shape="round"
        fullWidth
    >
        {isGenerating ? "Cancel" : "Generate timelapse"}
    </Button>
  </Stack>
</form>

<Stack>
  <div>
    <Text fontWeight="bold" class="mb-1">No progress</Text>
    <ProgressBar progress={0} />
  </div>
  <div>
    <Text fontWeight="bold" class="mb-1">Barely any progress</Text>
    <ProgressBar progress={0.001} />
  </div>
</Stack>

