import type { Ref } from "vue";
import type { DocumentId } from "@automerge/automerge-repo";

interface EntryInfo {
  doc_id: DocumentId,
  output_name: string,
  title: string,
}

export const useEntryInfo = async (name: string): Promise<Ref<EntryInfo>> => {
  const key = `nexus-entry-${name}`;
  const url = `/api/entry/${name}`;
  const { data } = await useAsyncData(key, () => $fetch(url));
  return data as Ref<EntryInfo>;
}
