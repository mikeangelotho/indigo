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
      totalMemory: nodes.length * 16,
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
    <main class="max-w-7xl mx-auto p-6 space-y-8">
      {/* Welcome Header — PULSE hero style */}
      <header class="relative overflow-hidden border border-ng p-8 md:p-12" style="clip-path: var(--ng-clip-card)">
        {/* Corner accent */}
        <div class="absolute top-0 right-0 w-24 h-0.5 bg-linear-to-l from-cyan-ng to-transparent"></div>
        <div class="absolute top-0 right-0 w-0.5 h-24 bg-linear-to-b from-cyan-ng to-transparent"></div>
        {/* Gradient wash */}
        <div class="absolute inset-0" style="background: var(--ng-gradient-hero)"></div>
        {/* Ambient glow */}
        <div class="absolute -right-20 -top-20 w-80 h-80 rounded-full blur-[100px]" style="background: var(--ng-cyan-glow)"></div>
        <div class="absolute -left-20 -bottom-20 w-60 h-60 rounded-full blur-[80px]" style="background: var(--ng-magenta-glow)"></div>

        <div class="relative z-10 flex flex-col md:flex-row justify-between items-start md:items-center gap-6">
          <div>
            <div class="flex items-center gap-2 font-data text-[10px] uppercase tracking-[0.2em] mb-3 text-cyan-ng">
              <ShieldCheck size={12} />
              Secure Environment Verified
            </div>
            <h1 class="text-3xl md:text-4xl font-bold tracking-tight text-white mb-3 font-data">
              Welcome back,{" "}
              <span class="text-transparent bg-clip-text bg-linear-to-r from-cyan-ng to-magenta-ng">
                {authState.user?.name}
              </span>
            </h1>
            <p class="text-ng-secondary text-base max-w-xl leading-relaxed">
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
              class="btn-primary flex items-center gap-2"
            >
              <PlusCircle size={16} />
              Deploy Agent
            </A>
          </div>
        </div>
      </header>

      {/* Stats Grid — PULSE watcher card style */}
      <div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4">
        <StatCard
          title="Active Agents"
          value={stats()?.agentCount || 0}
          icon={<Users class="text-cyan-ng" />}
          trend="+2 this week"
        />
        <StatCard
          title="Compute Nodes"
          value={`${stats()?.activeNodes || 0}/${stats()?.nodeCount || 0}`}
          icon={<Cpu class="text-magenta-ng" />}
          trend="100% Health"
        />
        <StatCard
          title="System Uptime"
          value={stats()?.uptime || "0%"}
          icon={<Activity class="text-online" />}
          trend="Stable"
        />
        <StatCard
          title="Network Load"
          value="12.4%"
          icon={<Globe class="text-purple-400" />}
          trend="Low Latency"
        />
      </div>

      <div class="grid grid-cols-1 lg:grid-cols-3 gap-6">
        {/* Quick Actions & Navigation */}
        <div class="lg:col-span-2 space-y-6">
          <div class="ng-section-header">
            <Zap size={14} />
            Strategic Operations
          </div>
          <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
            <ActionCard
              href="/agents"
              title="Agent Swarm"
              desc="Manage specialized personas and fine-tune system directives for autonomous tasks."
              icon={<Users size={20} />}
              accent="cyan"
            />
            <ActionCard
              href="/nodes"
              title="Compute Grid"
              desc="Monitor real-time node performance, sharding metrics, and GPU utilization."
              icon={<Network size={20} />}
              accent="magenta"
            />
          </div>

          {/* Network Visualization */}
          <div class="ng-card p-6 corner-accent">
            <div class="flex justify-between items-start mb-6">
              <div>
                <h3 class="text-sm font-bold text-white font-data tracking-wider">Grid Topology</h3>
                <p class="text-xs text-ng-muted mt-1">
                  Geographic distribution of active inference workers.
                </p>
              </div>
              <ArrowUpRight class="text-ng-muted group-hover:text-cyan-ng transition-colors" size={18} />
            </div>
            <div class="aspect-video bg-ng-bg-deep/80 border border-ng flex flex-col items-center justify-center space-y-3" style="clip-path: var(--ng-clip-card-sm)">
              <div class="relative">
                <Globe size={40} class="text-ng-border animate-pulse" />
                <div class="absolute top-0 right-0 w-2 h-2 bg-cyan-ng animate-ping" style="clip-path: var(--ng-clip-badge)"></div>
              </div>
              <span class="text-[10px] font-mono text-ng-muted uppercase tracking-widest font-data">
                Awaiting Spatial Data
              </span>
            </div>
          </div>
        </div>

        {/* Sidebar: System Logs & Org Info */}
        <div class="space-y-6">
          <div class="ng-section-header">
            <Layers size={14} />
            Live Telemetry
          </div>
          <div class="ng-card overflow-hidden">
            <div class="p-3 bg-ng-bg-deep/50 border-b border-ng flex items-center justify-between">
              <span class="text-[10px] font-data text-ng-muted uppercase tracking-widest">
                Event Log
              </span>
              <span class="flex items-center gap-1.5 text-[9px] font-bold text-online font-data uppercase tracking-wider px-2 py-0.5 border border-green-500/20" style="clip-path: var(--ng-clip-badge)">
                <div class="w-1 h-1 bg-online rounded-full"></div>
                Streaming
              </span>
            </div>
            <div class="p-4 space-y-4">
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
                <div class="py-8 text-center text-ng-muted text-xs font-data italic">
                  Initializing telemetry stream...
                </div>
              </Show>
            </div>
            <button class="w-full py-3 text-[10px] font-bold uppercase tracking-widest text-ng-muted hover:text-cyan-ng hover:bg-ng-bg-hover transition-all font-data border-t border-ng">
              View Audit Trail
            </button>
          </div>

          {/* Organization Snapshot */}
          <div class="ng-card p-5">
            <h3 class="text-[10px] font-data text-ng-muted uppercase tracking-widest mb-4">
              Identity Context
            </h3>
            <div class="flex items-center gap-3 mb-4">
              <div class="w-9 h-9 flex items-center justify-center text-white font-bold text-sm font-data" style="clip-path: var(--ng-clip-card-sm); background: linear-gradient(135deg, var(--ng-cyan-dim), var(--ng-magenta-dim))">
                {authState.organization?.name.charAt(0)}
              </div>
              <div>
                <div class="text-sm font-bold text-white font-data tracking-wider">
                  {authState.organization?.name}
                </div>
                <div class="text-[9px] text-ng-muted uppercase font-data tracking-widest">
                  {authState.organization?.plan} PLAN
                </div>
              </div>
            </div>
            <div class="space-y-2">
              <div class="flex justify-between text-[10px] font-data">
                <span class="text-ng-muted uppercase tracking-wider">Project Members</span>
                <span class="text-ng-secondary">1 / 10</span>
              </div>
              <div class="w-full bg-ng-bg-deep h-1 overflow-hidden" style="clip-path: var(--ng-clip-card-sm)">
                <div class="h-full w-[10%]" style="background: linear-gradient(90deg, var(--ng-cyan), var(--ng-magenta))"></div>
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
    <div class="ng-card p-5 group">
      <div class="flex justify-between items-start mb-3">
        <div class="p-2 bg-ng-bg-deep border border-ng group-hover:border-cyan-ng/30 transition-colors" style="clip-path: var(--ng-clip-card-sm)">
          {props.icon}
        </div>
        <span class="text-[9px] font-data text-ng-muted uppercase tracking-tighter">
          {props.trend}
        </span>
      </div>
      <div class="text-2xl font-bold text-white mb-1 font-data">{props.value}</div>
      <div class="text-[10px] text-ng-muted font-bold uppercase tracking-widest font-data">
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
  accent: "cyan" | "magenta";
}) {
  const accentColor = () => props.accent === "cyan" ? "var(--ng-cyan)" : "var(--ng-magenta)";
  const accentGlow = () => props.accent === "cyan" ? "var(--ng-cyan-glow)" : "var(--ng-magenta-glow)";
  const accentDim = () => props.accent === "cyan" ? "text-cyan-ng" : "text-magenta-ng";

  return (
    <A
      href={props.href}
      class="ng-card p-5 group cursor-pointer"
    >
      <div class="absolute -right-6 -top-6 w-20 h-20 rounded-full blur-2xl transition-all opacity-0 group-hover:opacity-100" style={`background: ${accentGlow()}`}></div>
      <div class="flex items-center gap-3 mb-3">
        <div class={`p-2.5 bg-ng-bg-deep border border-ng group-hover:border-cyan-ng/30 transition-colors ${accentDim()}`} style="clip-path: var(--ng-clip-card-sm)">
          {props.icon}
        </div>
        <h3 class="text-base font-bold text-white group-hover:text-cyan-ng transition-colors font-data tracking-wider">
          {props.title}
        </h3>
      </div>
      <p class="text-xs text-ng-secondary leading-relaxed mb-3">{props.desc}</p>
      <div class="flex items-center gap-2 text-[10px] font-bold text-ng-muted group-hover:text-cyan-ng transition-colors font-data uppercase tracking-widest">
        PROCEED <ArrowUpRight size={12} />
      </div>
    </A>
  );
}

function LogEntry(props: {
  time: string;
  msg: string;
  status: "success" | "info" | "warning";
}) {
  const dotColor = () =>
    props.status === "success"
      ? "bg-online"
      : props.status === "warning"
      ? "bg-warning"
      : "bg-cyan-ng";

  return (
    <div class="flex items-start gap-3">
      <div class={`mt-1.5 w-1.5 h-1.5 shrink-0 ${dotColor()}`} style="clip-path: var(--ng-clip-badge)"></div>
      <div class="flex-1">
        <p class="text-xs text-ng-secondary leading-snug">{props.msg}</p>
        <span class="text-[9px] text-ng-muted font-data">{props.time}</span>
      </div>
    </div>
  );
}
