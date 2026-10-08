<script lang="ts">
  import { _ } from "svelte-i18n";
  import Modal from "./Modal.svelte";
  import type { AppImage, AppImageInfoInput } from "../lib/appimages";

  export let appimage: AppImage;
  export let onSave: (input: AppImageInfoInput) => void;
  export let onClose: () => void;

  let name = appimage.name;
  let version = appimage.version;
  let comment = appimage.comment;
  let categories = appimage.categories || "Utility;";
  let updateUrl = appimage.updateUrl;
  let runAsRoot = appimage.runAsRoot;

  $: sizeMb = (appimage.size / (1024 * 1024)).toFixed(1);

  function submit() {
    onSave({
      name: name.trim() || appimage.name,
      version: version.trim(),
      comment: comment.trim(),
      categories: categories.trim() || "Utility;",
      updateUrl: updateUrl.trim(),
      runAsRoot,
    });
  }
</script>

<Modal
  variant="confirm"
  maxWidth={460}
  title={$_("appimage.properties_.title")}
  confirmLabel={$_("common.save")}
  {onClose}
  onConfirm={submit}
>
  <div class="flex flex-col gap-3.5">
    <p class="text-[0.72rem] text-text-muted">
      {appimage.filePath}<br />
      {sizeMb} MB · {$_("appimage.typeValue", { values: { type: appimage.appimageType } })}
    </p>

    <label class="flex flex-col gap-1">
      <span class="text-[0.78rem] text-text-muted">{$_("appimage.properties_.name")}</span>
      <input
        type="text"
        bind:value={name}
        class="px-3 py-1.5 rounded bg-surface-2 border border-border text-text-bright text-[0.85rem] focus:outline-none focus:border-primary"
      />
    </label>

    <label class="flex flex-col gap-1">
      <span class="text-[0.78rem] text-text-muted">{$_("appimage.properties_.version")}</span>
      <input
        type="text"
        bind:value={version}
        placeholder={$_("appimage.properties_.versionPlaceholder")}
        class="px-3 py-1.5 rounded bg-surface-2 border border-border text-text-bright text-[0.85rem] focus:outline-none focus:border-primary"
      />
    </label>

    <label class="flex flex-col gap-1">
      <span class="text-[0.78rem] text-text-muted">{$_("appimage.properties_.description")}</span>
      <input
        type="text"
        bind:value={comment}
        placeholder={$_("appimage.properties_.descriptionPlaceholder")}
        class="px-3 py-1.5 rounded bg-surface-2 border border-border text-text-bright text-[0.85rem] focus:outline-none focus:border-primary"
      />
    </label>

    <label class="flex flex-col gap-1">
      <span class="text-[0.78rem] text-text-muted">{$_("appimage.properties_.categories")}</span>
      <input
        type="text"
        bind:value={categories}
        placeholder="Utility;"
        class="px-3 py-1.5 rounded bg-surface-2 border border-border text-text-bright text-[0.85rem] focus:outline-none focus:border-primary"
      />
    </label>

    <label class="flex items-start gap-2.5 px-3 py-2.5 rounded-lg border border-border cursor-pointer hover:border-primary/50">
      <input type="checkbox" class="mt-1" bind:checked={runAsRoot} />
      <span>
        <span class="block text-[0.85rem] font-semibold text-text-bright">{$_("appimage.properties_.runAsRootTitle")}</span>
        <span class="block text-[0.75rem] text-text-muted">{$_("appimage.properties_.runAsRootHint")}</span>
      </span>
    </label>

    <label class="flex flex-col gap-1">
      <span class="text-[0.78rem] font-semibold text-text-bright">{$_("appimage.properties_.updateUrlTitle")}</span>
      <span class="text-[0.72rem] text-text-muted">{$_("appimage.properties_.updateUrlHint")}</span>
      <input
        type="text"
        bind:value={updateUrl}
        placeholder={$_("appimage.properties_.updateUrlPlaceholder")}
        class="mt-1 px-3 py-1.5 rounded bg-surface-2 border border-border text-text-bright text-[0.85rem] focus:outline-none focus:border-primary"
      />
    </label>
  </div>
</Modal>
