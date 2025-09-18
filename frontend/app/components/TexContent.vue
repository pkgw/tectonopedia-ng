<template>
  <div v-html="htmlContent" />
</template>

<script setup lang="ts">

interface Props {
  docId: string
  outputName: string
}

const { docId, outputName } = defineProps<Props>();

const config = useRuntimeConfig();
const key = `html/${docId}/${outputName}`;
const url = `${config.public.dataUrl}/html/${docId}/${outputName}`;
const { data: htmlContent, error: fetchError } = await useAsyncData<string>(
    key,
    () => $fetch(url, { parseResponse: (txt: string) => txt })
);
</script>