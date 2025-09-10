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
  const fedb = await openDB("ttpedia-frontend", 1, {
    "upgrade": (db, _oldVersion, _newVersion, _txn) => {
      db.createObjectStore("identity");
    }
  });

  const txn = fedb.transaction("identity", "readonly");
  const store = txn.objectStore("identity");
  let pubKey = await store.get("publicKeyEd25519");
  let privKey = await store.get("privateKeyEd25519");
  await txn.done;
  console.log("check keypair:", pubKey, privKey);

  // NB, can't do other async stuff inside idb transactions. So we have to close
  // the previous txn before we can decide if we need to generate the keypair
  // and try to save it.

  if (privKey === undefined) {
    // Need to create a new keypair.
    const keypair = await window.crypto.subtle.generateKey("Ed25519", false, ["sign"]);
    pubKey = keypair.publicKey;
    privKey = keypair.privateKey;
    const txn = fedb.transaction("identity", "readwrite");
    const store = txn.objectStore("identity");
    console.log("generated keypair:", pubKey, privKey);
    await store.put(pubKey, "publicKeyEd25519");
    await store.put(privKey, "privateKeyEd25519");
    await txn.done;
  }
});

</script>