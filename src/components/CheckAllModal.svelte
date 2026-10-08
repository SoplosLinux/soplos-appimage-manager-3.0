<script lang="ts">
  import { onMount } from "svelte";
  import { _ } from "svelte-i18n";
  import { Loader2, CheckCircle2, AlertTriangle } from "lucide-svelte";
  import { api } from "../lib/api";
  import Modal from "./Modal.svelte";
  import type { AppImage, UpdateCheckResult, UpdateMode } from "../lib/appimages";

  export let appimages: AppImage[];
  export let onClose: (updated: boolean) => void;

  interface Row {
    appimage: AppImage;
    mode: UpdateMode;
    status: "checking" | "available" | "upToDate" | "error" | "updating" | "updated" | "failed";
    result: UpdateCheckResult | null;
  }

  let rows: Row[] = [];
  let checkable: { appimage: AppImage; mode: UpdateMode }[] = [];
  let allChecked = false;
  let updatingAll = false;
  let didUpdate = false;

  function fileStem(path: string): string {
    const base = path.split("/").pop() ?? path;
    return base.replace(/\.appimage$/i, "");
  }

  $: availableCount = rows.filter((r) => r.status === "available").length;

  onMount(async () => {
    for (const appimage of appimages) {
      const mode = await api.invoke<UpdateMode>("get_update_mode", { updateUrl: appimage.updateUrl });
      if (mode !== "none") checkable.push({ appimage, mode });
    }
    rows = checkable.map(({ appimage, mode }) => ({ appimage, mode, status: "checking", result: null }));

    for (const row of rows) {
      let result: UpdateCheckResult;
      if (row.mode === "api") {
        result = await api.invoke<UpdateCheckResult>("check_for_update_api", {
          updateUrl: row.appimage.updateUrl,
          currentVersion: row.appimage.version,
          currentFileStem: fileStem(row.appimage.filePath),
        });
      } else if (row.mode === "direct") {
        result = await api.invoke<UpdateCheckResult>("check_for_update_direct", { updateUrl: row.appimage.updateUrl, currentFilePath: row.appimage.filePath });
      } else {
        result = await api.invoke<UpdateCheckResult>("check_for_update_tool", { filePath: row.appimage.filePath });
      }
      row.result = result;
      row.status = result.hasUpdate === true ? "available" : result.hasUpdate === false ? "upToDate" : "error";
      rows = rows;
    }
    allChecked = true;
  });

  async function updateRow(row: Row) {
    row.status = "updating";
    rows = rows;
    try {
      if (row.mode === "api" || row.mode === "direct") {
        const newPath = await api.invoke<string>("download_update", {
          downloadUrl: row.result?.downloadUrl ?? "",
          assetName: row.result?.assetName ?? "",
          oldFilePath: row.appimage.filePath,
        });
        await api.invoke("update_appimage_info", {
          filePath: newPath,
          desktopFilePath: row.appimage.desktopFilePath,
          iconPath: row.appimage.iconPath,
          input: {
            name: row.appimage.name,
            version: row.result?.latestVersion || row.appimage.version,
            comment: row.appimage.comment,
            categories: row.appimage.categories,
            updateUrl: row.appimage.updateUrl,
            runAsRoot: row.appimage.runAsRoot,
          },
        });
      } else {
        const success = await api.invoke<boolean>("run_update_tool", { filePath: row.appimage.filePath });
        if (!success) throw new Error("failed");
      }
      row.status = "updated";
      didUpdate = true;
    } catch {
      row.status = "failed";
    }
    rows = rows;
  }

  async function updateAll() {
    updatingAll = true;
    for (const row of rows) {
      if (row.status === "available") await updateRow(row);
    }
    updatingAll = false;
  }
</script>

<Modal
  variant="confirm"
  maxWidth={500}
  title={$_("appimage.checkAll_.title")}
  confirmLabel={updatingAll ? $_("appimage.checkAll_.updating") : $_("appimage.checkAll_.updateAll")}
  cancelLabel={$_("common.close")}
  confirmDisabled={!allChecked || availableCount === 0 || updatingAll}
  onClose={() => onClose(didUpdate)}
  onConfirm={updateAll}
>
  {#if checkable.length === 0 && allChecked}
    <p class="text-text-bright">{$_("appimage.checkAll_.noneConfigured")}</p>
    <p class="text-[0.78rem] text-text-muted mt-1">{$_("appimage.checkAll_.noneConfiguredHint")}</p>
  {:else}
    <p class="text-[0.8rem] text-text-muted mb-3">
      {#if !allChecked}
        {$_("appimage.checkAll_.checking", { values: { n: checkable.length } })}
      {:else if availableCount > 0}
        {$_("appimage.checkAll_.summary", { values: { count: availableCount, total: rows.length } })}
      {:else}
        {$_("appimage.checkAll_.upToDateAll")}
      {/if}
    </p>
    <div class="flex flex-col gap-2 max-h-[280px] overflow-y-auto content-scroll">
      {#each rows as row (row.appimage.id)}
        <div class="flex items-center gap-2.5 px-2.5 py-2 rounded border border-border">
          {#if row.appimage.iconPreview}
            <img src={row.appimage.iconPreview} alt="" class="w-6 h-6 object-contain shrink-0" />
          {/if}
          <span class="text-[0.82rem] text-text-bright flex-1 truncate">{row.appimage.name}</span>
          {#if row.status === "checking" || row.status === "updating"}
            <Loader2 size={14} class="animate-spin text-primary shrink-0" />
          {:else if row.status === "available"}
            <span class="text-[0.72rem] text-success">{$_("appimage.update_.available")}</span>
          {:else if row.status === "upToDate"}
            <span class="text-[0.72rem] text-text-muted">{$_("appimage.update_.upToDate")}</span>
          {:else if row.status === "updated"}
            <CheckCircle2 size={14} class="text-success shrink-0" />
          {:else}
            <AlertTriangle size={14} class="text-danger shrink-0" />
          {/if}
        </div>
      {/each}
    </div>
  {/if}
</Modal>
