import { useEffect, useState } from "react";
import { AnimatePresence, motion } from "motion/react";
import { Sun, Moon } from "lucide-react";
import { Button } from "@/components/ui/button";

export function ThemeToggle() {
  const [isDark, setIsDark] = useState(false);

  useEffect(() => {
    const stored = localStorage.getItem("theme");
    const sysDark = window.matchMedia("(prefers-color-scheme: dark)").matches;
    const dark = stored === "dark" || (!stored && sysDark);
    setIsDark(dark);
    document.documentElement.classList.toggle("dark", dark);
  }, []);

  const toggle = () => {
    const next = !isDark;
    setIsDark(next);
    document.documentElement.classList.toggle("dark", next);
    localStorage.setItem("theme", next ? "dark" : "light");
  };

  return (
    <Button
      variant="ghost"
      size="icon"
      onClick={toggle}
      className="w-9 h-9 rounded-full transition-colors text-muted-foreground hover:text-foreground"
      title={isDark ? "Switch to light mode" : "Switch to dark mode"}
    >
      <AnimatePresence mode="wait" initial={false}>
        {isDark ? (
          <motion.div key="sun" initial={{ opacity: 0, rotate: -45, scale: 0.6 }} animate={{ opacity: 1, rotate: 0, scale: 1 }} exit={{ opacity: 0, rotate: 45, scale: 0.6 }} transition={{ duration: 0.18 }}>
            <Sun className="h-5 w-5 text-amber-500" />
          </motion.div>
        ) : (
          <motion.div key="moon" initial={{ opacity: 0, rotate: 45, scale: 0.6 }} animate={{ opacity: 1, rotate: 0, scale: 1 }} exit={{ opacity: 0, rotate: -45, scale: 0.6 }} transition={{ duration: 0.18 }}>
            <Moon className="h-5 w-5 text-muted-foreground" />
          </motion.div>
        )}
      </AnimatePresence>
    </Button>
  );
}
