import { createApp } from "vue";

async function bootstrapEntry() {
  const search = typeof window !== "undefined" ? window.location.search : "";

  if (search.includes("window=desktop_lyric")) {
    await import("./styles/tokens-base.css");
    const { default: DesktopLyricWindow } = await import(
      "./components/DesktopLyricWindow.vue"
    );
    createApp(DesktopLyricWindow).mount("#app");
    return;
  }

  if (search.includes("window=mini_player")) {
    await import("./styles/tokens-base.css");
    const { default: MiniPlayerWindow } = await import(
      "./components/MiniPlayerWindow.vue"
    );
    createApp(MiniPlayerWindow, { standalone: true }).mount("#app");
    return;
  }

  await import("./styles/main.css");
  const [{ createPinia }, { default: App }, { vTilt }] = await Promise.all([
    import("pinia"),
    import("./App.vue"),
    import("./directives/vTilt"),
  ]);

  const app = createApp(App);
  app.directive("tilt", vTilt);
  app.use(createPinia());
  app.mount("#app");
}

void bootstrapEntry();
