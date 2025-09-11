// Client-side only, of course!

import { Repo } from "@automerge/automerge-repo";
import { IndexedDBStorageAdapter } from "@automerge/automerge-repo-storage-indexeddb";

export const useRepo = (): Repo => {
  // Enforce client-only usage

  const nuxtApp = useNuxtApp();

  if (nuxtApp.ssrContext !== undefined) {
    throw new Error("cannot useRepo() on server side");
  }

  // OK to proceed

  const repo = new Repo({
    network: [],
    storage: new IndexedDBStorageAdapter(),
    //sharePolicy: async (peerId: PeerId, documentId: DocumentId) => true,
  })

  return repo;
}