/**
 * Nuxt UI theme — Vuesax look:
 *  • buttons: 12px radius, lift on hover with a glow in the button's own colour
 *  • inputs: filled grey, no border, turn white with a soft shadow on focus
 *  • cards/modals: borderless, 20px radius, soft shadow
 * Colours: primary = zaokaiy orange (static scale in main.css), neutral = slate.
 */
const glow = (v: string) => `hover:shadow-[0_10px_20px_-10px_var(${v})] disabled:shadow-none aria-disabled:shadow-none`

export default defineAppConfig({
  ui: {
    colors: {
      primary: 'zaokaiy',
      secondary: 'violet',
      success: 'emerald',
      info: 'sky',
      warning: 'amber',
      error: 'red',
      neutral: 'slate',
    },
    icons: {
      // Nuxt UI's defaults are Lucide already; listed here to keep them stable.
      close: 'i-lucide-x',
      chevronDown: 'i-lucide-chevron-down',
      loading: 'i-lucide-loader-circle',
    },
    button: {
      slots: {
        base: 'rounded-xl font-semibold justify-center transition-all duration-200 ease-out hover:-translate-y-0.5 active:translate-y-0 active:scale-[.98] disabled:translate-y-0 aria-disabled:translate-y-0',
      },
      compoundVariants: [
        { color: 'primary', variant: 'solid', class: `hover:bg-primary ${glow('--ui-primary')}` },
        { color: 'success', variant: 'solid', class: `hover:bg-success ${glow('--ui-success')}` },
        { color: 'error', variant: 'solid', class: `hover:bg-error ${glow('--ui-error')}` },
        { color: 'warning', variant: 'solid', class: `hover:bg-warning ${glow('--ui-warning')}` },
        { color: 'info', variant: 'solid', class: `hover:bg-info ${glow('--ui-info')}` },
        { color: 'secondary', variant: 'solid', class: `hover:bg-secondary ${glow('--ui-secondary')}` },
        { color: 'neutral', variant: 'solid', class: `hover:bg-inverted ${glow('--ui-bg-inverted')}` },
        // Flat (tinted) and ghost buttons don't lift — Vuesax "flat" style.
        { variant: ['soft', 'subtle', 'ghost', 'link', 'outline'], class: 'hover:translate-y-0' },
        { color: 'neutral', variant: 'soft', class: 'bg-[var(--field)] hover:bg-[var(--field-hover)] text-highlighted' },
        { color: 'primary', variant: 'soft', class: 'bg-primary/12 hover:bg-primary/20 text-primary' },
      ],
      defaultVariants: { size: 'lg' },
    },
    input: {
      slots: {
        root: 'w-full',
        base: 'rounded-xl transition-all duration-200',
      },
      variants: {
        variant: {
          soft: 'text-highlighted bg-[var(--field)] hover:bg-[var(--field-hover)] focus:bg-default focus:shadow-[0_5px_20px_-5px_rgb(0_0_0/0.14)] focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-primary/40 disabled:bg-[var(--field)]',
        },
      },
      defaultVariants: { variant: 'soft', size: 'lg' },
    },
    textarea: {
      slots: { root: 'w-full', base: 'rounded-xl transition-all duration-200' },
      variants: {
        variant: {
          soft: 'text-highlighted bg-[var(--field)] hover:bg-[var(--field-hover)] focus:bg-default focus:shadow-[0_5px_20px_-5px_rgb(0_0_0/0.14)] focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-primary/40',
        },
      },
      defaultVariants: { variant: 'soft', size: 'lg' },
    },
    select: {
      slots: { base: 'rounded-xl transition-all duration-200', content: 'rounded-xl' },
      variants: { variant: { soft: 'text-highlighted bg-[var(--field)] hover:bg-[var(--field-hover)] focus:bg-default data-[state=open]:bg-default' } },
      defaultVariants: { variant: 'soft', size: 'lg' },
    },
    selectMenu: {
      slots: { base: 'rounded-xl transition-all duration-200', content: 'rounded-xl' },
      variants: { variant: { soft: 'text-highlighted bg-[var(--field)] hover:bg-[var(--field-hover)] focus:bg-default data-[state=open]:bg-default' } },
      defaultVariants: { variant: 'soft', size: 'lg' },
    },
    card: {
      slots: {
        root: 'rounded-[20px] shadow-[var(--shadow-card)] overflow-hidden',
        header: 'p-5 sm:px-6',
        body: 'p-5 sm:p-6',
        footer: 'p-5 sm:px-6',
      },
      variants: {
        variant: {
          outline: { root: 'bg-default ring-0 dark:ring dark:ring-default', header: 'border-b border-default', footer: 'border-t border-default' },
        },
      },
    },
    badge: {
      slots: { base: 'rounded-lg font-semibold' },
      defaultVariants: { variant: 'soft' },
    },
    modal: {
      slots: {
        overlay: 'bg-[rgb(16_14_12/0.45)] backdrop-blur-sm',
        content: 'rounded-[20px] ring-0 shadow-[var(--shadow-pop)] divide-default',
      },
    },
    dropdownMenu: {
      slots: {
        content: 'rounded-2xl ring-0 shadow-[var(--shadow-pop)] p-1.5',
        item: 'rounded-xl px-2.5 py-2 transition-colors',
      },
    },
    popover: { slots: { content: 'rounded-2xl ring-0 shadow-[var(--shadow-pop)]' } },
    tooltip: { slots: { content: 'rounded-lg' } },
    avatar: { slots: { root: 'rounded-xl' } },
    checkbox: { slots: { base: 'rounded-md' } },
    tabs: {
      slots: { list: 'rounded-2xl', indicator: 'rounded-xl shadow-[0_6px_16px_-8px_var(--ui-primary)]', trigger: 'rounded-xl font-semibold' },
    },
    toast: { slots: { root: 'rounded-2xl ring-0 shadow-[var(--shadow-pop)]' } },
    navigationMenu: {
      slots: {
        link: 'rounded-xl font-semibold transition-all duration-200 before:rounded-xl',
        linkLeadingIcon: 'size-[18px]',
      },
      compoundVariants: [
        // Vuesax sidebar: active item tinted with a glowing bar on the left edge.
        {
          orientation: 'vertical',
          active: true,
          class: {
            link: 'text-primary before:bg-primary/10 after:absolute after:-left-3 after:top-1/2 after:h-6 after:w-1 after:-translate-y-1/2 after:rounded-r-full after:bg-primary after:shadow-[0_0_12px_var(--ui-primary)]',
            linkLeadingIcon: 'text-primary',
          },
        },
        {
          orientation: 'vertical',
          active: false,
          class: { link: 'text-toned hover:text-highlighted hover:translate-x-1 hover:before:bg-transparent', linkLeadingIcon: 'text-muted group-hover:text-highlighted' },
        },
        { orientation: 'vertical', collapsed: true, active: true, class: { link: 'after:-left-3' } },
      ],
    },
    dashboardSidebar: {
      slots: {
        root: 'bg-default lg:m-3 lg:mr-0 lg:rounded-[24px] lg:border-0 lg:shadow-[0_5px_30px_0_rgb(0_0_0/0.06)] lg:min-h-0 lg:h-[calc(100dvh-1.5rem)] dark:lg:ring dark:lg:ring-default',
        content: 'bg-default',
      },
    },
  },
})
