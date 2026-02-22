"""Image Manipulation Tool — resize, crop, convert, and batch-convert images."""

import fnmatch
from pathlib import Path
from typing import Annotated

from tooli import Tooli, Argument, Option
from tooli.annotations import Destructive, Idempotent
from tooli.errors import InputError, StateError, ToolRuntimeError, Suggestion

try:
    from PIL import Image
except ImportError as _pil_exc:
    raise ImportError(
        "Pillow is required for the image manipulation tool. "
        "Install it with: pip install Pillow"
    ) from _pil_exc

app = Tooli(
    name="img-manipulate",
    description="Resize, crop, convert, and batch-process images.",
    version="0.1.0",
)

SUPPORTED_FORMATS = {"jpeg", "jpg", "png", "gif", "bmp", "webp", "tiff", "tif"}
FORMAT_EXTENSIONS = {"jpeg": "jpg", "tiff": "tif"}  # canonical extension overrides


def _resolve_format(fmt: str) -> str:
    fmt = fmt.lower().lstrip(".")
    if fmt not in SUPPORTED_FORMATS:
        raise InputError(
            message=f"Unsupported format '{fmt}'. Supported: {', '.join(sorted(SUPPORTED_FORMATS))}",
            code="E1004",
            field="format",
        )
    return fmt


def _open_image(path: str) -> tuple[Image.Image, Path]:
    p = Path(path)
    if not p.exists():
        raise StateError(
            message=f"Image file not found: {path}",
            code="E3001",
            suggestion=Suggestion(
                action="retry_with_modified_input",
                fix="Check the file path is correct.",
                example="img-manipulate resize photo.jpg --width 800",
            ),
        )
    try:
        img = Image.open(p)
        img.load()
        return img, p
    except Exception as exc:
        raise ToolRuntimeError(
            message=f"Could not open image: {path} — {exc}",
            code="E4001",
            details={"path": str(p)},
        ) from exc


def _default_output(source: Path, suffix: str, fmt: str | None = None) -> Path:
    ext = FORMAT_EXTENSIONS.get(fmt, fmt) if fmt else source.suffix.lstrip(".")
    return source.parent / f"{source.stem}{suffix}.{ext}"


def _save_image(img: Image.Image, output: Path, fmt: str) -> None:
    pil_fmt = "JPEG" if fmt in {"jpg", "jpeg"} else fmt.upper()
    try:
        output.parent.mkdir(parents=True, exist_ok=True)
        if pil_fmt == "JPEG" and img.mode in ("RGBA", "P"):
            img = img.convert("RGB")
        img.save(output, format=pil_fmt)
    except Exception as exc:
        raise ToolRuntimeError(
            message=f"Failed to save image to {output}: {exc}",
            code="E4002",
            details={"output": str(output)},
        ) from exc


# ---------------------------------------------------------------------------
# Commands
# ---------------------------------------------------------------------------

@app.command(
    annotations=Destructive | Idempotent,
    task_group="Transform",
    when_to_use="Resize an image to specific pixel dimensions or a scale factor",
    supports_dry_run=True,
    examples=[
        {"args": ["photo.jpg", "--width", "800"], "description": "Resize to 800px wide, preserve aspect ratio"},
        {"args": ["photo.jpg", "--width", "200", "--height", "200"], "description": "Resize to 200×200 (contain)"},
        {"args": ["photo.jpg", "--scale", "0.5"], "description": "Resize to 50% of original"},
    ],
    error_codes={
        "E1001": "Neither --width, --height, nor --scale was provided",
        "E1002": "--scale must be greater than 0",
        "E1003": "--width and --height must be positive integers",
        "E3001": "Image file not found",
        "E4001": "Could not open image",
        "E4002": "Failed to save output",
    },
    output_example={"output": "photo_resized.jpg", "original_size": [1920, 1080], "new_size": [800, 450]},
)
def resize(
    input_path: Annotated[str, Argument(help="Source image file")],
    out_file: Annotated[str | None, Option(help="Output file path (default: <name>_resized.<ext>)")] = None,
    width: Annotated[int | None, Option(help="Target width in pixels")] = None,
    height: Annotated[int | None, Option(help="Target height in pixels")] = None,
    scale: Annotated[float | None, Option(help="Scale factor (e.g. 0.5 for 50%)")] = None,
    fit: Annotated[str, Option(help="How to fit when both width and height are given: contain, cover, stretch")] = "contain",
) -> dict:
    """Resize an image by dimensions or scale factor.

    When only one of --width or --height is given the other is computed from
    the original aspect ratio. When both are given, --fit controls behaviour:
    contain (default) — fit within the box; cover — fill the box; stretch — ignore aspect ratio.
    """
    if width is None and height is None and scale is None:
        raise InputError(
            message="Provide at least one of --width, --height, or --scale.",
            code="E1001",
        )
    if scale is not None and scale <= 0:
        raise InputError(message="--scale must be greater than 0.", code="E1002", field="scale")
    if (width is not None and width <= 0) or (height is not None and height <= 0):
        raise InputError(message="--width and --height must be positive integers.", code="E1003")
    if fit not in ("contain", "cover", "stretch"):
        raise InputError(message=f"--fit must be contain, cover, or stretch; got '{fit}'", code="E1005", field="fit")

    img, src = _open_image(input_path)
    orig_w, orig_h = img.size

    if scale is not None:
        new_w = max(1, int(orig_w * scale))
        new_h = max(1, int(orig_h * scale))
    elif width and height:
        if fit == "stretch":
            new_w, new_h = width, height
        elif fit == "cover":
            ratio = max(width / orig_w, height / orig_h)
            new_w, new_h = int(orig_w * ratio), int(orig_h * ratio)
        else:  # contain
            ratio = min(width / orig_w, height / orig_h)
            new_w, new_h = int(orig_w * ratio), int(orig_h * ratio)
    elif width:
        new_w = width
        new_h = max(1, int(orig_h * width / orig_w))
    else:
        new_h = height
        new_w = max(1, int(orig_w * height / orig_h))

    out_path = Path(out_file) if out_file else _default_output(src, "_resized")
    fmt = out_path.suffix.lstrip(".").lower() or src.suffix.lstrip(".").lower()
    fmt = _resolve_format(fmt)

    resized = img.resize((new_w, new_h), Image.LANCZOS)
    _save_image(resized, out_path, fmt)

    return {
        "output": str(out_path),
        "original_size": [orig_w, orig_h],
        "new_size": [new_w, new_h],
        "format": fmt,
    }


@app.command(
    annotations=Destructive | Idempotent,
    task_group="Transform",
    when_to_use="Crop an image to a rectangular region",
    supports_dry_run=True,
    examples=[
        {"args": ["photo.jpg", "--x", "100", "--y", "50", "--width", "400", "--height", "300"],
         "description": "Crop a 400×300 region from (100, 50)"},
        {"args": ["photo.jpg", "--width", "200", "--height", "200"],
         "description": "Center crop to 200×200"},
    ],
    error_codes={
        "E1001": "Crop dimensions would produce an empty image",
        "E3001": "Image file not found",
        "E4001": "Could not open image",
        "E4002": "Failed to save output",
    },
    output_example={"output": "photo_cropped.jpg", "crop_box": [100, 50, 500, 350], "new_size": [400, 300]},
)
def crop(
    input_path: Annotated[str, Argument(help="Source image file")],
    out_file: Annotated[str | None, Option(help="Output file path (default: <name>_cropped.<ext>)")] = None,
    x: Annotated[int, Option(help="Left edge of crop box (pixels from left)")] = 0,
    y: Annotated[int, Option(help="Top edge of crop box (pixels from top)")] = 0,
    width: Annotated[int | None, Option(help="Crop width in pixels (default: full width)")] = None,
    height: Annotated[int | None, Option(help="Crop height in pixels (default: full height)")] = None,
) -> dict:
    """Crop an image to a rectangular region.

    Coordinates are clamped to image bounds. When --x and --y are omitted and
    both --width and --height are provided, the crop is centered on the image.
    """
    img, src = _open_image(input_path)
    orig_w, orig_h = img.size

    crop_w = min(width or orig_w, orig_w)
    crop_h = min(height or orig_h, orig_h)

    # Center if x/y not specified but width/height were given
    if x == 0 and y == 0 and (width or height):
        x = max(0, (orig_w - crop_w) // 2)
        y = max(0, (orig_h - crop_h) // 2)

    # Clamp box to image bounds
    left = max(0, min(x, orig_w))
    top = max(0, min(y, orig_h))
    right = min(left + crop_w, orig_w)
    bottom = min(top + crop_h, orig_h)

    if right <= left or bottom <= top:
        raise InputError(
            message=f"Crop region ({left},{top})-({right},{bottom}) produces an empty image.",
            code="E1001",
            details={"image_size": [orig_w, orig_h], "crop_box": [left, top, right, bottom]},
        )

    out_path = Path(out_file) if out_file else _default_output(src, "_cropped")
    fmt = out_path.suffix.lstrip(".").lower() or src.suffix.lstrip(".").lower()
    fmt = _resolve_format(fmt)

    cropped = img.crop((left, top, right, bottom))
    _save_image(cropped, out_path, fmt)

    return {
        "output": str(out_path),
        "crop_box": [left, top, right, bottom],
        "new_size": [right - left, bottom - top],
        "format": fmt,
    }


@app.command(
    annotations=Destructive | Idempotent,
    task_group="Transform",
    when_to_use="Convert an image to a different file format",
    supports_dry_run=True,
    examples=[
        {"args": ["photo.bmp", "--format", "webp"], "description": "Convert BMP to WebP"},
        {"args": ["photo.png", "--format", "jpeg"], "description": "Convert PNG to JPEG"},
    ],
    error_codes={
        "E1004": "Unsupported target format",
        "E3001": "Image file not found",
        "E4001": "Could not open image",
        "E4002": "Failed to save output",
    },
    output_example={"output": "photo.webp", "original_format": "PNG", "new_format": "webp"},
)
def convert(
    input_path: Annotated[str, Argument(help="Source image file")],
    format: Annotated[str, Option(help="Target format: jpeg, png, webp, gif, bmp, tiff")] = "png",
    out_file: Annotated[str | None, Option(help="Output file path (default: same name, new extension)")] = None,
) -> dict:
    """Convert an image to a different format."""
    fmt = _resolve_format(format)
    img, src = _open_image(input_path)
    orig_format = img.format or src.suffix.lstrip(".").upper()

    ext = FORMAT_EXTENSIONS.get(fmt, fmt)
    out_path = Path(out_file) if out_file else src.with_suffix(f".{ext}")
    _save_image(img, out_path, fmt)

    return {
        "output": str(out_path),
        "original_format": orig_format,
        "new_format": fmt,
    }


@app.command(
    name="batch-convert",
    annotations=Destructive | Idempotent,
    task_group="Batch",
    when_to_use="Convert all images in a directory to a target format",
    supports_dry_run=True,
    examples=[
        {"args": ["./photos", "--format", "webp"], "description": "Convert all images in ./photos to WebP"},
        {"args": ["./assets", "--format", "png", "--pattern", "*.bmp"],
         "description": "Convert only BMP files to PNG"},
    ],
    error_codes={
        "E1004": "Unsupported target format",
        "E3001": "Directory not found",
        "E3003": "No matching images found",
        "E4001": "Could not open one or more images",
        "E4002": "Failed to save one or more outputs",
    },
    output_example=[{"input": "photo.bmp", "output": "photo.webp", "ok": True}],
    paginated=True,
)
def batch_convert(
    directory: Annotated[str, Argument(help="Directory containing images to convert")],
    format: Annotated[str, Option(help="Target format: jpeg, png, webp, gif, bmp, tiff")] = "png",
    output_dir: Annotated[str | None, Option(help="Output directory (default: same as input directory)")] = None,
    pattern: Annotated[str, Option(help="Glob pattern to filter input files (e.g. '*.bmp')")] = "*",
) -> list[dict]:
    """Convert all images in a directory to a target format.

    Original files are never modified. Converted files are written to --output-dir
    (or alongside the originals when omitted).
    """
    fmt = _resolve_format(format)
    src_dir = Path(directory)
    if not src_dir.is_dir():
        raise StateError(
            message=f"Directory not found: {directory}",
            code="E3001",
            suggestion=Suggestion(
                action="retry_with_modified_input",
                fix="Check the directory path is correct.",
                example=f"img-manipulate batch-convert ./images --format {fmt}",
            ),
        )

    out_dir = Path(output_dir) if output_dir else src_dir
    ext = FORMAT_EXTENSIONS.get(fmt, fmt)

    candidates = [
        f for f in src_dir.iterdir()
        if f.is_file() and fnmatch.fnmatch(f.name, pattern)
        and f.suffix.lstrip(".").lower() in SUPPORTED_FORMATS
    ]

    if not candidates:
        raise StateError(
            message=f"No matching image files found in {directory} (pattern: {pattern})",
            code="E3003",
            suggestion=Suggestion(
                action="retry_with_modified_input",
                fix="Broaden the --pattern or check the directory.",
                example=f"img-manipulate batch-convert {directory} --format {fmt} --pattern '*.png'",
            ),
        )

    results = []
    for src in sorted(candidates):
        out = out_dir / f"{src.stem}.{ext}"
        try:
            img = Image.open(src)
            img.load()
            _save_image(img, out, fmt)
            results.append({"input": str(src), "output": str(out), "ok": True})
        except Exception as exc:
            results.append({"input": str(src), "output": str(out), "ok": False, "error": str(exc)})

    return results


@app.command(
    name="add-background",
    annotations=Destructive | Idempotent,
    task_group="Transform",
    when_to_use="Fill the transparent areas of a PNG with a solid background colour",
    supports_dry_run=True,
    examples=[
        {"args": ["icon.png"], "description": "Add default black background to icon.png"},
        {"args": ["icon.png", "--color", "white"], "description": "Add white background"},
        {"args": ["icon.png", "--color", "#ff0000", "--output", "icon_bg.png"],
         "description": "Add red background and save to a new file"},
    ],
    error_codes={
        "E1006": "--color is not a valid CSS hex colour or named colour",
        "E3001": "Image file not found",
        "E4001": "Could not open image",
        "E4002": "Failed to save output",
    },
    output_example={"output": "icon_bg.png", "size": [512, 512], "background": "black", "had_alpha": True},
)
def add_background(
    input_path: Annotated[str, Argument(help="Source PNG file")],
    out_file: Annotated[str | None, Option(help="Output file path (default: <name>_bg.png)")] = None,
    color: Annotated[str, Option(help="Background colour — CSS name or hex (e.g. black, white, #ff0000)")] = "black",
) -> dict:
    """Flatten a transparent PNG onto a solid background colour.

    Any pixels with an alpha channel (including partial transparency) are
    composited over the chosen colour. The output is always a PNG so that
    the result can be further processed without lossy re-encoding.
    """
    img, src = _open_image(input_path)
    w, h = img.size
    had_alpha = img.mode in ("RGBA", "LA", "PA") or (img.mode == "P" and "transparency" in img.info)

    try:
        background = Image.new("RGBA", (w, h), color)
    except (ValueError, AttributeError) as exc:
        raise InputError(
            message=f"Invalid colour '{color}': {exc}",
            code="E1006",
            field="color",
            suggestion=Suggestion(
                action="retry_with_modified_input",
                fix="Use a CSS colour name (black, white, red, …) or a hex value like #rrggbb.",
                example=f"img-manipulate add-background {input_path} --color white",
            ),
        ) from exc

    # Composite the source over the background
    rgba = img.convert("RGBA")
    background.paste(rgba, mask=rgba.split()[3])  # use alpha channel as mask
    result = background.convert("RGB")

    out_path = Path(out_file) if out_file else _default_output(src, "_bg", "png")
    _save_image(result, out_path, "png")

    return {
        "output": str(out_path),
        "size": [w, h],
        "background": color,
        "had_alpha": had_alpha,
    }


if __name__ == "__main__":
    app()
