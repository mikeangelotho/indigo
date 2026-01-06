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

  // Extract unique model names from active nodes
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
    lower.includes("omni")
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

export default function AgentsDashboard() {
  const navigate = useNavigate();
  const [data, { refetch }] = createResource(
    () => settingsStore.activeHub,
    fetchData
  );
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
    <main class="max-w-7xl mx-auto p-6 space-y-8">
      <div class="flex flex-col md:flex-row justify-between items-start md:items-end gap-4 border-b border-zinc-800 pb-6">
        <div>
          <h1 class="text-3xl font-bold tracking-tight text-white flex items-center gap-3">
            <Zap class="text-indigo-500" /> Agent Swarm
          </h1>
          <p class="text-zinc-400 mt-2">
            Deploy and orchestrate specialized autonomous personas.
          </p>
        </div>
        <div class="flex items-center gap-4 text-sm text-zinc-500 font-mono">
          <span>NETWORK STATUS:</span>
          <span class="flex items-center gap-2 text-green-400">
            <span class="w-2 h-2 bg-green-500 rounded-full animate-pulse"></span>
            ONLINE
          </span>
        </div>
      </div>

      <div class="grid grid-cols-1 xl:grid-cols-4 gap-8 items-start">
        {/* Create Agent Panel */}
        <div class="xl:col-span-1">
          <div class="bg-zinc-900/50 backdrop-blur border border-zinc-800 rounded-xl p-6 sticky top-6 shadow-xl shadow-black/50">
            <div class="flex items-center gap-2 mb-6 text-indigo-400">
              <Plus size={20} />
              <h2 class="text-lg font-semibold text-white">Deploy Agent</h2>
            </div>

            <form onSubmit={createAgent} class="space-y-5">
              <div class="space-y-1.5">
                <label class="text-xs font-mono uppercase text-zinc-500">
                  Designation
                </label>
                <input
                  type="text"
                  required
                  class="w-full bg-zinc-950 border border-zinc-800 rounded-lg px-4 py-2.5 text-white placeholder:text-zinc-600 focus:border-indigo-500 focus:ring-1 focus:ring-indigo-500/50 outline-none transition-all"
                  value={newAgent().name}
                  onInput={(e) =>
                    setNewAgent({ ...newAgent(), name: e.currentTarget.value })
                  }
                  placeholder="e.g. Code Reviewer"
                />
              </div>

              <div class="space-y-1.5">
                <label class="text-xs font-mono uppercase text-zinc-500">
                  Inference Model
                </label>
                <div class="relative">
                  <select
                    class="w-full appearance-none bg-zinc-950 border border-zinc-800 rounded-lg px-4 py-2.5 text-white focus:border-indigo-500 outline-none transition-all"
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
                        <option value={model as string}>
                          {model as string}
                        </option>
                      )}
                    </For>
                  </select>
                  <div class="absolute right-3 top-3 pointer-events-none text-zinc-500">
                    <Box size={16} />
                  </div>
                </div>
                <Show when={data()?.availableModels.length === 0}>
                  <div class="flex items-center gap-2 text-amber-500 bg-amber-950/20 p-3 rounded-lg border border-amber-900/30">
                    <AlertCircle size={16} />
                    <p class="text-xs">No active nodes detected.</p>
                  </div>
                </Show>
              </div>

              <div class="space-y-1.5">
                <label class="text-xs font-mono uppercase text-zinc-500">
                  System Directive
                </label>
                <textarea
                  class="w-full bg-zinc-950 border border-zinc-800 rounded-lg px-4 py-2.5 text-white placeholder:text-zinc-600 focus:border-indigo-500 focus:ring-1 focus:ring-indigo-500/50 outline-none min-h-30 resize-none text-sm leading-relaxed"
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
                class="w-full bg-indigo-600 hover:bg-indigo-500 disabled:opacity-50 disabled:cursor-not-allowed text-white font-medium py-3 rounded-lg shadow-lg shadow-indigo-900/20 transition-all flex items-center justify-center gap-2 group"
              >
                <Show
                  when={!isCreating()}
                  fallback={<span>Initializing...</span>}
                >
                  <Zap
                    size={18}
                    class="group-hover:text-yellow-300 transition-colors"
                  />
                  Deploy Agent
                </Show>
              </button>
            </form>
          </div>
        </div>

        {/* Agent Grid */}
        <div class="xl:col-span-3">
          <Show when={data.loading}>
            <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6 animate-pulse">
              <div class="h-48 bg-zinc-900/50 rounded-xl border border-zinc-800"></div>
              <div class="h-48 bg-zinc-900/50 rounded-xl border border-zinc-800"></div>
              <div class="h-48 bg-zinc-900/50 rounded-xl border border-zinc-800"></div>
            </div>
          </Show>

          <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
            <For each={data()?.agents}>
              {(agent) => (
                <A
                  href={`/agents/${agent.id}`}
                  class="group relative bg-zinc-900 border border-zinc-800 hover:border-indigo-500/50 rounded-xl p-5 transition-all hover:shadow-xl hover:shadow-indigo-500/5 hover:-translate-y-1 overflow-hidden"
                >
                  {/* Decoration */}
                  <div class="absolute -right-10 -top-10 w-32 h-32 bg-linear-to-br from-indigo-500/10 to-transparent rounded-full blur-2xl group-hover:from-indigo-500/20 transition-all"></div>

                  <div class="relative z-10 flex flex-col h-full">
                    <div class="flex justify-between items-start mb-4">
                      <div class="p-3 bg-zinc-950 rounded-lg border border-zinc-800 group-hover:border-indigo-500/30 transition-colors">
                        <Terminal
                          size={24}
                          class="text-zinc-400 group-hover:text-indigo-400 transition-colors"
                        />
                      </div>
                      <div
                        class={`flex items-center gap-1.5 text-[10px] font-medium uppercase tracking-wider px-2 py-1 rounded-full border ${
                          agent.status === "Online"
                            ? "bg-green-500/10 text-green-400 border-green-500/20"
                            : "bg-red-500/10 text-red-400 border-red-500/20"
                        }`}
                      >
                        <div
                          class={`w-1.5 h-1.5 rounded-full ${
                            agent.status === "Online"
                              ? "bg-green-400 animate-pulse"
                              : "bg-red-400"
                          }`}
                        ></div>
                        {agent.status}
                      </div>
                    </div>

                    <h3 class="text-lg font-bold text-white mb-1 group-hover:text-indigo-300 transition-colors">
                      {agent.name}
                    </h3>
                    <div class="flex items-center gap-2 text-xs text-zinc-500 font-mono mb-2">
                      <Cpu size={12} />
                      <span class="truncate max-w-45">{agent.model}</span>
                    </div>

                    <div class="flex flex-wrap gap-1.5 mb-4">
                      <For each={getModelTags(agent.model)}>
                        {(tag) => (
                          <span
                            class={`text-[9px] font-mono px-1.5 py-0.5 rounded border ${tag.color}`}
                          >
                            {tag.label}
                          </span>
                        )}
                      </For>
                    </div>

                    <p class="text-sm text-zinc-400 line-clamp-2 leading-relaxed mb-4 flex-1">
                      {agent.system_prompt}
                    </p>

                    <div class="pt-4 border-t border-zinc-800/50 flex items-center text-xs text-zinc-500">
                      <span>ID: {agent.id.slice(0, 8)}...</span>
                    </div>
                  </div>
                </A>
              )}
            </For>
          </div>

          <Show when={!data.loading && data()?.agents.length === 0}>
            <div class="flex flex-col items-center justify-center py-20 border-2 border-dashed border-zinc-800 rounded-xl bg-zinc-900/30">
              <div class="w-16 h-16 bg-zinc-900 rounded-full flex items-center justify-center mb-4 text-zinc-500">
                <Terminal size={32} />
              </div>
              <h3 class="text-xl font-medium text-white mb-2">
                No Agents Deployed
              </h3>
              <p class="text-zinc-400 max-w-sm text-center">
                Create your first AI agent to start distributed inference tasks
                on the network.
              </p>
            </div>
          </Show>
        </div>
      </div>
    </main>
  );
}
