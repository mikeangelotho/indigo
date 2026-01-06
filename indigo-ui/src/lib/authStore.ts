import { createStore } from "solid-js/store";
import { generateUUID } from "./utils";

export interface User {
  id: string;
  name: string;
  email: string;
  avatar?: string;
  role: "admin" | "member";
}

export interface Organization {
  id: string;
  name: string;
  slug: string;
  plan: "free" | "pro" | "enterprise";
}

interface AuthState {
  user: User | null;
  organization: Organization | null;
  availableOrganizations: Organization[];
  isAuthenticated: boolean;
}

// Mock initial state for groundwork
const initialId = generateUUID();
const [authState, setAuthState] = createStore<AuthState>({
  user: {
    id: "user-1",
    name: "System Architect",
    email: "architect@indigo.network",
    role: "admin"
  },
  organization: {
    id: "org-1",
    name: "Indigo Core Team",
    slug: "indigo-core",
    plan: "enterprise"
  },
  availableOrganizations: [
    { id: "org-1", name: "Indigo Core Team", slug: "indigo-core", plan: "enterprise" },
    { id: "org-2", name: "Personal Lab", slug: "personal", plan: "pro" }
  ],
  isAuthenticated: true
});

export { authState };

export const authActions = {
  switchOrganization: (orgId: string) => {
    const org = authState.availableOrganizations.find(o => o.id === orgId);
    if (org) {
      setAuthState("organization", org);
    }
  },
  logout: () => {
    setAuthState("isAuthenticated", false);
    setAuthState("user", null);
  }
};
