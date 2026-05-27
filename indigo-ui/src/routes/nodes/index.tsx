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
    <main class="max-w-7xl mx-auto p-6 space-y-6">
      <div class="flex flex-col md:flex-row justify-between items-start md:items-end gap-4 border-b border-ng pb-4">
        <div>
          <h1 class="text-2xl font-bold tracking-tight text-white font-data flex items-center gap-3">
            <Server class="text-cyan-ng" size={22} /> Compute Grid
          </h1>
          <p class="text-ng-secondary text-sm mt-1">
            Manage distributed inference workers and GPU sharding.
          </p>
        </div>

        <button
          onClick={() => refetchNodes()}
          class="btn-secondary flex items-center gap-2 group"
        >
          <RefreshCw
            size={14}
            class={`text-ng-secondary group-hover:text-cyan-ng ${
              nodes.loading ? "animate-spin" : ""
            }`}
          />
          <span class="text-xs font-bold uppercase tracking-wider font-data">{nodes.loading ? "Syncing" : "Refresh"}</span>
        </button>
      </div>

      <Show when={nodes.error}>
        <div class="ng-card border-offline/20 p-4 flex items-center gap-3">
          <TriangleAlert size={18} class="text-offline" />
          <p class="text-sm text-offline">
            Failed to connect to Hub at <span class="font-data text-xs px-1.5 py-0.5 bg-offline/10" style="clip-path: var(--ng-clip-badge)">{settingsStore.activeHub}</span>. Is the backend running?
          </p>
        </div>
      </Show>

      <div class="grid grid-cols-1 xl:grid-cols-3 gap-6 items-start">
        <div class="xl:col-span-1">
          <SpawnNode
            activeHostModels={activeHostModels}
            models={models}
            refetchNodes={refetchNodes}
          />
        </div>

        <div class="xl:col-span-2 space-y-3">
          <NodeGrid nodes={nodeStore.data} loading={nodes.loading} />
        </div>
      </div>
    </main>
  );
}
