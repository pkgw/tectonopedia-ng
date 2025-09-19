<template>
  <div class="tex" v-html="htmlContent" />
</template>

<style>
.tex {
  font-family: "tduxMain";
  text-size-adjust: none;
}
</style>

<script setup lang="ts">

interface Props {
  docId: string
  outputName: string
}

const { docId, outputName } = defineProps<Props>();
const config = useRuntimeConfig();

// Make sure that we get the fonts CSS.
useHead({
  link: [{ rel: "stylesheet", href: `${config.public.backendApiBase}/nexus/asset/tdux-fonts.css` }]
});

const key = `html/${docId}/${outputName}`;
const url = `/api/html/${docId}/${outputName}`;
const { data: htmlContent, error: fetchError } = await useAsyncData<string>(
    key,
    () => $fetch(url, { parseResponse: (txt: string) => txt })
);
</script>