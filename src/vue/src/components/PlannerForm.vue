<template>
  <fieldset class="border p-3 mb-3">
    <legend>Planner options</legend>
    <div class="form-check">
      <input
        type="checkbox"
        class="form-check-input"
        id="use-underground-pipes"
        v-model="useUndergroundPipes"
      />
      <label class="form-check-label" for="use-underground-pipes">
        <AlgorithmStep v-bind="Steps.UndergroundPipesStep" :show-as-option="true" />
      </label>
    </div>
    <div class="form-check">
      <input type="checkbox" class="form-check-input" id="optimize-pipes" v-model="optimizePipes" />
      <label class="form-check-label" for="optimize-pipes">
        <AlgorithmStep v-bind="Steps.OptimizeStep" :show-as-option="true" />
      </label>
    </div>

    <div class="form-check">
      <input
        type="checkbox"
        class="form-check-input"
        id="validate-solution"
        v-model="validateSolution"
      />
      <label class="form-check-label" for="validate-solution">Validate solution 🐛</label> (slower
      but checks for problems in the resulting blueprint)
    </div>
    <div class="form-check">
      <input type="checkbox" class="form-check-input" id="show-progress" v-model="showProgress" />
      <label class="form-check-label" for="show-progress">Show planning progress</label> (slower;
      plans one strategy at a time so a progress bar can update)
    </div>
    <div class="form-check">
      <input
        type="checkbox"
        class="form-check-input"
        id="use-staging-api"
        v-model="useStagingApi"
      />
      <label class="form-check-label" for="use-staging-api">Use staging API 🤠</label>
    </div>
    <div class="form-check">
      <input type="checkbox" class="form-check-input" id="auto-plan" v-model="autoPlan" />
      <label class="form-check-label" for="auto-plan">Start planning on page load</label> (useful
      for share URLs)
    </div>
    <fieldset class="border p-3 mt-3">
      <legend>Pipe strategies</legend>
      <div class="form-check">
        <input
          type="checkbox"
          class="form-check-input"
          id="pipes-fbe-original"
          v-model="pipeStrategyFbeOriginal"
        />
        <label class="form-check-label" for="pipes-fbe-original">
          <AlgorithmStep v-bind="Steps.PipeSteps.FbeOriginal" :show-as-option="true" />
        </label>
      </div>
      <div class="form-check">
        <input type="checkbox" class="form-check-input" id="pipes-fbe" v-model="pipeStrategyFbe" />
        <label class="form-check-label" for="pipes-fbe">
          <AlgorithmStep v-bind="Steps.PipeSteps.Fbe" :show-as-option="true" />
        </label>
      </div>
      <div class="form-check">
        <input
          type="checkbox"
          class="form-check-input"
          id="pipes-connected-centers-delaunay"
          v-model="pipeStrategyConnectedCentersDelaunay"
        />
        <label class="form-check-label" for="pipes-connected-centers-delaunay">
          <AlgorithmStep v-bind="Steps.PipeSteps.ConnectedCentersDelaunay" :show-as-option="true" />
        </label>
      </div>
      <div class="form-check">
        <input
          type="checkbox"
          class="form-check-input"
          id="pipes-connected-centers-delaunay-mst"
          v-model="pipeStrategyConnectedCentersDelaunayMst"
        />
        <label class="form-check-label" for="pipes-connected-centers-delaunay-mst">
          <AlgorithmStep
            v-bind="Steps.PipeSteps.ConnectedCentersDelaunayMst"
            :show-as-option="true"
          />
        </label>
      </div>
      <div class="form-check">
        <input
          type="checkbox"
          class="form-check-input"
          id="pipes-connected-centers-flute"
          v-model="pipeStrategyConnectedCentersFlute"
        />
        <label class="form-check-label" for="pipes-connected-centers-flute">
          <AlgorithmStep v-bind="Steps.PipeSteps.ConnectedCentersFlute" :show-as-option="true" />
        </label>
      </div>
    </fieldset>
    <fieldset class="border p-3 mt-3" :disabled="!addBeacons">
      <legend>
        Beacon strategies
        <span v-if="!addBeacons" class="badge bg-secondary">disabled</span>
      </legend>
      <div class="form-check">
        <input
          type="checkbox"
          class="form-check-input"
          id="beacons-fbe-original"
          v-model="beaconStrategyFbeOriginal"
        />
        <label class="form-check-label" for="beacons-fbe-original">
          <AlgorithmStep v-bind="Steps.BeaconSteps.FbeOriginal" :show-as-option="true" />
        </label>
      </div>
      <div class="form-check">
        <input
          type="checkbox"
          class="form-check-input"
          id="beacons-fbe"
          v-model="beaconStrategyFbe"
        />
        <label class="form-check-label" for="beacons-fbe">
          <AlgorithmStep v-bind="Steps.BeaconSteps.Fbe" :show-as-option="true" />
        </label>
      </div>
      <div class="form-check">
        <input
          type="checkbox"
          class="form-check-input"
          id="beacons-snug"
          v-model="beaconStrategySnug"
        />
        <label class="form-check-label" for="beacons-snug">
          <AlgorithmStep v-bind="Steps.BeaconSteps.Snug" :show-as-option="true" />
        </label>
      </div>
    </fieldset>
  </fieldset>
</template>

<script lang="ts">
import { storeToRefs } from "pinia"
import { pick } from "../lib/helpers"
import { useAutoPlanStore } from "../stores/AutoPlanStore"
import { useOilFieldStore } from "../stores/OilFieldStore"
import { resetPlannerAdvancedOptions } from "../lib/advancedOptions"
import { Steps } from "../lib/steps"
import AlgorithmStep from "./AlgorithmStep.vue"

export default {
  props: {
    showAdvancedOptions: {
      type: Boolean,
      required: true,
    },
  },
  data() {
    return Object.assign(
      storeToRefs(useAutoPlanStore()),
      pick(
        storeToRefs(useOilFieldStore()),
        "addBeacons",
        "useUndergroundPipes",
        "useStagingApi",
        "optimizePipes",
        "validateSolution",
        "showProgress",
        "pipeStrategyFbeOriginal",
        "pipeStrategyFbe",
        "pipeStrategyConnectedCentersDelaunay",
        "pipeStrategyConnectedCentersDelaunayMst",
        "pipeStrategyConnectedCentersFlute",
        "beaconStrategyFbeOriginal",
        "beaconStrategyFbe",
        "beaconStrategySnug",
      ),
      {
        Steps: Steps,
      },
    )
  },
  watch: {
    showAdvancedOptions: function (newVal: boolean) {
      if (!newVal) {
        this.reset()
      }
    },
  },
  methods: {
    reset() {
      resetPlannerAdvancedOptions(this)
    },
  },
  components: { AlgorithmStep },
}
</script>
