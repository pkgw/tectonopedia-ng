<template>
  <div>
    <h1>Welcome to the editor</h1>

    <ClientOnly>
      <code-mirror :basic="true" v-model="editorContent" :extensions="editorExtensions" />
    </ClientOnly>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from "vue";
import type { Ref } from "vue";
import type { Extension } from "@codemirror/state";
import { StreamLanguage, syntaxHighlighting, defaultHighlightStyle } from "@codemirror/language";
import { stex } from "@codemirror/legacy-modes/mode/stex";
import { oneDark } from "@codemirror/theme-one-dark";
import CodeMirror from "vue-codemirror6";
import { automergeSyncPlugin } from "@automerge/automerge-codemirror";
import type { DocumentId } from "@automerge/automerge-repo";

interface MinimalDoc {
  content: string
}

const editorContent = ref("");
const editorExtensions: Ref<Extension[]> = ref([]);

// Sample Rust source => 2BoTCivmRyvhiiKaMHMHB99Peerm
// index.tex => tsquNfquQC6eLNYP7ZmgmNkbwXP
// dump.tex => gxhZkppeZEXBb7LXnwvHWEuavAd
// end.tex => 25spacqQwZqMUBMkrCJB1ot1EmGq
// message.tex => 3huRDC2cWvQhEeFxezP58NWxnMk9
// why-tex.tex => 3XuSpKARAcsShsAFTZBJKBxwjRsz

const DOC_ID = "gxhZkppeZEXBb7LXnwvHWEuavAd" as DocumentId;

onMounted(async () => {
  const config = useRuntimeConfig();
  const keypair = await useKeypair();
  const repo = useRepo(config.public.repoWebsocketsBase);

  const handle = await repo.find<MinimalDoc>(DOC_ID);
  await handle.whenReady();
  editorContent.value = `${handle.doc().content}`;
  editorExtensions.value = [
    oneDark,
    StreamLanguage.define(stex),
    syntaxHighlighting(defaultHighlightStyle, { fallback: true }),
    automergeSyncPlugin({handle, path: ["content"]}),
  ];
});

</script>