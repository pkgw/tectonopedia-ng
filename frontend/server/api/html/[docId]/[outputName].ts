export default defineEventHandler(async (event) => {
    const config = useRuntimeConfig();
    const docId = getRouterParam(event, "docId");
    const outputName = getRouterParam(event, "outputName");
    const url = `${config.internalDataUrl}/html/${docId}/${outputName}`;
    return await $fetch(url, { parseResponse: (txt: string) => txt });
});

