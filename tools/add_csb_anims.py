#!/usr/bin/env python3
"""Ergaenzt splash_animations.h um die Animationen des Claude Session Browser.

Hermanns Splash-Animationen und der Buddy im Claude Session Browser stammen
aus derselben Quelle (claudepix, 20x20, Palette mit 10 Farben, 400 Zellen pro
Frame). Zehn Animationen gibt es deshalb schon namensgleich in der Firmware --
diese fuenf fehlen und werden hier aus claude_sessions.py nachgezogen:

    done, think, write, allow, limit

Absichtlich ein eigenes Skript und kein Eingriff von Hand: splash_animations.h
ist generiert, und der Block muss sich neu erzeugen lassen wenn sich die
Sprites im Session Browser aendern. Der Lauf ist idempotent -- ein bereits
vorhandener Block wird zuerst entfernt.

Aufruf:
    python tools/add_csb_anims.py [--csb PFAD_ZU_claude_sessions.py]
"""
import argparse
import importlib.util
import os
import re
import sys

# Namen die aus dem Session Browser uebernommen werden. Alles andere ist
# entweder schon in der Firmware oder gehoert nicht aufs Geraet.
EXTRA = ["done", "think", "write", "allow", "limit"]

# Der Session Browser kennt keine Haltezeiten pro Frame -- sein Buddy laeuft
# mit fester Rate (_FRAME_MS). Dieselbe Rate hier, sonst laufen dieselben
# Animationen auf Desktop und Geraet unterschiedlich schnell.
FRAME_MS = 180

BEGIN = "// ==== CSB-ANIMATIONEN ANFANG (tools/add_csb_anims.py) ===="
END = "// ==== CSB-ANIMATIONEN ENDE ===="

DEFAULT_CSB = r"C:\Users\Flori\claude-session-browser\claude_sessions.py"


def rgb565(hex_color: str) -> int:
    """'#RRGGBB' -> RGB565. Gleiche Rechnung wie in tools/convert_to_c.js."""
    h = hex_color.lstrip("#")
    r, g, b = int(h[0:2], 16), int(h[2:4], 16), int(h[4:6], 16)
    return ((r & 0xF8) << 8) | ((g & 0xFC) << 3) | (b >> 3)


def load_anims(csb_path: str) -> dict:
    """Sprites aus clawd_sprites.py neben claude_sessions.py laden.

    Bewusst direkt aus dem Sprite-Modul statt ueber claude_sessions.BUDDY_ANIMS:
    das dortige _decode_buddy_anims() faengt jeden Fehler ab und liefert im
    Zweifel ein leeres Dict -- eine fehlende Datei saehe dann aus wie
    "keine Animationen vorhanden" statt wie ein Fehler.
    """
    import base64
    import json
    import zlib

    sprite_path = os.path.join(os.path.dirname(os.path.abspath(csb_path)),
                               "clawd_sprites.py")
    if not os.path.exists(sprite_path):
        raise SystemExit(f"clawd_sprites.py nicht gefunden neben {csb_path}")
    spec = importlib.util.spec_from_file_location("_clawd_sprites", sprite_path)
    mod = importlib.util.module_from_spec(spec)
    sys.modules["_clawd_sprites"] = mod
    spec.loader.exec_module(mod)

    arr = json.loads(zlib.decompress(base64.b64decode(mod.BLOB)).decode("utf-8"))
    return {a["n"]: {"palette": a["p"], "frames": a["f"]} for a in arr}


def c_ident(name: str) -> str:
    return "splash_" + re.sub(r"[^a-z0-9]+", "_", name.lower()).strip("_")


def emit(name: str, anim: dict) -> tuple[str, str]:
    """(Definitionen, Tabellenzeile) fuer eine Animation."""
    ident = c_ident(name)
    pal = [rgb565(c) for c in anim["palette"]]
    pal += [0] * (10 - len(pal))
    frames = anim["frames"]
    for i, f in enumerate(frames):
        if len(f) != 400:
            raise ValueError(f"{name}: Frame {i} hat {len(f)} statt 400 Zellen")
        if max(f) > 9 or min(f) < 0:
            raise ValueError(f"{name}: Frame {i} hat Werte ausserhalb 0..9")

    out = [f"static const uint16_t {ident}_palette[10] = {{"
           + ",".join(f"0x{v:04X}" for v in pal[:10]) + "};"]
    out.append(f"static const uint8_t {ident}_frames[{len(frames)}][400] = {{")
    for f in frames:
        out.append("    {" + ",".join(str(v) for v in f) + "},")
    out.append("};")
    out.append(f"static const uint16_t {ident}_holds[{len(frames)}] = {{"
               + ",".join([str(FRAME_MS)] * len(frames)) + "};")
    row = (f'    {{"{name}", "Session Browser", {len(frames)}, '
           f"{ident}_palette, {ident}_frames, {ident}_holds}},")
    return "\n".join(out), row


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--csb", default=DEFAULT_CSB,
                    help="Pfad zu claude_sessions.py")
    ap.add_argument("--out", default=os.path.join(
        os.path.dirname(__file__), "..", "firmware", "src",
        "splash_animations.h"))
    args = ap.parse_args()

    anims = load_anims(args.csb)
    missing = [n for n in EXTRA if n not in anims]
    if missing:
        print(f"FEHLER: im Session Browser nicht gefunden: {missing}")
        return 1

    path = os.path.abspath(args.out)
    with open(path, encoding="utf-8") as fh:
        src = fh.read()

    # Frueheren Block entfernen, damit wiederholte Laeufe nicht anhaeufen.
    src = re.sub(re.escape(BEGIN) + r".*?" + re.escape(END) + r"\n?",
                 "", src, flags=re.S)

    m = re.search(r"#define SPLASH_ANIM_COUNT (\d+)\n"
                  r"static const splash_anim_def_t splash_anims\[SPLASH_ANIM_COUNT\] = \{\n"
                  r"(.*?)\n\};", src, re.S)
    if not m:
        print("FEHLER: Animationstabelle nicht gefunden - Format geaendert?")
        return 1
    rows = m.group(2)

    # Eigene Zeilen (Kategorie "Session Browser") rauswerfen: bei einem
    # erneuten Lauf sollen geaenderte Sprites uebernommen werden, nicht
    # uebersprungen. Uebrig bleiben nur die Zeilen von upstream.
    upstream = [ln for ln in rows.splitlines()
                if ln.strip() and '"Session Browser"' not in ln]
    base_count = len(upstream)

    # Was upstream unter gleichem Namen schon liefert, wird nicht ersetzt.
    known = set(re.findall(r'\{"([^"]+)"', "\n".join(upstream)))
    todo = [n for n in EXTRA if n not in known]
    skipped = [n for n in EXTRA if n in known]
    if skipped:
        print(f"kommt schon von upstream, uebersprungen: {skipped}")
    if not todo:
        print("nichts zu ergaenzen")
        return 0

    defs, new_rows = [], []
    for name in todo:
        d, r = emit(name, anims[name])
        defs.append(d)
        new_rows.append(r)

    block = (BEGIN + "\n"
             + "// Aus claude_sessions.py (BUDDY_ANIMS) erzeugt. Nicht von Hand\n"
             + "// aendern - stattdessen tools/add_csb_anims.py erneut laufen lassen.\n"
             + "\n".join(defs) + "\n" + END)

    total = base_count + len(todo)
    table = ("#define SPLASH_ANIM_COUNT " + str(total) + "\n"
             "static const splash_anim_def_t splash_anims[SPLASH_ANIM_COUNT] = {\n"
             + "\n".join(upstream) + "\n" + "\n".join(new_rows) + "\n};")
    src = src[:m.start()] + block + "\n" + table + src[m.end():]

    with open(path, "w", encoding="utf-8", newline="\n") as fh:
        fh.write(src)

    frames = sum(len(anims[n]["frames"]) for n in todo)
    print(f"{len(todo)} Animationen ergaenzt ({frames} Frames, "
          f"{frames * 400 / 1024:.1f} KB Flash), SPLASH_ANIM_COUNT {base_count} -> {total}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
