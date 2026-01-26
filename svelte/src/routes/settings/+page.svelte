<script lang="ts">
  import SettingAccordion from "$lib/components/setting-accordion.svelte";
  import {
    mdiImageSearch,
    mdiFaceRecognition,
    mdiVideo,
    mdiFormatTextbox,
    mdiFilter,
  } from "@mdi/js";
  import {
    type SelectOption,
    Button,
    DatePicker,
    Field,
    HelperText,
    Input,
    NumberInput,
    Select,
    Stack,
    Switch,
  } from "@immich/ui";
  import { DateTime } from "luxon";

  const labelPositions: SelectOption[] = [
    { label: "Bottom Left", value: "bottom_left" },
    { label: "Bottom Right", value: "bottom_right" },
    { label: "Top Left", value: "top_left" },
    { label: "Top Right", value: "top_right" },
  ];

  export let data;

  let searchTakenAfter: DateTime = DateTime.fromISO(data.searchTakenAfter);
  let searchTakenBefore: DateTime = DateTime.fromISO(data.searchTakenBefore);
  let searchAlbumId: string = data.searchAlbumId;
  let searchIsFavorite: boolean = data.searchIsFavorite;

  let keep: boolean = data.keep;
  let keepHourly: number = data.keepHourly;
  let keepDaily: number = data.keepDaily;
  let keepWeekly: number = data.keepWeekly;
  let keepMonthly: number = data.keepMonthly;
  let keepYearly: number = data.keepYearly;

  let faceResizeSize: number = data.faceResizeSize;
  let faceResolutionThreshold: number = data.faceResolutionThreshold;
  let facePaddingRatio: number = data.facePaddingRatio;

  let label: boolean = data.label;
  let labelSize: number = data.labelSize;
  let labelPos: SelectOption =
    labelPositions.find((p) => p.value === data.labelPos) || labelPositions[0];
  let labelFormat: string = data.labelFormat;
  let labelPadding: number = data.labelPadding;
  let labelFill: string = data.labelFill;

  let videoCompile: boolean = data.videoCompile;
  let videoFramerate: number = data.videoFramerate;
  let videoWorkers: number = data.videoWorkers;

  // Computed disabled states
  $: labelDisabled = !label;
  $: keepDisabled = !keep;

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
      // TODO: handle error
    }
  };
</script>

<form onsubmit={onSubmit} method="post" class="flex flex-col gap-4">
  <SettingAccordion
    title="Search assets by metadata"
    key="search"
    icon={mdiImageSearch}
  >
    <Stack gap={4}>
      <Field label="Taken from">
        <DatePicker bind:value={searchTakenAfter} />
        <HelperText>
          If set, we will only select assets from this date onward. You can
          leave it blank to select from the earliest date.
        </HelperText>
      </Field>
      <Field label="Taken before">
        <DatePicker bind:value={searchTakenBefore} />
        <HelperText>
          If set, we will only select assets up to this date. You can leave it
          blank to select without an upper date limit.
        </HelperText>
      </Field>
      <Field label="Album ID">
        <Input bind:value={searchAlbumId} />
        <HelperText>
          If set, we will only select assets from this album UUID. You can leave
          it blank to select from all albums.
        </HelperText>
      </Field>
      <Field label="Favorites only">
        <Switch bind:checked={searchIsFavorite} />
        <HelperText>
          If checked, we will only select favorite assets.
        </HelperText>
      </Field>
    </Stack>
  </SettingAccordion>

  <SettingAccordion
    title="Keep a limited number of faces"
    key="keep"
    icon={mdiFilter}
  >
    <Stack gap={4}>
      <Field label="Limit number of faces kept">
        <Switch bind:checked={keep} />
        <HelperText>
          If enabled, we will limit the number of faces kept per time interval.
        </HelperText>
      </Field>
      <Field label="Keep Hourly" disabled={keepDisabled}>
        <NumberInput bind:value={keepHourly} disabled={keepDisabled} />
        <HelperText>Maximum number of faces to keep per hour</HelperText>
      </Field>
      <Field label="Keep Daily" disabled={keepDisabled}>
        <NumberInput bind:value={keepDaily} disabled={keepDisabled} />
        <HelperText>Maximum number of faces to keep per day</HelperText>
      </Field>
      <Field label="Keep Weekly" disabled={keepDisabled}>
        <NumberInput bind:value={keepWeekly} disabled={keepDisabled} />
        <HelperText>Maximum number of faces to keep per week</HelperText>
      </Field>
      <Field label="Keep Monthly" disabled={keepDisabled}>
        <NumberInput bind:value={keepMonthly} disabled={keepDisabled} />
        <HelperText>Maximum number of faces to keep per month</HelperText>
      </Field>
      <Field label="Keep Yearly" disabled={keepDisabled}>
        <NumberInput bind:value={keepYearly} disabled={keepDisabled} />
        <HelperText>Maximum number of faces to keep per year</HelperText>
      </Field>
    </Stack>
  </SettingAccordion>

  <SettingAccordion
    title="Retrieve faces for asset"
    key="face"
    icon={mdiFaceRecognition}
  >
    <Stack gap={4}>
      <Field label="Face resolution threshold" required="indicator">
        <NumberInput bind:value={faceResolutionThreshold} />
        <HelperText>
          Minimum face resolution in pixels to include the face. Faces smaller
          than this will be ignored.
        </HelperText>
      </Field>
      <Field label="Face padding ratio" required="indicator">
        <NumberInput bind:value={facePaddingRatio} step={0.1} />
        <HelperText>
          Padding ratio around the detected face (1.0 = no padding). If the face
          bounding box height is 100 pixels, a padding ratio of 1.2 will result
          in a 120x120 pixel crop.
        </HelperText>
      </Field>
      <Field label="Face resize size" required="indicator">
        <NumberInput bind:value={faceResizeSize} />
        <HelperText>
          We will resize all extracted face images to a square of this size (in
          pixels)
        </HelperText>
      </Field>
    </Stack>
  </SettingAccordion>

  <SettingAccordion title="Date overlay" key="label" icon={mdiFormatTextbox}>
    <Stack gap={4}>
      <Field label="Add date labels to processed face images">
        <Switch bind:checked={label} />
      </Field>
      <Field label="Font size" disabled={labelDisabled}>
        <NumberInput bind:value={labelSize} disabled={labelDisabled} />
      </Field>
      <Field label="Position" disabled={labelDisabled}>
        <Select
          bind:value={labelPos}
          data={labelPositions}
          disabled={labelDisabled}
        />
      </Field>
      <Field label="Date format" disabled={labelDisabled}>
        <Input bind:value={labelFormat} disabled={labelDisabled} />
        <HelperText>e.g., "%Y-%m-%d" for 2024-01-15</HelperText>
      </Field>
      <Field label="Padding" disabled={labelDisabled}>
        <NumberInput bind:value={labelPadding} disabled={labelDisabled} />
        <HelperText>Padding in pixels from the image edge</HelperText>
      </Field>
      <Field label="Fill Color" disabled={labelDisabled}>
        <Input bind:value={labelFill} disabled={labelDisabled} />
        <HelperText
          >Hex color code for the label text (e.g., #FFFFFF for white)</HelperText
        >
      </Field>
    </Stack>
  </SettingAccordion>

  <SettingAccordion title="Video output settings" key="video" icon={mdiVideo}>
    <Stack gap={4}>
      <Field label="Compile video">
        <Switch bind:checked={videoCompile} />
      </Field>
      <Field label="Framerate" required="indicator">
        <NumberInput bind:value={videoFramerate} />
        <HelperText>Frames per second for the output video</HelperText>
      </Field>
      <Field label="Workers" required="indicator">
        <NumberInput bind:value={videoWorkers} />
        <HelperText>Number of parallel workers for video encoding</HelperText>
      </Field>
    </Stack>
  </SettingAccordion>
  <Button class="mt-4" type="submit" size="giant" shape="round" fullWidth>
    Save
  </Button>
</form>
