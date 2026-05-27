import { createSignal, createResource, For, Show } from "solid-js";
import { A, useNavigate } from "@solidjs/router";
import { Cpu, Terminal, Zap, Plus, AlertCircle, Box, Tag } from "lucide-solid";
import { settingsStore } from "~/lib/settingsStore";

interface AgentConfig {
  id: string;
  name: string;
  system_prompt: string;
  model: string;
  status: "Online" | "Offline";
  created_at: number;
}

const fetchData = async (hubUrl: string) => {
  const [agentsRes, nodesRes] = await Promise.all([
    fetch(`${hubUrl}/agents`),
    fetch(`${hubUrl}/nodes`),
  ]);
  const agents = await agentsRes.json();
  const nodes = await nodesRes.json();

  const availableModels = [
    ...new Set(nodes.map((n: any) => n.model_name).filter(Boolean)),
  ];

  return {
    agents: agents as AgentConfig[],
    availableModels: availableModels as string[],
  };
};

const getModelTags = (model: string) => {
  const tags = [];
  const lower = model.toLowerCase();

  if (lower.includes("instruct") || lower.includes("chat"))
    tags.push({ label: "Instruct", color: "bg-blue-500/20 text-blue-400 border-blue-500/30" });
  if (lower.includes("code") || lower.includes("coder") || lower.includes("deepseek"))
    tags.push({ label: "Coding", color: "bg-emerald-500/20 text-emerald-400 border-emerald-500/30" });
  if (lower.includes("vision") || lower.includes("llava") || lower.includes("omni"))
    tags.push({ label: "Vision", color: "bg-purple-500/20 text-purple-400 border-purple-500/30" });
  if (lower.includes("math"))
    tags.push({ label: "Math", color: "bg-amber-500/20 text-amber-400 border-amber-500/30" });
  if (lower.includes("q4") || lower.includes("q5") || lower.includes("q8"))
    tags.push({ label: "Quantized", color: "bg-zinc-500/20 text-zinc-400 border-zinc-500/30" });

  if (tags.length === 0)
    tags.push({ label: "General", color: "bg-zinc-500/20 text-zinc-400 border-zinc-500/30" });

  return tags;
};

export default function AgentsDashboard() {
  const navigate = useNavigate();
  const [data, { refetch }] = createResource(() => settingsStore.activeHub, fetchData);
  const [isCreating, setIsCreating] = createSignal(false);
  const [newAgent, setNewAgent] = createSignal({
    name: "",
    system_prompt: "You are a helpful AI assistant.",
    model: "",
  });

  const createAgent = async (e: Event) => {
    e.preventDefault();
    setIsCreating(true);
    try {
      const res = await fetch(`${settingsStore.activeHub}/agents`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(newAgent()),
      });

      if (res.ok) {
        const createdAgent = await res.json();
        navigate(`/agents/${createdAgent.id}`);
      }
    } catch (err) {
      console.error(err);
      alert("Failed to create agent");
      setIsCreating(false);
    }
  };

  return (
    <main class="max-w-7xl mx-auto p-6 space-y-6">
      <div class="flex flex-col md:flex-row justify-between items-start md:items-end gap-4 border-b border-ng pb-4">
        <div>
          <h1 class="text-2xl font-bold tracking-tight text-white font-data flex items-center gap-3">
            <Zap class="text-cyan-ng" size={22} /> Agent Swarm
          </h1>
          <p class="text-ng-secondary text-sm mt-1">
            Deploy and orchestrate specialized autonomous personas.
          </p>
        </div>
        <div class="flex items-center gap-3 text-[10px] font-data uppercase tracking-widest text-ng-muted">
          <span>Network Status:</span>
          <span class="flex items-center gap-2 text-online font-bold">
            <span class="w-1.5 h-1.5 bg-online animate-pulse" style="clip-path: var(--ng-clip-badge)"></span>
            Online
          </span>
        </div>
      </div>

      <div class="grid grid-cols-1 xl:grid-cols-4 gap-6 items-start">
        {/* Create Agent Panel */}
        <div class="xl:col-span-1">
          <div class="ng-card p-5 sticky top-6">
            <div class="flex items-center gap-2 mb-5 text-cyan-ng">
              <Plus size={16} />
              <h2 class="text-sm font-bold text-white font-data uppercase tracking-widest">Deploy Agent</h2>
            </div>

            <form onSubmit={createAgent} class="space-y-4">
              <div class="space-y-1">
                <label class="label">Designation</label>
                <input
                  type="text"
                  required
                  class="input-field font-data text-sm"
                  value={newAgent().name}
                  onInput={(e) =>
                    setNewAgent({ ...newAgent(), name: e.currentTarget.value })
                  }
                  placeholder="e.g. Code Reviewer"
                />
              </div>

              <div class="space-y-1">
                <label class="label">Inference Model</label>
                <div class="relative">
                  <select
                    class="input-field appearance-none pr-8"
                    required
                    value={newAgent().model}
                    onChange={(e) =>
                      setNewAgent({
                        ...newAgent(),
                        model: e.currentTarget.value,
                      })
                    }
                  >
                    <option value="" disabled selected>
                      Select Neural Backbone...
                    </option>
                    <For each={data()?.availableModels}>
                      {(model) => (
                        <option value={model as string}>{model as string}</option>
                      )}
                    </For>
                  </select>
                  <div class="absolute right-3 top-2.5 pointer-events-none text-ng-muted">
                    <Box size={14} />
                  </div>
                </div>
                <Show when={data()?.availableModels.length === 0}>
                  <div class="flex items-center gap-2 text-warning bg-warning/5 p-2.5 border border-warning/20" style="clip-path: var(--ng-clip-card-sm)">
                    <AlertCircle size={14} />
                    <p class="text-xs font-data">No active nodes detected.</p>
                  </div>
                </Show>
              </div>

              <div class="space-y-1">
                <label class="label">System Directive</label>
                <textarea
                  class="input-field min-h-[120px] resize-none text-sm leading-relaxed"
                  value={newAgent().system_prompt}
                  onInput={(e) =>
                    setNewAgent({
                      ...newAgent(),
                      system_prompt: e.currentTarget.value,
                    })
                  }
                ></textarea>
              </div>

              <button
                type="submit"
                disabled={isCreating() || data()?.availableModels.length === 0}
                class="btn-primary w-full flex items-center justify-center gap-2 group py-3"
              >
                <Show when={!isCreating()} fallback={<span class="font-data text-xs uppercase tracking-wider">Initializing...</span>}>
                  <Zap size={16} class="group-hover:text-yellow-300 transition-colors" />
                  <span class="font-data text-xs uppercase tracking-wider">Deploy Agent</span>
                </Show>
              </button>
            </form>
          </div>
        </div>

        {/* Agent Grid */}
        <div class="xl:col-span-3">
          <Show when={data.loading}>
            <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4 animate-pulse">
              <div class="h-44 bg-ng-bg-surface/50 border border-ng" style="clip-path: var(--ng-clip-card)"></div>
              <div class="h-44 bg-ng-bg-surface/50 border border-ng" style="clip-path: var(--ng-clip-card)"></div>
              <div class="h-44 bg-ng-bg-surface/50 border border-ng" style="clip-path: var(--ng-clip-card)"></div>
            </div>
          </Show>

          <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
            <For each={data()?.agents}>
              {(agent) => (
                <A
                  href={`/agents/${agent.id}`}
                  class="ng-card p-4 group cursor-pointer"
                >
                  <div class="flex flex-col h-full">
                    <div class="flex justify-between items-start mb-3">
                      <div class="p-2 bg-ng-bg-deep border border-ng group-hover:border-cyan-ng/30 transition-colors" style="clip-path: var(--ng-clip-card-sm)">
                        <Terminal size={20} class="text-ng-secondary group-hover:text-cyan-ng transition-colors" />
                      </div>
                      <span class={`status-badge ${agent.status === "Online" ? "status-online" : "status-offline"}`}>
                        {agent.status}
                      </span>
                    </div>

                    <h3 class="text-base font-bold text-white mb-1 font-data tracking-wider group-hover:text-cyan-ng transition-colors">
                      {agent.name}
                    </h3>
                    <div class="flex items-center gap-2 text-[10px] text-ng-muted font-data mb-2">
                      <Cpu size={11} />
                      <span class="truncate max-w-40">{agent.model}</span>
                    </div>

                    <div class="flex flex-wrap gap-1 mb-3">
                      <For each={getModelTags(agent.model)}>
                        {(tag) => (
                          <span class={`text-[8px] font-data px-1.5 py-0.5 border ${tag.color}`} style="clip-path: var(--ng-clip-badge)">
                            {tag.label}
                          </span>
                        )}
                      </For>
                    </div>

                    <p class="text-xs text-ng-secondary line-clamp-2 leading-relaxed mb-3 flex-1">
                      {agent.system_prompt}
                    </p>

                    <div class="pt-3 border-t border-ng flex items-center text-[10px] text-ng-muted font-data">
                      <span>ID: {agent.id.slice(0, 8)}...</span>
                    </div>
                  </div>
                </A>
              )}
            </For>
          </div>

          <Show when={!data.loading && data()?.agents.length === 0}>
            <div class="flex flex-col items-center justify-center py-20 border border-ng bg-ng-bg-deep/30" style="clip-path: var(--ng-clip-card)">
              <div class="w-14 h-14 bg-ng-bg-surface border border-ng flex items-center justify-center mb-4 text-ng-muted" style="clip-path: var(--ng-clip-card-sm)">
                <Terminal size={28} />
              </div>
              <h3 class="text-lg font-bold text-white font-data tracking-wider mb-2">
                No Agents Deployed
              </h3>
              <p class="text-ng-secondary text-sm max-w-sm text-center">
                Create your first AI agent to start distributed inference tasks on the network.
              </p>
            </div>
          </Show>
        </div>
      </div>
    </main>
  );
}
