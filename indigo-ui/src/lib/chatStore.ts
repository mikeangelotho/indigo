import { createStore } from "solid-js/store";
import { createEffect } from "solid-js";
import { generateUUID } from "./utils";

export interface Attachment {
  id: string;
  name: string;
  type: "image" | "file";
  url: string; // Base64 or Blob URL
}

// Multimodal content types
export type TextPart = { type: 'text'; text: string };
export type ImagePart = { type: 'image_url'; image_url: { url: string } };
export type MessageContent = string | (TextPart | ImagePart)[];

export interface ToolCall {
  id: string;
  function_name: string;
  arguments: string;
  result?: string;
  status?: "pending" | "executing" | "completed" | "error";
}

export interface Message {
  id: string;
  role: "user" | "assistant" | "system";
  content: MessageContent;
  attachments?: Attachment[];
  timestamp: number;
  toolCalls?: ToolCall[];
}

export interface Conversation {
  id: string;
  agentId: string;
  title: string;
  messages: Message[];
  createdAt: number;
  updatedAt: number;
  systemPrompt?: string; // Optional override
}

interface ChatState {
  conversations: Conversation[];
  activeConversationId: string | null;
}

const STORAGE_KEY = "indigo-chat-history";

function loadState(): ChatState {
  if (typeof window === "undefined") return { conversations: [], activeConversationId: null };
  const stored = localStorage.getItem(STORAGE_KEY);
  if (stored) {
    try {
      return JSON.parse(stored);
    } catch (e) {
      console.error("Failed to parse chat history", e);
    }
  }
  return { conversations: [], activeConversationId: null };
}

export const [chatStore, setChatStore] = createStore<ChatState>(loadState());

// Persistence Effect
createEffect(() => {
  if (typeof window !== "undefined") {
    try {
      const stateToSave = {
        ...chatStore,
        conversations: chatStore.conversations.map((c) => ({
          ...c,
          messages: c.messages.map((m) => ({
            ...m,
            // Sanitize content array if it has base64 images
            content: Array.isArray(m.content)
              ? m.content.map((part) => {
                  if (
                    part.type === "image_url" &&
                    part.image_url.url.startsWith("data:")
                  ) {
                    return {
                      ...part,
                      image_url: { ...part.image_url, url: "" }, // Don't save base64 to local storage
                    };
                  }
                  return part;
                })
              : m.content,
            // Sanitize attachments array
            attachments: m.attachments?.map((a) => {
              if (a.url.startsWith("data:")) {
                return { ...a, url: "" };
              }
              return a;
            }),
          })),
        })),
      };
      localStorage.setItem(STORAGE_KEY, JSON.stringify(stateToSave));
    } catch (e) {
      console.error("Failed to save chat history (Quota Exceeded?)", e);
    }
  }
});

export const chatActions = {
  createConversation: (agentId: string, title: string = "New Chat") => {
    const id = generateUUID();
    const newConv: Conversation = {
      id,
      agentId,
      title,
      messages: [],
      createdAt: Date.now(),
      updatedAt: Date.now(),
    };
    setChatStore("conversations", (prev) => [newConv, ...prev]);
    setChatStore("activeConversationId", id);
    return id;
  },

  selectConversation: (id: string) => {
    setChatStore("activeConversationId", id);
  },

  addMessage: (conversationId: string, message: Omit<Message, "id" | "timestamp">) => {
    const msg: Message = {
      ...message,
      id: generateUUID(),
      timestamp: Date.now(),
    };
    
    setChatStore("conversations", (c) => c.id === conversationId, "messages", (msgs) => [...msgs, msg]);
    setChatStore("conversations", (c) => c.id === conversationId, "updatedAt", Date.now());
    
    // Update title if it's the first user message
    const conv = chatStore.conversations.find(c => c.id === conversationId);
    if (conv && conv.messages.length === 1 && message.role === "user") {
        let newTitle = "New Chat";
        if (typeof message.content === 'string') {
            newTitle = message.content.slice(0, 30);
        } else if (Array.isArray(message.content)) {
            const textPart = message.content.find(p => p.type === 'text') as TextPart | undefined;
            if (textPart) {
                newTitle = textPart.text.slice(0, 30);
            } else {
                newTitle = "Image Message";
            }
        }
        if (newTitle.length === 30) newTitle += "...";
        setChatStore("conversations", (c) => c.id === conversationId, "title", newTitle);
    }
    
    return msg.id;
  },

  updateMessageContent: (conversationId: string, messageId: string, newContent: string) => {
    setChatStore(
      "conversations",
      (c) => c.id === conversationId,
      "messages",
      (m) => m.id === messageId,
      "content",
      newContent
    );
  },
  
  deleteConversation: (id: string) => {
      setChatStore("conversations", (prev) => prev.filter(c => c.id !== id));
      if (chatStore.activeConversationId === id) {
          setChatStore("activeConversationId", null);
      }
  },

  clearHistory: (agentId: string) => {
      setChatStore("conversations", (prev) => prev.filter(c => c.agentId !== agentId));
  },

  addToolCall: (conversationId: string, messageId: string, toolCall: Omit<ToolCall, "id">) => {
    const newToolCall: ToolCall = {
      ...toolCall,
      id: generateUUID(),
    };
    
    setChatStore(
      "conversations",
      (c) => c.id === conversationId,
      "messages",
      (m) => m.id === messageId,
      "toolCalls",
      (existing = []) => [...existing, newToolCall]
    );
    
    setChatStore("conversations", (c) => c.id === conversationId, "updatedAt", Date.now());
  },

  updateToolCall: (conversationId: string, messageId: string, toolCallId: string, updates: Partial<ToolCall>) => {
    setChatStore(
      "conversations",
      (c) => c.id === conversationId,
      "messages",
      (m) => m.id === messageId,
      "toolCalls",
      (toolCalls = []) => toolCalls?.map(tc => 
        tc.id === toolCallId ? { ...tc, ...updates } : tc
      )
    );
    
    setChatStore("conversations", (c) => c.id === conversationId, "updatedAt", Date.now());
  },
};