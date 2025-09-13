class RepoApi {
    base_url: string;

    constructor() {
        const config = useRuntimeConfig();
        this.base_url = config.public.backendApiBase + "/repo";
    }

    async submit(doc_id: string) {
        const req: RepoSubmitRequest = {
            doc_id
        };

        const resp: RepoSubmitResponse = await $fetch(this.base_url + "/submit", {
            method: "POST",
            body: req,
        });
        console.log("submitted:", resp);
    }
}

export const useRepoApi = (): RepoApi => {
    return new RepoApi();
}