// Client-side only, of course!

import { openDB } from "idb";

export const useKeypair = async (): Promise<CryptoKeyPair> => {
  // Enforce client-only usage

  const nuxtApp = useNuxtApp();

  if (nuxtApp.ssrContext !== undefined) {
    throw new Error("cannot useKeypair() on server side");
  }

  // Try to pull a saved keypair out of indexeddb. (Saving a non-exportable
  // private key to indexeddb allows us to persist it without the JavaScript
  // code ever being able to access the key material -- nice.)

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
    await store.put(pubKey, "publicKeyEd25519");
    await store.put(privKey, "privateKeyEd25519");
    await txn.done;
  }

  return { "publicKey": pubKey, "privateKey": privKey };
}