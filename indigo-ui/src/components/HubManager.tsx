import { createSignal, For, Show } from "solid-js";
import { Server, Plus, Trash2, Check, X } from "lucide-solid";
import { settingsStore, settingsActions } from "~/lib/settingsStore";

export default function HubManager(props: { onClose: () => void }) {
  const [newHubUrl, setNewHubUrl] = createSignal("");
  const [error, setError] = createSignal("");

  const handleAddHub = (e: Event) => {
    e.preventDefault();
    setError("");
    let url = newHubUrl().trim();
    if (!url) return;

    if (!url.startsWith("http")) {
        url = "http://" + url;
    }

    try {
        new URL(url);
        settingsActions.addHub(url);
        setNewHubUrl("");
    } catch (e) {
        setError("Invalid URL format");
    }
  };

  return (
    <div class="absolute right-0 mt-2 w-80 bg-ng-bg-surface border border-ng shadow-2xl p-4 z-110" style="clip-path: var(--ng-clip-card)">
      <div class="flex items-center justify-between mb-4">
        <h3 class="text-xs font-bold text-white font-data uppercase tracking-widest flex items-center gap-2">
            <Server size={14} class="text-cyan-ng" />
            Hub Connections
        </h3>
        <button onClick={props.onClose} class="text-ng-muted hover:text-cyan-ng">
            <X size={14} />
        </button>
      </div>

      <div class="space-y-2 mb-4 max-h-52 overflow-y-auto custom-scrollbar">
        <For each={settingsStore.hubs}>
            {(hub) => (
                <div class={`flex items-center justify-between p-2 border transition-all ${
                    settingsStore.activeHub === hub
                    ? "border-cyan-ng/30"
                    : "border-ng bg-ng-bg-deep hover:border-ng-border-hover"
                }`} style="clip-path: var(--ng-clip-card-sm)">
                    <button
                        onClick={() => settingsActions.setActiveHub(hub)}
                        class="flex-1 text-left flex items-center gap-2 truncate"
                    >
                        <div class={`w-1.5 h-1.5 ${settingsStore.activeHub === hub ? "bg-online" : "bg-ng-border"}`} style="clip-path: var(--ng-clip-badge)"></div>
                        <span class={`text-xs font-data truncate ${settingsStore.activeHub === hub ? "text-cyan-ng" : "text-ng-secondary"}`}>
                            {hub.replace(/^https?:\/\//, '')}
                        </span>
                    </button>

                    <Show when={settingsStore.hubs.length > 1}>
                        <button
                            onClick={() => settingsActions.removeHub(hub)}
                            class="text-ng-muted hover:text-offline p-1"
                        >
                            <Trash2 size={12} />
                        </button>
                    </Show>
                </div>
            )}
        </For>
      </div>

      <form onSubmit={handleAddHub} class="relative">
        <input
            type="text"
            placeholder="http://192.168.1.50:3001"
            class="input-field pr-8 text-xs font-data"
            value={newHubUrl()}
            onInput={(e) => setNewHubUrl(e.currentTarget.value)}
        />
        <button
            type="submit"
            class="absolute right-2 top-2 text-ng-muted hover:text-cyan-ng"
            disabled={!newHubUrl()}
        >
            <Plus size={14} />
        </button>
      </form>
      <Show when={error()}>
          <p class="text-[10px] text-offline mt-2 font-data">{error()}</p>
      </Show>
    </div>
  );
}
