import { createResource, createEffect, onCleanup, onMount, Show } from "solid-js";
import { createStore, reconcile } from "solid-js/store";
import { Server, RefreshCw, TriangleAlert } from "lucide-solid";
import { settingsStore } from "~/lib/settingsStore";
import type { NodeInfo, ModelInfo } from "~/lib/types";
import NodeGrid from "~/components/Nodes/NodeGrid";
import SpawnNode from "~/components/Nodes/SpawnNode";

const fetchNodes = async (hubUrl: string) => {
  const res = await fetch(`${hubUrl}/nodes`);
  return (await res.json()) as NodeInfo[];
};

const fetchModels = async (hubUrl: string) => {
  const res = await fetch(`${hubUrl}/models`);
  return (await res.json()) as ModelInfo[];
};

export default function NodesDashboard() {
  const [nodeStore, setNodeStore] = createStore<{ data: NodeInfo[] }>({
    data: [],
  });

  const [nodes, { refetch: refetchNodes }] = createResource(
    () => settingsStore.activeHub,
    fetchNodes
  );

  // Reconcile resource → store
  createEffect(() => {
    const data = nodes();
    if (data) {
      setNodeStore("data", reconcile(data));
    }
  });

  const [models] = createResource(() => settingsStore.activeHub, fetchModels);

  onMount(() => {
    const interval = setInterval(() => {
      if (!document.hidden) refetchNodes();
    }, 3000);

    onCleanup(() => clearInterval(interval));
  });

  const activeHostModels = () => {
    const localNodes = nodeStore.data.filter((n) => n.is_local && n.model_name);
    return [...new Set(localNodes.map((n) => n.model_name))];
  };

  return (
    <main class="max-w-7xl mx-auto p-6 space-y-8">
      <div class="flex flex-col md:flex-row justify-between items-start md:items-end gap-4 border-b border-zinc-800 pb-6">
        <div>
          <h1 class="text-3xl font-bold tracking-tight text-white flex items-center gap-3">
            <Server class="text-indigo-500" /> Compute Grid
          </h1>
          <p class="text-zinc-400 mt-2">
            Manage distributed inference workers and GPU sharding.
          </p>
        </div>

        <button
          onClick={() => refetchNodes()}
          class="flex items-center gap-2 px-4 py-2 bg-zinc-900 hover:bg-zinc-800 border border-zinc-700 rounded-lg text-sm font-medium transition-all group"
        >
          <RefreshCw
            size={16}
            class={`text-zinc-400 group-hover:text-white ${
              nodes.loading ? "animate-spin" : ""
            }`}
          />
          <span>{nodes.loading ? "Syncing..." : "Refresh Signal"}</span>
        </button>
      </div>

      <Show when={nodes.error}>
        <div class="bg-red-500/10 border border-red-500/20 text-red-400 p-4 rounded-lg flex items-center gap-3">
          <TriangleAlert size={20} />
          <p>
            Failed to connect to Hub at <span class="font-mono text-xs bg-red-500/20 px-1 py-0.5 rounded">{settingsStore.activeHub}</span>. Is the backend running?
          </p>
        </div>
      </Show>

      <div class="grid grid-cols-1 xl:grid-cols-3 gap-8 items-start">
        <div class="xl:col-span-1">
          <SpawnNode
            activeHostModels={activeHostModels}
            models={models}
            refetchNodes={refetchNodes}
          />
        </div>

        <div class="xl:col-span-2 space-y-4">
          <NodeGrid nodes={nodeStore.data} loading={nodes.loading} />
        </div>
      </div>
    </main>
  );
}
