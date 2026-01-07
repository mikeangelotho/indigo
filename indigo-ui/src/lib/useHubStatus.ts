import { createSignal, createEffect } from "solid-js";
import { settingsStore } from "./settingsStore";

// Helper function to check if a hub is responsive
async function checkHubAvailability(url: string): Promise<boolean> {
  try {
    const response = await fetch(`${url}/nodes`, {
      method: 'GET',
      signal: AbortSignal.timeout(2000) // 2 second timeout
    });
    return response.ok;
  } catch {
    return false;
  }
}

export function useHubStatus() {
  const [isHubOnline, setIsHubOnline] = createSignal<boolean | null>(null);
  const [checkingHub, setCheckingHub] = createSignal(false);

  const checkHubStatus = async () => {
    setCheckingHub(true);
    try {
      const status = await checkHubAvailability(settingsStore.activeHub);
      setIsHubOnline(status);
    } catch {
      setIsHubOnline(false);
    } finally {
      setCheckingHub(false);
    }
  };

  // Check status when hub changes
  createEffect(() => {
    if (settingsStore.activeHub) {
      checkHubStatus();
    }
  });

  return { isHubOnline, checkingHub, checkHubStatus };
}