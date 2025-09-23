<template>
  <div>
    <h1>{{ info.title }}</h1>

    <p>Before.</p>

    <TexContent :doc-id="info.doc_id" :outputName="info.output_name"></TexContent>

    <h1>Welcome to an editor</h1>

    <UButton loading-auto @click="onSubmit">Submit</UButton>

    <ClientOnly>
      <code-mirror :basic="true" v-model="editorContent" :extensions="editorExtensions" />
    </ClientOnly>
  </div>
</template>

<script setup lang="ts">
const route = useRoute();
const info = await useEntryInfo(route.params.entryName as string);


import { ref, onMounted } from "vue";
import type { Ref } from "vue";
import type { Extension } from "@codemirror/state";
import { StreamLanguage, syntaxHighlighting, defaultHighlightStyle } from "@codemirror/language";
import { stex } from "@codemirror/legacy-modes/mode/stex";
import { oneDark } from "@codemirror/theme-one-dark";
import CodeMirror from "vue-codemirror6";
import { automergeSyncPlugin } from "@automerge/automerge-codemirror";

interface MinimalDoc {
  content: string
}

const editorContent = ref("");
const editorExtensions: Ref<Extension[]> = ref([]);

onMounted(async () => {
  const config = useRuntimeConfig();
  const keypair = await useKeypair();
  const repo = useRepo(config.public.repoWebsocketsUrl);

  const handle = await repo.find<MinimalDoc>(info.value.doc_id);
  await handle.whenReady();
  editorContent.value = `${handle.doc().content}`;
  editorExtensions.value = [
    oneDark,
    StreamLanguage.define(stex),
    syntaxHighlighting(defaultHighlightStyle, { fallback: true }),
    automergeSyncPlugin({handle, path: ["content"]}),
  ];
});

async function onSubmit() {
  await useRepoApi().submit(info.value.doc_id);
}
</script>