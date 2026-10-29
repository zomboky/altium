/** @type {import("@sveltejs/vite-plugin-svelte").SvelteConfig} */
export default {
  compilerOptions: {
    // Views read their props once on purpose: App re-creates them with
    // {#key} when the project or the node changes.
    warningFilter: (warning) => warning.code !== 'state_referenced_locally',
  },
}
