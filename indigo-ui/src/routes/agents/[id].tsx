import {
  createSignal,
  createEffect,
  onMount,
  onCleanup,
  Show,
  For,
  createResource,
} from "solid-js";
import { useParams, useNavigate } from "@solidjs/router";
import { chatStore, chatActions, Attachment, ImagePart, MessageContent } from "~/lib/chatStore";
import { settingsStore } from "~/lib/settingsStore";
import ChatSidebar from "~/components/Chat/ChatSidebar";
import MessageItem from "~/components/Chat/MessageItem";
import InputArea from "~/components/Chat/InputArea";
import {
  Settings,
  Download,
  Cpu,
  TriangleAlert,
  EllipsisVertical,
  Menu,
  X,
} from "lucide-solid";

interface AgentConfig {
  id: string;
  name: string;
  system_prompt: string;
  model: string;
  status: "Online" | "Offline";
}

interface ResourceData {
  agent: AgentConfig;
  availableModels: string[];
}

const fetchAgentData = async (args: {
  id: string;
  hubUrl: string;
}): Promise<ResourceData> => {
  const { id, hubUrl } = args;
  const [agentRes, nodesRes] = await Promise.all([
    fetch(`${hubUrl}/agents/${id}`),
    fetch(`${hubUrl}/nodes`),
  ]);

  if (!agentRes.ok) throw new Error("Agent not found");
  const agent = await agentRes.json();
  const nodes = await nodesRes.json();

  const availableModels = [
    ...new Set(nodes.map((n: any) => n.model_name).filter(Boolean)),
  ] as string[];

  return { agent: agent as AgentConfig, availableModels };
};

export default function AgentDetail() {
  const params = useParams();
  const navigate = useNavigate();

  // Guarded resource: only runs when id and hubUrl are truthy
  const [data, { refetch }] = createResource(() => {
    const id = params.id;
    const hubUrl = settingsStore.activeHub;
    if (!id || !hubUrl) return undefined;
    return { id, hubUrl };
  }, fetchAgentData);

  const [activeTab, setActiveTab] = createSignal<"chat" | "settings">("chat");
  const [loading, setLoading] = createSignal(false);
  const [editForm, setEditForm] = createSignal<Partial<AgentConfig>>({});
  const [isSidebarOpen, setIsSidebarOpen] = createSignal(false);

  // Update form when data loads
  createEffect(() => {
    const res = data.latest;
    if (res) setEditForm(res.agent);
  });

  let ws: WebSocket | null = null;
  let scrollContainer: HTMLDivElement | undefined;

  onMount(() => {
    const recent = chatStore.conversations.find((c) => c.agentId === params.id);
    if (recent) {
      chatActions.selectConversation(recent.id);
    } else {
      chatActions.createConversation(params.id as string);
    }
  });

  const currentConversation = () =>
    chatStore.conversations.find(
      (c) => c.id === chatStore.activeConversationId
    );

  const agentConversations = () =>
    chatStore.conversations
      .filter((c) => c.agentId === params.id)
      .sort((a, b) => b.updatedAt - a.updatedAt);

  createEffect(() => {
    // Track dependencies: messages array and the content of the last message
    // This ensures we scroll not just on new messages, but as the last message streams in.
    const conv = currentConversation();
    if (conv?.messages.length) {
      // Accessing the content property of the last message creates a subscription
      conv.messages[conv.messages.length - 1].content;
    }

    if (scrollContainer) {
      scrollContainer.scrollTop = scrollContainer.scrollHeight;
    }
  });

  const handleSendMessage = (text: string, attachments?: Attachment[]) => {
    if (!currentConversation() || loading()) return;

    const convId = currentConversation()!.id;
    let messageContent: MessageContent = text;

    // If there are attachments, build the multimodal content array
    if (attachments && attachments.length > 0) {
        const imageParts: ImagePart[] = attachments
            .filter(att => att.type === 'image')
            .map(att => ({
                type: 'image_url',
                image_url: { url: att.url }
            }));
        
        messageContent = [{ type: 'text', text }];
        if (imageParts.length > 0) {
            messageContent.push(...imageParts);
        }
    }

    chatActions.addMessage(convId, { role: "user", content: messageContent, attachments });
    streamResponse(convId);
  };

  const handleEditMessage = (index: number, newContent: string) => {
    const conv = currentConversation();
    if (!conv) return;
    const msgs = conv.messages;
    chatActions.updateMessageContent(conv.id, msgs[index].id, newContent);
    streamResponse(conv.id);
  };

  const streamResponse = (convId: string) => {
    const conv = chatStore.conversations.find((c) => c.id === convId);
    if (!conv) return;

    setLoading(true);
    const assistantMsgId = chatActions.addMessage(convId, {
      role: "assistant",
      content: "",
    });

    let wsUrl = "";
    try {
      const hubUrl = new URL(settingsStore.activeHub);
      const wsProtocol = hubUrl.protocol === "https:" ? "wss:" : "ws:";
      wsUrl = `${wsProtocol}//${hubUrl.host}/ws`;
    } catch (e) {
      chatActions.updateMessageContent(
        convId,
        assistantMsgId,
        "Error: Invalid Hub Configuration."
      );
      setLoading(false);
      return;
    }

    ws = new WebSocket(wsUrl);
    
    // Set initial "connecting" state immediately
    chatActions.updateMessageContent(
      convId,
      assistantMsgId,
      "..."
    );
    
    ws.onopen = async () => {
      const history = conv.messages
        .filter((m) => m.id !== assistantMsgId)
        .map((m) => {
            // The content is now already in the correct format for the API
            return { role: m.role, content: m.content };
        });

      // Fetch available tools from the hub (web chat context)
      let availableTools = [];
      try {
        const toolsResponse = await fetch(`${settingsStore.activeHub}/v1/tools?context=web`);
        if (toolsResponse.ok) {
          availableTools = await toolsResponse.json();
        }
      } catch (error) {
        console.error("Failed to fetch tools:", error);
      }

      ws?.send(
        JSON.stringify({
          prompt: "",
          messages: history,
          max_tokens: 2048,
          temperature: 0.7,
          agent_id: params.id,
          context: "web",
          tools: availableTools,
        })
      );
    };

    ws.onmessage = (event) => {
      try {
        const msgData = JSON.parse(event.data);
        
        // Handle token content
        if (msgData.token) {
          const currentContent =
            chatStore.conversations
              .find((c) => c.id === convId)
              ?.messages.find((m) => m.id === assistantMsgId)?.content || "";
          
          // Clear initial "..." state on first real token
          const updatedContent = currentContent === "..." ? msgData.token : currentContent + msgData.token;
          
          chatActions.updateMessageContent(
            convId,
            assistantMsgId,
            updatedContent
          );
        }
        
        // Handle ToolCall status messages
        if (typeof msgData.status === "object" && "ToolCall" in msgData.status) {
          const toolCall = msgData.status.ToolCall;
          console.log("ToolCall detected:", toolCall);
          
          // Add or update tool call
          const conversation = chatStore.conversations.find(c => c.id === convId);
          const message = conversation?.messages.find(m => m.id === assistantMsgId);
          
          if (message) {
            // Check if this tool call already exists
            const existingToolCall = message.toolCalls?.find(tc => 
              tc.function_name === toolCall.function_name && 
              tc.arguments === toolCall.arguments_json
            );
            
            if (existingToolCall) {
              // Update existing tool call
              chatActions.updateToolCall(convId, assistantMsgId, existingToolCall.id, {
                result: msgData.token || "",
                status: "completed"
              });
            } else {
              // Add new tool call
              chatActions.addToolCall(convId, assistantMsgId, {
                function_name: toolCall.function_name,
                arguments: toolCall.arguments_json,
                result: msgData.token || "",
                status: "completed"
              });
            }
          }
        }
        
        // Handle completion or error
        if (
          msgData.status === "Success" ||
          (typeof msgData.status === "object" && "Error" in msgData.status)
        ) {
          ws?.close();
        }
      } catch (err) {
        console.error("WebSocket message error:", err);
      }
    };

    ws.onerror = () => {
      setLoading(false);
      chatActions.updateMessageContent(
        convId,
        assistantMsgId,
        "Error: Connection failed."
      );
    };

    ws.onclose = () => setLoading(false);
  };

  onCleanup(() => {
    if (ws) ws.close();
  });

  const exportChat = () => {
    const conv = currentConversation();
    if (!conv) return;
    const text = conv.messages
      .map((m) => `[${m.role.toUpperCase()}]: ${m.content}`)
      .join("\n\n");
    const blob = new Blob([text], { type: "text/plain" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `${
      data.latest?.agent.name || "chat"
    }-${new Date().toISOString()}.txt`;
    a.click();
  };

  const updateAgent = async () => {
    try {
      const res = await fetch(
        `${settingsStore.activeHub}/agents/${params.id}`,
        {
          method: "PUT",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify(editForm()),
        }
      );
      if (res.ok) {
        alert("Agent updated!");
        refetch();
      }
    } catch (err) {
      console.error(err);
    }
  };

  const deleteAgent = async () => {
    if (!confirm("Are you sure?")) return;
    await fetch(`${settingsStore.activeHub}/agents/${params.id}`, {
      method: "DELETE",
    });
    navigate("/agents");
  };

  return (
    <div class="flex h-[calc(100vh-56px)] overflow-hidden text-ng-primary">
      
      {/* Mobile Sidebar Overlay */}
      <Show when={isSidebarOpen()}>
        <div 
            class="fixed inset-0 z-40 bg-black/80 backdrop-blur-sm md:hidden"
            onClick={() => setIsSidebarOpen(false)}
        ></div>
        <div class="fixed inset-y-0 left-0 z-50 w-72 bg-ng-bg-deep border-r border-ng shadow-2xl transform transition-transform duration-300 md:hidden flex flex-col">
            <div class="p-3 flex justify-between items-center border-b border-ng">
                <span class="font-bold text-white">Conversations</span>
                <button onClick={() => setIsSidebarOpen(false)} class="text-zinc-400 hover:text-white">
                    <X size={20} />
                </button>
            </div>
             <ChatSidebar
                conversations={agentConversations()}
                activeId={chatStore.activeConversationId}
                onSelect={(id) => {
                    chatActions.selectConversation(id);
                    setIsSidebarOpen(false);
                }}
                onDelete={chatActions.deleteConversation}
                onNew={() => {
                    chatActions.createConversation(params.id as string);
                    setIsSidebarOpen(false);
                }}
            />
        </div>
      </Show>

      {/* Desktop Sidebar */}
      <div class="hidden md:block h-full">
         <ChatSidebar
            conversations={agentConversations()}
            activeId={chatStore.activeConversationId}
            onSelect={chatActions.selectConversation}
            onDelete={chatActions.deleteConversation}
            onNew={() => chatActions.createConversation(params.id as string)}
        />
      </div>

      <div class="flex-1 flex flex-col min-w-0 bg-ng-bg-deep">
        <header class="h-12 border-b border-ng flex items-center gap-3 justify-between px-4 sm:px-6 bg-ng-bg-deep/80 backdrop-blur z-20">
          <div class="flex items-center gap-4">
             {/* Mobile Sidebar Toggle */}
            <button
              onClick={() => setIsSidebarOpen(true)}
              class="md:hidden text-zinc-400 hover:text-white"
            >
              <Menu size={20} />
            </button>

            <span class="font-bold text-white text-base tracking-wider font-data truncate max-w-[150px] sm:max-w-xs">
              {data.latest?.agent.name || "Loading..."}
            </span>
            <div class="hidden sm:block h-4 w-px bg-ng-border"></div>
            <div class="hidden sm:flex items-center gap-2 text-xs font-data">
                          <Cpu size={14} class="text-ng-muted" />
                          <span class="text-cyan-ng">{data.latest?.agent.model}</span>
            </div>
            <Show when={data.latest?.agent}>
              <span
                class={`status-badge hidden sm:flex ${
                                  data.latest?.agent.status === "Online"
                                    ? "status-online"
                                    : "status-offline"
                                }`}
              >

                {data.latest?.agent.status}
              </span>
            </Show>
          </div>
          <div class="flex items-center gap-2">
            <button
              onClick={exportChat}
              class="p-2 text-zinc-400 hover:text-white rounded-lg hover:bg-zinc-800 transition-colors hidden sm:block"
              title="Export Chat"
            >
              <Download size={18} />
            </button>
            <button
              onClick={() =>
                setActiveTab(activeTab() === "chat" ? "settings" : "chat")
              }
              class={`p-2 transition-colors flex items-center gap-2 text-xs font-bold uppercase tracking-wider font-data ${
                              activeTab() === "settings"
                                ? "text-cyan-ng border border-cyan-ng/30"
                                : "text-ng-muted hover:text-ng-primary hover:border-ng-border-hover border border-transparent"
                            }`}
                            style={activeTab() === "settings" ? "clip-path: var(--ng-clip-card-sm)" : ""}
            >
              <Settings size={18} />
              <span class="hidden md:inline">Configure</span>
            </button>
          </div>
        </header>

        <Show when={activeTab() === "chat"}>
          <div class="flex-1 flex flex-col min-h-0 relative">
            <Show when={data.latest?.agent.status !== "Online" && !loading()}>
              <div class="bg-[--ng-offline-glow] border-b border-offline/20 p-3 flex items-center justify-center gap-3 text-offline text-xs font-data">
                <TriangleAlert size={16} />
                <span class="hidden sm:inline">This agent's model is currently offline.</span>
                <span class="sm:hidden">Model offline.</span>
                <button
                  onClick={() => setActiveTab("settings")}
                  class="underline hover:text-white font-medium"
                >
                  Switch
                </button>
              </div>
            </Show>

            <div
              ref={scrollContainer}
              class="flex-1 overflow-y-auto custom-scrollbar"
            >
              <div class="max-w-3xl flex flex-col mx-auto w-full pb-24 sm:pb-32 pt-4 sm:pt-8 px-2 sm:px-4">
                <Show when={currentConversation()?.messages.length === 0}>
                  <div class="flex flex-col items-center justify-center mt-20 text-zinc-500">
                    <div class="w-14 h-14 bg-ng-bg-surface flex items-center justify-center mb-5 border border-ng shadow-xl" style="clip-path: var(--ng-clip-card)">
                      <EllipsisVertical size={32} class="opacity-50" />
                    </div>
                    <h3 class="text-lg font-bold text-white font-data tracking-wider mb-2">
                                          Ready to Interact
                                        </h3>
                    <p class="max-w-md text-center text-sm text-zinc-400 leading-relaxed">
                      Ask{" "}
                      <span class="text-cyan-ng font-bold">
                        {data.latest?.agent.name}
                      </span>{" "}
                      anything.
                    </p>
                  </div>
                </Show>
                <For each={currentConversation()?.messages}>
                  {(msg, i) => (
                    <MessageItem
                      message={msg}
                      isStreaming={
                        loading() &&
                        i() === currentConversation()!.messages.length - 1 &&
                        msg.role === "assistant"
                      }
                      onEdit={(newContent) =>
                        handleEditMessage(i(), newContent)
                      }
                    />
                  )}
                </For>
              </div>
            </div>

            <div class="absolute bottom-0 left-0 w-full pt-12 pb-2 px-2 sm:px-4" style="background: linear-gradient(to top, var(--ng-bg-deep) 60%, transparent)">
              <InputArea
                onSend={handleSendMessage}
                loading={loading()}
                disabled={data.latest?.agent.status !== "Online"}
                placeholder={
                  data.latest?.agent.name
                    ? `Message ${data.latest?.agent.name}...`
                    : undefined
                }
                onStop={() => {
                  ws?.close();
                  setLoading(false);
                }}
              />
            </div>
          </div>
        </Show>

        <Show when={activeTab() === "settings"}>
          <div class="flex-1 overflow-y-auto p-6 md:p-12">
            <div class="max-w-2xl mx-auto space-y-8">
              <div class="space-y-2">
                <h2 class="text-lg font-bold text-white font-data tracking-wider">Agent Settings</h2>
                <p class="text-ng-secondary text-sm">
                  Configure how {data.latest?.agent.name} behaves.
                </p>
              </div>

              <div class="space-y-4">
                <div class="space-y-2">
                  <label class="label">Name</label>
                  <input
                    type="text"
                    class="w-full bg-zinc-900 border border-zinc-700 rounded-md px-4 py-2 focus:border-indigo-500 focus:outline-none"
                    value={editForm().name || ""}
                    onInput={(e) =>
                      setEditForm({
                        ...editForm(),
                        name: e.currentTarget.value,
                      })
                    }
                  />
                </div>

                <div class="space-y-2">
                  <label class="label">Model</label>
                  <select
                    class="w-full bg-zinc-900 border border-zinc-700 rounded-md px-4 py-2 focus:border-indigo-500 focus:outline-none"
                    value={editForm().model || ""}
                    onChange={(e) =>
                      setEditForm({
                        ...editForm(),
                        model: e.currentTarget.value,
                      })
                    }
                  >
                    <option value="" disabled>
                      Select a model...
                    </option>
                    <For each={data.latest?.availableModels}>
                      {(model) => <option value={model}>{model}</option>}
                    </For>
                    <Show
                      when={
                        editForm().model &&
                        !data.latest?.availableModels.includes(editForm().model!)
                      }
                    >
                      <option value={editForm().model} disabled>
                        {editForm().model} (Offline)
                      </option>
                    </Show>
                  </select>
                </div>

                <div class="space-y-2">
                  <label class="label">System Directive</label>
                  <textarea
                    class="input-field font-data text-sm min-h-[200px] resize-none"
                    value={editForm().system_prompt || ""}
                    onInput={(e) =>
                      setEditForm({
                        ...editForm(),
                        system_prompt: e.currentTarget.value,
                      })
                    }
                  />
                </div>
              </div>

              <div class="flex items-center justify-between pt-6 border-t border-ng">
                <button
                  onClick={deleteAgent}
                  class="btn-danger font-data text-xs uppercase tracking-wider"
                >
                  Delete Agent
                </button>
                <button
                  onClick={updateAgent}
                  class="btn-primary font-data text-xs uppercase tracking-wider"
                >
                  Save Changes
                </button>
              </div>
            </div>
          </div>
        </Show>
      </div>
    </div>
  );
}
