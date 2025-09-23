import type { Ref } from "vue";

interface EntryInfo {
  doc_id: string,
  output_name: string,
  title: string,
}

export const useEntryInfo = async (name: string): Promise<Ref<EntryInfo>> => {
  const key = `nexus-entry-${name}`;
  const url = `/api/entry/${name}`;
  const { data } = await useAsyncData(key, () => $fetch(url));
  return data as Ref<EntryInfo>;
}
