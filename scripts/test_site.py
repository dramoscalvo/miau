"""Check the static site's portable links and essential accessibility metadata."""

from html.parser import HTMLParser
from pathlib import Path
import unittest
from urllib.parse import unquote, urlsplit
import xml.etree.ElementTree as ET


SITE = Path(__file__).resolve().parents[1] / "site" / "dist"
BASE = "/miau/"


def local_path(path):
    if path.startswith("/"):
        assert path.startswith(BASE), f"Asset escapes GitHub Pages base: {path}"
        path = path.removeprefix(BASE)
    return SITE / unquote(path)


class Page(HTMLParser):
    def __init__(self, source):
        super().__init__()
        self.elements = []
        self.feed(source)

    def handle_starttag(self, tag, attrs):
        self.elements.append((tag, dict(attrs)))


class SiteTests(unittest.TestCase):
    def setUp(self):
        self.page = Page((SITE / "index.html").read_text())

    def test_links_work_under_a_github_project_path(self):
        ids = [attrs["id"] for _, attrs in self.page.elements if "id" in attrs]
        self.assertEqual(len(ids), len(set(ids)), "Duplicate anchor IDs")
        for tag, attrs in self.page.elements:
            for key in ("href", "src"):
                if key not in attrs:
                    continue
                url = urlsplit(attrs[key])
                with self.subTest(url=attrs[key]):
                    if url.scheme or url.netloc:
                        self.assertEqual(url.scheme, "https")
                        continue
                    if url.path:
                        self.assertTrue(local_path(url.path).is_file())
                    elif url.fragment:
                        self.assertIn(unquote(url.fragment), ids)
                    else:
                        self.fail("Empty link")

    def test_accessible_document_and_illustrations(self):
        self.assertTrue(any(tag == "html" and attrs.get("lang") == "en"
                            for tag, attrs in self.page.elements))
        self.assertEqual(sum(tag == "h1" for tag, _ in self.page.elements), 1)
        self.assertTrue(any(tag == "meta" and attrs.get("name") == "viewport"
                            for tag, attrs in self.page.elements))
        images = [attrs for tag, attrs in self.page.elements if tag == "img"]
        self.assertGreaterEqual(len(images), 2)
        for attrs in images:
            self.assertTrue(attrs.get("alt"))
            self.assertTrue(attrs.get("width") and attrs.get("height"))
            if attrs["src"].endswith(".svg"):
                ET.parse(local_path(attrs["src"]))


if __name__ == "__main__":
    unittest.main()
