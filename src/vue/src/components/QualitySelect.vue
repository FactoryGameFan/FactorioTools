<template>
  <div class="row" v-show="showAdvancedOptions">
    <div class="col">
      <label :for="idPrefix + '-quality'" class="form-label d-flex align-items-center gap-2">
        <svg
          class="quality-badge"
          viewBox="0 0 24 24"
          role="img"
          :aria-label="qualityLabel(modelValue as Quality) + ' quality'"
        >
          <circle
            v-for="(pip, i) in pips"
            :key="i"
            :cx="pip[0]"
            :cy="pip[1]"
            :r="pipRadius"
            :fill="badgeColor"
            stroke="#000"
            stroke-width="1.6"
          />
        </svg>
        {{ label }}
      </label>
      <select
        class="form-select"
        :id="idPrefix + '-quality'"
        :value="modelValue"
        @change="$emit('update:modelValue', ($event.target as HTMLSelectElement).value)"
      >
        <option v-for="q in order" :key="q" :value="q">{{ qualityLabel(q) }}</option>
      </select>
    </div>
  </div>
</template>

<script lang="ts">
import { Quality } from "../lib/FactorioToolsApi"
import {
  QUALITY_ORDER,
  qualityColor,
  qualityLabel,
  qualityPips,
  qualityPipRadius,
} from "../lib/quality"

export default {
  props: {
    showAdvancedOptions: { type: Boolean, required: true },
    label: { type: String, required: true },
    idPrefix: { type: String, required: true },
    modelValue: { type: String, required: true },
  },
  emits: ["update:modelValue"],
  computed: {
    order() {
      return QUALITY_ORDER
    },
    badgeColor(): string {
      return qualityColor(this.modelValue as Quality)
    },
    pips() {
      return qualityPips(this.modelValue as Quality)
    },
    pipRadius(): number {
      return qualityPipRadius(this.modelValue as Quality)
    },
  },
  methods: {
    qualityLabel,
  },
}
</script>

<style scoped>
.quality-badge {
  /* In-game quality pip cluster, recreated as SVG and tinted by the quality color. */
  display: inline-block;
  width: 1.1rem;
  height: 1.1rem;
  flex: none;
}
</style>
