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
        class={`flex items-center gap-2.5 px-3 py-2 rounded-lg text-sm font-medium transition-all ${
          isActive()
            ? "text-indigo-400 bg-indigo-500/5"
            : "text-zinc-400 hover:text-zinc-200 hover:bg-zinc-800/50"
        }`}
      >
        {props.icon}
        {props.children}
      </A>
    );
  };

  return (
    <nav class="border-b border-zinc-800 bg-black/80 backdrop-blur-xl sticky top-0 z-100">
      <div class="max-w-360 mx-auto px-6 h-16 flex items-center justify-between">
        <div class="flex items-center gap-10">
          <div class="flex items-center gap-3">
            {/* Mobile Menu Toggle */}
            <button
              onClick={() => setIsMobileMenuOpen(!isMobileMenuOpen())}
              class="md:hidden text-zinc-400 hover:text-white"
            >
              <Show when={isMobileMenuOpen()} fallback={<Menu size={24} />}>
                <X size={24} />
              </Show>
            </button>

            <A href="/" class="flex items-center gap-3 group">
              <div class="w-8 h-8 bg-linear-to-br from-indigo-500 to-cyan-500 rounded-lg flex items-center justify-center shadow-lg shadow-indigo-500/20 group-hover:scale-110 transition-transform">
                <Shield size={18} class="text-white" />
              </div>
              <span class="font-bold text-xl tracking-tight text-white hidden sm:inline-block">
                Indigo
              </span>
            </A>
          </div>

          <div class="hidden md:flex items-center gap-2">
            <NavLink href="/" icon={<LayoutDashboard size={18} />}>
              Overview
            </NavLink>
            <NavLink href="/agents" icon={<Users size={18} />}>
              Agents
            </NavLink>
            <NavLink href="/nodes" icon={<Cpu size={18} />}>
              Compute
            </NavLink>
          </div>
        </div>

        <div class="flex items-center gap-4 sm:gap-6">
          {/* Hub Manager Toggle */}
          <div class="relative hidden sm:block">
            <button
              onClick={() => {
                setShowHubMenu(!showHubMenu());
                setShowOrgMenu(false);
                setShowUserMenu(false);
              }}
              class={`flex items-center gap-2 px-3 py-1.5 rounded-lg border transition-all text-sm font-medium ${
                showHubMenu()
                  ? "bg-indigo-500/10 text-indigo-400 border-indigo-500/30"
                  : "border-zinc-800 bg-zinc-900/50 text-zinc-400 hover:text-white hover:border-zinc-700"
              }`}
              title="Manage Hub Connections"
            >
              <Server size={16} />
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
              class="flex items-center gap-2 px-3 py-1.5 rounded-lg border border-zinc-800 bg-zinc-900/50 hover:border-zinc-700 transition-all text-sm font-medium text-zinc-300"
            >
              <Building2 size={16} class="text-indigo-400" />
              <span>{authState.organization?.name}</span>
              <ChevronDown
                size={14}
                class={`transition-transform ${
                  showOrgMenu() ? "rotate-180" : ""
                }`}
              />
            </button>

            <Show when={showOrgMenu()}>
              <div class="absolute right-0 mt-2 w-56 bg-zinc-900 border border-zinc-800 rounded-xl shadow-2xl p-2 z-110">
                <div class="px-3 py-2 text-[10px] font-bold text-zinc-500 uppercase tracking-widest">
                  Select Organization
                </div>
                <For each={authState.availableOrganizations}>
                  {(org) => (
                    <button
                      onClick={() => {
                        authActions.switchOrganization(org.id);
                        setShowOrgMenu(false);
                      }}
                      class={`w-full text-left px-3 py-2 rounded-lg text-sm transition-colors flex items-center justify-between ${
                        authState.organization?.id === org.id
                          ? "bg-indigo-500/10 text-indigo-400"
                          : "text-zinc-400 hover:bg-zinc-800 hover:text-white"
                      }`}
                    >
                      {org.name}
                      <Show when={authState.organization?.id === org.id}>
                        <div class="w-1.5 h-1.5 rounded-full bg-indigo-500"></div>
                      </Show>
                    </button>
                  )}
                </For>
              </div>
            </Show>
          </div>

          <div class="h-6 w-px bg-zinc-800 hidden sm:block"></div>

          {/* User Menu */}
          <div class="relative hidden sm:block">
            <button
              onClick={() => {
                setShowUserMenu(!showUserMenu());
                setShowOrgMenu(false);
                setShowHubMenu(false);
              }}
              class="flex items-center gap-3 p-1 pl-3 rounded-full border border-zinc-800 bg-zinc-900/50 hover:border-indigo-500/50 transition-all group"
            >
              <span class="text-sm font-medium text-zinc-300 hidden md:block">
                {authState.user?.name.split(" ")[0]}
              </span>
              <div class="w-8 h-8 rounded-full bg-zinc-800 border border-zinc-700 flex items-center justify-center text-zinc-400 group-hover:bg-indigo-600 group-hover:text-white transition-colors">
                <User size={18} />
              </div>
            </button>

            <Show when={showUserMenu()}>
              <div class="absolute right-0 mt-2 w-64 bg-zinc-900 border border-zinc-800 rounded-xl shadow-2xl overflow-hidden z-110">
                <div class="p-4 bg-zinc-950/50 border-b border-zinc-800">
                  <div class="text-sm font-bold text-white">
                    {authState.user?.name}
                  </div>
                  <div class="text-xs text-zinc-500">
                    {authState.user?.email}
                  </div>
                </div>
                <div class="p-2">
                  <button class="w-full text-left px-3 py-2 rounded-lg text-sm text-zinc-400 hover:bg-zinc-800 hover:text-white transition-colors flex items-center gap-2">
                    <User size={16} /> Profile Settings
                  </button>
                  <button class="w-full text-left px-3 py-2 rounded-lg text-sm text-zinc-400 hover:bg-zinc-800 hover:text-white transition-colors flex items-center gap-2">
                    <Settings size={16} /> System Preferences
                  </button>
                  <div class="h-px bg-zinc-800 my-2"></div>
                  <button
                    onClick={() => authActions.logout()}
                    class="w-full text-left px-3 py-2 rounded-lg text-sm text-red-400 hover:bg-red-500/10 transition-colors flex items-center gap-2"
                  >
                    <LogOut size={16} /> Sign Out
                  </button>
                </div>
              </div>
            </Show>
          </div>
          
          {/* Mobile Profile Icon (Visible only on mobile) */}
             <div class="sm:hidden relative">
                <button
                onClick={() => {
                    setShowUserMenu(!showUserMenu());
                    setShowOrgMenu(false);
                    setShowHubMenu(false);
                }}
                 class="w-8 h-8 rounded-full bg-zinc-800 border border-zinc-700 flex items-center justify-center text-zinc-400 hover:bg-indigo-600 hover:text-white transition-colors"
                >
                    <User size={18} />
                </button>
                 <Show when={showUserMenu()}>
                  <div class="absolute right-0 mt-2 w-64 bg-zinc-900 border border-zinc-800 rounded-xl shadow-2xl overflow-hidden z-110">
                    <div class="p-4 bg-zinc-950/50 border-b border-zinc-800">
                      <div class="text-sm font-bold text-white">
                        {authState.user?.name}
                      </div>
                      <div class="text-xs text-zinc-500">
                        {authState.user?.email}
                      </div>
                    </div>
                     <div class="p-2">
                         <button
                            onClick={() => authActions.logout()}
                            class="w-full text-left px-3 py-2 rounded-lg text-sm text-red-400 hover:bg-red-500/10 transition-colors flex items-center gap-2"
                          >
                            <LogOut size={16} /> Sign Out
                          </button>
                     </div>
                  </div>
                </Show>
            </div>
        </div>
      </div>

      {/* Mobile Menu Overlay */}
      <Show when={isMobileMenuOpen()}>
        <div class="md:hidden absolute top-16 left-0 w-full bg-zinc-950 border-b border-zinc-800 p-4 flex flex-col gap-2 shadow-2xl animate-in slide-in-from-top-2 z-90">
          <NavLink href="/" icon={<LayoutDashboard size={18} />}>
            Overview
          </NavLink>
          <NavLink href="/agents" icon={<Users size={18} />}>
            Agents
          </NavLink>
          <NavLink href="/nodes" icon={<Cpu size={18} />}>
            Compute
          </NavLink>
          <div class="h-px bg-zinc-800 my-2"></div>
          
           {/* Mobile Hub Manager */}
           <div class="flex items-center justify-between px-3 py-2 text-zinc-400">
             <span class="text-sm font-medium flex items-center gap-2"><Server size={18}/> Hub Connection</span>
              <button
              onClick={() => {
                  setShowHubMenu(!showHubMenu());
              }}
              class="text-xs bg-zinc-800 px-2 py-1 rounded text-white"
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
             <div class="text-xs font-bold text-zinc-500 uppercase tracking-widest mb-2">Organization</div>
              <For each={authState.availableOrganizations}>
                  {(org) => (
                    <button
                      onClick={() => {
                        authActions.switchOrganization(org.id);
                        setIsMobileMenuOpen(false);
                      }}
                      class={`w-full text-left px-3 py-2 rounded-lg text-sm transition-colors flex items-center justify-between mb-1 ${
                        authState.organization?.id === org.id
                          ? "bg-indigo-500/10 text-indigo-400"
                          : "text-zinc-400 hover:bg-zinc-800 hover:text-white"
                      }`}
                    >
                      {org.name}
                      <Show when={authState.organization?.id === org.id}>
                        <div class="w-1.5 h-1.5 rounded-full bg-indigo-500"></div>
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
