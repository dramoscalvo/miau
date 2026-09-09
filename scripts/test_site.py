"""Check the static site's portable links and essential accessibility metadata."""

from html.parser import HTMLParser
from pathlib import Path
import json
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


class EnglishSiteTests(unittest.TestCase):
    locale = "en"
    route = "index.html"
    def setUp(self):
        self.page = Page((SITE / self.route).read_text(encoding="utf-8"))

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
                        target = local_path(url.path)
                        self.assertTrue(target.is_file() or (target / "index.html").is_file())
                    elif url.fragment:
                        self.assertIn(unquote(url.fragment), ids)
                    else:
                        self.fail("Empty link")

    def test_accessible_document_and_illustrations(self):
        self.assertTrue(any(tag == "html" and attrs.get("lang") == self.locale
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

    def test_language_switch_and_search_metadata(self):
        expected = {"en": BASE, "es": BASE + "es/"}
        switches = {attrs.get("hreflang"): attrs for tag, attrs in self.page.elements
                    if tag == "a" and attrs.get("hreflang")}
        self.assertEqual(set(switches), set(expected))
        for locale, path in expected.items():
            self.assertEqual(switches[locale]["href"], path)
            self.assertEqual(switches[locale].get("lang"), locale)
            self.assertEqual(switches[locale].get("aria-current"),
                             "page" if locale == self.locale else None)
        origin = "https://dramoscalvo.github.io"
        links = [attrs for tag, attrs in self.page.elements if tag == "link"]
        self.assertIn({"rel": "canonical", "href": origin + expected[self.locale]}, links)
        alternates = {attrs["hreflang"]: attrs["href"] for attrs in links
                      if attrs.get("rel") == "alternate"}
        self.assertEqual(alternates, {"en": origin + BASE, "es": origin + BASE + "es/",
                                      "x-default": origin + BASE})


class SpanishSiteTests(EnglishSiteTests):
    locale = "es"
    route = "es/index.html"

    def test_spanish_copy_and_accessibility_labels(self):
        source = (SITE / self.route).read_text(encoding="utf-8")
        self.assertIn("Las máquinas trabajan.", source)
        self.assertIn("Saltar al contenido", source)
        self.assertNotIn("Skip to content", source)
        self.assertNotIn("HUMAN DECISION", source)
        self.assertNotIn("Installation commands", source)


class TranslationTests(unittest.TestCase):
    def test_dictionaries_have_matching_shapes_and_nonempty_copy(self):
        directory = SITE.parent / "src" / "i18n"
        en = json.loads((directory / "en.json").read_text(encoding="utf-8"))
        es = json.loads((directory / "es.json").read_text(encoding="utf-8"))

        def compare(first, second):
            self.assertIs(type(first), type(second))
            if isinstance(first, dict):
                self.assertEqual(first.keys(), second.keys())
                for key in first:
                    with self.subTest(key=key):
                        compare(first[key], second[key])
            elif isinstance(first, list):
                self.assertEqual(len(first), len(second))
                for a, b in zip(first, second):
                    compare(a, b)
            else:
                self.assertIsInstance(first, str)
                self.assertTrue(first.strip())
                self.assertTrue(second.strip())

        compare(en, es)


if __name__ == "__main__":
    unittest.main()
