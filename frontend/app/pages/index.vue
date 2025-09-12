<template>
  <div>
    <h1>Welcome to the editor</h1>

    <ClientOnly>
      <code-mirror v-model="editorContent" :extensions="editorExtensions"/>
    </ClientOnly>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from "vue";
import type { Ref } from "vue";
import { basicSetup } from "codemirror";
import type { Extension } from "@codemirror/state";
import CodeMirror from "vue-codemirror6";
import { automergeSyncPlugin } from "@automerge/automerge-codemirror";
import type { DocumentId } from "@automerge/automerge-repo";

interface MinimalDoc {
  content: string
}

const editorContent = ref("This is some text");
const editorExtensions: Ref<Extension[]> = ref([]);

const WS_URL = "ws://127.0.0.1:20800/";
const DOC_ID = "2BoTCivmRyvhiiKaMHMHB99Peerm" as DocumentId;

onMounted(async () => {
  const keypair = await useKeypair();
  const repo = useRepo(WS_URL);

  const handle = await repo.find<MinimalDoc>(DOC_ID);
  await handle.whenReady();
  editorContent.value = `${handle.doc().content}`;
  editorExtensions.value = [basicSetup, automergeSyncPlugin({handle, path: ["content"]})];
});

</script>