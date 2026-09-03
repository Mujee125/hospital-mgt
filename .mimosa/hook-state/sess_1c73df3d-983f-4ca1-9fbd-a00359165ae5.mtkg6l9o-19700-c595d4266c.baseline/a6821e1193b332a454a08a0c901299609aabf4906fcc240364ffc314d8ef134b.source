interface LogoProps {
  className?: string;
}

/**
 * Brand mark for VitalFlow HMS.
 *
 * The SVG is rendered with `fill="currentColor"` / `stroke="currentColor"`
 * so the brand colour is driven by the parent's text colour (Tailwind
 * `text-primary` / `text-accent`), which respects the design-system CSS
 * variables in `src/index.css` (sky-blue primary + teal accent). The
 * previous version hard-coded `#14B8A6` (teal) and `#1D4ED8` (royal
 * blue) — the blue was from the obsolete "Mayo navy" palette and didn't
 * match the re-skinned VitalFlow sky-blue brand.
 *
 * Default parent class is `text-accent` (teal) which keeps the
 * historical cross+heart motif in the teal brand colour; pass
 * `className="... text-primary"` to tint the whole mark sky-blue, or
 * override per-element by removing the `text-*` class on the parent.
 *
 * The ECG line stays white because it's drawn ON TOP of the teal
 * medical cross — switching it to currentColor would render it
 * invisible on a card-coloured background.
 */
export default function RasheedMedicalLogo({
  className = "w-20 h-20",
}: LogoProps) {
  return (
    <svg
      viewBox="0 0 256 256"
      xmlns="http://www.w3.org/2000/svg"
      className={className}
      fill="none"
      aria-hidden="true"
      role="presentation"
    >
      {/* Circle */}
      <path
        d="M215 40C196 18 164 8 128 8C62 8 8 62 8 128S62 248 128 248c38 0 72-18 94-46"
        stroke="hsl(var(--accent))"
        strokeWidth="8"
        strokeLinecap="round"
      />

      {/* Medical Cross */}
      <rect x="102" y="42" width="52" height="84" rx="8" fill="hsl(var(--accent))" />
      <rect x="86" y="58" width="84" height="52" rx="8" fill="hsl(var(--accent))" />

      {/* ECG */}
      <polyline
        points="90,84 110,84 118,72 128,106 140,60 150,84 170,84"
        stroke="white"
        strokeWidth="4"
        fill="none"
        strokeLinecap="round"
        strokeLinejoin="round"
      />

      {/* Left Hand */}
      <path
        d="M65 150
           C65 110 92 112 92 148
           L92 180
           C80 165 60 160 50 175
           C68 192 84 210 118 228
           C122 200 118 168 108 150
           C100 136 82 132 65 150Z"
        fill="hsl(var(--primary))"
      />

      {/* Right Hand */}
      <path
        d="M191 150
           C191 110 164 112 164 148
           L164 180
           C176 165 196 160 206 175
           C188 192 172 210 138 228
           C134 200 138 168 148 150
           C156 136 174 132 191 150Z"
        fill="hsl(var(--primary))"
      />
    </svg>
  );
}
