import { createMemo, createEffect, Show, onMount, onCleanup } from "solid-js";
import { Message } from "~/lib/chatStore";
import { marked } from "marked";
import { User, Bot, Edit2, Copy, Check, File, BrainCircuit, ChevronDown } from "lucide-solid";
import { createSignal } from "solid-js";
import hljs from "highlight.js";
import "highlight.js/styles/atom-one-dark.css";
import { copyToClipboard } from "~/lib/clipboard";

interface MessageItemProps {
  message: Message;
  isStreaming?: boolean;
  onEdit?: (newContent: string) => void;
}

// SVG Icons as strings for the HTML renderer
const COPY_ICON_SVG = `<svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect width="14" height="14" x="8" y="8" rx="2" ry="2"/><path d="M4 16c-1.1 0-2-.9-2-2V4c0-1.1.9-2 2-2h10c1.1 0 2 .9 2 2"/></svg>`;
const CHECK_ICON_SVG = `<svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M20 6 9 17l-5-5"/></svg>`;

export default function MessageItem(props: MessageItemProps) {
  const [copied, setCopied] = createSignal(false);
  const [isEditing, setIsEditing] = createSignal(false);
  const [editContent, setEditContent] = createSignal("");
  const [displayedText, setDisplayedText] = createSignal("");
  const [isThinkingExpanded, setIsThinkingExpanded] = createSignal(false);
  let contentRef: HTMLDivElement | undefined;

  const getTextContent = (content: Message["content"]): string => {
    if (typeof content === "string") {
      return content;
    }
    if (Array.isArray(content)) {
      const textPart = content.find((p) => p.type === "text");
      return textPart ? (textPart as any).text : "";
    }
    return "";
  };

  // Smooth Streaming Logic
  createEffect(() => {
    const target = getTextContent(props.message.content);
    
    // If not streaming, or if we are editing, sync immediately
    if (!props.isStreaming || isEditing()) {
        setDisplayedText(target);
        return;
    }

    // If streaming, catch up smoothly
    const current = displayedText();
    
    if (current.length < target.length) {
         // Determine "catch up" speed
         const distance = target.length - current.length;
         // If far behind (e.g. paste or fast token generation), speed up.
         // Min speed 1 char/frame (~60 chars/sec). Max speed adaptive.
         const speed = distance > 50 ? 5 : (distance > 20 ? 3 : 1);
         
         let frameId = requestAnimationFrame(() => {
             setDisplayedText(target.slice(0, current.length + speed));
         });
         
         onCleanup(() => cancelAnimationFrame(frameId));
    } else if (current.length > target.length) {
        // If target shrunk (e.g. correction/reset), sync immediately
        setDisplayedText(target);
    }
  });

  const parseThought = (text: string) => {
      // Case-insensitive match for <think>, <thought>, or <reasoning>
      // Captures: 1=tag name, 2=content
      const regex = /<(think|thought|reasoning)>([\s\S]*?)(?:<\/\1>|$)/i;
      const match = text.match(regex);
      
      if (match) {
          const thought = match[2].trim();
          // Remove the match from the text to get the main content
          // We use match[0] to ensure we remove exactly what we matched
          const content = text.replace(match[0], "").trim();
          return { thought, content };
      }
      return { thought: null, content: text };
  };

  const parsedContent = createMemo(() => {
      return parseThought(displayedText());
  });

  const htmlContent = createMemo(() => {
    const { content } = parsedContent();
    // If we have thought but no content yet (streaming thought), return empty string for main content
    if (!content && props.isStreaming && parsedContent().thought) return "";
    
    // If no text at all, return empty
    if (!content && !parsedContent().thought && props.isStreaming) return "";
    
    let html = marked.parse(content, { breaks: true, gfm: true, async: false }) as string;
    
    const targetLength = getTextContent(props.message.content).length;
    const currentLength = displayedText().length;
    
    // Only show cursor if we are still streaming AND the content hasn't fully arrived
    if (props.isStreaming && content.length > 0 && currentLength < targetLength) {
        // Append cursor to the last element if it's a paragraph
        if (html.endsWith("</p>\n")) {
            html = html.replace(/<\/p>\n$/, '<span class="cursor-blink"></span></p>');
        } else if (html.endsWith("</p>")) {
             html = html.replace(/<\/p>$/, '<span class="cursor-blink"></span></p>');
        } else {
            html += '<span class="cursor-blink"></span>';
        }
    }
    return html;
  });

  // Post-process HTML for highlighting and copy buttons
  createEffect(() => {
    // Track htmlContent to re-run when it changes
    const html = htmlContent(); 
    
    // Use setTimeout to ensure DOM is updated by Solid's innerHTML binding before we query it
    // This is a microtask deferral to allow hydration/render to complete
    setTimeout(() => {
        if (!contentRef) return;

        // Find all pre codes
        const blocks = contentRef.querySelectorAll("pre code");
        blocks.forEach((block) => {
          // If already processed, skip
          if (block.classList.contains("hljs")) return;

          // Highlight
          hljs.highlightElement(block as HTMLElement);

          // Add wrapper and copy button
          const pre = block.parentElement as HTMLElement;
          if (pre.parentElement?.classList.contains("code-block-wrapper")) return; // Already processed

          // Create wrapper
          const wrapper = document.createElement("div");
          wrapper.className = "code-block-wrapper relative group my-4 rounded-lg overflow-hidden bg-[#282c34] text-base";
          
          // Header
          const header = document.createElement("div");
          header.className = "flex items-center justify-between px-4 py-2 bg-[#21252b] text-xs text-zinc-400 select-none";
          
          const langSpan = document.createElement("span");
          langSpan.className = "font-mono";
          // Try to get language from class
          const langClass = Array.from(block.classList).find(c => c.startsWith('language-'));
          langSpan.innerText = langClass ? langClass.replace('language-', '') : 'plaintext';
          
          const copyBtn = document.createElement("button");
          copyBtn.className = "copy-code-btn hover:text-white transition-colors cursor-pointer";
          copyBtn.innerHTML = COPY_ICON_SVG;
          
          // Store code in data attribute for easy access
          const codeText = block.textContent || "";
          copyBtn.dataset.code = encodeURIComponent(codeText);

          header.appendChild(langSpan);
          header.appendChild(copyBtn);
          wrapper.appendChild(header);

          // Wrap pre
          pre.parentNode?.insertBefore(wrapper, pre);
          wrapper.appendChild(pre);
          
          // Clean up pre class
          pre.classList.add("!m-0", "!p-0", "!bg-transparent");
          block.classList.add("!p-4", "block", "overflow-x-auto");
        });
    }, 0);
  });

  const handleCopy = async () => {
    const text = getTextContent(props.message.content);
    const success = await copyToClipboard(text);
    if (success) {
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    }
  };
  
  // ... rest of event handlers
  
  const handleContentClick = async (e: MouseEvent) => {
    // Handle code block copy buttons
    const target = (e.target as HTMLElement).closest(".copy-code-btn");
    if (target) {
      e.preventDefault();
      e.stopPropagation();
      
      const btn = target as HTMLElement;
      const encodedCode = btn.dataset.code;
      
      if (encodedCode) {
        const code = decodeURIComponent(encodedCode);
        const success = await copyToClipboard(code);

        if (success) {
          // Visual feedback
          const originalContent = btn.innerHTML;
          btn.innerHTML = CHECK_ICON_SVG;
          setTimeout(() => {
            btn.innerHTML = originalContent;
          }, 2000);
        }
      }
      return;
    }
  };

  const startEdit = () => {
    setEditContent(getTextContent(props.message.content));
    setIsEditing(true);
  };

  const saveEdit = () => {
    if (
      props.onEdit &&
      editContent() !== getTextContent(props.message.content)
    ) {
      props.onEdit(editContent());
    }
    setIsEditing(false);
  };

  return (
    <div
      class={`group flex gap-3 sm:gap-4 p-3 sm:p-6 ${
        props.message.role === "assistant" ? "bg-transparent" : "bg-zinc-900/30"
      } rounded-xl transition-colors hover:bg-zinc-900/40`}
    >
      <div class="shrink-0 flex flex-col items-center gap-2">
        <div
          class={`w-8 h-8 rounded-lg flex items-center justify-center shadow-lg ${
            props.message.role === "assistant"
              ? "bg-indigo-600 shadow-indigo-900/20"
              : "bg-zinc-700 shadow-black/20"
          }`}
        >
          {props.message.role === "assistant" ? (
            <Bot size={18} class="text-white" />
          ) : (
            <User size={18} class="text-zinc-300" />
          )}
        </div>
      </div>

      <div class="flex-1 min-w-0 overflow-hidden space-y-2 sm:space-y-3">
        <div class="flex items-center gap-2 mb-1">
          <span class="font-bold text-sm text-zinc-300">
            {props.message.role === "assistant" ? "Indigo" : "You"}
          </span>
          <span class="text-[10px] sm:text-xs text-zinc-600 font-mono">
            {new Date(props.message.timestamp).toLocaleTimeString([], {
              hour: "2-digit",
              minute: "2-digit",
            })}
          </span>
        </div>

        {/* Attachments Grid */}
        <Show
          when={
            props.message.attachments && props.message.attachments.length > 0
          }
        >
          <div class="flex flex-wrap gap-3">
            <For each={props.message.attachments}>
              {(att) => (
                <div class="relative group rounded-lg overflow-hidden border border-zinc-700 bg-zinc-800">
                  <Show
                    when={att.type === "image"}
                    fallback={
                      <div class="flex items-center gap-3 p-3 min-w-40">
                        <div class="p-2 bg-zinc-900 rounded text-indigo-400">
                          <File size={20} />
                        </div>
                        <div class="flex-1 min-w-0">
                          <div class="text-xs font-medium text-white truncate">
                            {att.name}
                          </div>
                          <div class="text-[10px] text-zinc-500 uppercase">
                            File
                          </div>
                        </div>
                      </div>
                    }
                  >
                    <img
                      src={att.url}
                      alt={att.name}
                      class="max-h-64 object-contain bg-black/20"
                    />
                  </Show>
                </div>
              )}
            </For>
          </div>
        </Show>

        {isEditing() ? (
          <div class="mt-2">
            <textarea
              class="w-full bg-zinc-950 border border-zinc-700 rounded-lg p-3 text-zinc-200 focus:border-indigo-500 focus:outline-none min-h-25 font-mono text-sm leading-relaxed"
              value={editContent()}
              onInput={(e) => setEditContent(e.currentTarget.value)}
            />
            <div class="flex gap-2 mt-2 justify-end">
              <button
                onClick={() => setIsEditing(false)}
                class="px-3 py-1.5 text-xs font-medium text-zinc-400 hover:text-white transition-colors"
              >
                Cancel
              </button>
              <button
                onClick={saveEdit}
                class="px-3 py-1.5 text-xs font-medium bg-indigo-600 text-white rounded hover:bg-indigo-500 transition-colors"
              >
                Save & Regenerate
              </button>
            </div>
          </div>
        ) : (
          <Show
            when={!props.isStreaming || displayedText().length > 0}
            fallback={
              <div class="flex gap-2.5 items-center py-3 pl-1">
                <div class="flex gap-1">
                    <div class="w-1.5 h-1.5 bg-indigo-500 rounded-full animate-bounce [animation-delay:-0.3s]"></div>
                    <div class="w-1.5 h-1.5 bg-indigo-500 rounded-full animate-bounce [animation-delay:-0.15s]"></div>
                    <div class="w-1.5 h-1.5 bg-indigo-500 rounded-full animate-bounce"></div>
                </div>
                <span class="text-xs font-medium text-indigo-400 uppercase tracking-widest animate-pulse">Thinking</span>
              </div>
            }
          >
             {/* Thinking Process Section */}
             <Show when={parsedContent().thought}>
                <div class="mb-4">
                    <button 
                        onClick={() => setIsThinkingExpanded(!isThinkingExpanded())}
                        class="flex items-center gap-2 text-xs font-medium text-zinc-500 hover:text-zinc-300 transition-colors bg-zinc-900/50 px-3 py-1.5 rounded-lg border border-zinc-800 hover:border-zinc-700 w-full sm:w-auto"
                    >
                        <BrainCircuit size={14} class={isThinkingExpanded() ? "text-indigo-400" : ""} />
                        <span>Thinking Process</span>
                        <ChevronDown 
                            size={14} 
                            class={`transition-transform duration-200 ${isThinkingExpanded() ? "rotate-180" : ""}`}
                        />
                    </button>
                    <Show when={isThinkingExpanded()}>
                        <div class="mt-2 pl-3 border-l-2 border-zinc-800 text-sm text-zinc-400 italic font-mono leading-relaxed animate-in fade-in slide-in-from-top-1 duration-200 whitespace-pre-wrap">
                           {parsedContent().thought}
                           {/* Add blinking cursor to thought if it's the active part and streaming */}
                           <Show when={props.isStreaming && !parsedContent().content && displayedText().endsWith(parsedContent().thought!)}>
                                <span class="cursor-blink"></span>
                           </Show>
                        </div>
                    </Show>
                </div>
             </Show>

            <div
              ref={contentRef}
              class="prose prose-invert prose-base max-w-none text-zinc-300 leading-relaxed
                         prose-pre:bg-zinc-950 prose-pre:border prose-pre:border-zinc-800 prose-pre:rounded-lg prose-pre:p-4
                         prose-code:text-indigo-300 prose-code:bg-zinc-800/50 prose-code:px-1.5 prose-code:py-0.5 prose-code:rounded prose-code:before:content-none prose-code:after:content-none prose-code:text-base
                         prose-a:text-indigo-400 prose-a:no-underline hover:prose-a:underline
                         prose-table:border-collapse prose-th:border prose-th:border-zinc-700 prose-th:p-2 prose-td:border prose-td:border-zinc-700 prose-td:p-2"
              innerHTML={htmlContent()}
              onClick={handleContentClick}
            />
          </Show>
        )}
      </div>

      <div class="shrink-0 opacity-0 group-hover:opacity-100 transition-opacity flex flex-col gap-1 pt-6">
        <button
          onClick={handleCopy}
          class="p-1.5 text-zinc-500 hover:text-white rounded hover:bg-zinc-800 transition-colors"
          title="Copy Message"
        >
          {copied() ? <Check size={14} /> : <Copy size={14} />}
        </button>
        {props.message.role === "user" && props.onEdit && !isEditing() && (
          <button
            onClick={startEdit}
            class="p-1.5 text-zinc-500 hover:text-white rounded hover:bg-zinc-800 transition-colors"
            title="Edit"
          >
            <Edit2 size={14} />
          </button>
        )}
      </div>
    </div>
  );
}
