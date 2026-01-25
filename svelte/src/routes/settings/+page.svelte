<script lang="ts">
	import SettingAccordion from '$lib/components/setting-accordion.svelte';
	import { mdiAccountOutline } from '@mdi/js';
	import { DatePicker, Field, Input, NumberInput, Select, Stack, Switch } from '@immich/ui';
</script>

<SettingAccordion title="Accordion title" key="accordion1" icon={mdiAccountOutline}>
	<Stack gap={4}>
		<Field label="Text input">
			<Input placeholder="placeholder" />
		</Field>
		<Field label="Date picker">
			<DatePicker />
		</Field>
		<Field label="Switch">
			<Switch checked />
		</Field>
		<Field label="Select">
			<Select
				data={[
					{ label: 'Svelte', value: 'svelte' },
					{ label: 'React', value: 'react' },
					{ label: 'Angular', value: 'angular' }
				]}
			/>
		</Field>
		<Field label="Number input">
			<NumberInput />
		</Field>
	</Stack>
</SettingAccordion>
