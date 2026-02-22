"""Tests for tools/img_manipulate.py"""

import pytest
from pathlib import Path
from PIL import Image

import sys
sys.path.insert(0, str(Path(__file__).parent.parent))

from tools.img_manipulate import app, _resolve_format, _default_output


# ---------------------------------------------------------------------------
# Fixtures
# ---------------------------------------------------------------------------

@pytest.fixture
def img_factory(tmp_path):
    """Return a factory that creates test images and returns their paths."""
    def _make(name: str = "test.png", size: tuple = (400, 300), color: str = "red",
              mode: str = "RGB", fmt: str = "PNG") -> str:
        p = tmp_path / name
        img = Image.new(mode, size, color)
        img.save(p, format=fmt)
        return str(p)
    return _make


@pytest.fixture
def img_dir(tmp_path, img_factory):
    """Create a directory with several test images."""
    d = tmp_path / "images"
    d.mkdir()
    for name, size in [("a.png", (100, 100)), ("b.png", (200, 150)), ("c.bmp", (50, 50))]:
        img = Image.new("RGB", size, "blue")
        img.save(d / name)
    return str(d)


# ---------------------------------------------------------------------------
# Helper unit tests
# ---------------------------------------------------------------------------

class TestResolveFormat:
    def test_valid_formats(self):
        assert _resolve_format("png") == "png"
        assert _resolve_format("JPEG") == "jpeg"
        assert _resolve_format(".webp") == "webp"
        assert _resolve_format("GIF") == "gif"

    def test_invalid_format(self):
        from tooli.errors import InputError
        with pytest.raises(InputError) as exc_info:
            _resolve_format("xyz")
        assert exc_info.value.code == "E1004"


# ---------------------------------------------------------------------------
# Resize command
# ---------------------------------------------------------------------------

class TestResizeCommand:
    def test_resize_by_width(self, img_factory, tmp_path):
        src = img_factory("photo.png", size=(800, 600))
        out = str(tmp_path / "out.png")
        result = app.call("resize", input_path=src, out_file=out, width=400)
        assert result.ok
        assert result.result["new_size"] == [400, 300]
        assert result.result["original_size"] == [800, 600]
        img = Image.open(out)
        assert img.size == (400, 300)

    def test_resize_by_height(self, img_factory, tmp_path):
        src = img_factory("photo.png", size=(800, 600))
        out = str(tmp_path / "out.png")
        result = app.call("resize", input_path=src, out_file=out, height=150)
        assert result.ok
        assert result.result["new_size"][1] == 150

    def test_resize_by_scale(self, img_factory, tmp_path):
        src = img_factory("photo.png", size=(400, 200))
        out = str(tmp_path / "out.png")
        result = app.call("resize", input_path=src, out_file=out, scale=0.5)
        assert result.ok
        assert result.result["new_size"] == [200, 100]

    def test_resize_both_dimensions_contain(self, img_factory, tmp_path):
        src = img_factory("photo.png", size=(800, 400))
        out = str(tmp_path / "out.png")
        result = app.call("resize", input_path=src, out_file=out, width=200, height=200, fit="contain")
        assert result.ok
        w, h = result.result["new_size"]
        assert w <= 200 and h <= 200

    def test_resize_both_dimensions_stretch(self, img_factory, tmp_path):
        src = img_factory("photo.png", size=(800, 400))
        out = str(tmp_path / "out.png")
        result = app.call("resize", input_path=src, out_file=out, width=100, height=100, fit="stretch")
        assert result.ok
        assert result.result["new_size"] == [100, 100]

    def test_resize_no_params_error(self, img_factory):
        src = img_factory()
        result = app.call("resize", input_path=src)
        assert not result.ok
        assert result.error.code == "E1001"

    def test_resize_invalid_scale(self, img_factory):
        src = img_factory()
        result = app.call("resize", input_path=src, scale=-1.0)
        assert not result.ok
        assert result.error.code == "E1002"

    def test_resize_missing_file(self):
        result = app.call("resize", input_path="/no/such/file.png", width=100)
        assert not result.ok
        assert result.error.code == "E3001"

    def test_default_output_name(self, img_factory):
        src = img_factory("photo.png", size=(100, 100))
        result = app.call("resize", input_path=src, width=50)
        assert result.ok
        assert "photo_resized" in result.result["output"]

    def test_invalid_fit(self, img_factory):
        src = img_factory()
        result = app.call("resize", input_path=src, width=100, height=100, fit="center")
        assert not result.ok
        assert result.error.code == "E1005"


# ---------------------------------------------------------------------------
# Crop command
# ---------------------------------------------------------------------------

class TestCropCommand:
    def test_basic_crop(self, img_factory, tmp_path):
        src = img_factory("photo.png", size=(400, 300))
        out = str(tmp_path / "cropped.png")
        result = app.call("crop", input_path=src, out_file=out, x=50, y=50, width=100, height=80)
        assert result.ok
        assert result.result["new_size"] == [100, 80]
        img = Image.open(out)
        assert img.size == (100, 80)

    def test_center_crop(self, img_factory, tmp_path):
        src = img_factory("photo.png", size=(400, 300))
        out = str(tmp_path / "center.png")
        result = app.call("crop", input_path=src, out_file=out, width=200, height=200)
        assert result.ok
        assert result.result["new_size"] == [200, 200]

    def test_crop_clamped_to_bounds(self, img_factory, tmp_path):
        """Coordinates exceeding image bounds are clamped — no error raised."""
        src = img_factory("photo.png", size=(100, 100))
        out = str(tmp_path / "clamped.png")
        result = app.call("crop", input_path=src, out_file=out, x=80, y=80, width=50, height=50)
        assert result.ok
        w, h = result.result["new_size"]
        assert w == 20 and h == 20  # clamped to image bounds

    def test_crop_empty_region(self, img_factory):
        """Crop starting at the image edge with no room produces InputError."""
        src = img_factory("photo.png", size=(100, 100))
        result = app.call("crop", input_path=src, x=100, y=100, width=50, height=50)
        assert not result.ok
        assert result.error.code == "E1001"

    def test_default_output_name(self, img_factory):
        src = img_factory("photo.png", size=(200, 200))
        result = app.call("crop", input_path=src, width=100, height=100)
        assert result.ok
        assert "photo_cropped" in result.result["output"]

    def test_missing_file(self):
        result = app.call("crop", input_path="/no/such.png")
        assert not result.ok
        assert result.error.code == "E3001"


# ---------------------------------------------------------------------------
# Convert command
# ---------------------------------------------------------------------------

class TestConvertCommand:
    def test_png_to_jpeg(self, img_factory, tmp_path):
        src = img_factory("photo.png", size=(100, 100))
        out = str(tmp_path / "photo.jpg")
        result = app.call("convert", input_path=src, out_file=out, format="jpeg")
        assert result.ok
        assert result.result["new_format"] == "jpeg"
        img = Image.open(out)
        assert img.format == "JPEG"

    def test_png_to_webp(self, img_factory, tmp_path):
        src = img_factory("photo.png", size=(50, 50))
        out = str(tmp_path / "photo.webp")
        result = app.call("convert", input_path=src, out_file=out, format="webp")
        assert result.ok
        assert result.result["new_format"] == "webp"

    def test_rgba_to_jpeg_converts_mode(self, img_factory, tmp_path):
        """RGBA images are auto-converted to RGB for JPEG output."""
        src = img_factory("rgba.png", size=(50, 50), mode="RGBA")
        out = str(tmp_path / "rgb.jpg")
        result = app.call("convert", input_path=src, out_file=out, format="jpeg")
        assert result.ok

    def test_default_output_extension(self, img_factory):
        src = img_factory("photo.bmp", size=(50, 50), fmt="BMP")
        result = app.call("convert", input_path=src, format="webp")
        assert result.ok
        assert result.result["output"].endswith(".webp")

    def test_unsupported_format(self, img_factory):
        src = img_factory()
        result = app.call("convert", input_path=src, format="svg")
        assert not result.ok
        assert result.error.code == "E1004"

    def test_missing_file(self):
        result = app.call("convert", input_path="/no/such.png")
        assert not result.ok
        assert result.error.code == "E3001"


# ---------------------------------------------------------------------------
# Batch-convert command
# ---------------------------------------------------------------------------

class TestBatchConvertCommand:
    def test_converts_all_images(self, img_dir, tmp_path):
        out_dir = str(tmp_path / "out")
        result = app.call("batch-convert", directory=img_dir, format="webp", output_dir=out_dir)
        assert result.ok
        ok_items = [r for r in result.result if r["ok"]]
        assert len(ok_items) >= 2  # at least the two PNGs

    def test_pattern_filter(self, img_dir, tmp_path):
        out_dir = str(tmp_path / "out")
        result = app.call("batch-convert", directory=img_dir, format="png",
                          output_dir=out_dir, pattern="*.bmp")
        assert result.ok
        assert len(result.result) == 1
        assert result.result[0]["input"].endswith(".bmp")

    def test_missing_directory(self, tmp_path):
        result = app.call("batch-convert", directory=str(tmp_path / "nonexistent"), format="png")
        assert not result.ok
        assert result.error.code == "E3001"

    def test_no_matching_files(self, img_dir):
        result = app.call("batch-convert", directory=img_dir, format="png", pattern="*.xyz")
        assert not result.ok
        assert result.error.code == "E3003"


# ---------------------------------------------------------------------------
# Agent interaction scenario tests
# ---------------------------------------------------------------------------

class TestAgentScenarios:
    def test_thumbnail_generation(self, img_factory, tmp_path):
        """Agent needs a 200×200 PNG thumbnail from a large image."""
        src = img_factory("hero.png", size=(1920, 1080))
        out = str(tmp_path / "thumb.png")
        result = app.call("resize", input_path=src, out_file=out, width=200, height=200, fit="contain")
        assert result.ok
        w, h = result.result["new_size"]
        assert w <= 200 and h <= 200
        assert Path(out).exists()

    def test_batch_bmp_to_webp(self, img_dir, tmp_path):
        """Agent converts all .bmp files in a directory to .webp."""
        out_dir = str(tmp_path / "converted")
        result = app.call("batch-convert", directory=img_dir, format="webp",
                          output_dir=out_dir, pattern="*.bmp")
        assert result.ok
        assert all(r["ok"] for r in result.result)
        assert all(r["output"].endswith(".webp") for r in result.result)

    def test_boundary_crop_graceful(self, img_factory):
        """Agent crops with coordinates exceeding image bounds — tool clamps, not crash."""
        src = img_factory("small.png", size=(100, 100))
        result = app.call("crop", input_path=src, x=0, y=0, width=9999, height=9999)
        assert result.ok
        assert result.result["new_size"] == [100, 100]

    def test_invalid_format_structured_error(self, img_factory):
        """Agent passes unsupported format — receives structured error for self-correction."""
        src = img_factory()
        result = app.call("convert", input_path=src, format="xyz123")
        assert not result.ok
        assert result.error.code == "E1004"
        assert result.error.message


# ---------------------------------------------------------------------------
# Add-background command
# ---------------------------------------------------------------------------

class TestAddBackgroundCommand:
    def test_rgba_gets_black_background(self, img_factory, tmp_path):
        """RGBA PNG with transparency gets a black background by default."""
        src = img_factory("icon.png", size=(100, 100), mode="RGBA", color=(0, 0, 0, 0))
        out = str(tmp_path / "icon_bg.png")
        result = app.call("add-background", input_path=src, out_file=out)
        assert result.ok
        assert result.result["had_alpha"] is True
        assert result.result["background"] == "black"
        img = Image.open(out)
        assert img.mode == "RGB"
        # All pixels should now be black (0, 0, 0)
        assert img.getpixel((50, 50)) == (0, 0, 0)

    def test_white_background(self, img_factory, tmp_path):
        src = img_factory("icon.png", size=(50, 50), mode="RGBA", color=(0, 0, 0, 0))
        out = str(tmp_path / "icon_white.png")
        result = app.call("add-background", input_path=src, out_file=out, color="white")
        assert result.ok
        img = Image.open(out)
        assert img.getpixel((25, 25)) == (255, 255, 255)

    def test_hex_color(self, img_factory, tmp_path):
        src = img_factory("icon.png", size=(50, 50), mode="RGBA", color=(0, 0, 0, 0))
        out = str(tmp_path / "icon_red.png")
        result = app.call("add-background", input_path=src, out_file=out, color="#ff0000")
        assert result.ok
        img = Image.open(out)
        assert img.getpixel((25, 25)) == (255, 0, 0)

    def test_opaque_image_has_no_alpha(self, img_factory, tmp_path):
        """Opaque RGB image: had_alpha is False, output is still written."""
        src = img_factory("solid.png", size=(50, 50), mode="RGB", color="blue")
        out = str(tmp_path / "solid_bg.png")
        result = app.call("add-background", input_path=src, out_file=out)
        assert result.ok
        assert result.result["had_alpha"] is False

    def test_partial_transparency_composited(self, img_factory, tmp_path):
        """Pixels with partial alpha are composited correctly over the background."""
        src = img_factory("semi.png", size=(10, 10), mode="RGBA", color=(255, 0, 0, 128))
        out = str(tmp_path / "semi_bg.png")
        result = app.call("add-background", input_path=src, out_file=out, color="white")
        assert result.ok
        img = Image.open(out)
        r, g, b = img.getpixel((5, 5))
        # 50% red over white -> should be approximately (255, 127, 127)
        assert r == 255
        assert 120 <= g <= 135
        assert 120 <= b <= 135

    def test_default_output_name(self, img_factory):
        src = img_factory("icon.png", size=(50, 50), mode="RGBA")
        result = app.call("add-background", input_path=src)
        assert result.ok
        assert result.result["output"].endswith("icon_bg.png")

    def test_output_is_always_png(self, img_factory, tmp_path):
        src = img_factory("icon.png", size=(50, 50), mode="RGBA")
        out = str(tmp_path / "out.png")
        result = app.call("add-background", input_path=src, out_file=out)
        assert result.ok
        img = Image.open(result.result["output"])
        assert img.format == "PNG"

    def test_invalid_color(self, img_factory):
        src = img_factory("icon.png", size=(50, 50), mode="RGBA")
        result = app.call("add-background", input_path=src, color="notacolor")
        assert not result.ok
        assert result.error.code == "E1006"

    def test_missing_file(self):
        result = app.call("add-background", input_path="/no/such/icon.png")
        assert not result.ok
        assert result.error.code == "E3001"

    def test_size_reported(self, img_factory, tmp_path):
        src = img_factory("icon.png", size=(200, 150), mode="RGBA")
        out = str(tmp_path / "out.png")
        result = app.call("add-background", input_path=src, out_file=out)
        assert result.ok
        assert result.result["size"] == [200, 150]
