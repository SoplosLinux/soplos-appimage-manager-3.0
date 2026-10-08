<script lang="ts">
  import { _ } from "svelte-i18n";
  import { Play, Settings2, RefreshCw, Trash2, Package } from "lucide-svelte";
  import type { AppImage } from "../lib/appimages";

  export let appimage: AppImage;
  export let onRun: (a: AppImage) => void;
  export let onProperties: (a: AppImage) => void;
  export let onCheckUpdate: (a: AppImage) => void;
  export let onIntegrate: (a: AppImage) => void;
  export let onDelete: (a: AppImage) => void;

  $: integrated = appimage.desktopFilePath !== null;
  $: sizeMb = (appimage.size / (1024 * 1024)).toFixed(1);
</script>

<div class="border border-border rounded p-3 flex items-center gap-3">
  <div class="w-11 h-11 rounded-lg bg-surface-2 grid place-items-center shrink-0 overflow-hidden">
    {#if appimage.iconPreview}
      <img src={appimage.iconPreview} alt="" class="w-full h-full object-contain" />
    {:else}
      <Package size={20} class="text-text-muted" />
    {/if}
  </div>
  <div class="min-w-0 flex-1">
    <div class="text-[0.88rem] font-semibold text-text-bright truncate">{appimage.name}</div>
    <div class="text-[0.7rem] text-text-muted truncate">{appimage.filePath}</div>
    <div class="text-[0.68rem] text-text-muted mt-0.5">
      {$_("appimage.typeValue", { values: { type: appimage.appimageType } })}
      {"  ·  "}{sizeMb} MB
      {#if appimage.version}
        {"  ·  v" + appimage.version}
      {/if}
    </div>
  </div>
  <div class="flex items-center gap-1 shrink-0">
    {#if integrated}
      <button
        class="p-1.5 rounded text-text-muted hover:bg-surface-2 hover:text-text-bright"
        title={$_("appimage.properties")}
        on:click={() => onProperties(appimage)}
      >
        <Settings2 size={15} />
      </button>
      <button
        class="p-1.5 rounded text-text-muted hover:bg-surface-2 hover:text-text-bright"
        title={$_("appimage.checkUpdate")}
        on:click={() => onCheckUpdate(appimage)}
      >
        <RefreshCw size={15} />
      </button>
      <button
        class="p-1.5 rounded text-text-muted hover:bg-surface-2 hover:text-primary"
        title={$_("appimage.run")}
        on:click={() => onRun(appimage)}
      >
        <Play size={15} />
      </button>
    {:else}
      <button
        class="px-2.5 py-1.5 rounded bg-primary hover:bg-primary-hover text-text-bright text-[0.72rem] font-semibold"
        on:click={() => onIntegrate(appimage)}
      >
        {$_("appimage.integrate")}
      </button>
    {/if}
    <button
      class="p-1.5 rounded text-text-muted hover:bg-surface-2 hover:text-danger"
      title={$_("appimage.delete")}
      on:click={() => onDelete(appimage)}
    >
      <Trash2 size={15} />
    </button>
  </div>
</div>
