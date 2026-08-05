<template>
  <fieldset class="border p-3 h-100">
    <legend>Pumpjacks</legend>
    <ModuleSelect
      label="Pumpjack module"
      :showAdvancedOptions="showAdvancedOptions"
      showProductivityModules
      :defaultValue="pumpjackModule"
      :defaultIsCustom="pumpjackModuleIsCustom"
      v-model="pumpjackModule"
      v-model:isCustom="pumpjackModuleIsCustom"
    />
    <QualitySelect
      label="Pumpjack quality"
      idPrefix="pumpjack"
      :showAdvancedOptions="showAdvancedOptions"
      v-model="pumpjackQuality"
    />
    <QualitySelect
      label="Pumpjack module quality"
      idPrefix="pumpjack-module"
      :showAdvancedOptions="showAdvancedOptions"
      v-model="pumpjackModuleQuality"
    />
  </fieldset>
</template>

<script lang="ts">
import { storeToRefs } from "pinia"
import { pick } from "../lib/helpers"
import { useOilFieldStore } from "../stores/OilFieldStore"
import { resetPumpjackAdvancedOptions } from "../lib/advancedOptions"
import ModuleSelect from "./ModuleSelect.vue"
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
      "pumpjackModule",
      "pumpjackModuleIsCustom",
      "pumpjackQuality",
      "pumpjackModuleQuality",
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
      resetPumpjackAdvancedOptions(this)
    },
  },
  components: { ModuleSelect, QualitySelect },
}
</script>
