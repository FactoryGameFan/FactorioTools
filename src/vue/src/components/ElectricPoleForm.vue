<template>
  <fieldset class="border p-3 h-100">
    <legend>Electric poles</legend>
    <div class="form-check">
      <input
        type="checkbox"
        class="form-check-input"
        id="add-electric-poles"
        v-model="addElectricPoles"
      />
      <label class="form-check-label" for="add-electric-poles">Add electric poles</label>
    </div>
    <CustomizeSelect
      v-if="addElectricPoles"
      custom-label="Entity"
      label="Entity"
      idPrefix="electric-pole"
      :showAdvancedOptions="showAdvancedOptions"
      :defaultValue="electricPoleEntityName"
      :defaultIsCustom="electricPoleIsCustom"
      v-model="electricPoleEntityName"
      v-model:isCustom="electricPoleIsCustom"
    >
      <option value="small-electric-pole">Small electric pole</option>
      <option value="medium-electric-pole">Medium electric pole</option>
      <option value="big-electric-pole">Big electric pole</option>
      <option value="substation">Substation</option>
    </CustomizeSelect>
    <QualitySelect
      v-if="addElectricPoles"
      label="Electric pole quality"
      idPrefix="electric-pole"
      :showAdvancedOptions="showAdvancedOptions"
      v-model="electricPoleQuality"
    />
    <p class="form-text" v-show="showAdvancedOptions && addElectricPoles">
      Effective coverage at {{ electricPoleQuality }}: supply
      {{ Number(electricPoleSupplyWidth) + 2 * level }}x{{
        Number(electricPoleSupplyHeight) + 2 * level
      }}, wire reach {{ Number(electricPoleWireReach) + 2 * level }}
    </p>
    <div class="row" v-show="showAdvancedOptions && addElectricPoles">
      <div class="col-lg-4 mt-3">
        <label class="form-label" for="electric-pole-wire-reach">Wire reach</label>
        <input
          type="text"
          pattern="\d+(\.\d+)"
          min="1"
          max="99"
          class="form-control"
          id="electric-pole-wire-reach"
          v-model="electricPoleWireReach"
          :disabled="!electricPoleIsCustom"
          required
          autocomplete="off"
        />
      </div>
      <div class="col-lg-4 mt-3">
        <p class="form-label">
          Entity size (<label for="electric-pole-width">width</label> x
          <label for="electric-pole-height">height</label>)
        </p>
        <div class="input-group">
          <input
            type="number"
            min="0"
            max="9"
            class="form-control"
            id="electric-pole-width"
            v-model="electricPoleWidth"
            :disabled="!electricPoleIsCustom"
            required
          />
          <span class="input-group-text">x</span>
          <input
            type="number"
            min="1"
            max="9"
            class="form-control"
            id="electric-pole-height"
            v-model="electricPoleHeight"
            :disabled="!electricPoleIsCustom"
            required
          />
        </div>
      </div>
      <div class="col-lg-4 mt-3">
        <p class="form-label">
          Supply area (<label for="electric-pole-supply-width">width</label> x
          <label for="electric-pole-supply-height">height</label>)
        </p>
        <div class="input-group">
          <input
            type="number"
            min="1"
            max="99"
            class="form-control"
            id="electric-pole-supply-width"
            v-model="electricPoleSupplyWidth"
            :disabled="!electricPoleIsCustom"
            required
          />
          <span class="input-group-text">x</span>
          <input
            type="number"
            min="1"
            max="99"
            class="form-control"
            id="electric-pole-supply-height"
            v-model="electricPoleSupplyHeight"
            :disabled="!electricPoleIsCustom"
            required
          />
        </div>
      </div>
    </div>
  </fieldset>
</template>

<script lang="ts">
import { storeToRefs } from "pinia"
import { pick } from "../lib/helpers"
import { useOilFieldStore } from "../stores/OilFieldStore"
import { resetElectricPoleAdvancedOptions, setKnownElectricPole } from "../lib/advancedOptions"
import { Quality } from "../lib/FactorioToolsApi"
import { qualityLevel } from "../lib/quality"
import CustomizeSelect from "./CustomizeSelect.vue"
import QualitySelect from "./QualitySelect.vue"

export default {
  props: {
    showAdvancedOptions: {
      type: Boolean,
      required: true,
    },
  },
  data() {
    return pick(
      storeToRefs(useOilFieldStore()),
      "addElectricPoles",
      "electricPoleEntityName",
      "electricPoleIsCustom",
      "electricPoleWidth",
      "electricPoleHeight",
      "electricPoleSupplyWidth",
      "electricPoleSupplyHeight",
      "electricPoleWireReach",
      "electricPoleQuality",
    )
  },
  computed: {
    level(): number {
      return qualityLevel(this.electricPoleQuality as Quality)
    },
  },
  watch: {
    showAdvancedOptions: function (newVal: boolean) {
      if (!newVal) {
        this.reset()
      }
    },
    electricPoleEntityName: function (newVal: string) {
      this.setKnownElectricPole(newVal)
    },
  },
  methods: {
    reset() {
      resetElectricPoleAdvancedOptions(this)
    },
    setKnownElectricPole(electricPoleEntityName: string) {
      return setKnownElectricPole(this, electricPoleEntityName)
    },
  },
  components: { CustomizeSelect, QualitySelect },
}
</script>
