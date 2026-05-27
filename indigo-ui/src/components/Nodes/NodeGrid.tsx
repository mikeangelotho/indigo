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

  const statusColor = (status: string) => {
    switch (status) {
      case "Online": return "var(--ng-online)";
      case "Starting": return "var(--ng-warning)";
      default: return "var(--ng-offline)";
    }
  };

  const statusClass = (status: string) => {
    switch (status) {
      case "Online": return "text-online border-green-500/20";
      case "Starting": return "text-warning border-amber-500/20";
      default: return "text-offline border-red-500/20";
    }
  };

  return (
    <div class="space-y-4">
      <Show when={props.loading && props.nodes.length === 0}>
        <div class="flex items-center justify-center py-20 text-ng-muted font-data text-xs uppercase tracking-widest animate-pulse">
          Scanning network topography...
        </div>
      </Show>

      <Show when={!props.loading && props.nodes.length === 0}>
        <div class="flex flex-col items-center justify-center py-20 border border-ng-dashed rounded-xl bg-ng-bg-deep/30" style="clip-path: var(--ng-clip-card)">
          <div class="w-14 h-14 bg-ng-bg-surface border border-ng flex items-center justify-center mb-4 text-ng-muted" style="clip-path: var(--ng-clip-card-sm)">
            <Server size={28} />
          </div>
          <h3 class="text-lg font-bold text-white font-data tracking-wider mb-2">Grid Offline</h3>
          <p class="text-ng-secondary text-sm max-w-sm text-center">
            No active nodes detected. Spawn a local node or connect a remote worker to begin.
          </p>
        </div>
      </Show>

      <div class="grid grid-cols-1 gap-3">
        <Index each={props.nodes}>
          {(node) => (
            <div class="ng-card p-4 group">
              {/* Status Bar — left accent */}
              <div
                class="absolute left-0 top-0 bottom-0 w-0.5"
                style={`background: ${statusColor(node().status)}; box-shadow: 0 0 8px ${statusColor(node().status)}44`}
              />

              <div class="flex flex-col sm:flex-row justify-between items-start sm:items-center gap-3 pl-4">
                <div class="space-y-1">
                  <div class="flex items-center gap-3">
                    <h3 class="font-data font-bold text-base text-white tracking-wider">
                      {node().id}
                    </h3>

                    <span class={`px-2 py-0.5 text-[9px] uppercase font-bold tracking-wider border font-data ${statusClass(node().status)}`} style="clip-path: var(--ng-clip-badge)">
                      {node().status}
                    </span>

                    <Show when={node().is_local}>
                      <span class="px-2 py-0.5 text-[9px] uppercase font-bold tracking-wider bg-ng-bg-elevated text-ng-muted border border-ng font-data" style="clip-path: var(--ng-clip-badge)">
                        Localhost
                      </span>
                    </Show>
                  </div>

                  <div class="flex flex-wrap items-center gap-x-4 gap-y-1 text-xs text-ng-muted font-data mt-1">
                    <div class="flex items-center gap-1.5">
                      <Monitor size={11} />
                      <span class="text-ng-secondary">
                        {node().address}:{node().port}
                      </span>
                    </div>

                    <div class="flex items-center gap-1.5">
                      <Cpu size={11} />
                      <span class="text-cyan-ng">
                        {node().model_name || "Unknown Model"}
                      </span>
                    </div>

                    {/* Capability Tags */}
                    <div class="flex flex-wrap gap-1">
                      <For each={getModelTags(node().model_name || "")}>
                        {(tag) => (
                          <span
                            class={`text-[8px] font-data px-1.5 py-0.5 border ${tag.color}`}
                            style="clip-path: var(--ng-clip-badge)"
                          >
                            {tag.label}
                          </span>
                        )}
                      </For>
                    </div>
                  </div>
                </div>

                <div class="hidden sm:flex items-center gap-3">
                  <Layers
                    size={20}
                    class="text-ng-border group-hover:text-ng-border-hover transition-colors"
                  />
                  <button
                    onClick={() => deleteNode(node().id)}
                    class="p-2 text-ng-muted hover:text-offline hover:bg-[--ng-offline-glow] transition-colors"
                    style="clip-path: var(--ng-clip-card-sm)"
                    title="Kill Node"
                  >
                    <Trash2 size={16} />
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
