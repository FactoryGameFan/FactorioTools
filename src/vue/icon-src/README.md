# Site icons

The sources for the green oil derrick icons in `../public/`. Nothing here is served.

| source | builds |
| --- | --- |
| `derrick.svg` | `public/favicon.svg` (a straight copy), and the 32 and 48 px sizes in `public/favicon.ico` |
| `derrick-small.svg` | the 16 px size in `public/favicon.ico`. It has fewer, thicker lines, because the full lattice blurs at 16 px |
| `apple-touch-icon.svg` | `public/apple-touch-icon.png`, 180 px on a dark background. iOS fills a clear background with black and rounds the corners itself, so the PNG is square and opaque |

To rebuild after editing a source, from `src/vue` (needs `rsvg-convert` from librsvg, and ImageMagick 7):

```bash
cp icon-src/derrick.svg public/favicon.svg
rsvg-convert --width 16 --height 16 icon-src/derrick-small.svg --output /tmp/i16.png
rsvg-convert --width 32 --height 32 icon-src/derrick.svg --output /tmp/i32.png
rsvg-convert --width 48 --height 48 icon-src/derrick.svg --output /tmp/i48.png
magick /tmp/i16.png /tmp/i32.png /tmp/i48.png public/favicon.ico
rsvg-convert --width 180 --height 180 icon-src/apple-touch-icon.svg | magick - -background '#1b211c' -alpha remove -alpha off -strip PNG24:public/apple-touch-icon.png
```
