import { For, Index, Show } from "solid-js";
import { Server, Monitor, Cpu, Layers, Trash2 } from "lucide-solid";
import type { NodeInfo } from "~/lib/types";
import { settingsStore } from "~/lib/settingsStore";

interface NodeGridProps {
  nodes: NodeInfo[];
  loading: boolean;
}

const getModelTags = (model: string) => {
  const tags = [];
  const lower = model.toLowerCase();

  if (lower.includes("instruct") || lower.includes("chat"))
    tags.push({
      label: "Instruct",
      color: "bg-blue-500/20 text-blue-400 border-blue-500/30",
    });
  if (
    lower.includes("code") ||
    lower.includes("coder") ||
    lower.includes("deepseek")
  )
    tags.push({
      label: "Coding",
      color: "bg-emerald-500/20 text-emerald-400 border-emerald-500/30",
    });
  if (
    lower.includes("vision") ||
    lower.includes("llava") ||
    lower.includes("omni") ||
    lower.includes("moondream") ||
    lower.includes("clip")
  )
    tags.push({
      label: "Vision",
      color: "bg-purple-500/20 text-purple-400 border-purple-500/30",
    });
  if (lower.includes("math"))
    tags.push({
      label: "Math",
      color: "bg-amber-500/20 text-amber-400 border-amber-500/30",
    });
  if (lower.includes("q4") || lower.includes("q5") || lower.includes("q8"))
    tags.push({
      label: "Quantized",
      color: "bg-zinc-500/20 text-zinc-400 border-zinc-500/30",
    });

  if (tags.length === 0)
    tags.push({
      label: "General",
      color: "bg-zinc-500/20 text-zinc-400 border-zinc-500/30",
    });

  return tags;
};

export default function NodeGrid(props: NodeGridProps) {
  const deleteNode = async (id: string) => {
    if (!confirm("Are you sure you want to kill this node?")) return;
    try {
      await fetch(`${settingsStore.activeHub}/nodes/${id}`, {
        method: "DELETE",
      });
    } catch (e) {
      console.error("Failed to delete node:", e);
      alert("Failed to delete node.");
    }
  };

  return (
    <div class="space-y-4">
      <Show when={props.loading && props.nodes.length === 0}>
        <div class="flex items-center justify-center py-20 text-zinc-500 animate-pulse">
          Scanning network topography...
        </div>
      </Show>

      <Show when={!props.loading && props.nodes.length === 0}>
        <div class="flex flex-col items-center justify-center py-20 border-2 border-dashed border-zinc-800 rounded-xl bg-zinc-900/30">
          <div class="w-16 h-16 bg-zinc-900 rounded-full flex items-center justify-center mb-4 text-zinc-500">
            <Server size={32} />
          </div>
          <h3 class="text-xl font-medium text-white mb-2">Grid Offline</h3>
          <p class="text-zinc-400 max-w-sm text-center">
            No active nodes detected. Spawn a local node or connect a remote worker to begin.
          </p>
        </div>
      </Show>

      <div class="grid grid-cols-1 gap-4">
        <Index each={props.nodes}>
          {(node) => (
            <div class="group relative bg-zinc-900 border border-zinc-800 rounded-xl p-5 hover:border-indigo-500/30 transition-all overflow-hidden">
              {/* Status Bar */}
              <div
                class={`absolute left-0 top-0 bottom-0 w-1 ${
                  node().status === "Online"
                    ? "bg-green-500"
                    : node().status === "Starting"
                    ? "bg-amber-500"
                    : "bg-red-500"
                }`}
              />

              <div class="flex flex-col sm:flex-row justify-between items-start sm:items-center gap-4 pl-3">
                <div class="space-y-1">
                  <div class="flex items-center gap-3">
                    <h3 class="font-bold text-lg text-white font-mono tracking-tight">
                      {node().id}
                    </h3>

                    <div
                      class={`px-2 py-0.5 rounded text-[10px] uppercase font-bold tracking-wider border ${
                        node().status === "Online"
                          ? "bg-green-900/20 text-green-400 border-green-800"
                          : node().status === "Starting"
                          ? "bg-amber-900/20 text-amber-400 border-amber-800"
                          : "bg-red-900/20 text-red-400 border-red-800"
                      }`}
                    >
                      {node().status}
                    </div>

                    <Show when={node().is_local}>
                      <span class="px-2 py-0.5 rounded text-[10px] uppercase font-bold tracking-wider bg-zinc-800 text-zinc-400 border border-zinc-700">
                        Localhost
                      </span>
                    </Show>
                  </div>

                  <div class="flex flex-wrap items-center gap-x-6 gap-y-2 text-xs text-zinc-500 font-mono mt-2">
                    <div class="flex items-center gap-1.5">
                      <Monitor size={12} />
                      <span class="text-zinc-400">
                        {node().address}:{node().port}
                      </span>
                    </div>

                    <div class="flex items-center gap-1.5">
                      <Cpu size={12} />
                      <span class="text-indigo-400">
                        {node().model_name || "Unknown Model"}
                      </span>
                    </div>

                    {/* Capability Tags */}
                    <div class="flex flex-wrap gap-1.5">
                      <For each={getModelTags(node().model_name || "")}>
                        {(tag) => (
                          <span
                            class={`text-[9px] font-mono px-1.5 py-0.5 rounded border ${tag.color}`}
                          >
                            {tag.label}
                          </span>
                        )}
                      </For>
                    </div>
                  </div>
                </div>

                <div class="hidden sm:flex items-center gap-4">
                  <Layers
                    size={24}
                    class="text-zinc-800 group-hover:text-zinc-700 transition-colors"
                  />
                  <button
                    onClick={() => deleteNode(node().id)}
                    class="p-2 text-zinc-600 hover:text-red-400 hover:bg-red-400/10 rounded-lg transition-colors"
                    title="Kill Node"
                  >
                    <Trash2 size={18} />
                  </button>
                </div>
              </div>
            </div>
          )}
        </Index>
      </div>
    </div>
  );
}
