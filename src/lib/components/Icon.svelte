<script lang="ts">
  let {
    name,
    size = 20,
  }: {
    name:
      | "library"
      | "book"
      | "headphones"
      | "diary"
      | "voice"
      | "account"
      | "sync"
      | "settings"
      | "more"
      | "search"
      | "bell"
      | "alert"
      | "info"
      | "sun"
      | "moon"
      | "grid"
      | "list"
      | "play"
      | "pause"
      | "stop"
      | "close"
      | "plus"
      | "download"
      | "upload"
      | "folder"
      | "globe"
      | "check"
      | "chevron"
      | "flame"
      | "clock"
      | "sparkles"
      | "star"
      | "wave"
      | "mail"
      | "lock"
      | "expand"
      | "edit"
      | "trash"
      | "bookmark"
      | "chart"
      | "sort"
      | "mic"
      | "waveform"
      | "chapters"
      | "sliders"
      | "share"
      | "speed";
    size?: number;
  } = $props();
</script>

<svg
  width={size}
  height={size}
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width="1.8"
  stroke-linecap="round"
  stroke-linejoin="round"
  aria-hidden="true"
>
  {#if name === "library"}
    <path d="M4 19V5a1 1 0 0 1 1-1h4v16H5a1 1 0 0 1-1-1Z" />
    <path d="M9 4h4v16H9" />
    <path d="m14 5 4-1 2 15-4 1-2-15Z" />
  {:else if name === "book"}
    <path d="M4 5.5A2.5 2.5 0 0 1 6.5 3H20v16H6.5A2.5 2.5 0 0 0 4 21.5v-16Z" />
    <path d="M4 18.5A2.5 2.5 0 0 1 6.5 16H20" />
  {:else if name === "headphones"}
    <path d="M4 14v-2a8 8 0 0 1 16 0v2" />
    <path d="M4 14h3v5H5a1 1 0 0 1-1-1v-4Z" />
    <path d="M20 14h-3v5h2a1 1 0 0 0 1-1v-4Z" />
  {:else if name === "diary"}
    <path d="M6 3h13a1 1 0 0 1 1 1v16a1 1 0 0 1-1 1H6" />
    <path d="M6 3a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2" />
    <path d="M8 8h8M8 12h8M8 16h5" />
  {:else if name === "voice"}
    <rect x="9" y="3" width="6" height="11" rx="3" />
    <path d="M5 11a7 7 0 0 0 14 0M12 18v3M9 21h6" />
  {:else if name === "account"}
    <circle cx="12" cy="8" r="4" />
    <path d="M4 21a8 8 0 0 1 16 0" />
  {:else if name === "sync"}
    <path d="M20 7h-9a5 5 0 0 0-5 5v1" />
    <path d="m17 4 3 3-3 3" />
    <path d="M4 17h9a5 5 0 0 0 5-5v-1" />
    <path d="m7 20-3-3 3-3" />
  {:else if name === "settings"}
    <circle cx="12" cy="12" r="3" />
    <path d="M19.4 15a1.7 1.7 0 0 0 .3 1.9l.1.1-2.8 2.8-.1-.1a1.7 1.7 0 0 0-1.9-.3 1.7 1.7 0 0 0-1 1.6V21h-4v-.1a1.7 1.7 0 0 0-1-1.6 1.7 1.7 0 0 0-1.9.3l-.1.1L4.2 17l.1-.1a1.7 1.7 0 0 0 .3-1.9A1.7 1.7 0 0 0 3 14H3v-4h.1a1.7 1.7 0 0 0 1.6-1 1.7 1.7 0 0 0-.3-1.9L4.2 7 7 4.2l.1.1A1.7 1.7 0 0 0 9 4.6a1.7 1.7 0 0 0 1-1.6V3h4v.1a1.7 1.7 0 0 0 1 1.6 1.7 1.7 0 0 0 1.9-.3l.1-.1L19.8 7l-.1.1a1.7 1.7 0 0 0-.3 1.9 1.7 1.7 0 0 0 1.6 1H21v4h-.1a1.7 1.7 0 0 0-1.5 1Z" />
  {:else if name === "more"}
    <circle cx="5" cy="12" r="1" fill="currentColor" />
    <circle cx="12" cy="12" r="1" fill="currentColor" />
    <circle cx="19" cy="12" r="1" fill="currentColor" />
  {:else if name === "search"}
    <circle cx="11" cy="11" r="7" />
    <path d="m16 16 5 5" />
  {:else if name === "bell"}
    <path d="M18 8a6 6 0 0 0-12 0c0 7-3 7-3 9h18c0-2-3-2-3-9" />
    <path d="M10.3 21a2 2 0 0 0 3.4 0" />
  {:else if name === "alert"}
    <path d="M12 3 2 20h20L12 3Z" />
    <path d="M12 9v5M12 18h.01" />
  {:else if name === "info"}
    <circle cx="12" cy="12" r="9" />
    <path d="M12 11v5M12 8h.01" />
  {:else if name === "sun"}
    <circle cx="12" cy="12" r="4" />
    <path d="M12 2v2M12 20v2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M2 12h2M20 12h2M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4" />
  {:else if name === "moon"}
    <path d="M20 15.5A8.5 8.5 0 0 1 8.5 4 8.5 8.5 0 1 0 20 15.5Z" />
  {:else if name === "grid"}
    <rect x="3" y="3" width="7" height="7" rx="1" />
    <rect x="14" y="3" width="7" height="7" rx="1" />
    <rect x="3" y="14" width="7" height="7" rx="1" />
    <rect x="14" y="14" width="7" height="7" rx="1" />
  {:else if name === "list"}
    <path d="M8 6h13M8 12h13M8 18h13M3 6h.01M3 12h.01M3 18h.01" />
  {:else if name === "play"}
    <path d="m8 5 11 7-11 7V5Z" fill="currentColor" />
  {:else if name === "pause"}
    <rect x="7" y="5" width="3" height="14" rx="1" fill="currentColor" />
    <rect x="14" y="5" width="3" height="14" rx="1" fill="currentColor" />
  {:else if name === "stop"}
    <rect x="6" y="6" width="12" height="12" rx="2" fill="currentColor" />
  {:else if name === "close"}
    <path d="m6 6 12 12M18 6 6 18" />
  {:else if name === "plus"}
    <path d="M12 5v14M5 12h14" />
  {:else if name === "download"}
    <path d="M12 3v12m0 0 4-4m-4 4-4-4M4 19h16" />
  {:else if name === "folder"}
    <path d="M3 6a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v10a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V6Z" />
  {:else if name === "globe"}
    <circle cx="12" cy="12" r="9" />
    <path d="M3 12h18M12 3a14 14 0 0 1 0 18M12 3a14 14 0 0 0 0 18" />
  {:else if name === "check"}
    <path d="m5 12 4 4L19 6" />
  {:else if name === "chevron"}
    <path d="m9 6 6 6-6 6" />
  {:else if name === "flame"}
    <path d="M12 3c1.6 3.2 4.8 4.2 4.8 8.1A4.8 4.8 0 0 1 12 16a4.8 4.8 0 0 1-4.8-4.9C7.2 8.6 9 7.2 9.4 4.8c.9 1 1.7 1.9 2.1 3.1.5-1.7.7-3.2.5-4.9Z" />
    <path d="M8.4 14.8A3.8 3.8 0 0 0 12 21a3.8 3.8 0 0 0 3.6-6.2" />
  {:else if name === "clock"}
    <circle cx="12" cy="12" r="8.5" />
    <path d="M12 7v5l3 2" />
  {:else if name === "sparkles"}
    <path d="m12 3 1.3 4.2L17.5 8.5l-4.2 1.3L12 14l-1.3-4.2L6.5 8.5l4.2-1.3L12 3Z" />
    <path d="m5.5 14 .8 2.7 2.7.8-2.7.8L5.5 21l-.8-2.7-2.7-.8 2.7-.8.8-2.7ZM18.5 13l.7 2.3 2.3.7-2.3.7-.7 2.3-.7-2.3-2.3-.7 2.3-.7.7-2.3Z" />
  {:else if name === "star"}
    <path d="m12 3 2.8 5.7 6.2.9-4.5 4.4 1.1 6.2L12 17.3l-5.6 2.9 1.1-6.2L3 9.6l6.2-.9L12 3Z" />
  {:else if name === "wave"}
    <path d="M4 12h2l2-5 2.5 10L13 5l2.5 12 2-5H20" />
  {:else if name === "mail"}
    <rect x="3" y="5" width="18" height="14" rx="2" />
    <path d="m3.5 6 8.5 6 8.5-6" />
  {:else if name === "lock"}
    <rect x="5" y="10" width="14" height="10" rx="2" />
    <path d="M8 10V7a4 4 0 0 1 8 0v3" />
  {:else if name === "expand"}
    <path d="M8 3H5a2 2 0 0 0-2 2v3M16 3h3a2 2 0 0 1 2 2v3M21 16v3a2 2 0 0 1-2 2h-3M3 16v3a2 2 0 0 0 2 2h3" />
  {:else if name === "edit"}
    <path d="M12 20h9" />
    <path d="M16.5 3.5a2.1 2.1 0 0 1 3 3L7 19l-4 1 1-4 12.5-12.5Z" />
  {:else if name === "trash"}
    <path d="M3 6h18M8 6V4h8v2M19 6l-1 14H6L5 6M10 11v5M14 11v5" />
  {:else if name === "chart"}
    <path d="M4 20V4M4 20h16" />
    <path d="m7 15 4-5 4 3 5-7" />
  {:else if name === "bookmark"}
    <path d="M6 3h12a1 1 0 0 1 1 1v17l-7-4-7 4V4a1 1 0 0 1 1-1Z" />
  {:else if name === "upload"}
    <path d="M12 21V9m0 0 4 4m-4-4L8 13M4 5h16" />
  {:else if name === "sort"}
    <path d="M7 4v16m0 0-3-3m3 3 3-3M17 20V4m0 0-3 3m3-3 3 3" />
  {:else if name === "mic"}
    <rect x="9" y="3" width="6" height="11" rx="3" />
    <path d="M5 11a7 7 0 0 0 14 0M12 18v3M9 21h6" />
  {:else if name === "waveform"}
    <path d="M3 12h2l1.5-5L9 19l2.5-14L14 15l2-6 1.5 3H21" />
  {:else if name === "chapters"}
    <path d="M4 5.5A2.5 2.5 0 0 1 6.5 3H20v16H6.5A2.5 2.5 0 0 0 4 21.5v-16Z" />
    <path d="M8 7h8M8 11h8M8 15h5" />
  {:else if name === "sliders"}
    <path d="M4 6h9M17 6h3M4 12h4M12 12h8M4 18h10M18 18h2" />
    <circle cx="15" cy="6" r="2" />
    <circle cx="10" cy="12" r="2" />
    <circle cx="16" cy="18" r="2" />
  {:else if name === "share"}
    <circle cx="18" cy="5" r="2.5" />
    <circle cx="6" cy="12" r="2.5" />
    <circle cx="18" cy="19" r="2.5" />
    <path d="m8.3 10.8 7.4-4.3M8.3 13.2l7.4 4.3" />
  {:else if name === "speed"}
    <path d="M4 17a8 8 0 1 1 16 0" />
    <path d="m12 13 4-4M6.5 17h11" />
  {/if}
</svg>
