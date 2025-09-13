class RepoApi {
    base_url: string;

    constructor() {
        const config = useRuntimeConfig();
        this.base_url = config.public.backendApiBase + "/repo";
    }

    async submit() {
        const resp = await $fetch(this.base_url + "/submit", {
            method: "POST",
            body: {}
        });
        console.log("submitted:", resp);
    }
}

export const useRepoApi = (): RepoApi => {
    return new RepoApi();
}