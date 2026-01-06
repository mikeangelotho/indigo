import { createStore } from "solid-js/store";
import { createEffect, createRoot } from "solid-js";

interface SettingsState {
  hubs: string[];
  activeHub: string;
}

const STORAGE_KEY = "indigo-settings";

function loadState(): SettingsState {
  // Dynamic default based on where the app is served from
  let defaultHub = "http://localhost:3001";
  let currentHostHub = "http://localhost:3001";
  
  if (typeof window !== "undefined") {
    const protocol = window.location.protocol;
    const hostname = window.location.hostname;
    currentHostHub = `${protocol}//${hostname}:3001`;
    defaultHub = currentHostHub;
  }

  if (typeof window === "undefined") return { hubs: [defaultHub], activeHub: defaultHub };
  
  const stored = localStorage.getItem(STORAGE_KEY);
  if (stored) {
    try {
      const parsed = JSON.parse(stored);
      // Ensure defaults if corrupted or empty
      if (!parsed.hubs || parsed.hubs.length === 0) {
          parsed.hubs = [defaultHub];
      }
      if (!parsed.activeHub) {
          parsed.activeHub = parsed.hubs[0];
      }

      // Auto-Migration: If accessing from a non-localhost IP (e.g. 192.168.x.x),
      // and the stored active hub is "localhost", automatically switch to the current IP.
      // This prevents "Blocked by Client" errors for users moving from localhost -> LAN.
      if (typeof window !== "undefined" && 
          window.location.hostname !== "localhost" && 
          window.location.hostname !== "127.0.0.1") {
          
          if (parsed.activeHub.includes("localhost") || parsed.activeHub.includes("127.0.0.1")) {
              console.log("Auto-migrating Hub URL to match current hostname:", currentHostHub);
              parsed.activeHub = currentHostHub;
              if (!parsed.hubs.includes(currentHostHub)) {
                  parsed.hubs.push(currentHostHub);
              }
          }
      }

      return parsed;
    } catch (e) {
      console.error("Failed to parse settings", e);
    }
  }
  return { hubs: [defaultHub], activeHub: defaultHub };
}

export const [settingsStore, setSettingsStore] = createStore<SettingsState>(loadState());

// Persistence Effect
createRoot(() => {
  createEffect(() => {
    if (typeof window !== "undefined") {
      localStorage.setItem(STORAGE_KEY, JSON.stringify(settingsStore));
    }
  });
});

export const settingsActions = {
  addHub: (url: string) => {
    // Normalize URL: remove trailing slash
    let cleanUrl = url.trim().replace(/\/$/, "");
    if (!cleanUrl.startsWith("http")) {
        cleanUrl = "http://" + cleanUrl;
    }
    
    if (!settingsStore.hubs.includes(cleanUrl)) {
      setSettingsStore("hubs", (prev) => [...prev, cleanUrl]);
    }
    // Auto-select if it's the first one (shouldn't happen due to default) or if requested? 
    // For now just add.
  },

  removeHub: (url: string) => {
    setSettingsStore("hubs", (prev) => prev.filter((h) => h !== url));
    // If active hub was removed, switch to the first available or default
    if (settingsStore.activeHub === url) {
        setSettingsStore("activeHub", settingsStore.hubs[0] || "http://localhost:3001");
    }
  },

  setActiveHub: (url: string) => {
    setSettingsStore("activeHub", url);
  }
};
