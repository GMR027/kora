<script lang="ts">
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { save } from "@tauri-apps/plugin-dialog";
  import { onMount } from "svelte";
  import Icon from "../lib/components/Icon.svelte";
  import {
    errorMessage,
    exportAuditLog,
    getPeers,
    getSyncCollections,
    getSyncStatus,
    listSyncDevices,
    revokePeer,
    setSyncCategories,
    listAuditLog,
    startShare,
    startSync,
    stopShare,
    stopSync,
    syncNow,
  } from "../lib/api";
  import { formatDate as formatI18nDate, t, translate } from "../lib/i18n";
  import { pushNotification } from "../lib/notifications";
  import type { AppTab } from "../lib/navigation";
  import type {
    AuditEntry,
    PeerInfo,
    SyncCollections,
    SyncDeviceInfo,
    SyncHandoffEvent,
    SyncStatus,
  } from "../lib/types";

  let { onNavigate }: { onNavigate?: (tab: AppTab) => void } = $props();

  let status = $state<SyncStatus | null>(null);
  let peers = $state<PeerInfo[]>([]);
  let devices = $state<SyncDeviceInfo[]>([]);
  let collections = $state<SyncCollections | null>(null);
  let selectedDeviceId = $state<string | null>(null);
  let lastHandoff = $state<SyncHandoffEvent | null>(null);
  let busy = $state<string | null>(null);
  let error = $state<string | null>(null);
  let notice = $state<string | null>(null);
  let syncAll = $state(true);
  let selected = $state<string[]>([]);
  let unlisten: UnlistenFn[] = [];
  let timer: ReturnType<typeof setInterval> | undefined;
  let audit = $state<AuditEntry[]>([]);
  let exportingAudit = $state(false);

  onMount(() => {
    void load();
    timer = setInterval(() => void pollPeers(), 1600);
    void initEvents().catch((cause) => {
      error = errorMessage(cause);
    });
    return () => {
      for (const stop of unlisten) stop();
      if (timer) clearInterval(timer);
    };
  });

  async function initEvents() {
    unlisten.push(
      await listen<SyncStatus>("sync::status", (event) => {
        status = event.payload;
      }),
    );
    unlisten.push(
      await listen<SyncHandoffEvent>("sync::handoff", (event) => {
        lastHandoff = event.payload;
        notice = translate("sync.handoffNotice", {
          peer: event.payload.peerName,
          chapter: event.payload.handoff.chapter + 1,
          offset: event.payload.handoff.offset,
        });
      }),
    );
  }

  async function load() {
    try {
      const [current, peerList, deviceList] = await Promise.all([
        getSyncStatus(),
        getPeers(),
        listSyncDevices(),
      ]);
      status = current;
      peers = peerList;
      devices = deviceList;
      audit = await listAuditLog(12);

      const initial = deviceList.find(
        (device) => !device.isLocal && device.trusted,
      );
      if (initial) {
        selectedDeviceId = initial.id;
        applyCollections(await getSyncCollections(initial.id));
      } else {
        selectedDeviceId = null;
        collections = null;
      }
    } catch (cause) {
      error = errorMessage(cause);
    }
  }

  function applyCollections(value: SyncCollections) {
    collections = value;
    selectedDeviceId = value.deviceId;
    syncAll = value.syncAll;
    selected = value.categories
      .filter((item) => item.enabled)
      .map((item) => item.category.id);
  }

  async function selectDevice(id: string) {
    if (!id) return;
    busy = "collections";
    error = null;
    try {
      selectedDeviceId = id;
      applyCollections(await getSyncCollections(id));
    } catch (cause) {
      error = errorMessage(cause);
    } finally {
      busy = null;
    }
  }

  async function pollPeers() {
    if (!status?.running) return;
    try {
      peers = await getPeers();
    } catch {
      // El sondeo no debe llenar la interfaz de errores.
    }
  }

  async function loadPeers() {
    try {
      peers = await getPeers();
    } catch (cause) {
      error = errorMessage(cause);
    }
  }

  async function exportAudit() {
    exportingAudit = true;
    error = null;
    try {
      const destination = await save({
        defaultPath: "kora-auditoria.json",
        filters: [{ name: "JSON", extensions: ["json"] }],
      });
      if (!destination) return;
      const path = await exportAuditLog(destination);
      notice = translate("sync.auditExported", { path });
    } catch (cause) {
      error = errorMessage(cause);
    } finally {
      exportingAudit = false;
    }
  }

  async function toggleSync() {
    busy = "sync";
    error = null;
    try {
      status = status?.running ? await stopSync() : await startSync();
      await loadPeers();
    } catch (cause) {
      error = errorMessage(cause);
    } finally {
      busy = null;
    }
  }

  async function toggleShare() {
    busy = "share";
    error = null;
    try {
      status = status?.sharing ? await stopShare() : await startShare();
    } catch (cause) {
      error = errorMessage(cause);
    } finally {
      busy = null;
    }
  }

  async function syncWith(peer: PeerInfo) {
    busy = `sync:${peer.id}`;
    error = null;
    try {
      const outcome = await syncNow(peer.id);
      notice = translate("sync.syncDone", {
        peer: peer.name,
        received: outcome.received,
        sent: outcome.sent,
        duration: outcome.durationMs,
      });
      pushNotification({
        kind: "success",
        title: translate("notifications.syncDone"),
        description: translate("notifications.syncBody", {
          peer: peer.name,
          received: outcome.received,
          sent: outcome.sent,
        }),
        action: "sync",
      });
      status = await getSyncStatus();
    } catch (cause) {
      error = errorMessage(cause);
    } finally {
      busy = null;
    }
  }

  async function forget(peer: PeerInfo) {
    if (!window.confirm(translate("sync.forgetConfirm", { name: peer.name }))) {
      return;
    }
    try {
      await revokePeer(peer.id);
      await loadPeers();
      const deviceList = await listSyncDevices();
      devices = deviceList;
      if (selectedDeviceId === peer.id) {
        const next = deviceList.find(
          (device) => !device.isLocal && device.trusted,
        );
        if (next) {
          await selectDevice(next.id);
        } else {
          selectedDeviceId = null;
          collections = null;
        }
      }
    } catch (cause) {
      error = errorMessage(cause);
    }
  }

  function toggleCategory(id: string) {
    selected = selected.includes(id)
      ? selected.filter((value) => value !== id)
      : [...selected, id];
  }

  async function saveCollections() {
    if (!selectedDeviceId) return;
    busy = "collections";
    error = null;
    try {
      applyCollections(
        await setSyncCategories(selectedDeviceId, selected, syncAll),
      );
      notice = translate("sync.filterSaved", {
        device: collections?.deviceName ?? "",
      });
    } catch (cause) {
      error = errorMessage(cause);
    } finally {
      busy = null;
    }
  }

  function formatDate(value: string | null) {
    if (!value) return translate("common.never");
    return formatI18nDate(value, {
      day: "2-digit",
      month: "short",
      hour: "2-digit",
      minute: "2-digit",
    });
  }

  function peerLabel(peer: PeerInfo) {
    if (peer.trusted) return translate("sync.peerTrusted");
    return peer.purpose === "pair"
      ? translate("sync.peerPair")
      : translate("sync.peerDiscovered");
  }
</script>

<section class="sync-view">
  <header class="hero">
    <div>
      <p class="eyebrow">{$t("nav.sync")}</p>
      <h1 class="page-title">{$t("sync.title")}</h1>
      <p class="page-subtitle">{$t("sync.subtitle")}</p>
    </div>
    <div class="hero-actions">
      <span class="status-pill" class:active={status?.running}>
        <i aria-hidden="true"></i>
        {status?.running ? $t("sync.active") : $t("sync.stopped")}
      </span>
      <button class="primary" onclick={toggleSync} disabled={busy !== null}>
        {status?.running ? $t("sync.stop") : $t("sync.start")}
      </button>
    </div>
  </header>

  {#if error}
    <p class="notice" role="alert">{error}</p>
  {/if}
  {#if notice}
    <p class="ok" role="status">{notice}</p>
  {/if}

  {#if status?.running && status.peerCount === 0}
    <div class="sync-hint">
      <span class="card-icon"><Icon name="sync" size={18} /></span>
      <div>
        <strong>{$t("sync.listening", { port: status.listenPort })}</strong>
        <p>{$t("sync.listeningHelp")}</p>
      </div>
    </div>
  {/if}

  <div class="cards">
    <article class="card">
      <span class="card-icon"><Icon name="sync" size={18} /></span>
      <h2>{$t("sync.status")}</h2>
      <p class="card-value">{status?.running ? $t("sync.active") : $t("sync.stopped")}</p>
      <p class="hint">
        {status?.trustedPeers ?? 0} {$t("account.devices").toLowerCase()} ·
        {$t("sync.pending", { count: status?.pendingChanges ?? 0 })}
      </p>
      <p class="hint">{$t("sync.lastSync")}: {formatDate(status?.lastSync ?? null)}</p>
      {#if status?.running}
        <p class="hint">{$t("sync.port")}: <code>{status.listenPort}</code></p>
      {/if}
      {#if status?.pairing || status?.sharing}
        <p class="badge">
          {status.pairing
            ? $t("sync.pairingAdvertised")
            : $t("sync.sharingAdvertised")}
        </p>
      {/if}
    </article>

    <article class="card">
      <span class="card-icon"><Icon name="account" size={18} /></span>
      <h2>{$t("sync.pair")}</h2>
      <p class="hint">{$t("sync.pairHelp")}</p>
      <div class="actions">
        <button class="ghost small" onclick={() => onNavigate?.("account")}>
          {$t("sync.openAccount")}
        </button>
      </div>
    </article>

    <article class="card">
      <span class="card-icon"><Icon name="book" size={18} /></span>
      <h2>{$t("sync.share")}</h2>
      <p class="hint">{$t("sync.shareHelp")}</p>
      <div class="actions">
        <button class="ghost small" onclick={toggleShare} disabled={busy !== null}>
          {status?.sharing ? $t("common.close") : $t("sync.share")}
        </button>
      </div>
    </article>
  </div>

  <section class="panel">
    <div class="panel-head">
      <h2>{$t("sync.peers")}</h2>
      <button class="ghost small" onclick={loadPeers}>{$t("common.retry")}</button>
    </div>
    {#if peers.length === 0}
      <p class="empty">{$t("sync.noPeers")}</p>
    {:else}
      <div class="peers">
        {#each peers as peer (peer.id)}
          <article class="peer">
            <div>
              <strong>{peer.name}</strong>
              <span class="hint">
                {peer.address ?? $t("sync.noAddress")}:{peer.port} ·
                {peerLabel(peer)}
                {#if peer.sasPrefix}· SAS {peer.sasPrefix}{/if}
              </span>
            </div>
            <div class="actions">
              {#if peer.trusted}
                <button
                  class="ghost small"
                  onclick={() => syncWith(peer)}
                  disabled={busy !== null || !peers.length}
                >
                  {$t("sync.start")}
                </button>
                <button class="danger small" onclick={() => forget(peer)}>{$t("common.remove")}</button>
              {/if}
            </div>
          </article>
        {/each}
      </div>
    {/if}
  </section>

  <section class="panel">
    <div class="panel-head">
      <h2>{$t("sync.categories")}</h2>
      <label class="toggle">
        <input type="checkbox" bind:checked={syncAll} />
        <span>{$t("sync.syncAll")}</span>
      </label>
    </div>
    <div class="device-select">
      <label for="sync-device">{$t("sync.shareWith")}</label>
      <select
        id="sync-device"
        value={selectedDeviceId ?? ""}
        onchange={(event) => selectDevice(event.currentTarget.value)}
        disabled={busy !== null || !devices.some((device) => !device.isLocal)}
      >
        {#each devices.filter((device) => !device.isLocal) as device (device.id)}
          <option value={device.id}>{device.name}</option>
        {/each}
      </select>
    </div>
    {#if selectedDeviceId}
      <p class="hint">
        {$t("settings.syncHelp")}
        <strong>{collections?.deviceName ?? ""}</strong>
      </p>
    {:else}
      <p class="hint">{$t("sync.noPeersHelp")}</p>
    {/if}
    {#if collections?.categories.length}
      <div class="collections">
        {#each collections.categories as item (item.category.id)}
          <label class="collection">
            <input
              type="checkbox"
              checked={syncAll || selected.includes(item.category.id)}
              disabled={syncAll}
              onchange={() => toggleCategory(item.category.id)}
            />
            <span>{item.category.name}</span>
          </label>
        {/each}
      </div>
    {:else}
      <p class="empty">{$t("sync.noCategories")}</p>
    {/if}
    <div class="actions">
      <button
        class="primary small"
        onclick={saveCollections}
        disabled={busy !== null || !selectedDeviceId}
      >
        {$t("sync.save")}
      </button>
    </div>
  </section>

  <section class="panel">
    <div class="panel-head">
      <h2>{$t("sync.audit")}</h2>
      <button class="ghost small" onclick={exportAudit} disabled={exportingAudit}>
        <Icon name="download" size={15} /> {$t("sync.exportAudit")}
      </button>
    </div>
    {#if audit.length === 0}
      <p class="empty">{$t("sync.noAudit")}</p>
    {:else}
      <ul class="audit-list">
        {#each audit as entry (entry.id)}
          <li>
            <span class="audit-action">{entry.action}</span>
            <span class="audit-resource">
              {entry.resourceType ?? $t("sync.auditSystem")}
              {#if entry.resourceId}<code>{entry.resourceId.slice(0, 10)}…</code>{/if}
            </span>
            <time>{formatI18nDate(entry.createdAt, { dateStyle: "short", timeStyle: "short" })}</time>
          </li>
        {/each}
      </ul>
    {/if}
  </section>

  {#if lastHandoff}
    <section class="panel">
      <h2>{$t("sync.handoffReceived")}</h2>
      <p class="hint">
        {$t("sync.handoffHelp", {
          peer: lastHandoff.peerName,
          chapter: lastHandoff.handoff.chapter + 1,
          offset: lastHandoff.handoff.offset,
        })}
      </p>
    </section>
  {/if}
</section>

<style>
  .sync-view {
    display: flex;
    flex-direction: column;
    gap: 1.35rem;
  }

  .hero {
    display: flex;
    flex-wrap: wrap;
    align-items: flex-end;
    justify-content: space-between;
    gap: 1rem 1.5rem;
  }

  .hero-actions {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.5rem;
  }

  .status-pill {
    display: inline-flex;
    align-items: center;
    gap: 0.45rem;
    padding: 0.4rem 0.75rem;
    border: 1px solid var(--border);
    border-radius: 999px;
    background: var(--surface);
    color: var(--muted);
    font-size: 0.74rem;
    font-weight: 550;
    box-shadow: var(--shadow-xs);
  }

  .status-pill i {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--muted);
  }

  .status-pill.active {
    border-color: color-mix(in srgb, var(--ok) 38%, var(--border));
    color: var(--ok);
  }

  .status-pill.active i {
    background: var(--ok);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--ok) 16%, transparent);
  }

  .cards {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(230px, 1fr));
    gap: 0.8rem;
  }

  .card {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 0.35rem;
    padding: 1.05rem;
    border: 1px solid var(--border);
    border-radius: 16px;
    background: var(--surface);
    box-shadow: var(--shadow-xs);
  }

  .card-icon {
    display: grid;
    place-items: center;
    width: 38px;
    height: 38px;
    margin-bottom: 0.25rem;
    border: 1px solid color-mix(in srgb, var(--accent) 20%, var(--border));
    border-radius: 11px;
    background: var(--accent-soft);
    color: var(--accent);
  }

  .card h2 {
    margin: 0;
    font-size: 0.9rem;
  }

  .card p {
    margin: 0;
  }

  .card-value {
    font-family: var(--font-display);
    font-size: 1.3rem;
    font-weight: 650;
    letter-spacing: -0.02em;
  }

  .panel {
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
    padding: 1.05rem;
    border: 1px solid var(--border);
    border-radius: 16px;
    background: var(--surface);
    box-shadow: var(--shadow-xs);
  }

  .panel-head {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: 0.7rem;
  }

  .panel h2,
  .panel-head h2 {
    margin: 0;
    font-size: 1rem;
  }

  h2 {
    margin: 0;
    font-size: 1rem;
  }

  .peers,
  .collections {
    display: flex;
    flex-direction: column;
    gap: 0.45rem;
  }

  .peer {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: 0.7rem;
    padding: 0.65rem 0.75rem;
    border: 1px solid var(--border);
    border-radius: 11px;
    background: var(--surface-2);
  }

  .peer > div:first-child {
    display: flex;
    flex-direction: column;
    gap: 0.12rem;
    min-width: 0;
  }

  .peer strong {
    font-size: 0.84rem;
  }

  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: 0.35rem;
  }

  .hint,
  .empty {
    margin: 0;
    color: var(--muted);
    font-size: 0.78rem;
    line-height: 1.5;
    overflow-wrap: anywhere;
  }

  .notice,
  .ok {
    margin: 0;
    padding: 0.65rem 0.85rem;
    border-radius: 10px;
    font-size: 0.82rem;
  }

  .notice {
    border: 1px solid color-mix(in srgb, var(--danger) 55%, var(--border));
    background: color-mix(in srgb, var(--danger) 9%, transparent);
  }

  .ok {
    border: 1px solid color-mix(in srgb, var(--ok) 55%, var(--border));
    background: color-mix(in srgb, var(--ok) 9%, transparent);
  }

  .badge {
    display: inline-flex;
    align-items: center;
    min-height: 22px;
    padding: 0.1rem 0.45rem;
    border: 1px solid var(--border);
    border-radius: 999px;
    color: var(--muted);
    font-size: 0.66rem;
  }

  .toggle,
  .collection {
    display: flex;
    align-items: center;
    gap: 0.45rem;
    color: var(--text);
    font-size: 0.8rem;
  }

  .toggle input,
  .collection input {
    width: 17px;
    height: 17px;
  }

  .device-select {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.55rem;
    color: var(--muted);
    font-size: 0.8rem;
  }

  .device-select select {
    flex: 1;
    min-width: 0;
    min-height: 40px;
    padding: 0.45rem 0.55rem;
    border: 1px solid var(--border);
    border-radius: 9px;
    background: var(--surface-2);
    color: var(--text);
    font: inherit;
    font-size: 0.82rem;
  }

  .collection {
    padding: 0.4rem 0.55rem;
    border: 1px solid var(--border);
    border-radius: 9px;
    background: var(--surface-2);
  }

  .primary {
    min-height: 40px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 0.4rem;
    padding: 0.55rem 0.9rem;
    border: 1px solid color-mix(in srgb, var(--accent) 82%, #000);
    border-radius: 10px;
    background: linear-gradient(
      180deg,
      var(--accent),
      color-mix(in srgb, var(--accent) 86%, #000)
    );
    color: var(--accent-contrast);
    font: inherit;
    font-size: 0.82rem;
    font-weight: 650;
    box-shadow: 0 8px 18px -12px color-mix(in srgb, var(--accent) 80%, transparent);
  }

  .ghost {
    min-height: 36px;
    padding: 0.38rem 0.65rem;
    border: 1px solid var(--border);
    border-radius: 9px;
    background: var(--surface-2);
    color: var(--text);
    font: inherit;
    font-size: 0.78rem;
  }

  .ghost:hover {
    border-color: var(--accent-line);
    background: color-mix(in srgb, var(--accent) 6%, var(--surface-2));
  }

  .danger {
    min-height: 36px;
    padding: 0.38rem 0.65rem;
    border: 1px solid color-mix(in srgb, var(--danger) 45%, var(--border));
    border-radius: 9px;
    background: color-mix(in srgb, var(--danger) 8%, transparent);
    color: var(--danger);
    font: inherit;
    font-size: 0.78rem;
  }

  .small {
    min-height: 32px;
    padding: 0.28rem 0.55rem;
    font-size: 0.73rem;
  }

  button:disabled {
    opacity: 0.5;
    cursor: default;
  }

  @media (max-width: 620px) {
    .hero {
      align-items: flex-start;
    }

    .hero-actions {
      width: 100%;
    }

    .hero-actions .primary {
      flex: 1;
    }

    .peer {
      align-items: flex-start;
    }

    .device-select {
      flex-direction: column;
      align-items: stretch;
    }
  }

  .panel-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
  }

  .audit-list {
    display: flex;
    flex-direction: column;
    gap: 2px;
    margin: 8px 0 0;
    padding: 0;
    list-style: none;
  }

  .audit-list li {
    display: grid;
    grid-template-columns: minmax(110px, 0.9fr) minmax(0, 1.4fr) auto;
    align-items: center;
    gap: 8px;
    padding: 6px 7px;
    border-radius: 7px;
    color: var(--muted);
    font-size: 0.64rem;
  }

  .audit-list li:nth-child(odd) {
    background: var(--surface-2);
  }

  .audit-action {
    color: var(--text);
    font-weight: 600;
  }

  .audit-resource {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .audit-resource code {
    font-size: 0.58rem;
  }

  .audit-list time {
    color: color-mix(in srgb, var(--muted) 75%, transparent);
    font-size: 0.58rem;
    white-space: nowrap;
  }

  @media (max-width: 620px) {
    .audit-list li {
      grid-template-columns: minmax(0, 1fr) auto;
    }

    .audit-resource {
      display: none;
    }
  }

  .sync-hint {
    display: flex;
    align-items: flex-start;
    gap: 11px;
    padding: 12px 14px;
    border: 1px solid var(--accent-line);
    border-radius: 13px;
    background: var(--accent-soft);
  }

  .sync-hint .card-icon {
    margin: 0;
  }

  .sync-hint strong {
    display: block;
    color: var(--accent);
    font-size: 0.78rem;
    font-weight: 700;
  }

  .sync-hint p {
    margin: 3px 0 0;
    color: var(--muted);
    font-size: 0.7rem;
    line-height: 1.45;
  }
</style>
