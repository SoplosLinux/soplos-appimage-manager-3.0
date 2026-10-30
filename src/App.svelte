<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { _, locale } from "svelte-i18n";
  import WindowControls from "./components/WindowControls.svelte";
  import { frameless, maximized } from "./lib/window";
  import { getVersion } from "@tauri-apps/api/app";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { getCurrentWebview } from "@tauri-apps/api/webview";
  import { api } from "./lib/api";
  import { theme, toggleTheme } from "./lib/theme";
  import { SUPPORTED_LOCALES, LOCALE_NAMES, setLocale, type SupportedLocale } from "./i18n";
  import Modal from "./components/Modal.svelte";
  import AboutDialog from "./components/AboutDialog.svelte";
  import AppImageRow from "./components/AppImageRow.svelte";
  import PropertiesModal from "./components/PropertiesModal.svelte";
  import UpdateModal from "./components/UpdateModal.svelte";
  import CheckAllModal from "./components/CheckAllModal.svelte";
  import type { AppImage, AppImageInfoInput } from "./lib/appimages";
  import { Plus, RefreshCw, Moon, Sun, Package, Loader2 } from "lucide-svelte";

  let appimages: AppImage[] = [];
  let loading = true;
  let aboutOpen = false;

  let propertiesTarget: AppImage | null = null;
  let updateTarget: AppImage | null = null;
  let checkAllOpen = false;
  let deleteTarget: AppImage | null = null;
  let moveOrCopyTarget: { path: string; name: string; resolve: (copy: boolean | null) => void } | null = null;
  // Extracting metadata genuinely takes a few seconds (and the backend now
  // bounds it to at most 2 minutes) — this drives a visible "please wait"
  // overlay so that normal wait never reads as the app having frozen.
  let addingMode: "add" | "integrate" | null = null;
  let errorMessage: string | null = null;

  let envInfo = { desktopEnvironment: "unknown", displayProtocol: "unknown" };
  let appVersion = "";
  // "3.0.0-1-beta" → "3.0.0-beta": the footer shows the app version, not the
  // Debian package revision.
  $: cleanVersion = appVersion.replace(/-\d+-/, "-");
  let unlistenChanged: UnlistenFn | null = null;
  let unlistenDragDrop: UnlistenFn | null = null;

  async function loadAppImages() {
    try {
      appimages = await api.invoke<AppImage[]>("list_installed");
    } catch (error) {
      console.error("Failed to load AppImages", error);
    }
  }

  function cleanPath(p: string): string {
    return decodeURIComponent(p.trim().replace(/^file:\/\/(localhost)?/, ''));
  }

  onMount(async () => {
    try {
      envInfo = await api.invoke<{ desktopEnvironment: string; displayProtocol: string }>("get_environment_info");
    } catch (error) {
      console.error("Failed to read environment info", error);
    }
    try {
      appVersion = await getVersion();
    } catch {
      appVersion = "";
    }
    await loadAppImages();
    loading = false;

    unlistenChanged = await listen("appimages-changed", () => {
      loadAppImages();
    });

    // Tauri's native window layer intercepts OS file drops before the
    // webview ever sees them whenever dragDropEnabled is true (the
    // default) — confirmed in Tauri's own docs. That makes DOM
    // dragenter/drop + dataTransfer and this native listener mutually
    // exclusive, never simultaneous fallbacks: only one of them can ever
    // actually fire, so this is the only mechanism in the app.
    unlistenDragDrop = await getCurrentWebview().onDragDropEvent((event) => {
      if (event.payload.type === "enter" || event.payload.type === "over") {
        dragDepth = 1;
      } else if (event.payload.type === "leave") {
        dragDepth = 0;
      } else if (event.payload.type === "drop") {
        dragDepth = 0;
        if (propertiesTarget || updateTarget || checkAllOpen || deleteTarget || moveOrCopyTarget || aboutOpen || addingMode || errorMessage) return;
        const raw = event.payload.paths.find((p) => cleanPath(p).toLowerCase().endsWith(".appimage"));
        if (raw) addFromPath(cleanPath(raw));
      }
    });
  });

  onDestroy(() => {
    unlistenChanged?.();
    unlistenDragDrop?.();
  });

  async function addFromPath(path: string) {
    const copy = await new Promise<boolean | null>((resolve) => {
      moveOrCopyTarget = { path, name: path.split("/").pop() ?? path, resolve };
    });
    if (copy === null) return;
    addingMode = "add";
    try {
      await api.invoke("add_appimage", { sourcePath: path, copy });
      await loadAppImages();
    } catch (error) {
      console.error("Failed to add AppImage", error);
      errorMessage = error === "ALREADY_INSTALLED" ? $_("appimage.alreadyInstalled") : $_("appimage.addFailed");
    } finally {
      addingMode = null;
    }
  }

  async function onAddClicked() {
    const path = await api.invoke<string | null>("pick_appimage_file");
    if (path) await addFromPath(path);
  }

  async function onIntegrate(a: AppImage) {
    addingMode = "integrate";
    try {
      await api.invoke("integrate_existing", { filePath: a.filePath });
      await loadAppImages();
    } catch (error) {
      console.error("Failed to integrate AppImage", error);
    } finally {
      addingMode = null;
    }
  }

  async function onRun(a: AppImage) {
    try {
      await api.invoke("run_appimage", { filePath: a.filePath, desktopFilePath: a.desktopFilePath });
    } catch (error) {
      console.error("Failed to run AppImage", error);
    }
  }

  function askDelete(a: AppImage) {
    deleteTarget = a;
  }

  async function confirmDelete() {
    if (!deleteTarget) return;
    await api.invoke("delete_appimage", { filePath: deleteTarget.filePath, iconPath: deleteTarget.iconPath, desktopFilePath: deleteTarget.desktopFilePath });
    deleteTarget = null;
    await loadAppImages();
  }

  async function saveProperties(input: AppImageInfoInput) {
    if (!propertiesTarget) return;
    await api.invoke("update_appimage_info", {
      filePath: propertiesTarget.filePath,
      desktopFilePath: propertiesTarget.desktopFilePath,
      iconPath: propertiesTarget.iconPath,
      input,
    });
    propertiesTarget = null;
    await loadAppImages();
  }

  function onLanguageChange(e: Event) {
    setLocale((e.currentTarget as HTMLSelectElement).value as SupportedLocale);
  }

  // === Global keyboard shortcuts ===
  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "F1") {
      e.preventDefault();
      aboutOpen = true;
      return;
    }
    if (e.ctrlKey && (e.key === "q" || e.key === "Q")) {
      e.preventDefault();
      getCurrentWindow().close();
      return;
    }
    if (propertiesTarget || updateTarget || checkAllOpen || deleteTarget || moveOrCopyTarget || aboutOpen || addingMode || errorMessage) return;

    if (e.ctrlKey && (e.key === "n" || e.key === "N")) {
      e.preventDefault();
      onAddClicked();
    }
  }

  // Drag-and-drop visual state, driven by the native onDragDropEvent
  // listener wired in onMount above.
  let dragDepth = 0;
  $: isDraggingOver = dragDepth > 0;
</script>

<svelte:window on:keydown={handleKeydown} />

<div class="h-screen bg-dark {$frameless && !$maximized ? 'window-card' : ''}">
<div class="relative flex flex-col h-full overflow-hidden">
  {#if isDraggingOver}
    <div class="absolute inset-0 z-40 border-2 border-dashed border-primary bg-primary/10 flex items-center justify-center pointer-events-none">
      <p class="text-[0.95rem] font-semibold text-primary">{$_("appimage.dropHint")}</p>
    </div>
  {/if}

  {#if $frameless}
    <div class="relative h-7 shrink-0" data-tauri-drag-region>
      <div class="absolute right-2 top-[3px]"><WindowControls /></div>
    </div>
  {/if}

  <div class="p-4 pb-0 shrink-0 {$frameless ? 'pt-0' : ''}" data-tauri-drag-region="deep">
    <div
      class="relative rounded border border-border overflow-hidden flex flex-col items-start justify-end p-4 min-h-[110px]"
      style="background: linear-gradient(rgba(20, 20, 22, 0.25), rgba(20, 20, 22, 0.82)), url('/assets/hero/appimage-manager-background.jpg'); background-size: cover; background-position: center;"
    >
      <div class="absolute top-3 right-3">
        <select
          value={$locale}
          on:change={onLanguageChange}
          class="appearance-none bg-primary/20 backdrop-blur-sm border border-primary/30 text-primary rounded px-2 py-1 text-[0.7rem] focus:outline-none"
          title={$_("sidebar.language")}
        >
          {#each SUPPORTED_LOCALES as code}
            <option value={code}>{LOCALE_NAMES[code]}</option>
          {/each}
        </select>
      </div>

      <div class="absolute bottom-3 right-3 flex items-center gap-1.5">
        <button
          class="p-1.5 rounded bg-primary/20 backdrop-blur-sm border border-primary/30 text-primary hover:bg-primary/30"
          title={$_("appimage.addTooltip")}
          on:click={onAddClicked}
        >
          <Plus size={14} />
        </button>
        <button
          class="p-1.5 rounded bg-primary/20 backdrop-blur-sm border border-primary/30 text-primary hover:bg-primary/30"
          title={$_("appimage.checkAllTooltip")}
          on:click={() => (checkAllOpen = true)}
        >
          <RefreshCw size={14} />
        </button>
      </div>
      <p class="text-white font-semibold text-[0.95rem]">{$_("hero.title")}</p>
      <p class="text-[0.75rem] text-white/85 mt-0.5">{$_("hero.description")}</p>
    </div>
  </div>

  <div class="flex-1 overflow-y-auto content-scroll p-4">
    {#if !loading && appimages.length === 0}
      <div class="h-full flex flex-col items-center justify-center gap-2 text-center">
        <Package size={64} class="text-text-muted" />
        <p class="text-[0.9rem] text-text-bright">{$_("appimage.emptyTitle")}</p>
        <p class="text-[0.8rem] text-text-muted">{$_("appimage.emptyHint")}</p>
      </div>
    {:else}
      <div class="flex flex-col gap-2">
        {#each appimages as a (a.id + a.filePath)}
          <AppImageRow appimage={a} onRun={onRun} onProperties={(x) => (propertiesTarget = x)} onCheckUpdate={(x) => (updateTarget = x)} onIntegrate={onIntegrate} onDelete={askDelete} />
        {/each}
      </div>
    {/if}
  </div>

  <footer class="flex items-center justify-between px-4 py-1.5 border-t border-border text-[0.7rem] text-text-muted shrink-0">
    <div class="flex items-center gap-1.5">
      <button
        class="p-1 -m-1 rounded text-text-muted hover:bg-surface hover:text-text-bright"
        on:click={toggleTheme}
        title={$theme === "dark" ? $_("sidebar.switchToLight") : $_("sidebar.switchToDark")}
      >
        <svelte:component this={$theme === "dark" ? Moon : Sun} size={13} />
      </button>
      <span>
        {$_(`environment.${envInfo.desktopEnvironment}`)} · {$_(`environment.${envInfo.displayProtocol}`)}
      </span>
    </div>
    <button
      class="text-text-muted hover:text-text-bright hover:underline"
      on:click={() => (aboutOpen = true)}
      title={$_("sidebar.aboutTooltip")}
    >
      {cleanVersion}
    </button>
  </footer>
</div>

{#if propertiesTarget}
  <PropertiesModal appimage={propertiesTarget} onSave={saveProperties} onClose={() => (propertiesTarget = null)} />
{/if}

{#if updateTarget}
  <UpdateModal
    appimage={updateTarget}
    onClose={(updated) => {
      updateTarget = null;
      if (updated) loadAppImages();
    }}
  />
{/if}

{#if checkAllOpen}
  <CheckAllModal
    appimages={appimages.filter((a) => a.desktopFilePath !== null)}
    onClose={(updated) => {
      checkAllOpen = false;
      if (updated) loadAppImages();
    }}
  />
{/if}

{#if deleteTarget}
  <Modal
    variant="danger"
    title={$_("appimage.deleteConfirmTitle", { values: { name: deleteTarget.name } })}
    confirmLabel={$_("appimage.delete")}
    onClose={() => (deleteTarget = null)}
    onConfirm={confirmDelete}
  >
    <p>{$_("appimage.deleteConfirmBody")}</p>
  </Modal>
{/if}

{#if moveOrCopyTarget}
  <Modal
    variant="confirm"
    title={$_("appimage.moveOrCopyTitle", { values: { name: moveOrCopyTarget.name } })}
    showFooter={false}
    onClose={() => {
      moveOrCopyTarget?.resolve(null);
      moveOrCopyTarget = null;
    }}
  >
    <p class="text-text-muted mb-3">{$_("appimage.moveOrCopyBody")}</p>
    <div class="flex flex-col gap-2">
      <button
        class="px-3.5 py-3 rounded-lg border border-border hover:border-primary hover:bg-primary/5 text-left font-semibold text-text-bright text-[0.85rem]"
        on:click={() => {
          moveOrCopyTarget?.resolve(false);
          moveOrCopyTarget = null;
        }}
      >
        {$_("appimage.moveOrCopyMove")}
      </button>
      <button
        class="px-3.5 py-3 rounded-lg border border-border hover:border-primary hover:bg-primary/5 text-left font-semibold text-text-bright text-[0.85rem]"
        on:click={() => {
          moveOrCopyTarget?.resolve(true);
          moveOrCopyTarget = null;
        }}
      >
        {$_("appimage.moveOrCopyCopy")}
      </button>
    </div>
  </Modal>
{/if}

{#if aboutOpen}
  <AboutDialog onClose={() => (aboutOpen = false)} />
{/if}

{#if errorMessage}
  <Modal variant="info" title={$_("appimage.errorTitle")} onClose={() => (errorMessage = null)}>
    <p>{errorMessage}</p>
  </Modal>
{/if}

{#if addingMode}
  <div class="fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-4" role="presentation">
    <div class="w-full max-w-[360px] bg-surface border border-border rounded-xl p-6 flex flex-col items-center gap-3 shadow-2xl text-center">
      <Loader2 size={28} class="animate-spin text-primary" />
      <p class="text-text-bright font-semibold text-[0.9rem]">
        {addingMode === "add" ? $_("appimage.addingTitle") : $_("appimage.integratingTitle")}
      </p>
      <p class="text-[0.78rem] text-text-muted">
        {addingMode === "add" ? $_("appimage.addingHint") : $_("appimage.integratingHint")}
      </p>
    </div>
  </div>
{/if}
</div>
