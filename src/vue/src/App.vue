<template>
  <main class="py-3">
    <div class="container">
      <router-view></router-view>
    </div>
  </main>
  <footer class="footer bg-light">
    <div class="container">
      <p class="text-muted py-3">
        Site originally by
        <a class="link-body-emphasis" href="https://joelverhagen.com/">Joel Verhagen</a>, version
        {{ gitVersion }}, commit
        <a
          class="link-body-emphasis"
          :href="`https://github.com/FactoryGameFan/FactorioTools/tree/${gitCommit}`"
          >{{ gitCommit }}</a
        >, branch
        <a
          class="link-body-emphasis"
          :href="`https://github.com/FactoryGameFan/FactorioTools/tree/${gitBranch}`"
          >{{ gitBranch }}</a
        >, built on {{ buildDate }}. Report bugs at the
        <a class="link-body-emphasis" href="https://github.com/FactoryGameFan/FactorioTools"
          >GitHub repository</a
        >.
      </p>
      <!-- Required by FLUTE's Attribution Assurance License (rust/flute/LICENSE-FLUTE). Keep all three parts. -->
      <p class="text-muted pb-3">
        Pipe planning uses FLUTE by Dr. Chris C. N. Chu, Iowa State University,
        <a class="link-body-emphasis text-break" href="http://home.engineering.iastate.edu/~cnchu/"
          >http://home.engineering.iastate.edu/~cnchu/</a
        >.
      </p>
    </div>
  </footer>
</template>

<style>
html {
  height: 100%;
}
</style>

<script setup lang="ts">
const buildDate = __BUILD_DATE__
const gitBranch = __GIT_BRANCH__
const gitCommit = __GIT_COMMIT__
const gitVersion = __GIT_VERSION__
</script>
