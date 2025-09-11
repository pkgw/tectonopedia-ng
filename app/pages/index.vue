<template>
  <div>
    <h1>Welcome to the editor</h1>

    <ClientOnly>
      <code-mirror v-model="value" />
    </ClientOnly>
  </div>
</template>

<script setup lang="ts">
import { openDB } from "idb";
import { ref, onMounted } from "vue";
import CodeMirror from "vue-codemirror6";

const value = ref("This is some text");

onMounted(async () => {
  const keypair = await useKeypair();
  const repo = useRepo();
  console.log("keys and repo!");
});

</script>