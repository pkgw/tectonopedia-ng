<template>
  <div>
    <h1>Welcome to the editor</h1>

    <ClientOnly>
      <code-mirror v-model="editorContent" />
    </ClientOnly>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from "vue";
import CodeMirror from "vue-codemirror6";
import type { DocumentId } from "@automerge/automerge-repo";

interface MinimalDoc {
  content: string;
}

const editorContent = ref("This is some text");

const WS_URL = "ws://127.0.0.1:20800/";
const DOC_ID = "43W6zUkkrKdtpuB4Adqo2DwZF1tA";

onMounted(async () => {
  const keypair = await useKeypair();
  const repo = useRepo(WS_URL);

  // NB: very important to stringify the content here!
  const handle = await repo.find<MinimalDoc>(DOC_ID as DocumentId);
  editorContent.value = `${handle.doc().content}`;
});

</script>