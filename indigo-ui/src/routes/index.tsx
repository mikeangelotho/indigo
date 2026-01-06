import { createResource, For, Show } from "solid-js";
import { A } from "@solidjs/router";
import {
  Zap,
  Cpu,
  Users,
  Activity,
  ShieldCheck,
  ArrowUpRight,
  Layers,
  Globe,
  PlusCircle,
  Network,
} from "lucide-solid";
import { authState } from "~/lib/authStore";
import { settingsStore } from "~/lib/settingsStore";

const fetchStats = async (hubUrl: string) => {
  try {
    const [agentsRes, nodesRes] = await Promise.all([
      fetch(`${hubUrl}/agents`),
      fetch(`${hubUrl}/nodes`),
    ]);
    if (!agentsRes.ok || !nodesRes.ok) throw new Error("Failed to fetch stats");
    
    const agents = await agentsRes.json();
    const nodes = await nodesRes.json();
    return {
      agentCount: agents.length,
      nodeCount: nodes.length,
      activeNodes: nodes.filter((n: any) => n.status === "Online").length,
      totalMemory: nodes.length * 16, // Mock data
      uptime: "99.98%",
    };
  } catch (e) {
    console.error("Failed to fetch stats", e);
    return null;
  }
};

export default function Home() {
  const [stats] = createResource(() => settingsStore.activeHub, fetchStats);

  return (
    <main class="max-w-7xl mx-auto p-6 space-y-10">
      {/* Welcome Header */}
      <header class="relative overflow-hidden rounded-2xl bg-linear-to-br from-indigo-900/20 via-zinc-900 to-black border border-zinc-800 p-8 md:p-12 shadow-2xl">
        <div class="absolute -right-20 -top-20 w-80 h-80 bg-indigo-500/10 rounded-full blur-[100px]"></div>
        <div class="relative z-10 flex flex-col md:flex-row justify-between items-start md:items-center gap-6">
          <div>
            <div class="flex items-center gap-2 text-indigo-400 font-mono text-xs uppercase tracking-[0.2em] mb-3">
              <ShieldCheck size={14} />
              Secure Environment Verified
            </div>
            <h1 class="text-4xl md:text-5xl font-bold tracking-tight text-white mb-4">
              Welcome back,{" "}
              <span class="text-transparent bg-clip-text bg-linear-to-r from-indigo-400 to-cyan-400">
                {authState.user?.name}
              </span>
            </h1>
            <p class="text-zinc-400 text-lg max-w-xl leading-relaxed">
              Your decentralized compute grid is operational. Currently sharding
              inference across{" "}
              <span class="text-white font-semibold">
                {stats()?.activeNodes || 0} active nodes
              </span>
              .
            </p>
          </div>
          <div class="flex gap-3">
            <A
              href="/agents"
              class="bg-indigo-600 hover:bg-indigo-500 text-white px-6 py-3 rounded-xl font-semibold transition-all hover:scale-105 flex items-center gap-2 shadow-lg shadow-indigo-900/20"
            >
              <PlusCircle size={20} />
              Deploy Agent
            </A>
          </div>
        </div>
      </header>

      {/* Stats Grid */}
      <div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-6">
        <StatCard
          title="Active Agents"
          value={stats()?.agentCount || 0}
          icon={<Users class="text-indigo-400" />}
          trend="+2 this week"
        />
        <StatCard
          title="Compute Nodes"
          value={`${stats()?.activeNodes || 0}/${stats()?.nodeCount || 0}`}
          icon={<Cpu class="text-cyan-400" />}
          trend="100% Health"
        />
        <StatCard
          title="System Uptime"
          value={stats()?.uptime || "0%"}
          icon={<Activity class="text-green-400" />}
          trend="Stable"
        />
        <StatCard
          title="Network Load"
          value="12.4%"
          icon={<Globe class="text-purple-400" />}
          trend="Low Latency"
        />
      </div>

      <div class="grid grid-cols-1 lg:grid-cols-3 gap-8">
        {/* Quick Actions & Navigation */}
        <div class="lg:col-span-2 space-y-8">
          <h2 class="text-xl font-bold text-white flex items-center gap-2">
            <Zap size={20} class="text-yellow-400" />
            Strategic Operations
          </h2>
          <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
            <ActionCard
              href="/agents"
              title="Agent Swarm"
              desc="Manage specialized personas and fine-tune system directives for autonomous tasks."
              icon={<Users size={24} />}
              color="indigo"
            />
            <ActionCard
              href="/nodes"
              title="Compute Grid"
              desc="Monitor real-time node performance, sharding metrics, and GPU utilization."
              icon={<Network size={24} />}
              color="cyan"
            />
          </div>

          {/* Network Visualization Placeholder */}
          <div class="bg-zinc-900 border border-zinc-800 rounded-2xl p-8 relative overflow-hidden group">
            <div class="absolute inset-0 bg-linear-to-br from-indigo-500/5 to-transparent opacity-0 group-hover:opacity-100 transition-opacity"></div>
            <div class="flex justify-between items-start mb-8">
              <div>
                <h3 class="text-lg font-bold text-white">Grid Topology</h3>
                <p class="text-sm text-zinc-500">
                  Geographic distribution of active inference workers.
                </p>
              </div>
              <ArrowUpRight class="text-zinc-600 group-hover:text-white transition-colors" />
            </div>
            <div class="aspect-video bg-black/50 rounded-xl border border-zinc-800/50 flex flex-col items-center justify-center space-y-4 border-dashed">
              <div class="relative">
                <Globe size={48} class="text-zinc-800 animate-pulse" />
                <div class="absolute top-0 right-0 w-3 h-3 bg-indigo-500 rounded-full animate-ping"></div>
              </div>
              <span class="text-xs font-mono text-zinc-600 uppercase tracking-widest">
                Awaiting Spatial Data
              </span>
            </div>
          </div>
        </div>

        {/* Sidebar: System Logs & Org Info */}
        <div class="space-y-8">
          <h2 class="text-xl font-bold text-white flex items-center gap-2">
            <Layers size={20} class="text-zinc-400" />
            Live Telemetry
          </h2>
          <div class="bg-zinc-900 border border-zinc-800 rounded-2xl overflow-hidden divide-y divide-zinc-800/50">
            <div class="p-4 bg-zinc-950/50 flex items-center justify-between">
              <span class="text-xs font-mono text-zinc-500 uppercase">
                Event Log
              </span>
              <span class="flex items-center gap-1.5 text-[10px] text-green-500 font-bold bg-green-500/10 px-2 py-0.5 rounded-full border border-green-500/20">
                <div class="w-1.5 h-1.5 bg-green-500 rounded-full"></div>
                STREAMING
              </span>
            </div>
            <div class="p-6 space-y-6">
              <LogEntry
                time="2m ago"
                msg="Agent 'Code Master' initialized"
                status="success"
              />
              <LogEntry
                time="15m ago"
                msg="Local Node 50052 joined grid"
                status="info"
              />
              <LogEntry
                time="42m ago"
                msg="Automatic backup complete"
                status="success"
              />
              <LogEntry
                time="1h ago"
                msg="Inference burst detected (120 tps)"
                status="warning"
              />

              <Show when={!stats()}>
                <div class="py-10 text-center text-zinc-600 text-sm italic">
                  Initializing telemetry stream...
                </div>
              </Show>
            </div>
            <button class="w-full py-3 text-xs text-zinc-500 hover:text-white hover:bg-zinc-800 transition-all font-medium">
              View Audit Trail
            </button>
          </div>

          {/* Organization Snapshot */}
          <div class="bg-linear-to-br from-zinc-900 to-zinc-950 border border-zinc-800 rounded-2xl p-6">
            <h3 class="text-xs font-mono text-zinc-500 uppercase mb-4 tracking-widest">
              Identity Context
            </h3>
            <div class="flex items-center gap-4 mb-6">
              <div class="w-10 h-10 bg-indigo-600 rounded-lg flex items-center justify-center text-white font-bold">
                {authState.organization?.name.charAt(0)}
              </div>
              <div>
                <div class="text-sm font-bold text-white">
                  {authState.organization?.name}
                </div>
                <div class="text-[10px] text-zinc-500 uppercase font-mono">
                  {authState.organization?.plan} PLAN
                </div>
              </div>
            </div>
            <div class="space-y-3">
              <div class="flex justify-between text-xs">
                <span class="text-zinc-500">Project Members</span>
                <span class="text-zinc-300">1 / 10</span>
              </div>
              <div class="w-full bg-zinc-800 h-1 rounded-full overflow-hidden">
                <div class="bg-indigo-500 h-full w-[10%]"></div>
              </div>
            </div>
          </div>
        </div>
      </div>
    </main>
  );
}

function StatCard(props: {
  title: string;
  value: any;
  icon: any;
  trend: string;
}) {
  return (
    <div class="bg-zinc-900 border border-zinc-800 rounded-2xl p-6 hover:border-zinc-700 transition-all group shadow-lg shadow-black/20">
      <div class="flex justify-between items-start mb-4">
        <div class="p-2 bg-zinc-950 rounded-lg border border-zinc-800 group-hover:border-zinc-700 transition-colors">
          {props.icon}
        </div>
        <span class="text-[10px] font-mono text-zinc-500 uppercase tracking-tighter">
          {props.trend}
        </span>
      </div>
      <div class="text-2xl font-bold text-white mb-1">{props.value}</div>
      <div class="text-xs text-zinc-500 font-medium uppercase tracking-wider">
        {props.title}
      </div>
    </div>
  );
}

function ActionCard(props: {
  href: string;
  title: string;
  desc: string;
  icon: any;
  color: string;
}) {
  return (
    <A
      href={props.href}
      class="group relative bg-zinc-900 border border-zinc-800 rounded-2xl p-6 hover:border-indigo-500/50 transition-all overflow-hidden"
    >
      <div class="absolute -right-6 -top-6 w-24 h-24 bg-indigo-500/5 rounded-full blur-2xl group-hover:bg-indigo-500/10 transition-all"></div>
      <div class="flex items-center gap-4 mb-4">
        <div
          class={`p-3 bg-zinc-950 rounded-xl border border-zinc-800 group-hover:border-indigo-500/30 transition-colors ${
            props.color === "indigo" ? "text-indigo-400" : "text-cyan-400"
          }`}
        >
          {props.icon}
        </div>
        <h3 class="text-lg font-bold text-white group-hover:text-indigo-400 transition-colors">
          {props.title}
        </h3>
      </div>
      <p class="text-sm text-zinc-400 leading-relaxed mb-4">{props.desc}</p>
      <div class="flex items-center gap-2 text-xs font-bold text-zinc-500 group-hover:text-white transition-colors">
        PROCEED <ArrowUpRight size={14} />
      </div>
    </A>
  );
}

function LogEntry(props: {
  time: string;
  msg: string;
  status: "success" | "info" | "warning";
}) {
  return (
    <div class="flex items-start gap-3">
      <div
        class={`mt-1.5 w-1.5 h-1.5 rounded-full shrink-0 ${
          props.status === "success"
            ? "bg-green-500"
            : props.status === "warning"
            ? "bg-amber-500"
            : "bg-indigo-500"
        }`}
      ></div>
      <div class="flex-1">
        <p class="text-xs text-zinc-300 leading-snug">{props.msg}</p>
        <span class="text-[10px] text-zinc-600 font-mono">{props.time}</span>
      </div>
    </div>
  );
}
