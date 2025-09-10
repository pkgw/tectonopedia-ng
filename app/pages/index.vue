<template>
  <div>
    <h1>Welcome to the editor</h1>

    <ClientOnly>
      <code-mirror v-model="value" />
    </ClientOnly>
  </div>
</template>

<script setup lang="ts">
import { ref } from "vue";
import CodeMirror from "vue-codemirror6";

const value = ref("This is some text");
</script>