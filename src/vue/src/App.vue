<template>
  <nav class="navbar navbar-expand-lg navbar-dark bg-dark" v-if="false">
    <div class="container">
      <router-link class="navbar-brand" to="/">Factorio Tools</router-link>
      <div class="navbar-collapse">
        <div class="navbar-nav">
          <router-link class="nav-link active" aria-current="page" to="/oil-field"
            >Oil field planner</router-link
          >
        </div>
      </div>
    </div>
  </nav>
  <main class="py-3">
    <div class="container">
      <router-view></router-view>
    </div>
  </main>
  <footer class="footer bg-light">
    <div class="container">
      <p class="text-muted py-3">
        Site originally by <a href="https://joelverhagen.com/">Joel Verhagen</a>, version
        {{ gitVersion }}, commit
        <a :href="`https://github.com/wormeyman/FactorioTools/tree/${gitCommit}`">{{ gitCommit }}</a
        >, branch
        <a :href="`https://github.com/wormeyman/FactorioTools/tree/${gitBranch}`">{{ gitBranch }}</a
        >, built on {{ buildDate }}. Report bugs at the
        <a href="https://github.com/wormeyman/FactorioTools">GitHub repository</a>.
      </p>
      <!-- Required by FLUTE's Attribution Assurance License (submodules/FluteSharp/flute-c/license.txt). Keep all three parts. -->
      <p class="text-muted pb-3">
        Pipe planning uses FLUTE by Dr. Chris C. N. Chu, Iowa State University,
        <a href="http://home.engineering.iastate.edu/~cnchu/"
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
