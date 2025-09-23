export default defineEventHandler(async (event) => {
    const config = useRuntimeConfig();
    const entryName = getRouterParam(event, "entryName");
    const url = `${config.internalNexusUrl}/entry/${entryName}`;
    return await $fetch(url);
});
