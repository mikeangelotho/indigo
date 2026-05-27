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
    <div class="p-2 sm:p-4 bg-ng-bg-deep border-t border-ng">
      <div class="max-w-3xl mx-auto relative">
        {/* Attachment Previews */}
        <Show when={attachments().length > 0}>
          <div class="flex gap-3 mb-3 overflow-x-auto pb-2 custom-scrollbar snap-x">
            <For each={attachments()}>
              {(att) => (
                <div class="relative group shrink-0 w-18 h-18 bg-ng-bg-elevated border border-ng overflow-hidden flex flex-col items-center justify-center snap-start" style="clip-path: var(--ng-clip-card-sm)">
                  <Show
                    when={att.type === "image"}
                    fallback={<File size={20} class="text-ng-secondary" />}
                  >
                    <img
                      src={att.url}
                      alt={att.name}
                      class="w-full h-full object-cover"
                    />
                  </Show>
                  <button
                    onClick={() => removeAttachment(att.id)}
                    class="absolute top-1 right-1 p-0.5 bg-ng-bg-deep/80 hover:bg-offline text-ng-secondary hover:text-white opacity-0 group-hover:opacity-100 transition-all"
                    style="clip-path: var(--ng-clip-badge)"
                  >
                    <X size={10} />
                  </button>
                  <Show when={att.type === "file"}>
                    <span class="text-[7px] text-ng-muted px-1 truncate w-full text-center mt-1 font-data">
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
            class="shrink-0 p-3 mb-1 text-ng-secondary hover:text-cyan-ng transition-all active:scale-95 border border-ng"
            style="clip-path: var(--ng-clip-card-sm)"
            title="Attach files"
            disabled={props.disabled}
          >
            <Paperclip size={18} />
          </button>

          <textarea
            ref={textareaRef}
            value={input()}
            onInput={handleInput}
            onKeyDown={handleKeyDown}
            placeholder={props.placeholder || "Message Indigo..."}
            rows={1}
            disabled={props.disabled}
            class="flex-1 bg-ng-bg-surface/60 border border-ng text-ng-primary py-3 px-4 focus:outline-none focus:border-cyan-ng/50 resize-none overflow-hidden min-h-[46px] max-h-50 leading-relaxed custom-scrollbar font-data text-sm"
            style="clip-path: var(--ng-clip-card)"
          />

          <div class="shrink-0 mb-1">
            {props.loading ? (
              <button
                onClick={props.onStop}
                class="p-3 bg-ng-bg-elevated text-ng-secondary hover:text-offline border border-ng transition-all active:scale-95"
                style="clip-path: var(--ng-clip-card-sm)"
                title="Stop generation"
              >
                <StopCircle size={18} />
              </button>
            ) : (
              <button
                onClick={() => handleSubmit()}
                disabled={
                  (!input().trim() && attachments().length === 0) ||
                  props.disabled
                }
                class="p-3 text-black disabled:opacity-50 disabled:cursor-not-allowed transition-all active:scale-95"
                style="clip-path: var(--ng-clip-card-sm); background: linear-gradient(135deg, var(--ng-cyan-dim), #0088aa)"
              >
                <Send size={18} />
              </button>
            )}
          </div>
        </div>

        <div class="text-center mt-2 hidden sm:block min-h-[15px]">
          <p class="text-[10px] font-data tracking-wider transition-colors duration-300 text-ng-muted">
            {props.loading ? (
                <span class="text-cyan-ng animate-pulse font-bold flex items-center justify-center gap-1.5">
                    <span class="w-1 h-1 bg-cyan-ng animate-bounce" style="clip-path: var(--ng-clip-badge)"></span>
                    Generating...
                </span>
            ) : (
                "AI can make mistakes. Verify important information."
            )}
          </p>
        </div>
      </div>
    </div>
  );
}
