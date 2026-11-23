<script lang="ts">
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { convertFileSrc } from "@tauri-apps/api/core";
  import { save } from "@tauri-apps/plugin-dialog";
  import { onMount } from "svelte";
  import Icon from "../lib/components/Icon.svelte";
  import { formatDate as formatI18nDate, t, translate } from "../lib/i18n";
  import {
    acceptShare,
    changePin,
    createAccount,
    createShare,
    deleteAccount,
    errorMessage,
    exportNotes,
    getAccount,
    getPeers,
    libraryList,
    listDevices,
    listReceivedShares,
    listShares,
    removeReceivedShare,
    removeAccountAvatar,
    revokeDevice,
    revokeShare,
    recoverAccountPin,
    revealShareKey,
    updateAccountAvatar,
    updateDisplayName,
  } from "../lib/api";
  import type {
    AccountCreated,
    AccountView,
    BookSummary,
    DeviceInfo,
    PeerInfo,
    ReceivedShare,
    ResourceType,
    ShareCreateRequest,
    SharePermission,
    ShareScopeType,
    ShareView,
  } from "../lib/types";

  type Tab = "account" | "share" | "received";

  let activeTab = $state<Tab>("account");
  let account = $state<AccountView | null>(null);
  let devices = $state<DeviceInfo[]>([]);
  let peers = $state<PeerInfo[]>([]);
  let books = $state<BookSummary[]>([]);
  let shares = $state<ShareView[]>([]);
  let received = $state<ReceivedShare[]>([]);
  let createdShare = $state<ShareView | null>(null);
  let busy = $state<string | null>(null);
  let error = $state<string | null>(null);
  let notice = $state<string | null>(null);
  let unlisten: UnlistenFn[] = [];

  let username = $state("");
  let displayName = $state("");
  let pin = $state("");
  let newPin = $state("");
  let oldPin = $state("");
  let recoverPin = $state("");
  let deletePin = $state("");
  let editingProfile = $state(false);
  let profileName = $state("");
  let avatarInput: HTMLInputElement | undefined = $state();
  let avatarBusy = $state(false);
  let exportingNotes = $state(false);

  let shareScope = $state<ShareScopeType>("book");
  let shareBook = $state("");
  let sharePermission = $state<SharePermission>("copy");
  let shareDays = $state("7");
  let shareMaxUses = $state("");
  let sharePin = $state("");
  let receivePin = $state("");
  let receiveKey = $state("");
  let receivePeer = $state("");

  onMount(() => {
    void load();
    void initEvents();
    return () => {
      for (const stop of unlisten) stop();
    };
  });

  async function initEvents() {
    unlisten.push(
      await listen<ReceivedShare>("accounts::share_accepted", () => {
        void loadReceived();
      }),
    );
  }

  async function load() {
    try {
      const [current, deviceList, peerList, shareList, receivedList, bookList] =
        await Promise.all([
          getAccount(),
          listDevices(),
          getPeers(),
          listShares(),
          listReceivedShares(),
          libraryList(),
        ]);
      account = current;
      if (current) profileName = current.displayName;
      devices = deviceList;
      peers = peerList;
      shares = shareList;
      received = receivedList;
      books = bookList;
      if (!shareBook && books.length > 0) shareBook = books[0].id;
      if (current) {
        activeTab = "account";
      }
    } catch (cause) {
      error = errorMessage(cause);
    }
  }

  async function loadReceived() {
    try {
      received = await listReceivedShares();
      books = await libraryList();
    } catch (cause) {
      error = errorMessage(cause);
    }
  }

  async function createLocalAccount() {
    busy = "account";
    error = null;
    try {
      const created: AccountCreated = await createAccount(
        username.trim().toLowerCase(),
        displayName.trim() || username.trim(),
        pin,
      );
      account = created.account;
      pin = "";
      notice = "Cuenta creada. Ya puedes vincular dispositivos desde Sincronización.";
    } catch (cause) {
      error = errorMessage(cause);
    } finally {
      busy = null;
    }
  }

  async function saveProfile() {
    if (!account) return;
    busy = "profile";
    error = null;
    try {
      account = await updateDisplayName(profileName);
      editingProfile = false;
      notice = translate("account.profileSaved");
      publishAccount(account);
    } catch (cause) {
      error = errorMessage(cause);
    } finally {
      busy = null;
    }
  }

  function publishAccount(next: AccountView | null) {
    if (next) {
      window.dispatchEvent(
        new CustomEvent<AccountView>("kora:account-changed", { detail: next }),
      );
    }
  }

  function chooseAvatar() {
    avatarInput?.click();
  }

  async function changeAvatar(event: Event) {
    const input = event.currentTarget as HTMLInputElement;
    const file = input.files?.[0];
    input.value = "";
    if (!file) return;
    if (!file.type.startsWith("image/")) {
      error = translate("account.avatarInvalid");
      return;
    }
    avatarBusy = true;
    error = null;
    try {
      const image = await resizeAvatar(file);
      account = await updateAccountAvatar(image);
      notice = translate("account.avatarSaved");
      publishAccount(account);
    } catch (cause) {
      error = errorMessage(cause);
    } finally {
      avatarBusy = false;
    }
  }

  function resizeAvatar(file: File): Promise<string> {
    return new Promise((resolve, reject) => {
      const objectUrl = URL.createObjectURL(file);
      const image = new Image();
      image.onload = () => {
        try {
          const side = Math.min(image.width, image.height);
          const canvas = document.createElement("canvas");
          canvas.width = 512;
          canvas.height = 512;
          const context = canvas.getContext("2d");
          if (!context) {
            reject(new Error(translate("account.avatarInvalid")));
            return;
          }
          context.drawImage(
            image,
            (image.width - side) / 2,
            (image.height - side) / 2,
            side,
            side,
            0,
            0,
            512,
            512,
          );
          resolve(canvas.toDataURL("image/jpeg", 0.88));
        } catch (cause) {
          reject(cause);
        } finally {
          URL.revokeObjectURL(objectUrl);
        }
      };
      image.onerror = () => {
        URL.revokeObjectURL(objectUrl);
        reject(new Error(translate("account.avatarInvalid")));
      };
      image.src = objectUrl;
    });
  }

  async function clearAvatar() {
    if (!account?.avatarPath) return;
    avatarBusy = true;
    error = null;
    try {
      account = await removeAccountAvatar();
      notice = translate("account.avatarRemoved");
      publishAccount(account);
    } catch (cause) {
      error = errorMessage(cause);
    } finally {
      avatarBusy = false;
    }
  }

  async function exportAllNotes(format: "markdown" | "json") {
    exportingNotes = true;
    error = null;
    notice = null;
    try {
      const destination = await save({
        defaultPath: `kora-notas.${format === "json" ? "json" : "md"}`,
        filters: [
          {
            name: format === "json" ? "JSON" : "Markdown",
            extensions: [format === "json" ? "json" : "md"],
          },
        ],
      });
      if (!destination) return;
      const result = await exportNotes(null, format, destination);
      notice = translate("account.notesExported", {
        count: result.entries,
        path: result.path,
      });
    } catch (cause) {
      error = errorMessage(cause);
    } finally {
      exportingNotes = false;
    }
  }

  async function updatePin() {
    busy = "pin";
    error = null;
    try {
      await changePin(oldPin, newPin);
      oldPin = "";
      newPin = "";
      notice = "PIN actualizado.";
    } catch (cause) {
      error = errorMessage(cause);
    } finally {
      busy = null;
    }
  }

  async function recoverWithNewPin() {
    if (!window.confirm(translate("account.recoverPinConfirm"))) return;
    busy = "recover";
    error = null;
    try {
      await recoverAccountPin(recoverPin);
      recoverPin = "";
      notice = translate("account.recoverPinDone");
      await load();
    } catch (cause) {
      error = errorMessage(cause);
    } finally {
      busy = null;
    }
  }

  async function removeAccount() {
    if (!window.confirm(translate("account.deleteAccountConfirm"))) return;
    busy = "delete";
    error = null;
    try {
      await deleteAccount(deletePin);
      deletePin = "";
      window.localStorage.clear();
      notice = translate("account.deleteAccountPending");
    } catch (cause) {
      error = errorMessage(cause);
    } finally {
      busy = null;
    }
  }

  async function makeShare() {
    busy = "share";
    error = null;
    try {
      const daysText = String(shareDays).trim();
      const usesText = String(shareMaxUses).trim();
      const request: ShareCreateRequest = {
        scopeType: shareScope,
        resourceId: shareScope === "book" ? shareBook : null,
        permission: sharePermission,
        expiresAt: daysText
          ? new Date(Date.now() + Number(daysText) * 86400000).toISOString()
          : null,
        maxUses: usesText ? Number(usesText) : null,
      };
      createdShare = await createShare(sharePin, request);
      sharePin = "";
      shares = await listShares();
      notice = "Share creado. La clave solo se muestra ahora.";
    } catch (cause) {
      error = errorMessage(cause);
    } finally {
      busy = null;
    }
  }

  async function showShareKey(share: ShareView) {
    const pinValue = window.prompt("PIN de la cuenta para mostrar la clave");
    if (!pinValue) return;
    try {
      createdShare = await revealShareKey(pinValue, share.share.id);
    } catch (cause) {
      error = errorMessage(cause);
    }
  }

  async function removeShare(share: ShareView) {
    if (!window.confirm("¿Revocar este share?")) return;
    try {
      await revokeShare(share.share.id);
      shares = await listShares();
    } catch (cause) {
      error = errorMessage(cause);
    }
  }

  async function redeemShare() {
    busy = "receive";
    error = null;
    try {
      const result = await acceptShare(receivePin, receiveKey.trim(), receivePeer);
      notice = `Share aceptado: ${result.ownerUsername ?? "otra cuenta"} · ${result.permission}`;
      receivePin = "";
      receiveKey = "";
      await loadReceived();
    } catch (cause) {
      error = errorMessage(cause);
    } finally {
      busy = null;
    }
  }

  async function forgetReceived(share: ReceivedShare) {
    if (!window.confirm("¿Quitar este share y sus copias de solo lectura?")) return;
    try {
      await removeReceivedShare(share.shareId);
      await loadReceived();
    } catch (cause) {
      error = errorMessage(cause);
    }
  }

  async function forgetDevice(device: DeviceInfo) {
    if (!window.confirm(`¿Revocar el dispositivo «${device.name}»?`)) return;
    try {
      await revokeDevice(device.id);
      devices = await listDevices();
      notice = "Dispositivo revocado.";
    } catch (cause) {
      error = errorMessage(cause);
    }
  }

  function formatDate(value: string | null) {
    if (!value) return "—";
    return formatI18nDate(value, {
      day: "2-digit",
      month: "short",
      year: "2-digit",
      hour: "2-digit",
      minute: "2-digit",
    });
  }

  function scopeLabel(scope: ShareScopeType, resourceType?: ResourceType) {
    if (resourceType === "audiobook") return translate("account.audiobook");
    return scope === "library"
      ? translate("account.library")
      : scope === "book"
        ? translate("account.book")
        : translate("nav.voice");
  }

  function permissionLabel(permission: SharePermission) {
    return permission === "copy"
      ? translate("account.copy")
      : translate("account.readOnly");
  }

  function peerLabel(peer: PeerInfo) {
    return `${peer.name} · ${peer.address ?? "sin dirección"}${peer.trusted ? " · emparejado" : ""}`;
  }
</script>

<section class="account-view">
  <header class="hero">
    <div>
      <p class="eyebrow">{$t("nav.account")}</p>
      <h1 class="page-title">{$t("account.title")}</h1>
      <p class="page-subtitle">{$t("account.subtitle")}</p>
    </div>
    {#if account}
      <div class="hero-badge">
        <span class="hero-badge-icon"><Icon name="account" size={17} /></span>
        <span>{$t("account.devices")} · <strong>{devices.length}</strong></span>
      </div>
    {/if}
  </header>

  {#if error}
    <p class="notice" role="alert">{error}</p>
  {/if}
  {#if notice}
    <p class="ok" role="status">{notice}</p>
  {/if}

  {#if !account}
    <section class="panel">
      <h2>{$t("account.create")}</h2>
      <p class="hint">{$t("account.createHelp")}</p>
      <div class="grid">
        <label>
          <span>{$t("account.username")}</span>
          <input bind:value={username} maxlength="32" placeholder="edgar" />
        </label>
        <label>
          <span>{$t("account.displayName")}</span>
          <input bind:value={displayName} maxlength="80" placeholder="Edgar" />
        </label>
        <label>
          <span>{$t("account.pinDigits")}</span>
          <input type="password" bind:value={pin} maxlength="12" inputmode="numeric" />
        </label>
      </div>
      <button class="primary" onclick={createLocalAccount} disabled={busy !== null}>
        {$t("account.create")}
      </button>
    </section>
  {:else}
    <nav class="tabs" aria-label={$t("account.title")}>
      <button
        class:active={activeTab === "account"}
        aria-pressed={activeTab === "account"}
        onclick={() => (activeTab = "account")}
      >
        {$t("account.title")}
      </button>
      <button
        class:active={activeTab === "share"}
        aria-pressed={activeTab === "share"}
        onclick={() => (activeTab = "share")}
      >
        {$t("sync.share")}
      </button>
      <button
        class:active={activeTab === "received"}
        aria-pressed={activeTab === "received"}
        onclick={() => (activeTab = "received")}
      >
        {$t("account.received")} <span class="count">{received.length}</span>
      </button>
    </nav>

    {#if activeTab === "account"}
      <section class="panel profile-panel">
        <div class="profile-hero">
          <input
            class="avatar-input"
            type="file"
            accept="image/png,image/jpeg,image/webp"
            bind:this={avatarInput}
            onchange={changeAvatar}
          />
          <button
            class="avatar avatar-button"
            type="button"
            onclick={chooseAvatar}
            disabled={avatarBusy}
            aria-label={$t("account.avatarChange")}
            title={$t("account.avatarChange")}
          >
            {#if account.avatarPath}
              <img src={convertFileSrc(account.avatarPath)} alt="" />
            {:else}
              {(account.displayName || account.username).slice(0, 1).toUpperCase()}
            {/if}
            <span class="avatar-camera" aria-hidden="true">
              <Icon name="upload" size={14} />
            </span>
          </button>
          <div class="profile-titles">
            {#if editingProfile}
              <div class="profile-edit">
                <input
                  bind:value={profileName}
                  maxlength="80"
                  aria-label={$t("account.displayName")}
                  onkeydown={(event) => {
                    if (event.key === "Enter") void saveProfile();
                    if (event.key === "Escape") editingProfile = false;
                  }}
                />
                <button class="primary small" onclick={saveProfile} disabled={busy !== null}>
                  {$t("common.save")}
                </button>
                <button class="ghost small" onclick={() => (editingProfile = false)}>
                  {$t("common.cancel")}
                </button>
              </div>
            {:else}
              <h2>{account.displayName}</h2>
            {/if}
            <p>
              @{account.username}
              <span aria-hidden="true">·</span>
              {$t("account.localAccount")}
            </p>
            <small class="avatar-help">{$t("account.avatarHelp")}</small>
          </div>
          <div class="profile-actions">
            {#if !editingProfile}
              <button class="ghost small profile-edit-button" onclick={() => {
                profileName = account?.displayName ?? "";
                editingProfile = true;
              }}>
                <Icon name="edit" size={14} /> {$t("account.editProfile")}
              </button>
            {/if}
            {#if account.avatarPath}
              <button
                class="ghost small"
                onclick={clearAvatar}
                disabled={avatarBusy}
              >
                <Icon name="trash" size={14} /> {$t("account.avatarRemove")}
              </button>
            {/if}
          </div>
          <span class="badge">{$t("account.localAccount")}</span>
        </div>
        <p class="hint">
          {$t("account.publicKey")}:
          <code>{account.publicKey.slice(0, 24)}…</code>
        </p>
        <h3>{$t("account.changePin")}</h3>
        <div class="grid">
          <label>
            <span>{$t("account.oldPin")}</span>
            <input type="password" bind:value={oldPin} maxlength="12" inputmode="numeric" />
          </label>
          <label>
            <span>{$t("account.newPin")}</span>
            <input type="password" bind:value={newPin} maxlength="12" inputmode="numeric" />
          </label>
        </div>
        <button class="ghost small" onclick={updatePin} disabled={busy !== null}>
          {$t("account.updatePin")}
        </button>

        <details class="recovery">
          <summary>{$t("account.recoverPin")}</summary>
          <p class="hint">{$t("account.recoverPinHelp")}</p>
          <div class="grid">
            <label>
              <span>{$t("account.newPin")}</span>
              <input
                type="password"
                bind:value={recoverPin}
                maxlength="12"
                inputmode="numeric"
              />
            </label>
          </div>
          <button
            class="danger small"
            onclick={recoverWithNewPin}
            disabled={busy !== null || recoverPin.length < 4}
          >
            {$t("account.recoverPinAction")}
          </button>
        </details>
      </section>

      <section class="panel danger-zone">
        <h2>{$t("account.dangerZone")}</h2>
        <p class="hint">{$t("account.deleteAccountHelp")}</p>
        <div class="grid">
          <label>
            <span>{$t("account.pinToConfirm")}</span>
            <input
              type="password"
              bind:value={deletePin}
              maxlength="12"
              inputmode="numeric"
            />
          </label>
        </div>
        <button
          class="danger small"
          onclick={removeAccount}
          disabled={busy !== null || deletePin.length < 4}
        >
          <Icon name="trash" size={15} /> {$t("account.deleteAccount")}
        </button>
      </section>

      <section class="panel">
        <div class="panel-head">
          <h2>{$t("account.privacy")}</h2>
          <span class="badge">{$t("app.localBadge")}</span>
        </div>
        <p class="hint">{$t("account.notesHelp")}</p>
        <div class="actions">
          <button class="primary small" onclick={() => exportAllNotes("markdown")} disabled={exportingNotes}>
            <Icon name="download" size={15} /> {$t("account.exportMarkdown")}
          </button>
          <button class="ghost small" onclick={() => exportAllNotes("json")} disabled={exportingNotes}>
            <Icon name="download" size={15} /> {$t("account.exportJson")}
          </button>
        </div>
      </section>

      <section class="panel">
        <h2>{$t("account.devices")}</h2>
        <p class="hint">{$t("account.devicesHelp")}</p>
        {#if devices.length === 0}
          <p class="empty">{$t("account.noDevices")}</p>
        {:else}
          <div class="rows">
            {#each devices as device (device.id)}
              <div class="row-item">
                <div>
                  <strong>{device.name}</strong>
                  <span class="hint">
                    {device.isLocal ? $t("account.thisDevice") : device.peerUsername ?? "peer"}
                    · {device.platform}
                    {#if device.trusted}· {$t("account.trusted")}{:else}· {$t("account.revoked")}{/if}
                  </span>
                  <span class="hint">
                    {$t("account.lastSeen")}: {formatDate(device.lastSeenAt)}
                  </span>
                </div>
                {#if !device.isLocal && device.trusted}
                  <button class="danger small" onclick={() => forgetDevice(device)}>
                    {$t("account.revoke")}
                  </button>
                {/if}
              </div>
            {/each}
          </div>
        {/if}
      </section>
    {/if}

    {#if activeTab === "share"}
      <section class="panel">
        <h2>{$t("account.createShare")}</h2>
        <div class="grid">
          <label>
            <span>{$t("account.scope")}</span>
            <select bind:value={shareScope}>
              <option value="library">{$t("account.library")}</option>
              <option value="book">{$t("account.book")}</option>
              <option value="voice" disabled>{$t("account.voice")}</option>
            </select>
          </label>
          {#if shareScope === "book"}
            <label>
              <span>{$t("account.book")}</span>
              <select bind:value={shareBook}>
                {#each books as book (book.id)}
                  <option value={book.id}>{book.title}</option>
                {/each}
              </select>
            </label>
          {/if}
          <label>
            <span>{$t("account.permission")}</span>
            <select bind:value={sharePermission}>
              <option value="copy">{$t("account.copy")}</option>
              <option value="read">{$t("account.readOnly")}</option>
            </select>
          </label>
          <label>
            <span>{$t("account.validDays")}</span>
            <input type="number" min="0" bind:value={shareDays} placeholder="7" />
          </label>
          <label>
            <span>{$t("account.maxUses")}</span>
            <input type="number" min="1" bind:value={shareMaxUses} placeholder={$t("account.unlimited")} />
          </label>
          <label>
            <span>{$t("account.hostPin")}</span>
            <input type="password" bind:value={sharePin} maxlength="12" inputmode="numeric" />
          </label>
        </div>
        <button class="primary small" onclick={makeShare} disabled={busy !== null}>
          {$t("account.createShareAction")}
        </button>

        {#if createdShare?.key}
          <div class="qr-card">
            {#if createdShare.qrSvg}<div class="qr">{@html createdShare.qrSvg}</div>{/if}
            <div>
              <p class="hint">{$t("common.shareKey")} ({$t("common.onlyShownNow")}):</p>
              <code class="big-code">{createdShare.key}</code>
              {#if createdShare.uri}<p class="hint">{$t("common.uri")}: <code>{createdShare.uri}</code></p>{/if}
            </div>
          </div>
        {/if}
      </section>

      <section class="panel">
        <h2>{$t("account.acceptShare")}</h2>
        <div class="grid">
          <label>
            <span>{$t("account.ownerPeer")}</span>
            <select bind:value={receivePeer}>
              <option value="">{$t("account.choosePeer")}</option>
              {#each peers as peer (peer.id)}
                <option value={peer.id}>{peerLabel(peer)}</option>
              {/each}
            </select>
          </label>
          <label>
            <span>{$t("account.shareKey")}</span>
            <input bind:value={receiveKey} placeholder="SHARE-BOOK-XXXX-XXXX" />
          </label>
          <label>
            <span>{$t("account.hostPin")}</span>
            <input type="password" bind:value={receivePin} maxlength="12" inputmode="numeric" />
          </label>
        </div>
        <button class="primary small" onclick={redeemShare} disabled={busy !== null}>
          {$t("account.acceptShareAction")}
        </button>
      </section>

      <section class="panel">
        <h2>{$t("account.myShares")}</h2>
        {#if shares.length === 0}
          <p class="empty">{$t("account.noShares")}</p>
        {:else}
          <div class="rows">
            {#each shares as item (item.share.id)}
              <div class="row-item">
                <div>
                  <strong>{scopeLabel(item.share.scopeType, item.share.resourceType)} · {permissionLabel(item.share.permission)}</strong>
                  <span class="hint">
                    {item.share.uses}{item.share.maxUses ? `/${item.share.maxUses}` : ""} usos
                    · expira {formatDate(item.share.expiresAt)}
                    {#if item.share.revokedAt}· revocado{/if}
                  </span>
                </div>
                <div class="actions">
                  {#if !item.share.revokedAt}
                    <button class="ghost small" onclick={() => showShareKey(item)}>
                      {$t("account.showShareKey")}
                    </button>
                    <button class="danger small" onclick={() => removeShare(item)}>{$t("account.revoke")}</button>
                  {/if}
                </div>
              </div>
            {/each}
          </div>
        {/if}
      </section>
    {/if}

    {#if activeTab === "received"}
      <section class="panel">
        <h2>{$t("account.receivedShares")}</h2>
        {#if received.length === 0}
          <p class="empty">{$t("account.noReceivedShares")}</p>
        {:else}
          <div class="rows">
            {#each received as item (item.shareId)}
              <div class="row-item">
                <div>
                  <strong>{item.ownerUsername ?? $t("account.otherAccount")} · {scopeLabel(item.scopeType, item.resourceType)}</strong>
                  <span class="hint">
                    {permissionLabel(item.permission)} · aceptado {formatDate(item.acceptedAt)}
                    · {item.uses}{item.maxUses ? `/${item.maxUses}` : ""} usos
                  </span>
                </div>
                <div class="actions">
                  {#if item.permission === "read"}
                    <button class="danger small" onclick={() => forgetReceived(item)}>{$t("account.remove")}</button>
                  {/if}
                </div>
              </div>
            {/each}
          </div>
        {/if}
      </section>
    {/if}
  {/if}
</section>

<style>
  .account-view {
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

  .hero-badge {
    display: inline-flex;
    align-items: center;
    gap: 0.55rem;
    padding: 0.42rem 0.8rem 0.42rem 0.5rem;
    border: 1px solid var(--border);
    border-radius: 999px;
    background: var(--surface);
    box-shadow: var(--shadow-xs);
    color: var(--muted);
    font-size: 0.76rem;
  }

  .hero-badge strong {
    color: var(--text);
    font-size: 0.92rem;
  }

  .hero-badge-icon {
    display: grid;
    place-items: center;
    width: 30px;
    height: 30px;
    border-radius: 50%;
    background: var(--accent-soft);
    color: var(--accent);
  }

  .tabs {
    display: inline-flex;
    flex-wrap: wrap;
    align-self: flex-start;
    gap: 0.2rem;
    max-width: 100%;
    padding: 0.25rem;
    overflow-x: auto;
    border: 1px solid var(--border);
    border-radius: 999px;
    background: var(--surface-2);
  }

  .tabs button {
    display: inline-flex;
    align-items: center;
    gap: 0.35rem;
    min-height: 36px;
    padding: 0.35rem 0.82rem;
    border: 0;
    border-radius: 999px;
    background: transparent;
    color: var(--muted);
    font: inherit;
    font-size: 0.78rem;
    font-weight: 500;
    white-space: nowrap;
  }

  .tabs button:hover {
    color: var(--text);
  }

  .tabs button.active {
    background: var(--surface);
    color: var(--text);
    font-weight: 600;
    box-shadow: var(--shadow-xs);
  }

  .count {
    display: inline-grid;
    place-items: center;
    min-width: 19px;
    height: 19px;
    padding: 0 0.28rem;
    border-radius: 999px;
    background: var(--accent-soft);
    color: var(--accent);
    font-size: 0.62rem;
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

  .recovery {
    margin-top: 0.9rem;
    padding-top: 0.8rem;
    border-top: 1px solid color-mix(in srgb, var(--border) 65%, transparent);
  }

  .recovery summary {
    color: var(--muted);
    font-size: 0.8rem;
    font-weight: 600;
    cursor: pointer;
  }

  .recovery[open] summary {
    margin-bottom: 0.55rem;
  }

  .danger-zone {
    border-color: color-mix(in srgb, var(--danger) 38%, var(--border));
    background: color-mix(in srgb, var(--danger) 4%, var(--surface));
  }

  h2 {
    margin: 0;
    font-size: 1rem;
  }

  h3 {
    margin: 0.85rem 0 0.1rem;
    padding-top: 0.8rem;
    border-top: 1px solid color-mix(in srgb, var(--border) 65%, transparent);
    font-size: 0.88rem;
  }

  .profile-panel {
    position: relative;
    gap: 0.85rem;
    overflow: hidden;
    padding: 24px;
    background:
      radial-gradient(420px 260px at 92% 0%, color-mix(in srgb, var(--gold) 12%, transparent), transparent 72%),
      linear-gradient(115deg, color-mix(in srgb, var(--accent) 6%, var(--surface-2)), var(--surface) 62%);
  }

  .profile-hero {
    display: flex;
    align-items: center;
    gap: 1.1rem;
  }

  .avatar {
    position: relative;
    display: grid;
    place-items: center;
    flex: 0 0 auto;
    width: 84px;
    height: 84px;
    overflow: hidden;
    border: 3px solid var(--surface);
    border-radius: 50%;
    background: linear-gradient(145deg, var(--accent), var(--accent-2));
    color: var(--accent-contrast);
    font-family: var(--font-display);
    font-size: 2.25rem;
    font-weight: 500;
    box-shadow:
      0 12px 26px -16px color-mix(in srgb, var(--accent) 70%, transparent),
      inset 0 1px 0 color-mix(in srgb, #fff 20%, transparent);
  }

  .avatar img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .avatar-button {
    padding: 0;
    cursor: pointer;
  }

  .avatar-button:disabled {
    cursor: wait;
    opacity: 0.7;
  }

  .avatar-input {
    display: none;
  }

  .avatar-camera {
    position: absolute;
    right: 2px;
    bottom: 2px;
    display: grid;
    place-items: center;
    width: 26px;
    height: 26px;
    border: 2px solid var(--surface);
    border-radius: 50%;
    background: var(--surface);
    color: var(--accent);
    box-shadow: var(--shadow-sm);
  }

  .avatar-help {
    color: var(--muted);
    font-size: 0.68rem;
  }

  .profile-actions {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: 0.35rem;
    margin-left: auto;
  }

  .profile-titles {
    min-width: 0;
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 0.1rem;
  }

  .profile-titles h2 {
    font-family: var(--font-display);
    font-size: 1.85rem;
    font-weight: 500;
    letter-spacing: -0.02em;
  }

  .profile-titles p {
    margin: 0;
    color: var(--muted);
    font-size: 0.78rem;
  }

  .profile-titles p span {
    padding: 0 0.2rem;
  }

  .profile-hero .badge {
    flex: 0 0 auto;
  }

  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(170px, 1fr));
    gap: 0.6rem;
  }

  label {
    display: flex;
    flex-direction: column;
    gap: 0.28rem;
    color: var(--muted);
    font-size: 0.74rem;
  }

  input,
  select {
    min-height: 40px;
    padding: 0.45rem 0.55rem;
    border: 1px solid var(--border);
    border-radius: 9px;
    background: var(--surface-2);
    color: var(--text);
    font: inherit;
    font-size: 0.83rem;
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
    font-size: 0.64rem;
    white-space: nowrap;
  }

  .qr-card {
    display: flex;
    gap: 1rem;
    align-items: center;
    flex-wrap: wrap;
    margin-top: 0.35rem;
    padding: 0.8rem;
    border: 1px solid var(--border);
    border-radius: 12px;
    background: #fff;
    color: #182026;
  }

  .qr :global(svg) {
    display: block;
    width: 176px;
    height: 176px;
  }

  .big-code {
    display: inline-block;
    margin-top: 0.2rem;
    font-size: 0.95rem;
    font-weight: 700;
    letter-spacing: 0.04em;
  }

  code {
    font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
    font-size: 0.76rem;
  }

  .rows {
    display: flex;
    flex-direction: column;
    gap: 0.45rem;
  }

  .row-item {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: 0.7rem;
    padding: 0.6rem 0.7rem;
    border: 1px solid var(--border);
    border-radius: 11px;
    background: var(--surface-2);
  }

  .row-item > div:first-child {
    display: flex;
    flex-direction: column;
    gap: 0.12rem;
    min-width: 0;
  }

  .row-item strong {
    font-size: 0.84rem;
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
    padding: 0.4rem 0.7rem;
    border: 1px solid var(--border);
    border-radius: 9px;
    background: var(--surface-2);
    color: var(--text);
    font: inherit;
    font-size: 0.8rem;
  }

  .ghost:hover {
    border-color: var(--accent-line);
    background: color-mix(in srgb, var(--accent) 6%, var(--surface-2));
  }

  .danger {
    min-height: 36px;
    padding: 0.4rem 0.7rem;
    border: 1px solid color-mix(in srgb, var(--danger) 45%, var(--border));
    border-radius: 9px;
    background: color-mix(in srgb, var(--danger) 8%, transparent);
    color: var(--danger);
    font: inherit;
    font-size: 0.8rem;
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

    .hero-badge {
      order: -1;
    }

    .tabs {
      width: 100%;
    }

    .tabs button {
      flex: 1;
      justify-content: center;
      padding-inline: 0.55rem;
    }

    .profile-hero {
      flex-wrap: wrap;
    }

    .profile-hero .badge {
      order: 3;
    }
  }

  .profile-edit {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 5px;
  }

  .profile-edit input {
    min-width: 180px;
    max-width: 260px;
    min-height: 36px;
    padding: 0.35rem 0.5rem;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--surface);
    color: var(--text);
    font: inherit;
    font-size: 1.05rem;
  }

  .profile-edit-button {
    margin-left: 0;
  }
</style>
