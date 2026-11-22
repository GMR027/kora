<script lang="ts">
  import { comfort, updateComfort } from "../comfort";
  import { t } from "../i18n";

  let now = $state(Date.now());
  let reminder = $state(false);
  let remaining = $state(20);
  let nextReminder = Date.now() + 20 * 60_000;

  let nightTime = $derived.by(() => {
    const date = new Date(now);
    const hour = date.getHours() + date.getMinutes() / 60;
    return hour >= 23.5 || hour < 6;
  });

  let effectiveAmber = $derived(
    Math.min(
      100,
      $comfort.amber + ($comfort.bedtime ? 18 : 0) + (nightTime ? 8 : 0),
    ),
  );
  let effectiveDim = $derived(
    Math.min(
      80,
      $comfort.dim * 0.65 +
        ($comfort.bedtime ? 18 : 0) +
        (nightTime && $comfort.nightLimit ? 14 : 0),
    ),
  );

  $effect(() => {
    document.documentElement.style.fontSize = `${(16 * $comfort.scale) / 100}px`;
    document.body.style.fontSize = `${(15 * $comfort.scale) / 100}px`;
    document.documentElement.dataset.comfort = $comfort.bedtime
      ? "bedtime"
      : "day";
    return () => {
      document.documentElement.style.fontSize = "";
      document.body.style.fontSize = "";
      delete document.documentElement.dataset.comfort;
    };
  });

  $effect(() => {
    const clock = setInterval(() => {
      now = Date.now();
    }, 30_000);
    return () => clearInterval(clock);
  });

  $effect(() => {
    if (!$comfort.twenty) {
      reminder = false;
      remaining = 20;
      return;
    }
    nextReminder = Date.now() + 20 * 60_000;
    const timer = setInterval(() => {
      now = Date.now();
      if (reminder) {
        remaining -= 1;
        if (remaining <= 0) {
          reminder = false;
          remaining = 20;
          nextReminder = Date.now() + 20 * 60_000;
        }
      } else if (Date.now() >= nextReminder) {
        reminder = true;
        remaining = 20;
      }
    }, 1000);
    return () => clearInterval(timer);
  });

  function dismissReminder() {
    reminder = false;
    remaining = 20;
    nextReminder = Date.now() + 20 * 60_000;
  }
</script>

{#if effectiveDim > 0.5}
  <div
    class="comfort-dim"
    style:opacity={`${effectiveDim / 100}`}
    aria-hidden="true"
  ></div>
{/if}
{#if effectiveAmber > 0.5}
  <div
    class="comfort-amber"
    style:background={`rgba(255, 154, 60, ${(effectiveAmber / 100) * 0.22})`}
    aria-hidden="true"
  ></div>
{/if}

{#if reminder}
  <div class="comfort-reminder" role="dialog" aria-modal="true" aria-label={$t("comfort.reminderTitle")}>
    <div class="comfort-card">
      <span class="comfort-mark" aria-hidden="true">20·20·20</span>
      <h2>{$t("comfort.reminderTitle")}</h2>
      <p>{$t("comfort.reminderHelp")}</p>
      <strong class="comfort-count">{remaining}s</strong>
      <button class="comfort-skip" onclick={dismissReminder}>
        {$t("comfort.reminderSkip")}
      </button>
      <button
        class="comfort-disable"
        onclick={() => {
          updateComfort({ twenty: false });
          dismissReminder();
        }}
      >
        {$t("comfort.reminderDisable")}
      </button>
    </div>
  </div>
{/if}

<style>
  .comfort-dim,
  .comfort-amber {
    position: fixed;
    inset: 0;
    z-index: 240;
    pointer-events: none;
  }

  .comfort-dim {
    background: #000;
    transition: opacity 180ms ease;
  }

  .comfort-amber {
    mix-blend-mode: multiply;
    transition: background 180ms ease;
  }

  .comfort-reminder {
    position: fixed;
    inset: 0;
    z-index: 280;
    display: grid;
    place-items: center;
    padding: 24px;
    background: color-mix(in srgb, var(--bg) 76%, transparent);
    backdrop-filter: blur(4px);
  }

  .comfort-card {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    width: min(360px, 100%);
    padding: 28px 24px 20px;
    border: 1px solid var(--border);
    border-radius: 22px;
    background: var(--surface);
    text-align: center;
    box-shadow: var(--shadow-lg);
  }

  .comfort-mark {
    display: grid;
    place-items: center;
    width: 62px;
    height: 62px;
    border-radius: 50%;
    background: var(--accent-soft);
    color: var(--accent);
    font-family: var(--font-display);
    font-size: 0.82rem;
    font-weight: 600;
    letter-spacing: 0.04em;
  }

  .comfort-card h2 {
    margin: 4px 0 0;
    font-family: var(--font-display);
    font-size: 1.4rem;
    font-weight: 500;
  }

  .comfort-card p {
    margin: 0;
    max-width: 30ch;
    color: var(--muted);
    font-size: 0.78rem;
    line-height: 1.5;
  }

  .comfort-count {
    margin: 6px 0 2px;
    color: var(--accent);
    font-family: var(--font-display);
    font-size: 2rem;
    font-weight: 500;
    font-variant-numeric: tabular-nums;
  }

  .comfort-skip {
    min-height: 38px;
    padding: 0.45rem 1rem;
    border: 0;
    border-radius: 999px;
    background: linear-gradient(100deg, var(--accent), var(--accent-2));
    color: var(--accent-contrast);
    font: inherit;
    font-size: 0.78rem;
    font-weight: 600;
  }

  .comfort-disable {
    padding: 0;
    border: 0;
    background: transparent;
    color: var(--muted);
    font: inherit;
    font-size: 0.65rem;
    text-decoration: underline;
  }
</style>
