# Site icons

The sources for the green oil derrick icons in `../public/`. Nothing here is served.

The file set and the markup in `../index.html` follow RealFaviconGenerator's current advice (<https://realfavicongenerator.net/favicon-guides/understanding-favicon-elements>): an SVG icon, one 96 px PNG (the size Google uses in search results), a `favicon.ico` for anything that asks for `/favicon.ico` by name, one opaque 180 px Apple touch icon, and a web app manifest with 192 and 512 px icons for Android.

| source | builds |
| --- | --- |
| `derrick.svg` | `public/favicon.svg` (a straight copy), `public/favicon-96x96.png`, and the 32 and 48 px sizes in `public/favicon.ico` |
| `derrick-small.svg` | the 16 px size in `public/favicon.ico`. It has fewer, thicker lines, because the full lattice blurs at 16 px |
| `apple-touch-icon.svg` | `public/apple-touch-icon.png`, 180 px on a dark background. iOS fills a clear background with black and rounds the corners itself, so the PNG is square and opaque |
| `web-app-manifest.svg` | `public/web-app-manifest-192x192.png` and `-512x512.png`, named in `public/site.webmanifest`. The same picture as the Apple icon with the derrick smaller, because the manifest marks them `maskable`: Android may crop them to a circle as small as the middle 80%, and the whole derrick must sit inside it |

To rebuild after editing a source, from `src/vue` (needs `rsvg-convert` from librsvg, and ImageMagick 7):

```bash
cp icon-src/derrick.svg public/favicon.svg
rsvg-convert --width 96 --height 96 icon-src/derrick.svg --output public/favicon-96x96.png
rsvg-convert --width 16 --height 16 icon-src/derrick-small.svg --output /tmp/i16.png
rsvg-convert --width 32 --height 32 icon-src/derrick.svg --output /tmp/i32.png
rsvg-convert --width 48 --height 48 icon-src/derrick.svg --output /tmp/i48.png
magick /tmp/i16.png /tmp/i32.png /tmp/i48.png public/favicon.ico
rsvg-convert --width 180 --height 180 icon-src/apple-touch-icon.svg | magick - -background '#1b211c' -alpha remove -alpha off -strip PNG24:public/apple-touch-icon.png
rsvg-convert --width 192 --height 192 icon-src/web-app-manifest.svg | magick - -background '#1b211c' -alpha remove -alpha off -strip PNG24:public/web-app-manifest-192x192.png
rsvg-convert --width 512 --height 512 icon-src/web-app-manifest.svg | magick - -background '#1b211c' -alpha remove -alpha off -strip PNG24:public/web-app-manifest-512x512.png
```

To check the result, serve a build (`npm run build`, then `npx vite preview`) and run `npx realfavicon check 4173`. It prints a link to a report on realfavicongenerator.net.
