<template>
  <div class="card my-3 rounded-0">
    <h2 class="h4 card-header bg-danger bg-opacity-10">
      {{ error.title }}
    </h2>
    <template v-if="error.errors">
      <ul
        class="list-group list-group-flush"
        v-for="[key, values] in Object.entries(error.errors)"
        :key="key"
      >
        <template v-for="(value, i) in values" :key="i">
          <li class="list-group-item">{{ value }}</li>
        </template>
      </ul>
    </template>
    <template v-if="error.errorDetails">
      <div class="card-body" v-for="(value, i) in error.errorDetails" :key="i">
        <pre class="mb-0">{{ value }}</pre>
      </div>
    </template>
  </div>
</template>

<script lang="ts">
import { PropType } from "vue"
import { ApiError } from "../lib/OilFieldPlanner"

export default {
  props: {
    error: {
      type: Object as PropType<ApiError>,
      required: true,
    },
  },
}
</script>
