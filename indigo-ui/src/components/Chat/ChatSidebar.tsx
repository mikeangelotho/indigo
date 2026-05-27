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
    <div class="w-64 bg-ng-bg-deep border-r border-ng flex flex-col h-full">
      <div class="p-3 border-b border-ng">
        <button
          onClick={props.onNew}
          class="btn-primary w-full flex items-center justify-center gap-2 text-xs py-2"
        >
          <Plus size={14} />
          New Chat
        </button>
      </div>

      <div class="flex-1 overflow-y-auto py-2">
        <Show
            when={props.conversations.length > 0}
            fallback={
                <div class="px-4 py-8 text-center text-ng-muted text-xs font-data">
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
                            class={`w-full text-left px-3 py-2 text-xs font-data transition-colors flex items-center gap-2 pr-8 ${
                                props.activeId === conv.id
                                ? "text-cyan-ng border border-cyan-ng/20"
                                : "text-ng-secondary hover:bg-ng-bg-hover hover:text-ng-primary border border-transparent"
                            }`}
                            style={props.activeId === conv.id ? "clip-path: var(--ng-clip-card-sm)" : ""}
                        >
                            <MessageSquare size={12} class="shrink-0 opacity-70" />
                            <span class="truncate">{conv.title || "New Chat"}</span>
                        </button>
                        <button
                            onClick={(e) => {
                                e.stopPropagation();
                                props.onDelete(conv.id);
                            }}
                            class="absolute right-2 top-1/2 -translate-y-1/2 opacity-0 group-hover:opacity-100 text-ng-muted hover:text-offline p-1"
                            title="Delete Chat"
                        >
                            <Trash2 size={11} />
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
