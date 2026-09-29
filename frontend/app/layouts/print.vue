<template>
  <div class="print-layout theme-light min-h-dvh bg-white text-black">
    <slot />
  </div>
</template>

<style>
@media screen {
  .print-layout { background: #e9e6e1; padding: 24px 0; }
}
@media print {
  html, body { background: #fff !important; }
  .no-print { display: none !important; }
}
</style>
