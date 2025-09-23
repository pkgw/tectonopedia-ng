<template>
  <div>
    <h1>{{ info.title }}</h1>

    <p>Before.</p>

    <TexContent :doc-id="info.doc_id" :outputName="info.output_name"></TexContent>

    <UButton loading-auto @click="onSubmit">Submit</UButton>
  </div>
</template>

<script setup lang="ts">
const route = useRoute();
const info = await useEntryInfo(route.params.entryName as string);

async function onSubmit() {
  await useRepoApi().submit(info.value.doc_id);
}
</script>