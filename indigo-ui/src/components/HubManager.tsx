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
    
    // Basic validation
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
    <div class="absolute right-0 mt-2 w-80 bg-zinc-900 border border-zinc-800 rounded-xl shadow-2xl p-4 z-110">
      <div class="flex items-center justify-between mb-4">
        <h3 class="text-sm font-bold text-white flex items-center gap-2">
            <Server size={16} class="text-indigo-400" />
            Hub Connections
        </h3>
        <button onClick={props.onClose} class="text-zinc-500 hover:text-white">
            <X size={16} />
        </button>
      </div>

      <div class="space-y-2 mb-4 max-h-60 overflow-y-auto custom-scrollbar">
        <For each={settingsStore.hubs}>
            {(hub) => (
                <div class={`flex items-center justify-between p-2 rounded-lg border transition-all ${
                    settingsStore.activeHub === hub 
                    ? "bg-indigo-500/10 border-indigo-500/30" 
                    : "bg-zinc-950 border-zinc-800 hover:border-zinc-700"
                }`}>
                    <button 
                        onClick={() => settingsActions.setActiveHub(hub)}
                        class="flex-1 text-left flex items-center gap-2 truncate"
                    >
                        <div class={`w-2 h-2 rounded-full ${settingsStore.activeHub === hub ? "bg-green-400" : "bg-zinc-600"}`}></div>
                        <span class={`text-xs font-mono truncate ${settingsStore.activeHub === hub ? "text-white" : "text-zinc-400"}`}>
                            {hub.replace(/^https?:\/\//, '')}
                        </span>
                    </button>
                    
                    <Show when={settingsStore.hubs.length > 1}>
                        <button 
                            onClick={() => settingsActions.removeHub(hub)}
                            class="text-zinc-600 hover:text-red-400 p-1"
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
            class="w-full bg-zinc-950 border border-zinc-800 rounded-lg pl-3 pr-8 py-2 text-xs text-white focus:border-indigo-500 outline-none"
            value={newHubUrl()}
            onInput={(e) => setNewHubUrl(e.currentTarget.value)}
        />
        <button 
            type="submit"
            class="absolute right-2 top-2 text-zinc-500 hover:text-indigo-400"
            disabled={!newHubUrl()}
        >
            <Plus size={14} />
        </button>
      </form>
      <Show when={error()}>
          <p class="text-[10px] text-red-500 mt-2">{error()}</p>
      </Show>
    </div>
  );
}
