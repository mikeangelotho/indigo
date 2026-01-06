import { createSignal, createEffect, For, Show } from "solid-js";
import { Send, StopCircle, Paperclip, X, File, Image } from "lucide-solid";
import { Attachment } from "~/lib/chatStore";
import { generateUUID } from "~/lib/utils";

interface InputAreaProps {
  onSend: (text: string, attachments?: Attachment[]) => void;
  onStop?: () => void;
  disabled?: boolean;
  loading?: boolean;
  placeholder?: string;
}

export default function InputArea(props: InputAreaProps) {
  const [input, setInput] = createSignal("");
  const [attachments, setAttachments] = createSignal<Attachment[]>([]);
  let textareaRef: HTMLTextAreaElement | undefined;
  let fileInputRef: HTMLInputElement | undefined;

  const handleSubmit = (e?: Event) => {
    e?.preventDefault();
    if ((!input().trim() && attachments().length === 0) || props.disabled)
      return;

    props.onSend(input(), attachments());
    setInput("");
    setAttachments([]);
    if (textareaRef) {
      textareaRef.style.height = "auto";
    }
  };

  const handleInput = (e: InputEvent) => {
    const target = e.target as HTMLTextAreaElement;
    setInput(target.value);
    target.style.height = "auto";
    target.style.height = Math.min(target.scrollHeight, 200) + "px";
  };

  const handleKeyDown = (e: KeyboardEvent) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      handleSubmit();
    }
  };

  const handleFileSelect = async (e: Event) => {
    const files = (e.target as HTMLInputElement).files;
    if (!files) return;

    const newAttachments: Attachment[] = [];

    for (let i = 0; i < files.length; i++) {
      const file = files[i];
      const isImage = file.type.startsWith("image/");

      // Convert to Base64
      const reader = new FileReader();
      const result = await new Promise<string>((resolve) => {
        reader.onload = (e) => resolve(e.target?.result as string);
        reader.readAsDataURL(file);
      });

      newAttachments.push({
        id: generateUUID(),
        name: file.name,
        type: isImage ? "image" : "file",
        url: result,
      });
    }

    setAttachments([...attachments(), ...newAttachments]);
    if (fileInputRef) fileInputRef.value = "";
  };

  const removeAttachment = (id: string) => {
    setAttachments((prev) => prev.filter((a) => a.id !== id));
  };

  return (
    <div class="p-2 sm:p-4 bg-zinc-950 border-t border-zinc-800">
      <div class="max-w-3xl mx-auto relative">
        {/* Attachment Previews */}
        <Show when={attachments().length > 0}>
          <div class="flex gap-3 mb-3 overflow-x-auto pb-2 custom-scrollbar snap-x">
            <For each={attachments()}>
              {(att) => (
                <div class="relative group shrink-0 w-20 h-20 bg-zinc-800 rounded-lg border border-zinc-700 overflow-hidden flex flex-col items-center justify-center snap-start">
                  <Show
                    when={att.type === "image"}
                    fallback={<File size={24} class="text-zinc-400" />}
                  >
                    <img
                      src={att.url}
                      alt={att.name}
                      class="w-full h-full object-cover"
                    />
                  </Show>
                  <button
                    onClick={() => removeAttachment(att.id)}
                    class="absolute top-1 right-1 p-0.5 bg-black/50 hover:bg-red-500 rounded-full text-white opacity-0 group-hover:opacity-100 transition-all backdrop-blur active:scale-90"
                  >
                    <X size={12} />
                  </button>
                  <Show when={att.type === "file"}>
                    <span class="text-[8px] text-zinc-400 px-1 truncate w-full text-center mt-1">
                      {att.name}
                    </span>
                  </Show>
                </div>
              )}
            </For>
          </div>
        </Show>

        <div class="relative flex items-end gap-2">
          <input
            type="file"
            multiple
            class="hidden"
            ref={fileInputRef}
            onChange={handleFileSelect}
          />

          <button
            onClick={() => fileInputRef?.click()}
            class="shrink-0 p-3 mb-1 text-zinc-400 hover:text-white rounded-xl hover:bg-zinc-800 transition-all active:scale-95"
            title="Attach files"
            disabled={props.disabled}
          >
            <Paperclip size={20} />
          </button>

          <textarea
            ref={textareaRef}
            value={input()}
            onInput={handleInput}
            onKeyDown={handleKeyDown}
            placeholder={props.placeholder || "Message Indigo..."}
            rows={1}
            disabled={props.disabled}
            class="flex-1 bg-zinc-800/50 border border-zinc-700 text-white rounded-xl py-3 px-4 focus:outline-none focus:ring-2 focus:ring-indigo-500/50 focus:border-indigo-500 resize-none overflow-hidden min-h-[46px] max-h-50 leading-relaxed custom-scrollbar"
          />

          <div class="shrink-0 mb-1">
            {props.loading ? (
              <button
                onClick={props.onStop}
                class="p-3 bg-zinc-700 text-white rounded-xl hover:bg-red-500/80 transition-all active:scale-95 shadow-lg shadow-red-900/10"
                title="Stop generation"
              >
                <StopCircle size={20} />
              </button>
            ) : (
              <button
                onClick={() => handleSubmit()}
                disabled={
                  (!input().trim() && attachments().length === 0) ||
                  props.disabled
                }
                class="p-3 bg-indigo-600 text-white rounded-xl hover:bg-indigo-500 disabled:opacity-50 disabled:cursor-not-allowed transition-all active:scale-95 shadow-lg shadow-indigo-900/20"
              >
                <Send size={20} />
              </button>
            )}
          </div>
        </div>

        <div class="text-center mt-2 hidden sm:block min-h-[15px]">
          <p class="text-[10px] text-zinc-500 transition-colors duration-300">
            {props.loading ? (
                <span class="text-indigo-400 animate-pulse font-medium flex items-center justify-center gap-1.5">
                    <span class="w-1.5 h-1.5 bg-indigo-400 rounded-full animate-bounce"></span>
                    Generating response...
                </span>
            ) : (
                "AI can make mistakes. Please verify important information."
            )}
          </p>
        </div>
      </div>
    </div>
  );
}
