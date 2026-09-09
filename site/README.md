# Landing page

An English, dark-only, 1980s-inspired Astro site. This is the project's public
landing page; the miau application remains terminal-only. Astro produces static
HTML and CSS, with no client-side JavaScript required.

## Local development

Use Node.js 24 and npm. From this directory:

```sh
npm ci
npm run dev
```

Open `http://localhost:4321/miau/`. To verify the production output:

```sh
npm test
npm run preview
```

Open `http://localhost:4321/miau/` for the production preview. Astro 7 runs the
preview in the background; stop it with `npm run preview -- stop` when finished.

`npm test` builds the site and runs the Python 3 standard-library checks in
`../scripts/test_site.py`. The checks validate local links, the GitHub Pages
project prefix, image assets, and essential accessibility metadata. Also inspect
desktop and mobile layouts when changing presentation.

The page is in `src/pages/index.astro`, styles in `src/styles/global.css`, and
original SVG artwork in `public/`. Terminal images are labelled illustrations
with example content, not captured screenshots. Keep their behaviour consistent
with the application and their descriptions useful to screen-reader users.

Space Grotesk and Space Mono are bundled locally through Fontsource. Their SIL
Open Font License notices ship alongside the site in `public/`.

## GitHub Pages

The site is configured for `https://dramoscalvo.github.io/miau/` with the `/miau`
base path. In the repository's **Settings → Pages → Build and deployment**, set
**Source** to **GitHub Actions**. Merge the site and workflow into `main`, or run
the **Landing page** workflow manually from `main` after enabling Pages.

The workflow builds and checks pull requests without deploying them. Changes to
the site on `main` build, test, and deploy `dist/` to the `github-pages`
environment. No secrets or separate publishing branch are needed.

For a fork or custom domain, update `site` and `base` in `astro.config.mjs`, the
repository and canonical URLs in `src/pages/index.astro`, and `BASE` in the site
test. All local asset URLs must use the configured base.

References: [Astro's Pages guide](https://docs.astro.build/en/guides/deploy/github/)
and [GitHub's custom workflow guide](https://docs.github.com/en/pages/getting-started-with-github-pages/using-custom-workflows-with-github-pages).
