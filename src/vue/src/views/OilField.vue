<template>
  <h1 class="row gx-2">
    <div class="col-md-auto">
      Factorio oil field planner
      <span v-if="useAdvancedOptions">🧙‍♂️</span>
      <span v-else>🐣</span>
    </div>
    <div class="col-md-auto">
      <small
        v-if="usingQueryString"
        class="px-2 text-secondary bg-secondary bg-opacity-10 border border-secondary border-opacity-10 rounded-2"
      >
        from share URL
      </small>
    </div>
  </h1>
  <form>
    <div class="row row-cols-md-auto g-2 mb-3">
      <div class="col-12">
        <button type="button" class="btn btn-outline-secondary" @click="toggleAdvancedOptions">
          Use {{ useAdvancedOptions ? "simple" : "advanced" }} options
        </button>
      </div>
      <div class="col-12">
        <CopyButton class="btn btn-outline-secondary" :value="shareUrl">Copy share URL</CopyButton>
      </div>
      <div class="col-12">
        <button type="button" class="btn btn-outline-danger" @click="reset">Reset</button>
      </div>
      <div class="col-12">
        <button
          type="button"
          class="btn btn-outline-danger"
          v-if="usingQueryString"
          @click="overrideLocalSettings"
        >
          Override local settings
        </button>
      </div>
    </div>
    <fieldset class="border p-3 mb-3">
      <label for="input-blueprint" class="form-label fs-4">Input blueprint</label>
      <a
        class="btn btn-link btn-sm ms-1s mb-2"
        :href="fbeUrl"
        target="_blank"
        rel="noopener noreferrer"
        >View in FBE</a
      >
      <button
        type="button"
        class="btn btn-outline-secondary btn-sm mb-2 me-2"
        @click="normalize"
        :disabled="recentlyNormalized || cannotSubmit"
        title="Remove all entities except pumpjacks, sort and rotate pumpjacks"
      >
        Normalize blueprint
        {{ recentlyNormalized ? "☑️" : "" }}
      </button>
      <button
        type="button"
        class="btn btn-outline-secondary btn-sm mb-2"
        @click="addSampleBlueprint"
        title="Replace the current blueprint with a sample blueprint"
      >
        Add sample
      </button>
      <textarea
        class="form-control font-monospace"
        id="input-blueprint"
        aria-describedby="input-blueprint-help"
        placeholder="paste blueprint string here"
        rows="3"
        v-model="inputBlueprint"
        spellcheck="false"
        autocomplete="off"
        data-gramm="false"
        data-gramm_editor="false"
        data-enable-grammarly="false"
      ></textarea>
    </fieldset>
    <ResponseErrorView v-if="normalizeError" :error="normalizeError" />
    <div class="row">
      <div class="col-md-6 mb-3">
        <PumpjacksForm :show-advanced-options="useAdvancedOptions" />
      </div>
      <div class="col-md-6 mb-3">
        <BeaconForm :show-advanced-options="useAdvancedOptions" />
      </div>
    </div>
    <div class="row">
      <div class="col-md-6 mb-3">
        <ElectricPoleSelect :show-advanced-options="useAdvancedOptions" />
      </div>
      <div class="col-md-6 mb-3">
        <HeatPipeForm :show-advanced-options="useAdvancedOptions" />
      </div>
    </div>
    <PlannerForm :show-advanced-options="useAdvancedOptions" v-show="useAdvancedOptions" />
    <div v-if="addHeatPipes && addBeacons" class="alert alert-info mt-3" role="alert">
      Heating beacons runs entirely in your browser. Large oil fields can take a while to plan.
    </div>
    <div class="d-grid gap-2">
      <button
        type="submit"
        class="btn btn-primary btn-lg"
        @click.prevent="submit"
        :disabled="cannotSubmit"
      >
        <span
          v-if="submitting"
          class="spinner-border spinner-border-sm"
          role="status"
          aria-hidden="true"
        ></span>
        Plan oil field
      </button>
    </div>
    <div v-if="submitting && planProgress" class="alert alert-info mt-2 mb-0" role="status">
      <div class="mb-2">
        Pipe strategies: {{ planProgress.label }} ({{ progressStep }} of {{ planProgress.total }})
      </div>
      <div class="progress" style="height: 1.5rem">
        <div
          class="progress-bar progress-bar-striped progress-bar-animated"
          role="progressbar"
          :style="{ width: progressPercent + '%' }"
          :aria-valuenow="progressPercent"
          aria-valuemin="0"
          aria-valuemax="100"
        >
          {{ progressPercent }}%
        </div>
      </div>
    </div>
    <OilFieldPlanView v-if="plan" :plan="plan" />
    <ResponseErrorView v-if="planError" :error="planError" />
  </form>
  <div class="row">
    <div class="col p-3">
      <h3>About this tool</h3>
      <p>
        This tool finds a near-optimal oil field layout for a blueprint that contains pumpjacks.
      </p>
      <p>
        It looks for the best arrangement of pipes, beacons and electric poles, and returns the plan
        as a Factorio blueprint. The input blueprint should contain at least one pumpjack. Every
        other entity is ignored and left out of the output blueprint. You can use the "View in FBE"
        links on the input or output blueprint to preview it in your web browser with Teoxoy's
        wonderful <a href="https://fbe.factorygamefan.com/">Factorio Blueprint Editor</a>.
      </p>
      <p>
        The planner runs entirely in your browser. It is written in Rust and compiled to
        WebAssembly, and it works in a background thread so the page stays responsive. Nothing you
        paste here is uploaded. A blueprint only leaves your computer if you click a "View in FBE"
        link, which opens it in the editor.
      </p>
      <p>
        The planner has several pipe planning strategies (algorithms) and several beacon planning
        strategies. It tries every enabled combination and returns the blueprint for the best
        result. You can explore other plans, including less effective ones, by turning strategies
        off. For more about how well the strategies perform, see
        <a
          href="https://www.reddit.com/r/factorio/comments/11ply6h/i_want_to_share_an_oil_field_planner_tool_i_built/"
          >Joel Verhagen's Reddit post</a
        >
        about the original tool.
      </p>
      <p>
        There is only one electric pole strategy, so it runs only on the best pipe and beacon
        layout.
      </p>
      <p>
        You can pick the quality of the pumpjacks, beacons, modules and electric poles. For Aquilo,
        the planner can also add heat pipes so the oil field does not freeze.
      </p>
      <p>
        Only the base game (vanilla) pumpjack is supported. Other fluid extractors, such as those
        from mods, are not. There is a
        <a href="https://github.com/FactoryGameFan/FactorioTools/issues/184">GitHub issue #184</a>
        tracking this possible enhancement.
      </p>
      <p>For more details about the various steps performed, see the table below.</p>
      <h6 class="mt-2">Possible planner steps</h6>
      <table class="table table-sm">
        <tbody>
          <template v-for="(s, i) of allSteps" :key="i">
            <tr>
              <td>
                <AlgorithmStep v-bind="s" />
              </td>
              <td v-html="s.longDescription"></td>
            </tr>
          </template>
        </tbody>
      </table>
    </div>
  </div>
</template>

<script lang="ts">
import {
  ApiError,
  ApiResult,
  getPlan,
  getPlanWithProgress,
  normalize,
  PlanProgress,
} from "../lib/OilFieldPlanner"
import BeaconForm from "../components/BeaconForm.vue"
import ElectricPoleSelect from "../components/ElectricPoleForm.vue"
import HeatPipeForm from "../components/HeatPipeForm.vue"
import PlannerForm from "../components/PlannerForm.vue"
import PumpjacksForm from "../components/PumpjacksForm.vue"
import { pick } from "../lib/helpers"
import { storeToRefs } from "pinia"
import {
  generateQueryString,
  hasMatchingQueryString,
  initializeOilFieldStore,
  persistStore,
  useOilFieldStore,
} from "../stores/OilFieldStore"
import ResponseErrorView from "../components/ResponseErrorView.vue"
import OilFieldPlanView from "../components/OilFieldPlanView.vue"
import CopyButton from "../components/CopyButton.vue"
import { useAutoPlanStore } from "../stores/AutoPlanStore"
import { OilFieldPlanResponse } from "../lib/FactorioToolsApi"
import { AllSteps } from "../lib/steps"
import AlgorithmStep from "../components/AlgorithmStep.vue"

const sampleBlueprints = __SAMPLE_BLUEPRINTS__
sampleBlueprints.sort((a, b) => b.length - a.length)

const recentlyNormalizedMs = 3000

export default {
  data() {
    return Object.assign(
      {
        normalizedAt: new Date(0),
        recentlyNormalized: false,
        submitting: false,
        planProgress: null as null | PlanProgress,
        normalizeError: null as null | ApiError,
        plan: null as null | ApiResult<OilFieldPlanResponse>,
        planError: null as null | ApiError,
        allSteps: AllSteps,
      },
      storeToRefs(useAutoPlanStore()),
      pick(
        storeToRefs(useOilFieldStore()),
        "usingQueryString",
        "useAdvancedOptions",
        "inputBlueprint",
        "addHeatPipes",
        "addBeacons",
        "showProgress",
      ),
    )
  },
  async mounted() {
    addEventListener("paste", this.handlePasteEvent)
    if (this.autoPlan) {
      await this.submit()
    }
  },
  unmounted() {
    removeEventListener("paste", this.handlePasteEvent)
  },
  computed: {
    progressPercent(): number {
      const p = this.planProgress
      if (!p || p.total === 0) {
        return 0
      }
      return Math.round((p.current / p.total) * 100)
    },
    progressStep(): number {
      const p = this.planProgress
      if (!p) {
        return 0
      }
      return Math.min(p.current + 1, p.total)
    },
    cannotSubmit() {
      return !this.inputBlueprint.trim() || this.submitting
    },
    shareUrl() {
      const queryString = generateQueryString()
      return `${location.protocol}//${location.host}${location.pathname}?${queryString}`
    },
    fbeUrl() {
      return `https://fbe.factorygamefan.com/?source=${this.inputBlueprint}`
    },
  },
  watch: {
    addBeacons() {
      this.autoEnableProgressForHeatBeacons()
    },
    addHeatPipes() {
      this.autoEnableProgressForHeatBeacons()
    },
  },
  methods: {
    autoEnableProgressForHeatBeacons() {
      // Heating beacons is the slowest plan (see the heat+beacons notice), so
      // default the progress bar on when both are selected - the user sees
      // movement instead of a frozen spinner. They can still turn it back off.
      if (this.addBeacons && this.addHeatPipes) {
        this.showProgress = true
      }
    },
    toggleAdvancedOptions() {
      this.useAdvancedOptions = !this.useAdvancedOptions
    },
    async handlePasteEvent(event: Event) {
      if (!("clipboardData" in event)) {
        return
      }

      if (
        event.target &&
        (event.target instanceof HTMLTextAreaElement ||
          event.target instanceof HTMLSelectElement ||
          event.target instanceof HTMLInputElement ||
          event.target instanceof HTMLButtonElement) &&
        "form" in event.target &&
        event.target.form
      ) {
        return
      }

      const clipboardEvent = event as ClipboardEvent
      let plainText = clipboardEvent.clipboardData?.getData("text/plain")?.trim()
      if (!plainText) {
        return
      }

      // try parsing the input as a blueprint string
      if (plainText.startsWith("0") && /^[A-Za-z0-9\+\/= ]+$/.test(plainText)) {
        try {
          // plus signs are sometimes interpreted as spaces in URL, so handle this simple edge case.
          plainText = plainText.replace(" ", "+")
          atob(plainText.substring(1))
          event.preventDefault()
          this.inputBlueprint = plainText
          if (this.autoPlan) {
            await this.submit()
          }
        } catch {
          // ignore
        }
        return
      }

      // try getting a query string from a URL
      let params: URLSearchParams | undefined
      if (
        plainText.startsWith("http:") ||
        plainText.startsWith("https:") ||
        plainText.startsWith("file:")
      ) {
        try {
          const url = new URL(plainText)
          params = url.searchParams
        } catch {
          // ignore
        }
      } else if (/.+[=&].+/.test(plainText)) {
        try {
          params = new URLSearchParams(plainText)
        } catch {
          // ignore
        }
      }

      if (params && hasMatchingQueryString(params, false)) {
        event.preventDefault()
        const query: Record<string, string> = {}
        for (const [key, value] of params.entries()) {
          query[key] = value
        }
        await this.$router.replace({ query })
        initializeOilFieldStore(this.$route.query)
        if (this.autoPlan) {
          await this.submit()
        }
      }
    },
    addSampleBlueprint() {
      if (!this.inputBlueprint) {
        this.inputBlueprint = sampleBlueprints[0]
      } else {
        const index = Math.floor(Math.random() * sampleBlueprints.length)
        let sampleBlueprint = sampleBlueprints[index]
        if (sampleBlueprint == this.inputBlueprint) {
          sampleBlueprint = sampleBlueprints[(index + 1) % sampleBlueprints.length]
        }
        this.inputBlueprint = sampleBlueprint
      }
    },
    async reset() {
      this.normalizeError = null
      this.plan = null
      this.planError = null
      const store = useOilFieldStore()
      store.$reset()
      await this.$router.replace({ query: {} })
      initializeOilFieldStore(this.$route.query)
    },
    async normalize() {
      await this.invokeApi(async () => {
        const dataOrError = await normalize()
        if (dataOrError.isError) {
          this.normalizeError = dataOrError
        } else {
          this.inputBlueprint = dataOrError.data.blueprint
          this.normalizeError = null

          this.recentlyNormalized = true
          this.normalizedAt = new Date()
          setTimeout(() => {
            this.recentlyNormalized =
              new Date().getTime() - this.normalizedAt.getTime() < recentlyNormalizedMs
          }, recentlyNormalizedMs)
        }

        return dataOrError
      })
    },
    async submit() {
      await this.invokeApi(async () => {
        this.normalizeError = null
        const dataOrError = this.showProgress
          ? await getPlanWithProgress((p) => {
              this.planProgress = p
            })
          : await getPlan()
        if (dataOrError.isError) {
          this.plan = null
          this.planError = dataOrError
        } else {
          this.plan = dataOrError
          this.planError = null
        }
        return dataOrError
      })
    },
    async invokeApi<Data>(api: () => Promise<ApiResult<Data> | ApiError>) {
      if (this.cannotSubmit) {
        return
      }

      this.submitting = true
      // A small non-zero delay lets the browser paint the loading state before work begins.
      await new Promise((r) => setTimeout(r, 10))
      try {
        await api()
      } finally {
        this.submitting = false
        this.planProgress = null
      }
    },
    async overrideLocalSettings() {
      persistStore()
      await this.$router.replace({ query: {} })
      initializeOilFieldStore(this.$route.query)
    },
  },
  components: {
    ElectricPoleSelect,
    BeaconForm,
    PumpjacksForm,
    HeatPipeForm,
    PlannerForm,
    ResponseErrorView,
    OilFieldPlanView,
    CopyButton,
    AlgorithmStep,
  },
}
</script>
