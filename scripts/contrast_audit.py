"""WCAG contrast audit for VitalFlow HMS status tokens (light + dark)."""


def hsl_to_rgb(h, s, l):
    c = (1 - abs(2 * l - 1)) * s
    x = c * (1 - abs((h / 60) % 2 - 1))
    m = l - c / 2
    if h < 60:
        r, g, b = c, x, 0
    elif h < 120:
        r, g, b = x, c, 0
    elif h < 180:
        r, g, b = 0, c, x
    elif h < 240:
        r, g, b = 0, x, c
    elif h < 300:
        r, g, b = x, 0, c
    else:
        r, g, b = c, 0, x
    return (r + m) * 255, (g + m) * 255, (b + m) * 255


def lum(rgb):
    def f(v):
        v = v / 255
        return v / 12.92 if v <= 0.03928 else ((v + 0.055) / 1.055) ** 2.4

    r, g, b = rgb
    return 0.2126 * f(r) + 0.7152 * f(g) + 0.0722 * f(b)


def ratio(fg, bg):
    l1, l2 = lum(fg), lum(bg)
    if l1 < l2:
        l1, l2 = l2, l1
    return (l1 + 0.05) / (l2 + 0.05)


WHITE = (255, 255, 255)
DARK_CARD = hsl_to_rgb(198, 0.90, 0.13)

light = {
    "muted-foreground (214 40% 46%)": hsl_to_rgb(214, 0.40, 0.46),
    "success (157 95% 35%)": hsl_to_rgb(157, 0.95, 0.35),
    "warning (34 90% 44%)": hsl_to_rgb(34, 0.90, 0.44),
    "destructive (356 68% 42%)": hsl_to_rgb(356, 0.68, 0.42),
    "CANDIDATE success (156 84% 25%)": hsl_to_rgb(156, 0.84, 0.25),
    "CANDIDATE warning (30 95% 33%)": hsl_to_rgb(30, 0.95, 0.33),
    "CANDIDATE warning-on-tint (32 95% 35% on 10% tint)": None,
}
light.pop("CANDIDATE warning-on-tint (32 95% 35% on 10% tint)")

dark = {
    "muted-foreground (209 36% 65%)": hsl_to_rgb(209, 0.36, 0.65),
    "success (157 95% 45%)": hsl_to_rgb(157, 0.95, 0.45),
    "warning (34 85% 55%)": hsl_to_rgb(34, 0.85, 0.55),
    "destructive (356 60% 58%)": hsl_to_rgb(356, 0.60, 0.58),
    "status-completed (209 36% 70%)": hsl_to_rgb(209, 0.36, 0.70),
    "CANDIDATE destructive (356 70% 68%)": hsl_to_rgb(356, 0.70, 0.68),
}

print("=== LIGHT MODE (text on white card) — AA needs 4.5:1 ===")
for k, v in light.items():
    r = ratio(v, WHITE)
    print(f"{k}: {r:.3f} {'PASS' if r >= 4.5 else 'FAIL'}")

# StatusBadge reality check: colored text on a 10%-tint chip (not pure white).
warn_c = hsl_to_rgb(30, 0.95, 0.33)
tint = tuple(c * 0.10 + 255 * 0.90 for c in warn_c)
print(f"CANDIDATE warning on its own 10% tint chip: {ratio(warn_c, tint):.3f}")
succ_c = hsl_to_rgb(156, 0.84, 0.25)
tint2 = tuple(c * 0.10 + 255 * 0.90 for c in succ_c)
print(f"CANDIDATE success on its own 10% tint chip: {ratio(succ_c, tint2):.3f}")

print("=== DARK MODE (text on dark navy card) — AA needs 4.5:1 ===")
for k, v in dark.items():
    r = ratio(v, DARK_CARD)
    print(f"{k}: {r:.3f} {'PASS' if r >= 4.5 else 'FAIL'}")
