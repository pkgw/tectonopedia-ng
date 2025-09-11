// Client-side only, of course!

import { Repo } from "@automerge/automerge-repo";
import { IndexedDBStorageAdapter } from "@automerge/automerge-repo-storage-indexeddb";
import { BrowserWebSocketClientAdapter } from "@automerge/automerge-repo-network-websocket";

export const useRepo = (ws_url: string): Repo => {
  // Enforce client-only usage

  const nuxtApp = useNuxtApp();

  if (nuxtApp.ssrContext !== undefined) {
    throw new Error("cannot useRepo() on server side");
  }

  // OK to proceed

  const repo = new Repo({
    network: [new BrowserWebSocketClientAdapter(ws_url)],
    storage: new IndexedDBStorageAdapter(),
    //sharePolicy: async (peerId: PeerId, documentId: DocumentId) => true,
  })

  return repo;
}