import io
import math
import struct
from pathlib import Path

from PIL import Image, ImageDraw, ImageFilter

SS = 4
S = 1024 * SS
RADIUS = 228
TOP = (106, 166, 255)
BOTTOM = (40, 100, 214)
BADGE = (17, 19, 25)
MARK = [
    (23.529, 29.167, 19.437),
    (45.271, 41.063, 9.220),
    (57.171, 29.167, 9.217),
    (45.271, 17.271, 9.221),
]
MARK_BOX = (4.096, 8.05, 66.388, 50.283)


def lerp(a, b, t):
    return tuple(round(a[i] + (b[i] - a[i]) * t) for i in range(len(a)))


def gradient(a, b):
    small = Image.new("RGBA", (64, 64))
    for y in range(64):
        for x in range(64):
            small.putpixel((x, y), lerp(a, b, (x + y) / 126) + (255,))
    return small.resize((S, S), Image.BICUBIC)


def solid(color):
    return Image.new("RGBA", (S, S), color + (255,))


def shadow(canvas, layer, offset, blur, strength):
    alpha = layer.split()[3].filter(ImageFilter.GaussianBlur(blur * SS))
    alpha = alpha.point(lambda value: int(value * strength))
    shade = Image.new("RGBA", (S, S), (0, 0, 0, 0))
    shade.paste(solid((0, 0, 0)), (0, offset * SS), alpha)
    canvas.alpha_composite(shade)


def diamonds(draw, center, unit):
    left, top, right, bottom = MARK_BOX
    mid_x, mid_y = (left + right) / 2, (top + bottom) / 2
    for x, y, r in MARK:
        px = center[0] + (x - mid_x) * unit
        py = center[1] + (y - mid_y) * unit
        rr = r * unit
        draw.polygon([(px, py - rr), (px + rr, py), (px, py + rr), (px - rr, py)], fill=255)


def arrows(draw, center, radius, width):
    cx, cy = center
    half = width / 2
    mid = radius - half
    for start in (-60, 120):
        end = start + 118
        draw.arc([cx - radius, cy - radius, cx + radius, cy + radius], start, end, fill=255, width=int(width))
        theta = math.radians(end)
        nx, ny = math.cos(theta), math.sin(theta)
        tx, ty = -ny, nx
        wing, length = width * 1.12, width * 1.5
        draw.polygon(
            [
                (cx + (mid + wing) * nx, cy + (mid + wing) * ny),
                (cx + mid * nx + tx * length, cy + mid * ny + ty * length),
                (cx + (mid - wing) * nx, cy + (mid - wing) * ny),
            ],
            fill=255,
        )
        cap = math.radians(start)
        ex, ey = cx + mid * math.cos(cap), cy + mid * math.sin(cap)
        draw.ellipse([ex - half, ey - half, ex + half, ey + half], fill=255)


def render():
    tile_mask = Image.new("L", (S, S), 0)
    ImageDraw.Draw(tile_mask).rounded_rectangle([0, 0, S - 1, S - 1], RADIUS * SS, fill=255)

    canvas = Image.new("RGBA", (S, S), (0, 0, 0, 0))
    canvas.paste(gradient(TOP, BOTTOM), (0, 0), tile_mask)

    highlight = Image.new("L", (S, S), 0)
    ImageDraw.Draw(highlight).rounded_rectangle(
        [4 * SS, 4 * SS, S - 1 - 4 * SS, S - 1 - 4 * SS], (RADIUS - 4) * SS, outline=60, width=4 * SS
    )
    canvas.paste(solid((255, 255, 255)), (0, 0), highlight)

    badge_center = (768 * SS, 768 * SS)
    badge_radius = 170 * SS
    cutout = 22 * SS
    bx, by = badge_center

    mark_mask = Image.new("L", (S, S), 0)
    diamonds(ImageDraw.Draw(mark_mask), (448 * SS, 452 * SS), 10.6 * SS)
    ImageDraw.Draw(mark_mask).ellipse(
        [bx - badge_radius - cutout, by - badge_radius - cutout, bx + badge_radius + cutout, by + badge_radius + cutout],
        fill=0,
    )
    mark = Image.new("RGBA", (S, S), (0, 0, 0, 0))
    mark.paste(solid((255, 255, 255)), (0, 0), mark_mask)
    shadow(canvas, mark, 10, 18, 0.28)
    canvas.alpha_composite(mark)

    badge = Image.new("RGBA", (S, S), (0, 0, 0, 0))
    badge_mask = Image.new("L", (S, S), 0)
    ImageDraw.Draw(badge_mask).ellipse([bx - badge_radius, by - badge_radius, bx + badge_radius, by + badge_radius], fill=255)
    badge.paste(solid(BADGE), (0, 0), badge_mask)
    arrow_mask = Image.new("L", (S, S), 0)
    arrows(ImageDraw.Draw(arrow_mask), badge_center, 104 * SS, 32 * SS)
    badge.paste(solid((255, 255, 255)), (0, 0), arrow_mask)
    shadow(canvas, badge, 8, 16, 0.4)
    canvas.alpha_composite(badge)

    clipped = Image.new("RGBA", (S, S), (0, 0, 0, 0))
    clipped.paste(canvas, (0, 0), tile_mask)
    return clipped.resize((1024, 1024), Image.LANCZOS)


def png_bytes(image, size):
    buffer = io.BytesIO()
    image.resize((size, size), Image.LANCZOS).save(buffer, "PNG", optimize=True)
    return buffer.getvalue()


def ico(image, sizes):
    images = [png_bytes(image, size) for size in sizes]
    header = struct.pack("<HHH", 0, 1, len(sizes))
    offset = 6 + 16 * len(sizes)
    entries = b""
    for size, data in zip(sizes, images):
        side = 0 if size >= 256 else size
        entries += struct.pack("<BBBBHHII", side, side, 0, 0, 1, 32, len(data), offset)
        offset += len(data)
    return header + entries + b"".join(images)


def main():
    image = render()
    root = Path(__file__).resolve().parents[1]
    icon_png = png_bytes(image, 256)
    icon_ico = ico(image, [16, 24, 32, 48, 64, 128, 256])
    for folder in (root / "assets", root / "apps" / "accountchanger" / "assets"):
        (folder / "icon.png").write_bytes(icon_png)
        (folder / "icon.ico").write_bytes(icon_ico)


if __name__ == "__main__":
    main()
