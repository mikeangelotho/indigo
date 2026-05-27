import { useLocation, A } from "@solidjs/router";
import { createSignal, Show, For } from "solid-js";
import {
  LayoutDashboard,
  Users,
  Cpu,
  ChevronDown,
  LogOut,
  User,
  Settings,
  Building2,
  Shield,
  Server,
  Menu,
  X,
} from "lucide-solid";
import { authState, authActions } from "~/lib/authStore";
import HubManager from "./HubManager";

export default function Nav() {
  const location = useLocation();
  const [showOrgMenu, setShowOrgMenu] = createSignal(false);
  const [showUserMenu, setShowUserMenu] = createSignal(false);
  const [showHubMenu, setShowHubMenu] = createSignal(false);
  const [isMobileMenuOpen, setIsMobileMenuOpen] = createSignal(false);

  const NavLink = (props: { href: string; children: any; icon: any }) => {
    const isActive = () =>
      location.pathname === props.href ||
      (props.href !== "/" && location.pathname.startsWith(props.href));
    return (
      <A
        href={props.href}
        onClick={() => setIsMobileMenuOpen(false)}
        class={`flex items-center gap-2.5 px-3 py-2 text-xs font-bold uppercase tracking-wider transition-all ${
          isActive()
            ? "text-cyan-ng border-b-2 border-cyan-ng"
            : "text-ng-secondary hover:text-ng-primary border-b-2 border-transparent hover:border-ng-border-hover"
        }`}
      >
        {props.icon}
        {props.children}
      </A>
    );
  };

  return (
    <nav class="border-b border-ng bg-ng-bg-deep/90 backdrop-blur-xl sticky top-0 z-100">
      <div class="max-w-7xl mx-auto px-6 h-14 flex items-center justify-between">
        <div class="flex items-center gap-8">
          {/* Mobile Menu Toggle */}
          <button
            onClick={() => setIsMobileMenuOpen(!isMobileMenuOpen())}
            class="md:hidden text-ng-secondary hover:text-cyan-ng"
          >
            <Show when={isMobileMenuOpen()} fallback={<Menu size={20} />}>
              <X size={20} />
            </Show>
          </button>

          <A href="/" class="flex items-center gap-3 group">
            <div class="w-7 h-7 bg-linear-to-br from-cyan-ng to-magenta-ng flex items-center justify-center group-hover:shadow-[0_0_15px_var(--ng-cyan-glow-strong)] transition-shadow" style="clip-path: var(--ng-clip-card-sm)">
              <Shield size={14} class="text-black" />
            </div>
            <span class="font-data font-bold text-lg tracking-wider text-white hidden sm:inline-block">
              INDIGO
            </span>
          </A>

          <div class="hidden md:flex items-center gap-1">
            <NavLink href="/" icon={<LayoutDashboard size={14} />}>
              Overview
            </NavLink>
            <NavLink href="/agents" icon={<Users size={14} />}>
              Agents
            </NavLink>
            <NavLink href="/nodes" icon={<Cpu size={14} />}>
              Compute
            </NavLink>
          </div>
        </div>

        <div class="flex items-center gap-3 sm:gap-4">
          {/* Hub Manager Toggle */}
          <div class="relative hidden sm:block">
            <button
              onClick={() => {
                setShowHubMenu(!showHubMenu());
                setShowOrgMenu(false);
                setShowUserMenu(false);
              }}
              class={`flex items-center gap-2 px-3 py-1.5 text-xs font-bold uppercase tracking-wider transition-all ${
                showHubMenu()
                  ? "text-cyan-ng border border-cyan-ng/30"
                  : "border border-ng text-ng-secondary hover:text-ng-primary hover:border-ng-border-hover"
              }`}
              style="clip-path: var(--ng-clip-card-sm)"
              title="Manage Hub Connections"
            >
              <Server size={14} />
            </button>
            <Show when={showHubMenu()}>
              <HubManager onClose={() => setShowHubMenu(false)} />
            </Show>
          </div>

          {/* Organization Switcher */}
          <div class="relative hidden sm:block">
            <button
              onClick={() => {
                setShowOrgMenu(!showOrgMenu());
                setShowUserMenu(false);
                setShowHubMenu(false);
              }}
              class="flex items-center gap-2 px-3 py-1.5 text-xs font-bold uppercase tracking-wider border border-ng text-ng-secondary hover:text-ng-primary hover:border-ng-border-hover transition-all"
              style="clip-path: var(--ng-clip-card-sm)"
            >
              <Building2 size={14} class="text-cyan-ng" />
              <span>{authState.organization?.name}</span>
              <ChevronDown
                size={12}
                class={`transition-transform ${
                  showOrgMenu() ? "rotate-180" : ""
                }`}
              />
            </button>

            <Show when={showOrgMenu()}>
              <div class="absolute right-0 mt-2 w-56 bg-ng-bg-surface border border-ng rounded-xl shadow-2xl p-2 z-110" style="clip-path: var(--ng-clip-card)">
                <div class="px-3 py-2 text-[9px] font-bold text-ng-muted uppercase tracking-widest font-data">
                  Select Organization
                </div>
                <For each={authState.availableOrganizations}>
                  {(org) => (
                    <button
                      onClick={() => {
                        authActions.switchOrganization(org.id);
                        setShowOrgMenu(false);
                      }}
                      class={`w-full text-left px-3 py-2 text-sm transition-colors flex items-center justify-between ${
                        authState.organization?.id === org.id
                          ? "text-cyan-ng"
                          : "text-ng-secondary hover:bg-ng-bg-hover hover:text-ng-primary"
                      }`}
                    >
                      {org.name}
                      <Show when={authState.organization?.id === org.id}>
                        <div class="w-1.5 h-1.5 bg-cyan-ng" style="clip-path: var(--ng-clip-badge)"></div>
                      </Show>
                    </button>
                  )}
                </For>
              </div>
            </Show>
          </div>

          <div class="h-5 w-px bg-ng-border hidden sm:block"></div>

          {/* User Menu */}
          <div class="relative hidden sm:block">
            <button
              onClick={() => {
                setShowUserMenu(!showUserMenu());
                setShowOrgMenu(false);
                setShowHubMenu(false);
              }}
              class="flex items-center gap-3 p-1 pl-3 border border-ng hover:border-cyan-ng/50 transition-all group"
              style="clip-path: var(--ng-clip-card-sm)"
            >
              <span class="text-xs font-bold uppercase tracking-wider text-ng-secondary hidden md:block">
                {authState.user?.name.split(" ")[0]}
              </span>
              <div class="w-7 h-7 bg-ng-bg-elevated border border-ng flex items-center justify-center text-ng-secondary group-hover:bg-cyan-ng group-hover:text-black transition-colors" style="clip-path: var(--ng-clip-card-sm)">
                <User size={14} />
              </div>
            </button>

            <Show when={showUserMenu()}>
              <div class="absolute right-0 mt-2 w-64 bg-ng-bg-surface border border-ng shadow-2xl overflow-hidden z-110" style="clip-path: var(--ng-clip-card)">
                <div class="p-4 bg-ng-bg-deep/50 border-b border-ng">
                  <div class="text-sm font-bold text-white font-data">
                    {authState.user?.name}
                  </div>
                  <div class="text-xs text-ng-muted font-data">
                    {authState.user?.email}
                  </div>
                </div>
                <div class="p-2">
                  <button class="w-full text-left px-3 py-2 text-sm text-ng-secondary hover:bg-ng-bg-hover hover:text-ng-primary transition-colors flex items-center gap-2">
                    <User size={14} /> Profile Settings
                  </button>
                  <button class="w-full text-left px-3 py-2 text-sm text-ng-secondary hover:bg-ng-bg-hover hover:text-ng-primary transition-colors flex items-center gap-2">
                    <Settings size={14} /> System Preferences
                  </button>
                  <div class="h-px bg-ng-border my-2"></div>
                  <button
                    onClick={() => authActions.logout()}
                    class="w-full text-left px-3 py-2 text-sm text-offline hover:bg-[--ng-offline-glow] transition-colors flex items-center gap-2"
                  >
                    <LogOut size={14} /> Sign Out
                  </button>
                </div>
              </div>
            </Show>
          </div>

          {/* Mobile Profile Icon */}
          <div class="sm:hidden relative">
            <button
              onClick={() => {
                setShowUserMenu(!showUserMenu());
                setShowOrgMenu(false);
                setShowHubMenu(false);
              }}
              class="w-7 h-7 bg-ng-bg-elevated border border-ng flex items-center justify-center text-ng-secondary hover:border-cyan-ng/50 transition-colors"
              style="clip-path: var(--ng-clip-card-sm)"
            >
              <User size={14} />
            </button>
            <Show when={showUserMenu()}>
              <div class="absolute right-0 mt-2 w-64 bg-ng-bg-surface border border-ng shadow-2xl overflow-hidden z-110" style="clip-path: var(--ng-clip-card)">
                <div class="p-4 bg-ng-bg-deep/50 border-b border-ng">
                  <div class="text-sm font-bold text-white font-data">
                    {authState.user?.name}
                  </div>
                  <div class="text-xs text-ng-muted font-data">
                    {authState.user?.email}
                  </div>
                </div>
                <div class="p-2">
                  <button
                    onClick={() => authActions.logout()}
                    class="w-full text-left px-3 py-2 text-sm text-offline hover:bg-[--ng-offline-glow] transition-colors flex items-center gap-2"
                  >
                    <LogOut size={14} /> Sign Out
                  </button>
                </div>
              </div>
            </Show>
          </div>
        </div>
      </div>

      {/* Mobile Menu Overlay */}
      <Show when={isMobileMenuOpen()}>
        <div class="md:hidden absolute top-14 left-0 w-full bg-ng-bg-deep border-b border-ng p-4 flex flex-col gap-2 shadow-2xl z-90">
          <NavLink href="/" icon={<LayoutDashboard size={14} />}>
            Overview
          </NavLink>
          <NavLink href="/agents" icon={<Users size={14} />}>
            Agents
          </NavLink>
          <NavLink href="/nodes" icon={<Cpu size={14} />}>
            Compute
          </NavLink>
          <div class="h-px bg-ng-border my-2"></div>

          {/* Mobile Hub Manager */}
          <div class="flex items-center justify-between px-3 py-2 text-ng-secondary">
            <span class="text-xs font-bold uppercase tracking-wider flex items-center gap-2 font-data"><Server size={14}/> Hub Connection</span>
            <button
              onClick={() => setShowHubMenu(!showHubMenu())}
              class="text-[10px] font-bold uppercase tracking-wider bg-ng-bg-elevated px-2 py-1 text-ng-primary border border-ng"
              style="clip-path: var(--ng-clip-card-sm)"
            >
              {showHubMenu() ? "Close" : "Manage"}
            </button>
          </div>
          <Show when={showHubMenu()}>
            <div class="px-3 pb-2">
              <HubManager onClose={() => setShowHubMenu(false)} />
            </div>
          </Show>

          {/* Mobile Org Switcher */}
          <div class="px-3 py-2">
            <div class="text-[9px] font-bold text-ng-muted uppercase tracking-widest font-data mb-2">Organization</div>
            <For each={authState.availableOrganizations}>
              {(org) => (
                <button
                  onClick={() => {
                    authActions.switchOrganization(org.id);
                    setIsMobileMenuOpen(false);
                  }}
                  class={`w-full text-left px-3 py-2 text-sm transition-colors flex items-center justify-between mb-1 ${
                    authState.organization?.id === org.id
                      ? "text-cyan-ng"
                      : "text-ng-secondary hover:bg-ng-bg-hover hover:text-ng-primary"
                  }`}
                >
                  {org.name}
                  <Show when={authState.organization?.id === org.id}>
                    <div class="w-1.5 h-1.5 bg-cyan-ng" style="clip-path: var(--ng-clip-badge)"></div>
                  </Show>
                </button>
              )}
            </For>
          </div>
        </div>
      </Show>
    </nav>
  );
}
