<script lang="ts">
  import { onMount } from "svelte";
  import { _ } from "svelte-i18n";
  import { Loader2, CheckCircle2, AlertTriangle, CircleHelp } from "lucide-svelte";
  import { api } from "../lib/api";
  import Modal from "./Modal.svelte";
  import type { AppImage, UpdateCheckResult, UpdateMode } from "../lib/appimages";

  export let appimage: AppImage;
  export let onClose: (updated: boolean) => void;

  let mode: UpdateMode = "none";
  let checking = true;
  let result: UpdateCheckResult | null = null;
  let toolMessage = "";
  let updating = false;
  let done = false;
  let failed = false;
  let didUpdate = false;

  function fileStem(path: string): string {
    const base = path.split("/").pop() ?? path;
    return base.replace(/\.appimage$/i, "");
  }

  onMount(async () => {
    mode = await api.invoke<UpdateMode>("get_update_mode", { updateUrl: appimage.updateUrl });
    if (mode === "none") {
      checking = false;
      return;
    }
    if (mode === "api") {
      result = await api.invoke<UpdateCheckResult>("check_for_update_api", {
        updateUrl: appimage.updateUrl,
        currentVersion: appimage.version,
        currentFileStem: fileStem(appimage.filePath),
      });
    } else if (mode === "direct") {
      result = await api.invoke<UpdateCheckResult>("check_for_update_direct", {
        updateUrl: appimage.updateUrl,
        currentFilePath: appimage.filePath,
      });
    } else {
      const toolResult = await api.invoke<UpdateCheckResult>("check_for_update_tool", { filePath: appimage.filePath });
      result = toolResult;
      toolMessage = toolResult.message;
    }
    checking = false;
  });

  async function runUpdate() {
    updating = true;
    try {
      if (mode === "api" || mode === "direct") {
        const newPath = await api.invoke<string>("download_update", {
          downloadUrl: result?.downloadUrl ?? "",
          assetName: result?.assetName ?? "",
          oldFilePath: appimage.filePath,
        });
        await api.invoke("update_appimage_info", {
          filePath: newPath,
          desktopFilePath: appimage.desktopFilePath,
          iconPath: appimage.iconPath,
          input: {
            name: appimage.name,
            version: result?.latestVersion || appimage.version,
            comment: appimage.comment,
            categories: appimage.categories,
            updateUrl: appimage.updateUrl,
            runAsRoot: appimage.runAsRoot,
          },
        });
      } else {
        const success = await api.invoke<boolean>("run_update_tool", { filePath: appimage.filePath });
        if (!success) throw new Error("appimageupdatetool exited with an error");
      }
      done = true;
      didUpdate = true;
    } catch (error) {
      failed = true;
      toolMessage = String(error);
    } finally {
      updating = false;
    }
  }
</script>

<Modal
  variant={done ? "info" : "confirm"}
  maxWidth={440}
  title={appimage.name}
  confirmLabel={$_("appimage.update_.updateNow")}
  cancelLabel={$_("common.close")}
  confirmDisabled={checking || updating || done || result?.hasUpdate !== true}
  showFooter={!done}
  onClose={() => onClose(didUpdate)}
  onConfirm={runUpdate}
>
  <div class="flex items-start gap-2.5">
    {#if checking || updating}
      <Loader2 size={16} class="animate-spin text-primary shrink-0 mt-0.5" />
    {:else if failed}
      <AlertTriangle size={16} class="text-danger shrink-0 mt-0.5" />
    {:else if done || result?.hasUpdate === false}
      <CheckCircle2 size={16} class="text-success shrink-0 mt-0.5" />
    {:else if result?.hasUpdate === true}
      <CheckCircle2 size={16} class="text-success shrink-0 mt-0.5" />
    {:else}
      <CircleHelp size={16} class="text-warning shrink-0 mt-0.5" />
    {/if}

    <div class="min-w-0 flex-1">
      {#if checking}
        <p class="text-text-bright">{$_("appimage.update_.checking")}</p>
      {:else if mode === "none"}
        <p class="text-text-bright font-semibold">{$_("appimage.update_.noSource")}</p>
        <p class="text-[0.78rem] text-text-muted mt-1">{$_("appimage.update_.noSourceHint")}</p>
      {:else if updating}
        <p class="text-text-bright">{$_("appimage.update_.downloading")}</p>
      {:else if done}
        <p class="text-text-bright font-semibold">{$_("appimage.update_.completed")}</p>
      {:else if failed}
        <p class="text-text-bright font-semibold">{$_("appimage.update_.failed", { values: { message: toolMessage } })}</p>
      {:else if result?.hasUpdate === true}
        <p class="text-text-bright font-semibold">{$_("appimage.update_.available")}</p>
        <p class="text-[0.78rem] text-text-muted mt-1">{result.message}</p>
        {#if result.assetName}
          <p class="text-[0.72rem] text-text-muted mt-0.5">{result.assetName} ({(result.assetSize / (1024 * 1024)).toFixed(1)} MB)</p>
        {/if}
      {:else if result?.hasUpdate === false}
        <p class="text-text-bright">{$_("appimage.update_.upToDate")}</p>
        {#if toolMessage}<p class="text-[0.72rem] text-text-muted mt-1">{toolMessage}</p>{/if}
      {:else}
        <p class="text-text-bright">{$_("appimage.update_.checkError", { values: { message: result?.message ?? "" } })}</p>
      {/if}
    </div>
  </div>
</Modal>
