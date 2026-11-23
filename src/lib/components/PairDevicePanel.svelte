<script lang="ts">
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { onDestroy, onMount } from "svelte";
  import Icon from "./Icon.svelte";
  import { t, translate } from "../i18n";
  import {
    cancelDevicePairing,
    errorMessage,
    getPeers,
    joinDevicePairing,
    startDevicePairing,
  } from "../api";
  import type {
    PairedDeviceInfo,
    PairingStateEvent,
    PairingTicketView,
    PeerInfo,
  } from "../types";

  let {
    onclose,
    onpaired,
  }: {
    onclose: () => void;
    onpaired?: (device: PairedDeviceInfo) => void;
  } = $props();

  let ticket = $state<PairingTicketView | null>(null);
  let peers = $state<PeerInfo[]>([]);
  let selectedPeer = $state<PeerInfo | null>(null);
  let codeInput = $state("");
  let addressInput = $state("");
  let manualCode = $state("");
  let showManual = $state(false);
  let stateMessage = $state<string | null>(null);
  let error = $state<string | null>(null);
  let busy = $state<string | null>(null);
  let paired = $state<PairedDeviceInfo | null>(null);
  let remaining = $state(0);
  let finished = $state(false);
  let closed = false;
  let expiredHandled = false;
  let unlisten: UnlistenFn[] = [];
  let peersTimer: ReturnType<typeof setInterval> | undefined;
  let clockTimer: ReturnType<typeof setInterval> | undefined;

  const isMobile = /android|iphone|ipad|ipod/i.test(navigator.userAgent);
  let manualUri = $derived(addressInput.trim().startsWith("kora://"));

  onMount(() => {
    void start();
    peersTimer = setInterval(() => void loadPeers(), 1500);
    clockTimer = setInterval(() => updateClock(), 1000);
    void initEvents();
    return () => {
      cleanup();
    };
  });

  onDestroy(() => {
    closed = true;
    cleanup();
    if (!finished) void cancelDevicePairing();
  });

  function cleanup() {
    for (const stop of unlisten) stop();
    unlisten = [];
    if (peersTimer) clearInterval(peersTimer);
    if (clockTimer) clearInterval(clockTimer);
    peersTimer = undefined;
    clockTimer = undefined;
  }

  async function initEvents() {
    unlisten.push(
      await listen<PairingStateEvent>("sync::pairing_state", (event) => {
        stateMessage = event.payload.message;
        if (event.payload.state === "error") {
          error = event.payload.message;
          ticket = null;
        }
        if (event.payload.state === "paired" && !paired) {
          void completeFromHost(event.payload.peerName);
        }
      }),
    );
    unlisten.push(
      await listen<PairedDeviceInfo>("sync::paired", (event) => {
        if (!paired) {
          void complete(event.payload);
        }
      }),
    );
  }

  async function start() {
    error = null;
    paired = null;
    finished = false;
    ticket = null;
    expiredHandled = false;
    busy = "start";
    try {
      const created = await startDevicePairing();
      if (closed) {
        void cancelDevicePairing();
        return;
      }
      ticket = created;
      stateMessage = translate("sync.waitingForPeer");
      updateClock();
      await loadPeers();
    } catch (cause) {
      error = errorMessage(cause);
    } finally {
      busy = null;
    }
  }

  function updateClock() {
    if (!ticket) {
      remaining = 0;
      return;
    }
    remaining = Math.max(0, ticket.expiresAt - Math.floor(Date.now() / 1000));
    if (remaining === 0 && !expiredHandled) {
      expiredHandled = true;
      void cancelDevicePairing();
    }
  }

  async function loadPeers() {
    try {
      const list = await getPeers();
      peers = list.filter(
        (peer) => peer.purpose === "pair" && !peer.trusted && Boolean(peer.ticket),
      );
    } catch {
      // El sondeo no debe llenar la interfaz de errores.
    }
  }

  function formattedCode(code: string) {
    return code.length === 6 ? `${code.slice(0, 3)} ${code.slice(3)}` : code;
  }

  function normalizeCode(value: string) {
    return value.replace(/\D/g, "").slice(0, 6);
  }

  function formatRemaining(seconds: number) {
    const minutes = Math.floor(seconds / 60);
    return `${minutes}:${String(seconds % 60).padStart(2, "0")}`;
  }

  function choosePeer(peer: PeerInfo) {
    selectedPeer = peer;
    codeInput = "";
    error = null;
    stateMessage = translate("sync.enterPeerCode", { peer: peer.name });
  }

  async function connectSelected() {
    if (!selectedPeer) return;
    const code = normalizeCode(codeInput);
    if (code.length !== 6) {
      error = translate("sync.codeSixDigits");
      return;
    }
    busy = "connect";
    error = null;
    stateMessage = translate("sync.connectingTo", { peer: selectedPeer.name });
    try {
      const device = await joinDevicePairing({
        peerId: selectedPeer.id,
        code,
      });
      await complete(device);
    } catch (cause) {
      error = pairErrorMessage(cause);
      stateMessage = null;
      busy = null;
    }
  }

  async function connectManual() {
    const address = addressInput.trim();
    if (!address) {
      error = translate("sync.addressRequired");
      return;
    }
    const code = normalizeCode(manualCode);
    if (!manualUri && code.length !== 6) {
      error = translate("sync.codeSixDigits");
      return;
    }
    busy = "manual";
    error = null;
    stateMessage = translate("sync.connecting");
    try {
      const device = await joinDevicePairing(
        manualUri ? { uri: address } : { address, code },
      );
      await complete(device);
    } catch (cause) {
      error = pairErrorMessage(cause);
      stateMessage = null;
      busy = null;
    }
  }

  async function scanQr() {
    busy = "scan";
    error = null;
    try {
      const { scan, Format, checkPermissions, requestPermissions } = await import(
        "@tauri-apps/plugin-barcode-scanner"
      );
      let permission = await checkPermissions();
      if (permission !== "granted") {
        permission = await requestPermissions();
      }
      if (permission !== "granted") {
        error = translate("sync.cameraPermission");
        busy = null;
        return;
      }
      const scanned = await scan({
        windowed: true,
        formats: [Format.QRCode],
      });
      const device = await joinDevicePairing({ uri: scanned.content });
      await complete(device);
    } catch (cause) {
      const message = errorMessage(cause);
      if (!/cancel|cancelad/i.test(message)) {
        error = translate("sync.invalidQr");
      }
      busy = null;
    }
  }

  async function complete(device: PairedDeviceInfo) {
    if (paired) return;
    paired = device;
    finished = true;
    stateMessage = translate("sync.pairedWith", { peer: device.name });
    busy = null;
    onpaired?.(device);
    setTimeout(() => onclose(), 1200);
  }

  async function completeFromHost(peerName: string | null) {
    if (peerName) {
      stateMessage = translate("sync.pairedWith", { peer: peerName });
    }
    paired = {
      id: "",
      name: peerName ?? translate("sync.otherDevice"),
      platform: "",
      fingerprint: "",
      accountHint: "",
    };
    finished = true;
    busy = null;
    setTimeout(() => onclose(), 1200);
  }

  function pairErrorMessage(cause: unknown) {
    const message = errorMessage(cause);
    if (/c[oó]digo/i.test(message)) {
      return translate("sync.wrongCode");
    }
    return message;
  }

  async function close() {
    finished = true;
    closed = true;
    try {
      await cancelDevicePairing();
    } finally {
      onclose();
    }
  }
</script>

<div class="pair-overlay" role="presentation" onclick={(event) => {
  if (event.target === event.currentTarget) void close();
}}>
  <div
    class="pair-panel"
    role="dialog"
    aria-modal="true"
    aria-labelledby="pair-title"
  >
    <header class="pair-head">
      <div>
        <p class="eyebrow">{$t("sync.linkDevice")}</p>
        <h2 id="pair-title">{$t("sync.linkTitle")}</h2>
      </div>
      <button class="icon-button" type="button" onclick={close} aria-label={$t("common.close")}>
        <Icon name="close" size={18} />
      </button>
    </header>

    {#if error}
      <p class="notice" role="alert">{error}</p>
    {/if}
    {#if stateMessage && !paired}
      <p class="pair-state" role="status">{stateMessage}</p>
    {/if}
    {#if paired}
      <div class="paired" role="status">
        <span class="paired-icon"><Icon name="check" size={22} /></span>
        <div>
          <strong>{$t("sync.linked")}</strong>
          <p>{stateMessage}</p>
        </div>
      </div>
    {:else if ticket}
      <div class="pair-grid">
        <section class="code-card">
          <p class="eyebrow">{$t("sync.yourCode")}</p>
          <p class="pair-code">{formattedCode(ticket.code)}</p>
          <p class="hint">{$t("sync.codeHelp")}</p>
          <div class="countdown" class:expired={remaining === 0}>
            {remaining > 0
              ? $t("sync.codeExpires", { time: formatRemaining(remaining) })
              : $t("sync.codeExpired")}
          </div>
          {#if ticket.qrSvg}
            <div class="qr">{@html ticket.qrSvg}</div>
          {/if}
          <p class="hint address-line">
            {$t("sync.addressLabel")}:
            <code
              >{ticket.addresses.length ? ticket.addresses[0] : "—"}:{ticket.port}</code
            >
          </p>
          <details class="pair-details">
            <summary>{$t("sync.pairDetails")}</summary>
            <p class="hint">
              {$t("sync.addressLabel")}:
              <code
                >{ticket.addresses.length ? ticket.addresses.join(", ") : "—"}:{ticket.port}</code
              >
            </p>
            <p class="hint code-uri">{ticket.uri}</p>
          </details>
          {#if remaining === 0}
            <button class="primary small" onclick={start} disabled={busy !== null}>
              <Icon name="sync" size={15} /> {$t("sync.newCode")}
            </button>
          {/if}
        </section>

        <section class="nearby">
          <div class="nearby-head">
            <h3>{$t("sync.nearby")}</h3>
            <button class="ghost small" onclick={loadPeers} disabled={busy !== null}>
              {$t("common.retry")}
            </button>
          </div>
          {#if peers.length === 0}
            <p class="empty">{$t("sync.noNearby")}</p>
          {:else}
            <div class="peer-list">
              {#each peers as peer (peer.id)}
                <button
                  class="peer-row"
                  class:selected={selectedPeer?.id === peer.id}
                  onclick={() => choosePeer(peer)}
                  disabled={busy !== null || remaining === 0}
                >
                  <span class="peer-icon"><Icon name="account" size={17} /></span>
                  <span class="peer-copy">
                    <strong>{peer.name}</strong>
                    <small>{peer.platform || $t("sync.sameNetwork")}</small>
                  </span>
                  <Icon name="chevron" size={16} />
                </button>
              {/each}
            </div>
          {/if}

          {#if selectedPeer}
            <div class="join-box">
              <p class="hint">{$t("sync.enterPeerCode", { peer: selectedPeer.name })}</p>
              <input
                class="code-input"
                bind:value={codeInput}
                maxlength="6"
                inputmode="numeric"
                autocomplete="one-time-code"
                placeholder="000000"
                onkeydown={(event) => {
                  if (event.key === "Enter") void connectSelected();
                }}
              />
              <button
                class="primary small"
                onclick={connectSelected}
                disabled={busy !== null || normalizeCode(codeInput).length !== 6}
              >
                {busy === "connect" ? $t("sync.connecting") : $t("sync.connect")}
              </button>
            </div>
          {/if}

          <div class="pair-actions">
            {#if isMobile}
              <button class="primary small" onclick={scanQr} disabled={busy !== null}>
                <Icon name="grid" size={15} />
                {busy === "scan" ? $t("sync.scanning") : $t("sync.scanQr")}
              </button>
            {/if}
            <button class="ghost small" onclick={() => (showManual = !showManual)}>
              <Icon name="edit" size={15} /> {$t("sync.enterAddress")}
            </button>
          </div>

          {#if showManual}
            <div class="manual-box">
              <label>
                <span>{$t("sync.address")}</span>
                <input
                  bind:value={addressInput}
                  placeholder="192.168.1.24:47821"
                  autocomplete="off"
                />
              </label>
              <label>
                <span>{$t("sync.code")}</span>
                <input
                  bind:value={manualCode}
                  maxlength="6"
                  inputmode="numeric"
                  placeholder="000000"
                  disabled={manualUri}
                />
              </label>
              <button
                class="primary small"
                onclick={connectManual}
                disabled={busy !== null || (!manualUri && normalizeCode(manualCode).length !== 6)}
              >
                {busy === "manual" ? $t("sync.connecting") : $t("sync.connect")}
              </button>
            </div>
          {/if}
        </section>
      </div>
    {:else}
      <div class="pair-retry">
        <p class="hint">{error ? $t("sync.tryAgain") : $t("sync.codeExpired")}</p>
        <button class="primary" onclick={start} disabled={busy !== null}>
          <Icon name="sync" size={16} />
          {error ? $t("common.retry") : $t("sync.newCode")}
        </button>
      </div>
    {/if}
  </div>
</div>

<style>
  .pair-overlay {
    position: fixed;
    inset: 0;
    z-index: 80;
    display: grid;
    place-items: center;
    padding: 1rem;
    background: color-mix(in srgb, #101820 55%, transparent);
    backdrop-filter: blur(5px);
  }

  .pair-panel {
    display: flex;
    flex-direction: column;
    gap: 0.9rem;
    width: min(760px, 100%);
    max-height: min(90vh, 860px);
    overflow: auto;
    padding: 1.1rem;
    border: 1px solid var(--border);
    border-radius: 20px;
    background: var(--surface);
    box-shadow: 0 30px 80px -30px color-mix(in srgb, #000 55%, transparent);
  }

  .pair-head {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 0.8rem;
  }

  .pair-head h2 {
    margin: 0.15rem 0 0;
    font-family: var(--font-display);
    font-size: 1.35rem;
    font-weight: 550;
  }

  .icon-button {
    display: grid;
    place-items: center;
    width: 36px;
    height: 36px;
    border: 1px solid var(--border);
    border-radius: 10px;
    background: var(--surface-2);
    color: var(--muted);
  }

  .pair-grid {
    display: grid;
    grid-template-columns: minmax(0, 0.9fr) minmax(0, 1.1fr);
    gap: 0.9rem;
  }

  .code-card,
  .nearby {
    display: flex;
    flex-direction: column;
    gap: 0.55rem;
    padding: 0.9rem;
    border: 1px solid var(--border);
    border-radius: 14px;
    background: var(--surface-2);
  }

  .pair-code {
    margin: 0;
    font-family: var(--font-display);
    font-size: clamp(2rem, 7vw, 3.1rem);
    font-weight: 700;
    letter-spacing: 0.16em;
    font-variant-numeric: tabular-nums;
    color: var(--accent);
  }

  .countdown {
    align-self: flex-start;
    padding: 0.18rem 0.5rem;
    border-radius: 999px;
    background: var(--accent-soft);
    color: var(--accent);
    font-size: 0.7rem;
    font-weight: 650;
  }

  .countdown.expired {
    background: color-mix(in srgb, var(--danger) 10%, transparent);
    color: var(--danger);
  }

  .qr {
    display: grid;
    place-items: center;
    padding: 0.55rem;
    border-radius: 12px;
    background: #fff;
  }

  .qr :global(svg) {
    display: block;
    width: min(200px, 100%);
    height: auto;
  }

  .pair-details {
    font-size: 0.72rem;
  }

  .pair-details summary {
    cursor: pointer;
    color: var(--muted);
    font-weight: 600;
  }

  .code-uri {
    word-break: break-all;
  }

  .address-line {
    padding: 0.35rem 0.5rem;
    border-radius: 8px;
    background: color-mix(in srgb, var(--surface) 70%, transparent);
  }

  .nearby-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.5rem;
  }

  .nearby-head h3 {
    margin: 0;
    font-size: 0.9rem;
  }

  .peer-list {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
  }

  .peer-row {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    width: 100%;
    padding: 0.55rem;
    border: 1px solid var(--border);
    border-radius: 11px;
    background: var(--surface);
    color: var(--text);
    text-align: left;
  }

  .peer-row:hover,
  .peer-row.selected {
    border-color: var(--accent-line);
    background: color-mix(in srgb, var(--accent) 6%, var(--surface));
  }

  .peer-icon {
    display: grid;
    place-items: center;
    width: 34px;
    height: 34px;
    border-radius: 9px;
    background: var(--accent-soft);
    color: var(--accent);
  }

  .peer-copy {
    display: flex;
    flex-direction: column;
    gap: 0.05rem;
    min-width: 0;
    flex: 1;
  }

  .peer-copy strong {
    font-size: 0.82rem;
  }

  .peer-copy small {
    color: var(--muted);
    font-size: 0.68rem;
  }

  .join-box,
  .manual-box {
    display: flex;
    flex-direction: column;
    gap: 0.45rem;
    padding: 0.65rem;
    border: 1px solid var(--accent-line);
    border-radius: 12px;
    background: var(--accent-soft);
  }

  .code-input {
    min-height: 48px;
    padding: 0.5rem 0.7rem;
    border: 1px solid var(--border);
    border-radius: 10px;
    background: var(--surface);
    color: var(--text);
    font: inherit;
    font-size: 1.5rem;
    font-weight: 700;
    letter-spacing: 0.3em;
    text-align: center;
    font-variant-numeric: tabular-nums;
  }

  .manual-box label {
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
    color: var(--muted);
    font-size: 0.7rem;
  }

  .manual-box input {
    min-height: 38px;
    padding: 0.4rem 0.55rem;
    border: 1px solid var(--border);
    border-radius: 9px;
    background: var(--surface);
    color: var(--text);
    font: inherit;
    font-size: 0.85rem;
  }

  .pair-actions {
    display: flex;
    flex-wrap: wrap;
    gap: 0.4rem;
    margin-top: auto;
  }

  .pair-state {
    margin: 0;
    padding: 0.55rem 0.7rem;
    border-radius: 10px;
    background: var(--accent-soft);
    color: var(--accent);
    font-size: 0.76rem;
    font-weight: 600;
  }

  .pair-retry {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 0.6rem;
    padding: 1rem;
    border: 1px dashed var(--border);
    border-radius: 14px;
  }

  .paired {
    display: flex;
    align-items: center;
    gap: 0.8rem;
    padding: 1rem;
    border: 1px solid color-mix(in srgb, var(--ok) 45%, var(--border));
    border-radius: 14px;
    background: color-mix(in srgb, var(--ok) 8%, var(--surface));
  }

  .paired-icon {
    display: grid;
    place-items: center;
    width: 44px;
    height: 44px;
    border-radius: 50%;
    background: color-mix(in srgb, var(--ok) 15%, transparent);
    color: var(--ok);
  }

  .paired p {
    margin: 0.1rem 0 0;
    color: var(--muted);
    font-size: 0.78rem;
  }

  .hint,
  .empty {
    margin: 0;
    color: var(--muted);
    font-size: 0.76rem;
    line-height: 1.5;
  }

  .notice {
    margin: 0;
    padding: 0.6rem 0.75rem;
    border: 1px solid color-mix(in srgb, var(--danger) 55%, var(--border));
    border-radius: 10px;
    background: color-mix(in srgb, var(--danger) 8%, transparent);
    font-size: 0.8rem;
  }

  .primary,
  .ghost {
    min-height: 38px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 0.4rem;
    padding: 0.45rem 0.75rem;
    border-radius: 9px;
    font: inherit;
    font-size: 0.78rem;
    font-weight: 650;
  }

  .primary {
    border: 1px solid color-mix(in srgb, var(--accent) 82%, #000);
    background: linear-gradient(
      180deg,
      var(--accent),
      color-mix(in srgb, var(--accent) 86%, #000)
    );
    color: var(--accent-contrast);
  }

  .ghost {
    border: 1px solid var(--border);
    background: var(--surface);
    color: var(--text);
  }

  .small {
    min-height: 34px;
    font-size: 0.72rem;
  }

  button:disabled {
    opacity: 0.5;
    cursor: default;
  }

  @media (max-width: 640px) {
    .pair-grid {
      grid-template-columns: 1fr;
    }

    .pair-panel {
      max-height: calc(100vh - 1.5rem);
      border-radius: 16px;
    }
  }
</style>
