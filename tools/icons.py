#!/usr/bin/env python3
"""Ícones pixel 16x16 do Raster.

Gera `assets/icons/icons.txt` (fonte única, lida pelo app) e `docs/mockups/icons.html` (tabela).
Cada ícone é desenhado com primitivas em grade 16x16, traço de 1px e 1px de margem.
"""
import math
import pathlib

N = 16
ROOT = pathlib.Path(__file__).resolve().parent.parent


class G:
    def __init__(self):
        self.p = [[0] * N for _ in range(N)]

    def px(self, x, y, v=1):
        if 0 <= x < N and 0 <= y < N:
            self.p[y][x] = v

    def h(self, x0, x1, y, v=1):
        for x in range(min(x0, x1), max(x0, x1) + 1):
            self.px(x, y, v)

    def v(self, x, y0, y1, v=1):
        for y in range(min(y0, y1), max(y0, y1) + 1):
            self.px(x, y, v)

    def rect(self, x0, y0, x1, y1, v=1):
        self.h(x0, x1, y0, v); self.h(x0, x1, y1, v); self.v(x0, y0, y1, v); self.v(x1, y0, y1, v)

    def fill(self, x0, y0, x1, y1, v=1):
        for y in range(y0, y1 + 1):
            self.h(x0, x1, y, v)

    def line(self, x0, y0, x1, y1, v=1):
        dx, dy = abs(x1 - x0), -abs(y1 - y0)
        sx, sy = (1 if x0 < x1 else -1), (1 if y0 < y1 else -1)
        e = dx + dy
        while True:
            self.px(x0, y0, v)
            if x0 == x1 and y0 == y1:
                break
            e2 = 2 * e
            if e2 >= dy:
                e += dy; x0 += sx
            if e2 <= dx:
                e += dx; y0 += sy

    def disc(self, cx, cy, r, v=1):
        for y in range(N):
            for x in range(N):
                if (x - cx) ** 2 + (y - cy) ** 2 <= r * r + 0.25:
                    self.px(x, y, v)

    def ring(self, cx, cy, r, w=1.0):
        inner = G(); inner.disc(cx, cy, r - w)
        outer = G(); outer.disc(cx, cy, r)
        for y in range(N):
            for x in range(N):
                if outer.p[y][x] and not inner.p[y][x]:
                    self.px(x, y)

    def rounded(self, x0, y0, x1, y1):
        self.rect(x0, y0, x1, y1)
        for x, y in ((x0, y0), (x1, y0), (x0, y1), (x1, y1)):
            self.px(x, y, 0)

    def rows(self):
        return ["".join("#" if c else "." for c in r) for r in self.p]


ICONS = {}


def icon(group):
    def deco(f):
        g = G(); f(g); ICONS[f.__name__] = (group, g.rows()); return f
    return deco


# ---------- session and menu actions ----------
@icon("action")
def open(g):
    g.rect(1, 4, 11, 14)
    g.h(7, 11, 4, 0); g.v(11, 4, 8, 0)
    g.line(6, 9, 13, 2); g.line(7, 9, 14, 2)
    g.h(9, 14, 1); g.v(14, 1, 6)


@icon("action")
def rename(g):
    g.line(2, 11, 11, 2); g.line(4, 13, 13, 4); g.line(11, 2, 13, 4)
    g.line(9, 4, 11, 6); g.px(2, 12); g.px(2, 13); g.px(3, 13)


@icon("action")
def copy(g):
    g.rect(5, 1, 14, 10)
    g.fill(1, 5, 10, 14, 0)
    g.rect(1, 5, 10, 14)


@icon("action")
def folder(g):
    g.h(1, 5, 2); g.px(6, 3); g.h(7, 14, 4)
    g.v(1, 2, 13); g.v(14, 4, 13); g.h(1, 14, 13); g.h(2, 13, 6)


@icon("action")
def stop(g):
    g.rect(2, 2, 13, 13); g.fill(5, 5, 10, 10)


@icon("action")
def trash(g):
    g.h(1, 14, 3); g.h(5, 10, 1); g.v(5, 1, 3); g.v(10, 1, 3)
    g.v(3, 5, 13); g.v(12, 5, 13); g.h(4, 11, 14)
    g.v(6, 7, 12); g.v(9, 7, 12)


@icon("action")
def plus(g):
    g.fill(7, 2, 8, 13); g.fill(2, 7, 13, 8)


@icon("action")
def search(g):
    g.ring(6, 6, 5)
    g.line(10, 10, 14, 14); g.line(11, 10, 14, 13); g.line(10, 11, 13, 14)


@icon("action")
def settings(g):
    g.disc(7.5, 7.5, 5)
    for x0, y0, x1, y1 in ((6, 1, 9, 2), (6, 13, 9, 14), (1, 6, 2, 9), (13, 6, 14, 9)):
        g.fill(x0, y0, x1, y1)
    for x, y in ((3, 3), (11, 3), (3, 11), (11, 11)):
        g.fill(x - 1, y - 1, x + 1, y + 1)
    g.disc(7.5, 7.5, 2, 0)


@icon("action")
def send(g):
    g.fill(7, 3, 8, 14)
    g.line(7, 1, 2, 6); g.line(7, 2, 3, 6)
    g.line(8, 1, 13, 6); g.line(8, 2, 12, 6)


@icon("action")
def abort(g):
    g.fill(3, 3, 12, 12)


@icon("action")
def attach(g):
    g.v(10, 3, 11); g.px(9, 12); g.h(6, 8, 13); g.px(5, 12); g.v(4, 4, 11)
    g.px(5, 3); g.h(6, 7, 2); g.px(8, 3); g.v(8, 4, 10); g.px(7, 11); g.v(6, 5, 10)


@icon("action")
def mention(g):
    g.ring(7.5, 7.5, 6.5)
    g.ring(7.5, 7.5, 2.5)
    g.v(10, 5, 9); g.px(11, 10); g.px(12, 10); g.h(10, 14, 9, 0); g.px(10, 9); g.px(11, 10)
    g.h(12, 14, 8, 0)


@icon("action")
def plug(g):
    g.v(5, 1, 4); g.v(10, 1, 4)
    g.fill(3, 5, 12, 9)
    g.line(4, 10, 6, 12); g.line(11, 10, 9, 12); g.h(6, 9, 12)
    g.fill(7, 13, 8, 14)


@icon("action")
def check(g):
    g.line(2, 8, 6, 12); g.line(2, 9, 6, 13); g.line(6, 12, 13, 3); g.line(6, 13, 14, 3)


@icon("action")
def close(g):
    g.line(3, 3, 12, 12); g.line(4, 3, 12, 11); g.line(3, 4, 11, 12)
    g.line(12, 3, 3, 12); g.line(11, 3, 3, 11); g.line(12, 4, 4, 12)


@icon("action")
def chevron_down(g):
    g.line(2, 5, 7, 10); g.line(2, 6, 7, 11); g.line(8, 10, 13, 5); g.line(8, 11, 13, 6)


@icon("action")
def chevron_right(g):
    g.line(5, 2, 10, 7); g.line(6, 2, 11, 7); g.line(10, 8, 5, 13); g.line(11, 8, 6, 13)


@icon("action")
def refresh(g):
    g.ring(7.5, 7.5, 5.5)
    g.fill(9, 0, 15, 5, 0)
    g.h(9, 13, 2); g.v(13, 2, 6); g.line(9, 2, 9, 2)


@icon("action")
def download(g):
    g.fill(7, 1, 8, 9)
    g.line(3, 6, 7, 10); g.line(4, 6, 7, 9); g.line(12, 6, 8, 10); g.line(11, 6, 8, 9)
    g.v(1, 11, 14); g.v(14, 11, 14); g.h(1, 14, 14)


@icon("action")
def pin(g):
    g.fill(5, 1, 10, 2); g.v(6, 3, 6); g.v(9, 3, 6); g.h(3, 12, 7); g.fill(7, 8, 8, 14)


@icon("action")
def expand(g):
    g.h(1, 5, 1); g.v(1, 1, 5); g.line(1, 1, 6, 6)
    g.h(10, 14, 14); g.v(14, 10, 14); g.line(9, 9, 14, 14)


@icon("action")
def compress(g):
    g.h(2, 6, 6); g.v(6, 2, 6); g.line(1, 1, 6, 6)
    g.h(9, 13, 9); g.v(9, 9, 13); g.line(9, 9, 14, 14)


# ---------- agents ----------
@icon("agent")
def chat(g):
    g.rounded(1, 2, 14, 11)
    g.px(4, 12); g.px(4, 13); g.px(5, 12); g.h(5, 6, 11, 0); g.px(5, 11, 1)
    g.fill(4, 6, 5, 7); g.fill(7, 6, 8, 7); g.fill(10, 6, 11, 7)


@icon("agent")
def code(g):
    g.line(5, 3, 1, 7); g.line(1, 8, 5, 12)
    g.line(10, 3, 14, 7); g.line(14, 8, 10, 12)
    g.line(9, 1, 6, 14)


@icon("agent")
def scout(g):
    g.ring(7.5, 7.5, 7)
    g.ring(7.5, 7.5, 3.5)
    g.line(8, 7, 13, 2); g.fill(7, 7, 8, 8)


@icon("agent")
def test(g):
    g.h(4, 11, 1); g.v(6, 2, 5); g.v(9, 2, 5)
    g.line(6, 6, 2, 12); g.line(9, 6, 13, 12)
    g.h(2, 13, 13); g.px(2, 13, 0); g.px(13, 13, 0); g.h(3, 12, 13)
    for y in (10, 11, 12):
        g.h(14 - y + 0, y + 1, y)
    g.h(4, 11, 10); g.h(3, 12, 11); g.h(3, 12, 12)


# ---------- swarm states ----------
@icon("state")
def thinking(g):
    g.disc(5, 6, 3); g.disc(10, 5, 3.5); g.disc(8, 8, 3)
    g.disc(5, 6, 2, 0); g.disc(10, 5, 2.5, 0); g.disc(8, 8, 2, 0)
    g.fill(4, 6, 11, 7, 0)
    g.px(4, 12); g.fill(2, 14, 2, 14)


@icon("state")
def tool(g):
    g.ring(10.5, 4.5, 3.5, 2)
    g.fill(11, 1, 14, 4, 0)
    g.line(8, 7, 2, 13); g.line(9, 7, 2, 14); g.line(8, 6, 1, 13); g.line(9, 8, 3, 14)


@icon("state")
def writing(g):
    g.line(3, 10, 11, 2); g.line(4, 11, 12, 3); g.line(5, 12, 13, 4); g.line(11, 2, 13, 4)
    g.px(2, 11); g.px(2, 12); g.px(3, 12); g.h(1, 14, 14)


@icon("state")
def idle(g):
    g.h(7, 12, 1); g.line(12, 2, 7, 6); g.h(7, 12, 6)
    g.h(2, 6, 8); g.line(6, 9, 2, 12); g.h(2, 6, 13)


@icon("state")
def asking(g):
    g.h(5, 10, 1); g.px(4, 2); g.px(11, 2); g.v(3, 3, 4); g.v(12, 3, 5)
    g.px(11, 6); g.px(10, 7); g.px(9, 8); g.v(8, 9, 10); g.v(7, 9, 10)
    g.fill(7, 12, 8, 13)


@icon("state")
def dead(g):
    g.disc(7.5, 6.5, 6)
    g.fill(4, 5, 6, 7, 0); g.fill(9, 5, 11, 7, 0); g.px(7, 9, 0); g.px(8, 9, 0)
    g.fill(4, 12, 11, 14); g.px(5, 13, 0); g.px(7, 13, 0); g.px(9, 13, 0)


@icon("state")
def done(g):
    g.ring(7.5, 7.5, 7)
    g.line(4, 8, 6, 10); g.line(6, 10, 11, 5); g.line(4, 9, 6, 11); g.line(6, 11, 11, 6)


@icon("state")
def error(g):
    g.ring(7.5, 7.5, 7)
    g.line(5, 5, 10, 10); g.line(10, 5, 5, 10); g.line(6, 5, 10, 9); g.line(9, 5, 5, 9)


# ---------- providers and accounts ----------
@icon("provider")
def burst(g):
    g.fill(7, 1, 8, 14); g.fill(1, 7, 14, 8)
    g.line(3, 3, 12, 12); g.line(12, 3, 3, 12)
    g.fill(6, 6, 9, 9)


@icon("provider")
def knot(g):
    g.ring(7.5, 7.5, 7)
    g.ring(7.5, 7.5, 3.5)
    g.v(7, 1, 4); g.v(8, 11, 14); g.h(1, 4, 8); g.h(11, 14, 7)


@icon("provider")
def router(g):
    g.h(1, 6, 4); g.line(6, 4, 9, 11); g.h(9, 13, 11); g.line(11, 9, 13, 11); g.line(11, 13, 13, 11)
    g.h(1, 6, 11); g.line(6, 11, 9, 4); g.h(9, 13, 4); g.line(11, 2, 13, 4); g.line(11, 6, 13, 4)


@icon("provider")
def key(g):
    g.ring(4.5, 7.5, 3.5)
    g.h(8, 14, 7); g.h(8, 14, 8); g.v(12, 9, 11); g.v(14, 9, 10)


@icon("provider")
def lock(g):
    g.ring(7.5, 5.5, 4.5); g.fill(2, 6, 13, 7, 0)
    g.fill(2, 7, 13, 14); g.fill(7, 9, 8, 12, 0)


# ---------- content and system ----------
@icon("system")
def context(g):
    g.rect(4, 1, 11, 14)
    for y in (4, 7, 10):
        g.h(5, 10, y)
    g.fill(5, 11, 10, 13)


@icon("system")
def vault(g):
    g.rect(2, 1, 13, 14); g.v(4, 1, 14)
    g.h(6, 11, 4); g.h(6, 11, 6)
    g.fill(13, 9, 14, 11)


@icon("system")
def terminal(g):
    g.rect(1, 2, 14, 13); g.h(1, 14, 4)
    g.line(3, 6, 5, 8); g.line(5, 8, 3, 10); g.h(7, 11, 10)


@icon("system")
def file(g):
    g.h(2, 9, 1); g.v(2, 1, 14); g.h(2, 13, 14); g.v(13, 5, 14)
    g.line(9, 1, 13, 5); g.v(9, 1, 5); g.h(9, 13, 5)
    g.h(5, 10, 8); g.h(5, 10, 10); g.h(5, 8, 12)


@icon("system")
def image(g):
    g.rect(1, 2, 14, 13)
    g.disc(10, 5.5, 1.5)
    g.line(2, 12, 6, 7); g.line(6, 7, 9, 10); g.line(9, 10, 11, 8); g.line(11, 8, 13, 10)


@icon("system")
def table(g):
    g.rect(1, 2, 14, 13); g.fill(1, 2, 14, 4); g.v(6, 2, 13); g.h(1, 14, 8)


@icon("system")
def branch(g):
    g.ring(4, 3, 2); g.ring(4, 12, 2); g.ring(11.5, 3.5, 2)
    g.v(4, 5, 10); g.line(11, 6, 5, 10)


@icon("system")
def cpu(g):
    g.rect(3, 3, 12, 12); g.fill(6, 6, 9, 9)
    for k in (5, 7, 8, 10):
        g.v(k, 1, 2); g.v(k, 13, 14); g.h(1, 2, k); g.h(13, 14, k)


@icon("system")
def clock(g):
    g.ring(7.5, 7.5, 7); g.v(7, 3, 8); g.h(8, 11, 8)


@icon("system")
def warning(g):
    g.line(7, 1, 1, 13); g.line(8, 1, 14, 13); g.h(1, 14, 13); g.h(1, 14, 14)
    g.line(7, 2, 2, 13); g.line(8, 2, 13, 13)
    g.fill(7, 5, 8, 9); g.fill(7, 11, 8, 11)


@icon("system")
def info(g):
    g.ring(7.5, 7.5, 7); g.fill(7, 4, 8, 4); g.fill(7, 6, 8, 11); g.px(6, 6); g.h(6, 9, 11)


@icon("system")
def link(g):
    g.rounded(1, 5, 8, 10); g.rounded(7, 5, 14, 10)
    g.h(5, 10, 7); g.h(5, 10, 8)
    g.v(8, 6, 9, 0); g.v(7, 6, 9, 0)


@icon("system")
def bolt(g):
    g.line(10, 1, 4, 8); g.h(4, 11, 8); g.line(11, 8, 5, 14)
    g.line(9, 1, 3, 8); g.line(12, 8, 6, 14); g.h(4, 9, 7)


@icon("system")
def eye(g):
    g.line(1, 8, 5, 4); g.h(5, 10, 4); g.line(10, 4, 14, 8)
    g.line(1, 8, 5, 12); g.h(5, 10, 12); g.line(10, 12, 14, 8)
    g.disc(7.5, 8, 2)


@icon("system")
def keyboard(g):
    g.rect(1, 3, 14, 12)
    for x in (3, 5, 7, 9, 11):
        g.px(x, 5); g.px(x + 1, 7)
    g.h(4, 11, 10)


@icon("system")
def sliders(g):
    for x, k in ((3, 9), (8, 4), (12, 11)):
        g.v(x, 1, 14); g.fill(x - 1, k - 1, x + 1, k + 1)


@icon("system")
def swarm(g):
    g.fill(2, 2, 5, 5); g.fill(10, 2, 13, 5); g.fill(6, 10, 9, 13)
    g.line(5, 5, 7, 9); g.line(10, 5, 8, 9); g.h(6, 9, 3)


HAND = {
    "search": ["................", "...######.......", "..#......#......", ".#........#.....", ".#........#.....", ".#........#.....", ".#........#.....", ".#........#.....", "..#......#......", "...######.##....", "..........###...", "...........###..", "............###.", ".............##.", "................", "................"],
    "refresh": ["................", "....#####.#.....", "...#.....###....", "..#.......###...", ".#........##....", ".#........#.....", ".#..............", ".#............#.", ".#............#.", ".#............#.", "..#..........#..", "...#........#...", "....########....", "................", "................", "................"],
    "thinking": ["................", "....###.###.....", "...#...#...#....", "..#.........#...", ".#..#..#..#..#..", ".#...........#..", "..#.........#...", "...#########....", "................", "...##...........", "...##...........", "................", ".#..............", "................", "................", "................"],
    "swarm": ["................", "......####......", "......####......", "......####......", ".......##.......", "..############..", "..#....##....#..", "..#....##....#..", "..#....##....#..", ".###..####..###.", ".###..####..###.", ".###..####..###.", "................", "................", "................", "................"],
    "mention": ["................", ".....######.....", "...##......##...", "..#..........#..", ".#....###.....#.", ".#...#...#....#.", ".#..#....#....#.", ".#..#....#....#.", ".#..#...##...#..", ".#...###.####...", "..#.............", "...##.......#...", ".....#######....", "................", "................", "................"],
    "knot": ["................", "......####......", "....##....##....", "..##........##..", ".#............#.", ".#............#.", ".#....####....#.", ".#...#....#...#.", ".#...#....#...#.", ".#....####....#.", ".#............#.", "..##........##..", "....##....##....", "......####......", "................", "................"],
}


def mascots():
    rows = {}
    rows["blob"] = ["................", "................", ".....######.....", "...##########...", "..############..", ".##############.", ".##############.", "################", "################", "################", "################", ".##############.", ".##############.", "..##.##..##.##..", "................", "................"]
    rows["cat"] = ["................", "..#..........#..", "..##........##..", "..###......###..", "..############..", ".##############.", ".##############.", ".##############.", ".##############.", ".##############.", "..############..", "...##########...", "...##......##...", "...##......##...", "................", "................"]
    rows["ghost"] = ["................", "......####......", "....########....", "...##########...", "..############..", "..############..", "..############..", "..############..", "..############..", "..############..", "..############..", "..############..", "..############..", "..##.##..##.##..", "..#...#..#...#..", "................"]
    rows["bot"] = [".......##.......", ".......##.......", "...##########...", "...##########...", "..############..", "..############..", "..############..", "..############..", "..############..", "..############..", "...##########...", "....########....", "....##....##....", "...###....###...", "................", "................"]
    face = [(5, 6), (6, 6), (9, 6), (10, 6), (7, 10), (8, 10)]
    out = {}
    for k, r in rows.items():
        g = [list(x) for x in r]
        for x, y in face:
            g[y][x] = "."
        out["mascot_" + k] = ("mascot", ["".join(x) for x in g])
    return out


def main():
    ICONS.update(mascots())
    ICONS.pop("router", None)
    for k, rows in HAND.items():
        ICONS[k] = (ICONS[k][0], rows)
    for name, (_, rows) in ICONS.items():
        assert len(rows) == N and all(len(r) == N for r in rows), name
    txt = "\n".join(f"{name} {grp}\n" + "\n".join(rows) + "\n" for name, (grp, rows) in ICONS.items())
    out = ROOT / "assets/icons/icons.txt"
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(txt)
    groups = {}
    for name, (grp, rows) in ICONS.items():
        groups.setdefault(grp, []).append((name, rows))
    def svg(rows, size):
        rects = "".join(f'<rect x="{x}" y="{y}" width="1" height="1"/>' for y, r in enumerate(rows) for x, c in enumerate(r) if c == "#")
        return f'<svg width="{size}" height="{size}" viewBox="0 0 16 16" shape-rendering="crispEdges">{rects}</svg>'
    def grid(rows):
        cells = "".join(f'<i class="{"on" if c == "#" else ""}"></i>' for r in rows for c in r)
        return f'<div class="grid">{cells}</div>'
    titles = {"action": "actions", "agent": "agents", "state": "swarm states", "provider": "providers and accounts", "system": "content and system", "mascot": "swarm mascots"}
    html = ["""<!doctype html><meta charset="utf-8"><title>Raster Icons</title>
<link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=JetBrains+Mono:wght@400;600&family=Silkscreen&display=swap">
<style>
body{margin:0;background:#000;color:#ececec;font:12px/1.4 "JetBrains Mono",monospace;padding:24px;width:1500px}
h1{font:18px "Silkscreen",monospace;margin:0 0 4px}p{color:#8a8a8a;margin:0 0 20px}
h2{font:12px "Silkscreen",monospace;color:#8a8a8a;margin:22px 0 10px;border-bottom:1px dotted #8a8a8a;padding-bottom:4px}
.set{display:grid;grid-template-columns:repeat(8,1fr);gap:14px}
.c{border:1px solid #8a8a8a;box-shadow:4px 4px 0 #2e2e2e;padding:10px;display:flex;flex-direction:column;gap:8px;align-items:center}
.grid{display:grid;grid-template-columns:repeat(16,7px);grid-template-rows:repeat(16,7px);gap:1px;background:#1a1a1a;padding:1px}
.grid i{background:#000}.grid i.on{background:#ececec}
.sizes{display:flex;gap:10px;align-items:flex-end}.sizes svg{fill:#ececec}
.inv{background:#ececec;padding:3px}.inv svg{fill:#000}
.n{color:#8a8a8a}
</style><h1>Raster icons</h1><p>""" + f"{len(ICONS)} icons · 16x16 · 1px stroke, 1px margin · shown at 7x grid, 1x, 2x, and inverted. Source: assets/icons/icons.txt</p>"]
    for grp in ("action", "agent", "state", "provider", "system", "mascot"):
        html.append(f"<h2>{titles[grp]} · {len(groups.get(grp, []))}</h2><div class='set'>")
        for name, rows in groups.get(grp, []):
            html.append(f"<div class='c'>{grid(rows)}<div class='sizes'>{svg(rows, 16)}{svg(rows, 32)}<span class='inv'>{svg(rows, 16)}</span></div><span class='n'>{name}</span></div>")
        html.append("</div>")
    (ROOT / "docs/mockups/icons.html").write_text("".join(html))
    print(len(ICONS), "icons")


if __name__ == "__main__":
    main()
