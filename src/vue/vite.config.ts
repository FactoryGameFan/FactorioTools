import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'
import path from 'path'
import { execSync } from 'child_process'
import { readFileSync } from 'fs'

const buildDate = new Date().toISOString().slice(0, 10);

const branch = execSync('git rev-parse --abbrev-ref HEAD').toString().trim()

const commit = execSync('git rev-parse --short HEAD').toString().trim()
const shortVersion = execSync('git describe --abbrev=0').toString().trim()
let version = execSync('git describe --long --dirty').toString().trim()
if (version == `${shortVersion}-0-g${commit}` && branch == 'main') {
  version = shortVersion
} else {
  version += "-" + branch
}

const sampleBlueprints = Array.from(new Set(readFileSync("../../testdata/oil-field/small-list.txt", { encoding: 'utf8' })
  .split(/\r?\n/)
  .map(l => l.trim())
  .filter(l => l.length > 0 && !l.startsWith("#"))))

const basePath = (process.env.BASE_PATH || '').trim().replace(/\/+$/g, '') + '/'
const define = {
  __BASE_PATH__: JSON.stringify(basePath),
  __BUILD_DATE__: JSON.stringify(buildDate),
  __GIT_BRANCH__: JSON.stringify(branch),
  __GIT_COMMIT__: JSON.stringify(commit),
  __GIT_VERSION__: JSON.stringify(version),
  __SAMPLE_BLUEPRINTS__: JSON.stringify(sampleBlueprints),
}

console.log('build-time config:')
for (const [key, value] of Object.entries(define)) {
  if (key == "__SAMPLE_BLUEPRINTS__") {
    console.log(` - ${key}: (${sampleBlueprints.length} blueprints)`);
  } else {
    console.log(` - ${key}: ${value}`);
  }
}

// https://vitejs.dev/config/
export default defineConfig({
  resolve: {
    alias: {
      '~bootstrap': path.resolve(__dirname, 'node_modules/bootstrap'),
    }
  },
  css: {
    preprocessorOptions: {
      scss: {
        // Bootstrap's SCSS still uses Sass functions that newer Dart Sass has
        // deprecated. Mute deprecation warnings from dependencies (node_modules)
        // while keeping them for our own SCSS.
        quietDeps: true,
        // src/style.scss uses Bootstrap 5's @import-based "import stack"; @use
        // isn't cleanly supported until Bootstrap 6, so silence that category.
        silenceDeprecations: ['import'],
      },
    },
  },
  plugins: [vue()],
  // The planner runs in a module Web Worker (src/lib/plannerWorker.ts) that imports the
  // Rust planner's ES module wrapper, so build the worker as an ES module too.
  worker: { format: 'es' },
  define: define,
  base: basePath
})
