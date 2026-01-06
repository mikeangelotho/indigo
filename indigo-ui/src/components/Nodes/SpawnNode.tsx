import {
  createSignal,
  Show,
  For,
  createEffect,
  JSXElement,
  Setter,
  Accessor,
} from "solid-js";
import { Power, Play, Activity } from "lucide-solid";
import { settingsStore } from "~/lib/settingsStore";
import type { ModelInfo } from "~/lib/types";

interface SpawnNodeProps {
  activeHostModels: () => string[];
  models: () => ModelInfo[] | undefined;
  refetchNodes: () => void;
}

export default function SpawnNode(props: SpawnNodeProps) {
  const [isSpawning, setIsSpawning] = createSignal(false);
  const [mode, setMode] = createSignal<"local" | "hf" | "active">("local");

  const [spawnConfig, setSpawnConfig] = createSignal({
    port: 50052,
    model_repo: "",
    model_file: "",
    mmproj_file: "",
    gpu_layers: 0,
  });

  // Auto-select mmproj when model changes
  createEffect(() => {
    const modelFile = spawnConfig().model_file;
    if (!modelFile || !props.models()) {
      return;
    }

    // Heuristic: Extract the base model name by removing common quantization suffixes
    const baseName = modelFile
      .replace(/-[qf]\d{1,2}(_[a-z0-9_]{1,4})?\.gguf$/i, "")
      .replace(/\.gguf$/, "");

    // Find a corresponding mmproj file
    const mmprojFile = props
      .models()
      ?.find(
        (m) =>
          m.file.toLowerCase().includes("mmproj") && m.file.includes(baseName)
      );

    setSpawnConfig((prev) => {
      const newMmproj = mmprojFile ? mmprojFile.file : "";
      if (prev.mmproj_file === newMmproj) return prev;
      return {
        ...prev,
        mmproj_file: newMmproj,
      };
    });
  });

  const spawnNode = async (e: Event) => {
    e.preventDefault();
    setIsSpawning(true);

    try {
      const body: {
        port: number;
        gpu_layers: number;
        model_file?: string;
        model_repo?: string;
        mmproj_file?: string;
      } = {
        port: spawnConfig().port,
        gpu_layers: spawnConfig().gpu_layers,
      };

      if (mode() === "local") {
        body.model_file = spawnConfig().model_file;
        body.mmproj_file = spawnConfig().mmproj_file;
      }

      if (mode() === "hf") {
        body.model_repo = spawnConfig().model_repo;
        body.model_file = spawnConfig().model_file;
      }

      if (mode() === "active") {
        const selected = spawnConfig().model_file;
        if (selected.endsWith(".gguf")) {
          body.model_file = selected;
        } else {
          body.model_repo = selected;
        }
      }

      const res = await fetch(`${settingsStore.activeHub}/nodes`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(body),
      });

      if (!res.ok) throw new Error(await res.text());

      setSpawnConfig((prev) => ({
        ...prev,
        port: prev.port + 1,
        model_repo: "",
        model_file: "",
        mmproj_file: "",
      }));

      props.refetchNodes();
    } catch (err) {
      alert("Failed to spawn node: " + err);
    } finally {
      setIsSpawning(false);
    }
  };

  return (
    <div class="bg-zinc-900/50 backdrop-blur border border-zinc-800 rounded-xl p-6 sticky top-6 shadow-xl shadow-black/50">
      <div class="flex items-center gap-2 mb-6 text-indigo-400">
        <Power size={20} />
        <h2 class="text-lg font-semibold text-white">Initialize Node</h2>
      </div>

      {/* MODE SELECT */}
      <div class="flex bg-zinc-950 p-1 rounded-lg mb-6 border border-zinc-800">
        <button
          type="button"
          class={`flex-1 text-xs font-medium py-2 rounded-md transition-all ${
            mode() === "local"
              ? "bg-zinc-800 text-white shadow-sm"
              : "text-zinc-500 hover:text-zinc-300"
          }`}
          onClick={() => setMode("local")}
        >
          Local File
        </button>

        <button
          type="button"
          class={`flex-1 text-xs font-medium py-2 rounded-md transition-all ${
            mode() === "active"
              ? "bg-zinc-800 text-white shadow-sm"
              : "text-zinc-500 hover:text-zinc-300"
          }`}
          onClick={() => setMode("active")}
        >
          Clone
        </button>

        <button
          type="button"
          class={`flex-1 text-xs font-medium py-2 rounded-md transition-all ${
            mode() === "hf"
              ? "bg-zinc-800 text-white shadow-sm"
              : "text-zinc-500 hover:text-zinc-300"
          }`}
          onClick={() => setMode("hf")}
        >
          HuggingFace
        </button>
      </div>

      <form onSubmit={spawnNode} class="space-y-5">
        {/* PORT */}
        <div class="space-y-1.5">
          <label class="text-xs font-mono uppercase text-zinc-500">
            Port Allocation
          </label>
          <input
            type="number"
            required
            class="w-full bg-zinc-950 border border-zinc-800 rounded-lg px-4 py-2.5 text-white focus:border-indigo-500 outline-none transition-all font-mono"
            value={spawnConfig().port}
            onInput={(e) =>
              setSpawnConfig({
                ...spawnConfig(),
                port: parseInt(e.currentTarget.value),
              })
            }
          />
        </div>

        {/* LOCAL */}
        <Show when={mode() === "local"}>
          <div class="space-y-1.5">
            <label class="text-xs font-mono uppercase text-zinc-500">
              Model File
            </label>
            <select
              required
              class="w-full bg-zinc-950 border border-zinc-800 rounded-lg px-4 py-2.5 text-white focus:border-indigo-500 outline-none transition-all"
              value={spawnConfig().model_file}
              onChange={(e) =>
                setSpawnConfig({
                  ...spawnConfig(),
                  model_file: e.currentTarget.value,
                })
              }
            >
              <option value="" disabled>
                Select .gguf file...
              </option>
              <For each={props.models()}>
                {(m) => <option value={m.file}>{m.name}</option>}
              </For>
            </select>
          </div>

          <div class="space-y-1.5">
            <label class="text-xs font-mono uppercase text-zinc-500">
              Multimodal Projector (Optional)
            </label>
            <select
              class="w-full bg-zinc-950 border border-zinc-800 rounded-lg px-4 py-2.5 text-white focus:border-indigo-500 outline-none transition-all disabled:opacity-50 disabled:cursor-not-allowed"
              value={spawnConfig().mmproj_file}
              disabled={(() => {
                const m = spawnConfig().model_file.toLowerCase();
                return !(
                  m.includes("vision") ||
                  m.includes("llava") ||
                  m.includes("moondream") ||
                  m.includes("clip") ||
                  m.includes("omni") ||
                  m.includes("minicpm")
                );
              })()}
              onChange={(e) =>
                setSpawnConfig({
                  ...spawnConfig(),
                  mmproj_file: e.currentTarget.value,
                })
              }
            >
              <option value="">
                {(() => {
                  const m = spawnConfig().model_file.toLowerCase();
                  const isVision =
                    m.includes("vision") ||
                    m.includes("llava") ||
                    m.includes("moondream") ||
                    m.includes("clip") ||
                    m.includes("omni") ||
                    m.includes("minicpm");
                  return isVision
                    ? "None (Text-only)"
                    : "Not available for this model";
                })()}
              </option>
              <For
                each={props
                  .models()
                  ?.filter((m) => m.file.toLowerCase().includes("mmproj"))}
              >
                {(m) => <option value={m.file}>{m.name}</option>}
              </For>
            </select>
          </div>
        </Show>

        {/* ACTIVE */}
        <Show when={mode() === "active"}>
          <div class="space-y-1.5">
            <label class="text-xs font-mono uppercase text-zinc-500">
              Source Node
            </label>
            <select
              required
              class="w-full bg-zinc-950 border border-zinc-800 rounded-lg px-4 py-2.5 text-white focus:border-indigo-500 outline-none transition-all"
              value={spawnConfig().model_file}
              onChange={(e) =>
                setSpawnConfig({
                  ...spawnConfig(),
                  model_file: e.currentTarget.value,
                })
              }
            >
              <option value="" disabled>
                Select active model...
              </option>
              <For each={props.activeHostModels()}>
                {(m) => <option value={m}>{m}</option>}
              </For>
            </select>

            <Show when={props.activeHostModels().length === 0}>
              <p class="text-[10px] text-amber-500 flex items-center gap-1">
                <Activity size={10} />
                No local nodes running.
              </p>
            </Show>
          </div>
        </Show>

        {/* HUGGINGFACE */}
        <Show when={mode() === "hf"}>
          <div class="space-y-1.5">
            <label class="text-xs font-mono uppercase text-zinc-500">
              Repository ID
            </label>
            <input
              type="text"
              required
              class="w-full bg-zinc-950 border border-zinc-800 rounded-lg px-4 py-2.5 text-white focus:border-indigo-500 outline-none transition-all text-sm"
              placeholder="microsoft/Phi-3-mini-4k-instruct-gguf"
              value={spawnConfig().model_repo}
              onInput={(e) =>
                setSpawnConfig({
                  ...spawnConfig(),
                  model_repo: e.currentTarget.value,
                })
              }
            />
          </div>

          <div class="space-y-1.5">
            <label class="text-xs font-mono uppercase text-zinc-500">
              Filename
            </label>
            <input
              type="text"
              required
              class="w-full bg-zinc-950 border border-zinc-800 rounded-lg px-4 py-2.5 text-white focus:border-indigo-500 outline-none transition-all text-sm"
              placeholder="Phi-3-mini-4k-instruct-q4.gguf"
              value={spawnConfig().model_file}
              onInput={(e) =>
                setSpawnConfig({
                  ...spawnConfig(),
                  model_file: e.currentTarget.value,
                })
              }
            />
          </div>
        </Show>

        {/* GPU */}
        <div class="space-y-1.5">
          <label class="text-xs font-mono uppercase text-zinc-500">
            GPU Offload
          </label>
            <ToggleButton
              levels={["None", "Low", "High", "Max"]}
              selectedLevel="None"
              spawnConfig={{ get: spawnConfig, set: setSpawnConfig }}
            />
        </div>

        {/* SUBMIT */}
        <button
          type="submit"
          disabled={isSpawning()}
          class="w-full bg-indigo-600 hover:bg-indigo-500 disabled:opacity-50 disabled:cursor-not-allowed text-white font-medium py-3 rounded-lg shadow-lg shadow-indigo-900/20 transition-all flex items-center justify-center gap-2 group"
        >
          <Show when={!isSpawning()} fallback={<span>Initializing...</span>}>
            <Play
              size={18}
              class="group-hover:text-green-300 transition-colors"
            />
            Launch Node
          </Show>
        </button>
      </form>
    </div>
  );
}

interface ToggleButtonProps {
  levels: string[];
  selectedLevel: string;
  spawnConfig: {
    get: Accessor<{
      gpu_layers: number;
      port: number;
      model_repo: string;
      model_file: string;
      mmproj_file: string;
    }>;
    set: Setter<{
      gpu_layers: number;
      port: number;
      model_repo: string;
      model_file: string;
      mmproj_file: string;
    }>;
  };
  children?: JSXElement;
}

const ToggleButton = (props: ToggleButtonProps) => {
  const [level, setLevel] = createSignal(props.selectedLevel);

  createEffect(() => {
    const button = document.querySelector(
      'button[aria-controls="toggle-level"]'
    ) as HTMLButtonElement;
    if (button) {
      if (level() === props.selectedLevel) {
        button.classList.add("bg-indigo-900");
        button.classList.remove("bg-indigo-500/10");
      } else {
        button.classList.add("bg-indigo-500/10");
        button.classList.remove("bg-indigo-900");
      }
    }
  });

  return (
    <div class="flex justify-between items-center gap-3 flex-wrap">
      {props.levels.map((lvl) => (
        <button
          type="button"
          class={`px-4 py-2 rounded-lg font-medium transition-all duration-200 border ${
            level() === lvl
              ? "bg-indigo-900 text-white border-indigo-900"
              : "bg-indigo-500/10 text-gray-700 border-indigo-500/30"
          }`}
          onClick={() => {
            setLevel(lvl);
            let gpuLayers = 0;
            if (lvl === "Low") gpuLayers = 16;
            else if (lvl === "High") gpuLayers = 32;
            else if (lvl === "Max") gpuLayers = 64;
            
            props.spawnConfig.set({
              ...props.spawnConfig.get(),
              gpu_layers: gpuLayers,
            });
          }}
          aria-pressed={level() === lvl}
        >
          {lvl}
        </button>
      ))}
    </div>
  );
};
