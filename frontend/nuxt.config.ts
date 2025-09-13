// https://nuxt.com/docs/api/configuration/nuxt-config

import wasm from "vite-plugin-wasm";
import topLevelAwait from "vite-plugin-top-level-await";

export default defineNuxtConfig({
  compatibilityDate: "2025-07-15",
  devtools: { enabled: true },
  modules: ["@nuxt/eslint", "@nuxt/ui"],
  css: ['~/assets/css/main.css'],
  vite: {
    plugins: [
      wasm(),
      topLevelAwait()
    ]
  },

  runtimeConfig: {
    public: {
      backendApiBase: "http://localhost:29100/ttpapi1",
      repoWebsocketsUrl: "ws://localhost:29100/ttpapi1/repo/sync",
    }
  },
})