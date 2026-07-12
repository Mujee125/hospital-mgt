import { useState, useEffect, useRef } from "react";
import { listen } from "@tauri-apps/api/event";
import { useQueryClient } from "@tanstack/react-query";
import { motion, AnimatePresence } from "motion/react";
import { Send, Hash, Trash2, MessageSquare, Users, Shield, Loader2 } from "lucide-react";
import { useMessages, useSendMessage, useDeleteMessage, qk } from "@/lib/queries";
import type { ChatMessage } from "@/lib/models";
import { Input } from "@/components/ui/input";
import { Button } from "@/components/ui/button";

const ROOMS = [
  { id: "general", label: "General", icon: Hash, description: "All staff" },
  { id: "doctors", label: "Doctors", icon: Users, description: "Medical staff" },
  { id: "admin", label: "Admin", icon: Shield, description: "Administration" },
];

// Tokenized sender-name palette. Each entry is an `hsl(var(--token))` value
// sourced from the design-system status/success/warning/info/accent/
// primary/destructive tokens. The hashing logic (sender name → color index)
// is preserved unchanged — only the resolved color values were swapped for
// design-system tokens so the chat surface stays in lock-step with the
// rest of the app's theming (light + dark).
const COLORS = [
  "hsl(var(--success))",
  "hsl(var(--info))",
  "hsl(var(--accent))",
  "hsl(var(--destructive))",
  "hsl(var(--warning))",
  "hsl(var(--primary))",
];

function senderColor(name: string) {
  let h = 0;
  for (const c of name) h = c.charCodeAt(0) + ((h << 5) - h);
  return COLORS[Math.abs(h) % COLORS.length];
}

function fmtTime(iso: string) {
  return new Date(iso).toLocaleTimeString("en-US", { hour: "2-digit", minute: "2-digit" });
}

function fmtDate(iso: string) {
  const d = new Date(iso), t = new Date();
  if (d.toDateString() === t.toDateString()) return "Today";
  const y = new Date(t);
  y.setDate(t.getDate() - 1);
  if (d.toDateString() === y.toDateString()) return "Yesterday";
  return d.toLocaleDateString("en-US", { weekday: "long", month: "long", day: "numeric" });
}

export function Messaging() {
  const [room, setRoom] = useState("general");
  const [text, setText] = useState("");
  const [sender, setSender] = useState(() => localStorage.getItem("hms_sender") || "");
  const [editSender, setEditSender] = useState(!localStorage.getItem("hms_sender"));
  const [tmpSender, setTmpSender] = useState(sender);
  const bottomRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const queryClient = useQueryClient();

  const { data: messages = [], isLoading } = useMessages(room);
  const sendMessage = useSendMessage();
  const deleteMessage = useDeleteMessage();

  // Real-time push: new_message events update the cache directly for an
  // instant feel, on top of useMessages' own light polling as a fallback
  // if an event is ever missed (e.g. brief reconnect).
  useEffect(() => {
    const off = listen<ChatMessage>("new_message", (e) => {
      if (e.payload.room !== room) return;
      queryClient.setQueryData<ChatMessage[]>(qk.messages(room), (prev = []) =>
        prev.some((m) => m.id === e.payload.id) ? prev : [...prev, e.payload]
      );
    });
    return () => {
      off.then((f) => f());
    };
  }, [room, queryClient]);

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages]);

  const handleSend = () => {
    if (!text.trim() || !sender || sendMessage.isPending) return;
    sendMessage.mutate(
      { sender, content: text.trim(), room },
      {
        onSuccess: () => {
          setText("");
          inputRef.current?.focus();
        },
      }
    );
  };

  const handleDelete = (id: string) => {
    deleteMessage.mutate(id);
  };

  const saveSender = () => {
    if (!tmpSender.trim()) return;
    setSender(tmpSender.trim());
    localStorage.setItem("hms_sender", tmpSender.trim());
    setEditSender(false);
    inputRef.current?.focus();
  };

  const grouped = messages.reduce<{ date: string; msgs: ChatMessage[] }[]>((acc, m) => {
    const day = new Date(m.created_at).toDateString();
    const last = acc[acc.length - 1];
    if (last?.date === day) last.msgs.push(m);
    else acc.push({ date: day, msgs: [m] });
    return acc;
  }, []);

  const roomInfo = ROOMS.find((r) => r.id === room)!;

  return (
    <div className="flex flex-col sm:flex-row h-full overflow-hidden bg-background">
      {/* Conversations sidebar */}
      <aside className="w-full sm:w-56 bg-card border-r border-border flex flex-col shrink-0">
        <div className="flex-1 p-4 border-b border-border">
          <div className="flex items-center gap-2">
            <MessageSquare className="h-4 w-4 text-primary" />
            <h3 className="text-display-sm text-foreground">Staff chat</h3>
          </div>
          <p className="text-[10px] text-muted-foreground mt-0.5">
            Instant messaging
          </p>
        </div>

        <nav className="flex-1 p-3 space-y-1">
          <p className="text-[9px] uppercase tracking-widest font-bold text-muted-foreground px-2 mb-2">
            Channels
          </p>
          {ROOMS.map((r) => {
            const Icon = r.icon;
            const active = r.id === room;
            return (
              <button
                key={r.id}
                onClick={() => setRoom(r.id)}
                className={`relative w-full flex items-center gap-2.5 px-3 py-2.5 rounded-[var(--radius-md)] text-xs transition-colors ${
                  active
                    ? "text-primary font-semibold"
                    : "text-muted-foreground hover:bg-muted hover:text-foreground"
                }`}
              >
                {active && (
                  <motion.div
                    layoutId="chat-room-active"
                    className="absolute inset-0 rounded-[var(--radius-md)] bg-primary/10 -z-10"
                    transition={{ type: "spring", stiffness: 400, damping: 32 }}
                  />
                )}
                <Icon className="h-3.5 w-3.5 shrink-0" />
                {r.label}
              </button>
            );
          })}
        </nav>

        <div className="p-3 border-t border-border bg-muted/30">
          {editSender ? (
            <div className="space-y-2">
              <p className="text-[10px] font-bold uppercase tracking-wider text-muted-foreground">
                Your name
              </p>
              <Input
                autoFocus
                aria-label="Your name"
                className="h-8 text-xs px-2.5 py-1.5"
                placeholder="Enter your name..."
                value={tmpSender}
                onChange={(e) => setTmpSender(e.target.value)}
                onKeyDown={(e) => e.key === "Enter" && saveSender()}
              />
              <Button
                size="sm"
                onClick={saveSender}
                disabled={!tmpSender.trim()}
                className="w-full"
              >
                Set name
              </Button>
            </div>
          ) : (
            <button
              onClick={() => {
                setTmpSender(sender);
                setEditSender(true);
              }}
              className="w-full flex items-center gap-2 group"
            >
              <div
                className="h-7 w-7 rounded-full bg-primary/15 flex items-center justify-center font-bold text-xs"
                style={{ color: senderColor(sender) }}
              >
                {sender.charAt(0).toUpperCase()}
              </div>
              <div className="text-left flex-1 min-w-0">
                <p className="text-xs font-semibold text-foreground truncate">
                  {sender}
                </p>
                <p className="text-[9px] text-muted-foreground group-hover:text-primary transition-colors">
                  Click to change
                </p>
              </div>
            </button>
          )}
        </div>
      </aside>

      {/* Chat area */}
      <div className="flex-1 flex flex-col min-w-0">
        <div className="h-14 border-b border-border bg-card/60 backdrop-blur-sm px-6 flex items-center gap-3 shrink-0">
          <roomInfo.icon className="h-4 w-4 text-primary" />
          <div className="min-w-0">
            <h4 className="text-display-md text-foreground truncate">#{roomInfo.label}</h4>
            <p className="text-[10px] text-muted-foreground">
              {roomInfo.description}
            </p>
          </div>
          <div className="ml-auto flex items-center gap-1.5">
            <div className="h-2 w-2 rounded-full bg-success animate-pulse" />
            <span className="text-[10px] text-muted-foreground font-medium">
              Live
            </span>
          </div>
        </div>

        <div className="flex-1 overflow-y-auto px-6 py-4 space-y-1">
          {isLoading ? (
            <div className="flex items-center justify-center h-full">
              <Loader2 className="h-6 w-6 text-primary animate-spin" />
            </div>
          ) : messages.length === 0 ? (
            <div className="flex flex-col items-center justify-center h-full gap-3 text-center">
              <div className="h-14 w-14 rounded-[var(--radius-lg)] bg-primary/10 flex items-center justify-center">
                <MessageSquare className="h-7 w-7 text-primary" />
              </div>
              <div>
                <p className="font-semibold text-sm text-foreground">No messages yet</p>
                <p className="text-xs text-muted-foreground mt-1">
                  Be the first in #{roomInfo.label}
                </p>
              </div>
            </div>
          ) : (
            grouped.map(({ date, msgs }) => (
              <div key={date}>
                <div className="flex items-center gap-3 my-4">
                  <div className="flex-1 h-px bg-border" />
                  <span className="text-[10px] font-semibold text-muted-foreground uppercase tracking-wider px-2">
                    {fmtDate(msgs[0]?.created_at ?? "")}
                  </span>
                  <div className="flex-1 h-px bg-border" />
                </div>

                <AnimatePresence initial={false}>
                  {msgs.map((msg, idx) => {
                    const isOwn = msg.sender === sender;
                    const showSender =
                      idx === 0 || msgs[idx - 1]?.sender !== msg.sender;
                    return (
                      <motion.div
                        key={msg.id}
                        initial={{ opacity: 0, y: 8 }}
                        animate={{ opacity: 1, y: 0 }}
                        exit={{ opacity: 0 }}
                        transition={{ duration: 0.15 }}
                        className={`group flex gap-3 ${isOwn ? "flex-row-reverse" : ""} ${showSender ? "mt-4" : "mt-0.5"}`}
                      >
                        {showSender ? (
                          <div
                            className="h-8 w-8 rounded-full bg-primary/10 flex items-center justify-center font-bold text-xs shrink-0 mt-0.5"
                            style={{ color: senderColor(msg.sender) }}
                          >
                            {msg.sender.charAt(0).toUpperCase()}
                          </div>
                        ) : (
                          <div className="w-8 shrink-0" />
                        )}

                        <div
                          className={`max-w-[65%] flex flex-col ${isOwn ? "items-end" : "items-start"}`}
                        >
                          {showSender && (
                            <div
                              className={`flex items-baseline gap-2 mb-1 ${isOwn ? "flex-row-reverse" : ""}`}
                            >
                              <span
                                className="text-xs font-semibold"
                                style={{ color: senderColor(msg.sender) }}
                              >
                                {msg.sender}
                              </span>
                              <span className="text-[10px] text-muted-foreground">
                                {fmtTime(msg.created_at)}
                              </span>
                            </div>
                          )}
                          <div
                            className={`relative px-3.5 py-2 rounded-[var(--radius-lg)] text-sm leading-relaxed break-words ${
                              isOwn
                                ? "bg-primary text-primary-foreground rounded-tr-sm"
                                : "bg-card border border-border text-foreground rounded-tl-sm"
                            }`}
                          >
                            {msg.content}
                            <Button
                              variant="destructive"
                              size="icon"
                              onClick={() => handleDelete(msg.id)}
                              aria-label="Delete message"
                              className={`absolute -top-2 ${isOwn ? "-left-2" : "-right-2"} h-5 w-5 opacity-0 group-hover:opacity-100 transition-opacity`}
                            >
                              <Trash2 className="h-2.5 w-2.5" />
                            </Button>
                          </div>
                          {!showSender && (
                            <span className="text-[9px] text-muted-foreground mt-0.5 px-1">
                              {fmtTime(msg.created_at)}
                            </span>
                          )}
                        </div>
                      </motion.div>
                    );
                  })}
                </AnimatePresence>
              </div>
            ))
          )}
          <div ref={bottomRef} />
        </div>

        <div className="px-6 pb-5 pt-3 border-t border-border bg-card/30">
          {!sender || editSender ? (
            <div className="p-3 rounded-[var(--radius-md)] bg-warning/10 border border-warning/30 text-warning text-xs font-medium">
              Set your name in the sidebar to start chatting.
            </div>
          ) : (
            <div className="flex items-center gap-3 bg-card border border-border rounded-full px-4 py-2.5 shadow-sm focus-within:ring-2 focus-within:ring-ring/30 transition-all">
              <input
                ref={inputRef}
                aria-label={`Message #${roomInfo.label}`}
                className="flex-1 bg-transparent text-sm text-foreground placeholder:text-muted-foreground focus:outline-none"
                placeholder={`Message #${roomInfo.label}...`}
                value={text}
                onChange={(e) => setText(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "Enter" && !e.shiftKey) {
                    e.preventDefault();
                    handleSend();
                  }
                }}
                disabled={sendMessage.isPending}
                maxLength={1000}
              />
              <Button
                size="icon"
                onClick={handleSend}
                disabled={!text.trim() || sendMessage.isPending}
                aria-label="Send message"
                className="h-8 w-8 shrink-0"
              >
                {sendMessage.isPending ? (
                  <Loader2 className="h-3.5 w-3.5 animate-spin" />
                ) : (
                  <Send className="h-3.5 w-3.5" />
                )}
              </Button>
            </div>
          )}
          <p className="text-[10px] text-muted-foreground mt-1.5 px-1">
            Press{" "}
            <kbd className="px-1 py-0.5 rounded bg-muted text-[9px] font-mono">
              Enter
            </kbd>{" "}
            to send
          </p>
        </div>
      </div>
    </div>
  );
}
