import { For, Show } from "solid-js";
import { Conversation } from "~/lib/chatStore";
import { MessageSquare, Trash2, Plus, MessageCircle } from "lucide-solid";

interface ChatSidebarProps {
  conversations: Conversation[];
  activeId: string | null;
  onSelect: (id: string) => void;
  onDelete: (id: string) => void;
  onNew: () => void;
}

export default function ChatSidebar(props: ChatSidebarProps) {
  return (
    <div class="w-64 bg-zinc-950 border-r border-zinc-800 flex flex-col h-full">
      <div class="p-4 border-b border-zinc-800">
        <button
          onClick={props.onNew}
          class="w-full flex items-center justify-center gap-2 bg-zinc-800 hover:bg-zinc-700 text-white py-2 px-4 rounded-md transition-colors font-medium text-sm"
        >
          <Plus size={16} />
          New Chat
        </button>
      </div>
      
      <div class="flex-1 overflow-y-auto py-2">
        <Show 
            when={props.conversations.length > 0}
            fallback={
                <div class="px-4 py-8 text-center text-zinc-600 text-sm">
                    No history yet.
                </div>
            }
        >
            <div class="px-2 space-y-1">
                <For each={props.conversations}>
                {(conv) => (
                    <div class="group relative">
                        <button
                            onClick={() => props.onSelect(conv.id)}
                            class={`w-full text-left px-3 py-2 rounded-md text-sm transition-colors flex items-center gap-2 pr-8 ${
                                props.activeId === conv.id
                                ? "bg-zinc-800 text-white"
                                : "text-zinc-400 hover:bg-zinc-900 hover:text-zinc-200"
                            }`}
                        >
                            <MessageSquare size={14} class="shrink-0 opacity-70" />
                            <span class="truncate">{conv.title || "New Chat"}</span>
                        </button>
                        <button
                            onClick={(e) => {
                                e.stopPropagation();
                                props.onDelete(conv.id);
                            }}
                            class="absolute right-2 top-1/2 -translate-y-1/2 opacity-0 group-hover:opacity-100 text-zinc-500 hover:text-red-400 p-1 rounded"
                            title="Delete Chat"
                        >
                            <Trash2 size={12} />
                        </button>
                    </div>
                )}
                </For>
            </div>
        </Show>
      </div>
    </div>
  );
}
