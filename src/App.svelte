<script lang="ts">
  import {t} from './lib/i18n';
  import { onMount, onDestroy } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow';
  import { open } from '@tauri-apps/plugin-dialog';
  import {
    ImagePlus, Plus, Download, FolderOpen, ArrowLeftRight, X, Check,
    LoaderCircle, AlertCircle, CheckCircle2, Bookmark, Trash2, RotateCcw,
    Image as ImageIcon, Type, ChevronDown, FileImage, List, Square, Pipette, WandSparkles, SlidersHorizontal, Languages,
  } from '@lucide/svelte';
  import ModelPanel from './lib/ModelPanel.svelte';
  import ModelsConfig from './lib/ModelsConfig.svelte';
  import PatternSwatch from './lib/PatternSwatch.svelte';
  import PhotoViewport from './lib/PhotoViewport.svelte';
  import FontPicker from './lib/FontPicker.svelte';
  import WatermarkDetail from './lib/WatermarkDetail.svelte';
  import FrameTemplates from './lib/FrameTemplates.svelte';
  import {language} from './lib/i18n';
  import {startupDeadline} from './lib/startup';
  import {ensureWatermarkWorkspace} from './lib/window-layout';
  import type {OffsetLimits,AiConfig} from './lib/types';
  import {
    defaultSettings,
    type AppSnapshot, type BatchSnapshot, type ImportResult, type Photo,
    type Preset, type PreviewResult, type RenderSettings, type WatermarkKind,
    type WatermarkPosition, type WatermarkRegion, type FontOption, type FrameSuggestion, type FrameDecoration, type ArtScene, type FrameTemplate,
  } from './lib/types';

  const desktop = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
  const clone = <T,>(value: T): T => JSON.parse(JSON.stringify(value));
  const errorMessage = (error: unknown) => typeof error === 'string' ? error : error instanceof Error ? error.message : $t("操作未完成，请重试");
  const basename = (path: string) => path.split(/[\\/]/).filter(Boolean).pop() || path;
  const ratioPresets = ['1:1', '4:5', '5:4', '3:2', '2:3', '16:9', '9:16'];
  const positions: { value: WatermarkPosition; label: string }[] = [
    { value: 'top-left', label: '左上' }, { value: 'top-center', label: '上方居中' }, { value: 'top-right', label: '右上' },
    { value: 'center-left', label: '左侧居中' }, { value: 'center', label: '正中' }, { value: 'center-right', label: '右侧居中' },
    { value: 'bottom-left', label: '左下' }, { value: 'bottom-center', label: '下方居中' }, { value: 'bottom-right', label: '右下' },
  ];
  const jobLabels = { pending: '等待中', running: '处理中', completed: '已导出', failed: '失败', cancelled: '已取消' };

  let settings: RenderSettings = clone(defaultSettings);
  let photos: Photo[] = [];
  let presets: Preset[] = [];
  let frameTemplates:FrameTemplate[]=[],templatesOpen=false,templateDraft:RenderSettings|null=null,templateName='';
  function openTemplates(draft:RenderSettings|null=null,name=''){templateDraft=draft?clone(draft):null;templateName=name;templatesOpen=true;}
  async function saveFrameTemplate(name:string,style:RenderSettings){const saved=await invoke<FrameTemplate>('save_frame_template',{name,settings:style});frameTemplates=[...frameTemplates,saved];}
  async function deleteFrameTemplate(id:string){await invoke('delete_frame_template',{id});frameTemplates=frameTemplates.filter(t=>t.id!==id);}
  function applyFrameTemplate(style:RenderSettings){previousArt={ratio:clone(settings.ratio),frameColor:settings.frameColor,borderPercent:settings.borderPercent,decoration:clone(settings.decoration),artwork:settings.artwork?clone(settings.artwork):null};settings={...settings,ratio:clone(style.ratio),borderPercent:style.borderPercent,frameColor:style.frameColor,decoration:clone(style.decoration),artwork:style.artwork?clone(style.artwork):null};templatesOpen=false;modelOpen=false;syncControls();}
  function saveSuggestedFrame(suggestion:FrameSuggestion){openTemplates({...settings,frameColor:suggestion.frameColor,borderPercent:suggestion.borderPercent,decoration:clone(suggestion.decoration),artwork:clone(suggestion.artwork)},suggestion.title);}

  let selectedId = '';
  let batch: BatchSnapshot | null = null;
  let initialized = false;
  let loading = true;
  let importing = false;
  let importingCount = 0;
  let importReport: { added: number; skipped: number } | null = null;
  let starting = false;
  let cancelling = false;
  let dragActive = false;
  let dragCount = 0;
  let notices: string[] = [];
  let fatalError = '';
  let preview: PreviewResult | null = null;
  let previewError = '';
  let previewLoading = false;
  let previewPhotoId = '';
  let previewRevision = 0;
  let previewTimer: ReturnType<typeof setTimeout> | undefined;
  let saveTimer: ReturnType<typeof setTimeout> | undefined;
  let saveInFlight = false;
  let queuedSave: RenderSettings | null = null;
  let previewInFlight = false;
  let queuedPreview: { photoId: string; settings: RenderSettings; revision: number } | null = null;
  let unlisteners: (() => void)[] = [];
  let outputDirectory = '';
  let selectedPreset = '';
  let namingPreset = false;
  let presetName = '';
  let savingPreset = false;
  let ratioMode = 'auto';
  let customWidth = 4;
  let customHeight = 5;
  let frameHex = settings.frameColor;
  let frameHexError = '';
  let lastFrameColor = settings.frameColor;
  let advancedExport = false;
  let longEdgeMode = 'original';
  let customLongEdge = 3840;
  let jobDialog: HTMLDialogElement;
  let errorsExpanded = false;
  let pendingImports: string[] = [];
  let modelOpen = false;
  let modelsConfigOpen=false,returnToModel=false;
  let stagedModelConfig:AiConfig|null=null;
  function configureModels(current:AiConfig|null=null){stagedModelConfig=current?clone(current):null;returnToModel=modelOpen;modelOpen=false;modelsConfigOpen=true;}
  function closeModelConfig(){modelsConfigOpen=false;if(returnToModel&&selectedPhoto)modelOpen=true;else stagedModelConfig=null;returnToModel=false;}
  let modelMode:'design'|'refine'|'image'='image';
  let modelTarget:'frame'|'sticker'='frame';
  let sessionApiKey = '',sessionApiOrigin='';
  let sessionImageApiKey='',sessionImageApiOrigin='';
  let previousArt: {ratio:RenderSettings['ratio'];frameColor:string;borderPercent:number;decoration:FrameDecoration;artwork?:ArtScene|null} | null = null;
  const motifs: {kind:FrameDecoration['kind'];name:string}[]=[{kind:'none',name:'纯色'},{kind:'paper',name:'纸感'},{kind:'linen',name:'亚麻'},{kind:'dots',name:'点阵'},{kind:'lines',name:'细条纹'},{kind:'waves',name:'柔波纹'},{kind:'corners',name:'角标'}];
  function applyArt(suggestion:FrameSuggestion) {
    previousArt={ratio:clone(settings.ratio),frameColor:settings.frameColor,borderPercent:settings.borderPercent,decoration:clone(settings.decoration),artwork:settings.artwork?clone(settings.artwork):null};
    settings={...settings,frameColor:suggestion.frameColor,borderPercent:suggestion.borderPercent,decoration:clone(suggestion.decoration),artwork:clone(suggestion.artwork)};
    modelOpen=false;stagedModelConfig=null;syncControls();
  }
  function undoArt() {if(previousArt){settings={...settings,...previousArt};previousArt=null;syncControls();}}
  let pickingColor = false;
  let pickerButton: HTMLButtonElement;
  function closePicker() { pickingColor = false; pickerButton?.focus({ preventScroll: true }); }
  let offsetLimits:OffsetLimits|null=null;
  let fonts: FontOption[] = [];
  let fontsLoading = false;
  let fontsError = '';
  let watermarkWorkspaceShown = false;
  let watermarkPreviewCollapsed = false;
  try { watermarkPreviewCollapsed = localStorage.getItem('liubai.watermarkPreviewCollapsed') === 'true'; } catch { /* Use the expanded preview if storage is unavailable. */ }
  function collapseWatermarkPreview(collapsed:boolean) {
    watermarkPreviewCollapsed = collapsed;
    try { localStorage.setItem('liubai.watermarkPreviewCollapsed',String(collapsed)); } catch { /* The current session remains usable. */ }
  }
  const palette = [
    ['#FFFFFF','纯白'], ['#F3ECDD','奶油'], ['#DED5C4','燕麦'], ['#B7A58E','砂岩'], ['#8F877A','暖灰'], ['#171A17','墨黑'],
    ['#DDE5DC','雾绿'], ['#A5B59B','鼠尾草'], ['#67785E','橄榄'], ['#DBE5EC','雾蓝'], ['#7D96AA','灰蓝'], ['#344C60','深海'],
    ['#F2DCD5','浅桃'], ['#C99786','陶土'], ['#934D49','砖红'], ['#E9DEB5','麦黄'], ['#C9B7D4','淡紫'], ['#645B73','烟紫'],
  ];
  $: if (!selectedId || isRunning || importing) pickingColor = false;


  $: selectedPhoto = photos.find((photo) => photo.id === selectedId) ?? null;
  $: watermarkPreviewVisible = desktop && !!selectedPhoto && settings.watermark.kind !== 'none';
  $: watermarkWorkspaceNeeded = watermarkPreviewVisible && !watermarkPreviewCollapsed;
  $: if (watermarkWorkspaceNeeded !== watermarkWorkspaceShown) {
    watermarkWorkspaceShown = watermarkWorkspaceNeeded;
    if (watermarkWorkspaceNeeded) void ensureWatermarkWorkspace();
  }
  $: isRunning = batch?.status === 'running';
  $: completedCount = batch?.items.filter((item) => item.status === 'completed').length ?? 0;
  $: failedCount = batch?.items.filter((item) => item.status === 'failed').length ?? 0;
  $: pendingCount = batch?.items.filter((item) => item.status === 'pending' || item.status === 'cancelled').length ?? 0;
  $: totalCount = batch?.items.length ?? 0;
  $: progress = totalCount ? ((completedCount + failedCount) / totalCount) * 100 : 0;
  $: localError = $t(validateLocal(settings)) || $t(frameHexError);
  $: if (initialized && desktop) scheduleSettingsSave(JSON.stringify(settings));
  $: if (initialized) schedulePreview(selectedId, JSON.stringify(settings));
  $: if (settings.frameColor !== lastFrameColor) {
    lastFrameColor = settings.frameColor;
    frameHex = settings.frameColor;
    frameHexError = '';
  }

  function syncControls() {
    if (!settings.ratio) ratioMode = 'auto';
    else {
      const label = `${settings.ratio.width}:${settings.ratio.height}`;
      ratioMode = ratioPresets.includes(label) ? label : 'custom';
      customWidth = settings.ratio.width;
      customHeight = settings.ratio.height;
    }
    const edge = settings.export.longEdge;
    longEdgeMode = edge == null ? 'original' : [2560, 3840].includes(edge) ? String(edge) : 'custom';
    if (edge) customLongEdge = edge;
    frameHex = settings.frameColor;
    lastFrameColor = settings.frameColor;
    frameHexError = '';
  }

  let initializeRevision=0;
  async function initialize() {
    const attempt=++initializeRevision;
    loading = true;
    fatalError = '';
    try {
      const state = await startupDeadline(invoke<AppSnapshot>('get_state'));
      if(attempt!==initializeRevision)return;
      settings = state.settings;
      presets = state.presets;
      frameTemplates=state.frameTemplates??[];
      photos = state.photos;
      selectedId = photos[0]?.id ?? '';
      if (state.batch) acceptBatch(state.batch);
      outputDirectory = batch?.outputDirectory ?? '';
      notices = [...new Set([...notices, ...(state.warnings ?? [])])];
      syncControls();
      initialized = true;
    } catch (error) {
      if(attempt===initializeRevision)fatalError = errorMessage(error);
    } finally {
      if(attempt===initializeRevision)loading = false;
      if (initialized && pendingImports.length) {
        const paths = pendingImports;
        pendingImports = [];
        void importPaths(paths);
      }
    }
  }

  onMount(() => {
    if (!desktop) {
      loading = false;
      initialized = true;
      return;
    }
    let disposed = false;
    const registerProgress = async () => {
      const offProgress = await listen<BatchSnapshot>('batch-progress', ({ payload }) => acceptBatch(payload));
      if (disposed) offProgress(); else unlisteners.push(offProgress);
    };
    const registerDrop = async () => {
      const offDrop = await getCurrentWebviewWindow().onDragDropEvent(({ payload }) => {
        if (disposed) return;
        if (payload.type === 'enter') dragCount = payload.paths.length;
        if (payload.type === 'enter' || payload.type === 'over') dragActive = true;
        else dragActive = false;
        if (payload.type === 'drop') void importPaths(payload.paths);
        if (payload.type === 'leave' || payload.type === 'drop') dragCount = 0;
      });
      if (disposed) offDrop(); else unlisteners.push(offDrop);
    };
    // Accept drops before loading the workspace; importPaths queues early groups.
    // A progress-listener failure must not disable file import.
    void registerProgress().catch(error=>{if(!disposed)notices=[...notices,errorMessage(error)];});
    void registerDrop().catch(error=>{if(!disposed)notices=[...notices,$t("拖入照片暂不可用：{0}",{0:errorMessage(error)})];})
      .then(()=>{if(!disposed)return initialize();});
    return () => { disposed = true; };
  });

  onDestroy(() => {
    clearTimeout(previewTimer);
    clearTimeout(saveTimer);
    unlisteners.forEach((unlisten) => unlisten());
  });

  function validateLocal(value: RenderSettings): string {
    if (!/^#[0-9a-f]{6}$/i.test(value.frameColor)) return $t("请输入六位 HEX 边框颜色");
    if (!Number.isFinite(value.borderPercent) || value.borderPercent < 0 || value.borderPercent > 30) return $t("边框比例应在 0% 到 30% 之间");
    if (value.ratio && (!(value.ratio.width > 0) || !(value.ratio.height > 0))) return $t("自定义比例的宽和高必须大于 0");
    if (value.watermark.kind !== 'none' && (!Number.isFinite(value.watermark.offsetX) || !Number.isFinite(value.watermark.offsetY))) return $t("请输入有效的水印偏移");
    if (value.watermark.kind === 'text' && !value.watermark.text.trim()) return $t("请输入水印文字");
    if (value.watermark.kind === 'image' && !value.watermark.imagePath) return $t("请选择一张 PNG 水印");
    if (value.watermark.kind === 'text' && !/^#[0-9a-f]{6}$/i.test(value.watermark.color)) return $t("请选择有效的水印颜色");
    if (value.export.longEdge != null && (!Number.isFinite(value.export.longEdge) || value.export.longEdge < 64)) return $t("输出长边至少为 64 像素");
    return '';
  }

  function scheduleSettingsSave(serialized: string) {
    clearTimeout(saveTimer);
    const snapshot: RenderSettings = JSON.parse(serialized);
    if (validateLocal(snapshot)) return;
    saveTimer = setTimeout(() => { queuedSave = snapshot; void drainSave(); }, 500);
  }

  async function drainSave() {
    if (saveInFlight) return;
    saveInFlight = true;
    try {
      while (queuedSave) {
        const next = queuedSave; queuedSave = null;
        try { await invoke('save_settings', { settings: next }); }
        catch (error) {
          const message = $t("设置未保存：{0}", {0:errorMessage(error)});
          if (!notices.includes(message)) notices = [...notices, message];
        }
      }
    } finally { saveInFlight = false; }
  }

  function schedulePreview(photoId: string, serialized: string) {
    previewRevision += 1;
    const revision = previewRevision;
    const snapshot: RenderSettings = JSON.parse(serialized);
    if (photoId !== previewPhotoId) {
      preview = null;
      previewPhotoId = photoId;
    }
    previewError = '';
    if (!photoId || !desktop) { queuedPreview=null; previewLoading = false; return; }
    const validation = validateLocal(snapshot);
    if (validation) { queuedPreview=null; previewError = validation; previewLoading = false; return; }
    previewLoading = true;
    queuedPreview={photoId,settings:snapshot,revision};
    if (!previewTimer) previewTimer=setTimeout(()=>{previewTimer=undefined;void drainPreview();},60);
  }

  async function drainPreview() {
    if (previewInFlight) return;
    previewInFlight = true;
    try {
      while (queuedPreview) {
        const request = queuedPreview;
        queuedPreview = null;
        try {
          const result = await invoke<PreviewResult>('get_preview', { photoId: request.photoId, settings: request.settings });
          if(result.sourceThumbnail)photos=photos.map(p=>p.id===request.photoId?{...p,thumbnail:result.sourceThumbnail!}:p);
          if (request.photoId === previewPhotoId && !validateLocal(settings)) {
            preview = result;
            previewError = '';
            previewLoading = request.revision !== previewRevision;
          }
        } catch (error) {
          if (request.revision === previewRevision) {
            previewError = errorMessage(error);
            previewLoading = false;
          }
        }
      }
    } finally {
      previewInFlight = false;
    }
  }

  async function choosePhotos() {
    if (!desktop || importing || !initialized || loading) return;
    try {
      const paths = await open({ multiple: true, directory: false, title: $t("选择照片"), filters: [{ name: $t("照片"), extensions: ['jpg', 'jpeg', 'png', 'heic', 'heif'] }] });
      if (paths) await importPaths(Array.isArray(paths) ? paths : [paths]);
    } catch (error) { notices = [...notices, errorMessage(error)]; }
  }

  async function importPaths(paths: string[]) {
    if (!desktop || !paths.length) return;
    if (!initialized || loading || importing) { pendingImports = [...new Set([...pendingImports, ...paths])]; return; }
    importing = true;
    importingCount = paths.length;
    importReport = null;
    try {
      const result = await invoke<ImportResult>('import_photos', { paths });
      const existing = new Set(photos.map((photo) => photo.id));
      const added = result.photos.filter((photo) => !existing.has(photo.id));
      photos = [...photos, ...added];
      if (!selectedId && photos.length) selectedId = photos[0].id;
      importReport = { added: added.length, skipped: Math.max(0, paths.length - added.length) };
      if (result.errors.length) notices = [...notices, ...result.errors];
    } catch (error) { notices = [...notices, errorMessage(error)]; }
    finally {
      importing = false;
      importingCount = 0;
      if (pendingImports.length) {
        const nextPaths = pendingImports;
        pendingImports = [];
        void importPaths(nextPaths);
      }
    }
  }

  async function removePhotos(ids: string[]) {
    if (isRunning || !desktop) return;
    try {
      await invoke('remove_photos', { ids });
      const previousIndex = photos.findIndex((photo) => photo.id === selectedId);
      photos = photos.filter((photo) => !ids.includes(photo.id));
      if (ids.includes(selectedId)) selectedId = photos[Math.min(previousIndex, photos.length - 1)]?.id ?? '';
    } catch (error) { notices = [...notices, errorMessage(error)]; }
  }

  function changeRatio(mode: string) {
    ratioMode = mode;
    if (mode === 'auto') settings.ratio = null;
    else if (mode === 'custom') settings.ratio = { width: customWidth, height: customHeight };
    else {
      const [width, height] = mode.split(':').map(Number);
      settings.ratio = { width, height };
      customWidth = width;
      customHeight = height;
    }
  }

  function changeCustomRatio(axis: 'width' | 'height', value: number) {
    if (axis === 'width') customWidth = value; else customHeight = value;
    settings.ratio = { width: customWidth, height: customHeight };
  }

  function swapRatio() {
    if (!settings.ratio) return;
    settings.ratio = { width: settings.ratio.height, height: settings.ratio.width };
    syncControls();
  }

  function setFrameColor(value: string) {
    const normalized = value.startsWith('#') ? value : `#${value}`;
    frameHex = value;
    if (/^#[0-9a-f]{6}$/i.test(normalized)) {
      settings.frameColor = normalized.toUpperCase();
      frameHex = normalized.toUpperCase();
      frameHexError = '';
    } else frameHexError = $t("颜色格式如 #FFFFFF");
  }

  function changeWatermark(kind: WatermarkKind) {
    if (settings.watermark.kind !== kind && kind !== 'none') settings.watermark.sizePercent = kind === 'image' ? 15 : 3;
    settings.watermark.kind = kind;
  }

  function changeRegion(region: WatermarkRegion) {
    settings.watermark.region = region;
    settings.watermark.offsetX = 0; settings.watermark.offsetY = 0;
    if (region === 'border') {
      settings.watermark.position = settings.watermark.position.endsWith('left') ? 'bottom-left' : settings.watermark.position.endsWith('right') ? 'bottom-right' : 'bottom-center';
    }
  }

  async function chooseWatermarkImage() {
    try {
      const path = await open({ multiple: false, title: $t("选择透明 PNG 水印"), filters: [{ name: $t("PNG 水印"), extensions: ['png'] }] });
      if (typeof path === 'string') settings.watermark.imagePath = path;
    } catch (error) { notices = [...notices, errorMessage(error)]; }
  }

  async function chooseFont() {
    try {
      const path = await open({ multiple: false, title: $t("选择水印字体"), filters: [{ name: $t("字体"), extensions: ['ttf', 'otf', 'ttc', 'otc'] }] });
      if (typeof path === 'string') { settings.watermark.fontPath = path; settings.watermark.fontFamily = null; }
    } catch (error) { notices = [...notices, errorMessage(error)]; }
  }

  async function loadFonts() {
    if (fonts.length || fontsLoading || !desktop) return;
    fontsLoading = true; fontsError = '';
    try { fonts = await invoke<FontOption[]>('list_fonts'); }
    catch (error) { fontsError = errorMessage(error); }
    finally { fontsLoading = false; }
  }

  function applyPreset(id: string) {
    selectedPreset = id;
    const preset = presets.find((item) => item.id === id);
    if (preset) { settings = clone(preset.settings); syncControls(); }
  }

  async function savePreset() {
    const name = presetName.trim();
    if (!name || localError || savingPreset) return;
    savingPreset = true;
    try {
      const preset = await invoke<Preset>('save_preset', { name, settings: clone(settings) });
      presets = [...presets.filter((item) => item.id !== preset.id), preset];
      selectedPreset = preset.id;
      namingPreset = false;
    } catch (error) { notices = [...notices, errorMessage(error)]; }
    finally { savingPreset = false; }
  }

  async function deletePreset() {
    if (!selectedPreset) return;
    try {
      await invoke('delete_preset', { id: selectedPreset });
      presets = presets.filter((preset) => preset.id !== selectedPreset);
      selectedPreset = '';
    } catch (error) { notices = [...notices, errorMessage(error)]; }
  }

  function setLongEdge(mode: string) {
    longEdgeMode = mode;
    settings.export.longEdge = mode === 'original' ? null : mode === 'custom' ? customLongEdge : Number(mode);
  }

  async function chooseOutputDirectory() {
    if (!desktop || isRunning) return;
    try {
      const path = await open({ directory: true, multiple: false, title: $t("选择导出文件夹"), defaultPath: outputDirectory || undefined });
      if (typeof path === 'string') outputDirectory = path;
    } catch (error) { notices = [...notices, errorMessage(error)]; }
  }

  async function startBatch() {
    if (!desktop || isRunning || starting || !photos.length || localError) return;
    if (!outputDirectory) await chooseOutputDirectory();
    if (!outputDirectory) return;
    starting = true;
    try {
      const result = await invoke<BatchSnapshot>('start_batch', { outputDirectory, settings: clone(settings), photoIds: photos.map((photo) => photo.id) });
      acceptBatch(result);
    } catch (error) { notices = [...notices, errorMessage(error)]; }
    finally { starting = false; }
  }

  async function cancelBatch() {
    if (cancelling) return;
    cancelling = true;
    try { await invoke('cancel_batch'); }
    catch (error) { cancelling = false; notices = [...notices, errorMessage(error)]; }
  }

  async function continueBatch(command: 'resume_batch' | 'retry_failed') {
    if (isRunning || starting) return;
    starting = true;
    try { acceptBatch(await invoke<BatchSnapshot>(command)); }
    catch (error) { notices = [...notices, errorMessage(error)]; }
    finally { starting = false; }
  }

  async function revealPath(path: string) {
    try {
      if (path === batch?.outputDirectory) await invoke('open_export_directory');
      else if (path) await invoke('open_exported_image', { path });
    }
    catch (error) { notices = [...notices, errorMessage(error)]; }
  }

  function acceptBatch(incoming: BatchSnapshot) {
    if (batch && incoming.id === batch.id && incoming.revision < batch.revision) return;
    batch = incoming;
    if (incoming.status !== 'running') cancelling = false;
  }
</script>

<svelte:window on:keydown={(event) => {
  if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 'o') {
    event.preventDefault();
    void choosePhotos();
  }
}} />

<div class:drop-active={dragActive} class="app-shell">
  <header class="app-header">
    <div class="brand"><span class="brand-symbol" aria-hidden="true"><Square size={17} strokeWidth={1.5} /></span><h1>{$t("留白")}</h1><span class="brand-description">{$t("照片边框与水印")}</span></div>
    <div class="header-actions">
      <button type="button" class="button secondary compact model-config-trigger" disabled={!desktop} on:click={()=>configureModels()}><SlidersHorizontal size={15}/>{$t('模型配置')}</button>
      <div class="language-picker">
        <Languages size={16} aria-hidden="true" />
        <select id="interface-language" class="language-select" aria-label={$t("界面语言 / Interface language")} bind:value={$language}>
          <option value="en" lang="en">English</option>
          <option value="zh-CN" lang="zh-CN">简体中文</option>
        </select>
        <ChevronDown class="language-chevron" size={12} aria-hidden="true" />
      </div>
      {#if photos.length}<span class="photo-count">{photos.length} {$t("张照片",{count:photos.length})}</span>{/if}
      <button class="button secondary compact" on:click={choosePhotos} disabled={!desktop || importing || loading || !initialized}>
        {#if importing}<LoaderCircle size={15} class="spin" />{:else}<Plus size={16} />{/if}
        {importing ? $t("正在导入") : $t("添加照片")}
      </button>
    </div>
  </header>

  {#if !desktop}
    <div class="browser-notice"><AlertCircle size={15} /> {$t("当前为界面预览，请在桌面版中导入和处理照片")}</div>
  {/if}
  {#if fatalError}
    <div class="notice error" role="alert"><AlertCircle size={16} /><div><strong>{$t("未能载入本机数据")}</strong><p>{$t(fatalError)}</p></div><button class="text-button" on:click={initialize}>{$t("重新加载")}</button></div>
  {/if}
  {#if notices.length}
    <div class="notice" role="status">
      <AlertCircle size={16} />
      <div class="notice-content"><span>{$t(notices[0])}</span>{#if notices.length > 1}<button class="text-button" on:click={() => errorsExpanded = !errorsExpanded}>{errorsExpanded ? $t("收起") : $t("另有 {0} 条提示", {0:notices.length - 1})}</button>{/if}
        {#if errorsExpanded}<ul>{#each notices.slice(1) as notice}<li>{$t(notice)}</li>{/each}</ul>{/if}
      </div>
      <button class="icon-button" aria-label={$t("关闭提示")} on:click={() => { notices = []; errorsExpanded = false; }}><X size={16} /></button>
    </div>
  {/if}

  <div class="workspace">
    <main class="content-area">
      <div class="viewer">
        {#if loading}
          <div class="empty-state"><LoaderCircle size={25} class="spin" /><p>{$t("正在载入工作区")}</p></div>
        {:else if selectedPhoto}
          <div class="viewer-topline">
            <div class="file-heading"><FileImage size={14} /><span title={selectedPhoto.path}>{selectedPhoto.name}</span><span class="source-size">{selectedPhoto.width} × {selectedPhoto.height}</span></div>
            <button class="icon-button" title={$t("移除当前照片")} aria-label={$t("移除当前照片")} disabled={isRunning} on:click={() => removePhotos([selectedPhoto.id])}><X size={15} /></button>
          </div>
          {#key selectedPhoto.id}
            <PhotoViewport photo={selectedPhoto} {preview} picking={pickingColor} loading={previewLoading} oncolor={setFrameColor} onclose={closePicker}>
              {#if watermarkPreviewVisible}
                <WatermarkDetail photo={selectedPhoto} {settings} overview={preview} collapsed={watermarkPreviewCollapsed} oncollapse={collapseWatermarkPreview} onlimits={(value)=>offsetLimits=value}/>
              {/if}
            </PhotoViewport>
          {/key}
          <div class="viewer-bottomline" aria-live="polite">
            {#if previewError}<span class="preview-error"><AlertCircle size={14} />{$t(previewError)}</span>
            {:else if preview}<span>{$t("导出尺寸")} <strong>{preview.width} × {preview.height}</strong> px</span><span>{$t(preview.warning || "完整保留照片 · 居中补边")}</span>
            {:else}<span>{$t("正在准备预览")}</span>{/if}
          </div>
        {:else}
          <div class="empty-state">
            <div class="empty-frame"><ImageIcon size={35} strokeWidth={1.15} /><span></span></div>
            <h2>{$t("给照片留一点空间")}</h2>
            <p>{$t("拖入一组照片，或选择多个文件开始")}</p>
            <button class="button primary" on:click={choosePhotos} disabled={!desktop || importing || loading || !initialized}><ImagePlus size={17} />{$t("选择照片")}</button>
            <span class="empty-formats">JPG · PNG · HEIC</span>
          </div>
        {/if}
      </div>

      {#if batch && (batch.status === 'interrupted' || batch.status === 'cancelled') && pendingCount > 0}
        <div class="resume-banner"><RotateCcw size={15} /><span>{$t("有")} {pendingCount} {$t("张照片尚未完成")}</span><button class="text-button" disabled={starting} on:click={() => continueBatch('resume_batch')}>{$t("继续处理")}</button></div>
      {/if}
      {#if batch?.error}<div class="resume-banner batch-error" role="alert"><AlertCircle size={15} /><span>{$t(batch.error)}</span></div>{/if}

      <section class="filmstrip-section" class:has-import-status={importing||pendingImports.length>0||importReport!==null} aria-label={$t("已导入的照片")}>
        <div class="filmstrip-heading"><span>{$t("照片")} <span class="count">{photos.length}</span></span><span class="filmstrip-hint">{$t("可将一组照片拖入窗口任意位置")}</span>{#if photos.length}<button class="text-button muted" disabled={isRunning} on:click={() => removePhotos(photos.map((photo) => photo.id))}>{$t("清空列表")}</button>{/if}</div>
        <div class="import-feedback" role="status" aria-live="polite" aria-atomic="true">
          {#if importing}<LoaderCircle size={12} class="spin"/><span>{$t("正在导入 {0} 张照片…",{0:importingCount})}{#if pendingImports.length}{' · ' }{$t("另有 {0} 张排队",{0:pendingImports.length})}{/if}</span>
          {:else if pendingImports.length}<span>{$t("已接收 {0} 张照片，载入后导入",{0:pendingImports.length})}</span>
          {:else if importReport}<CheckCircle2 size={12}/><span>{$t("已添加 {0} 张照片",{0:importReport.added})}{#if importReport.skipped}{' · ' }{$t("跳过 {0} 个重复、不支持或无法读取的文件",{0:importReport.skipped})}{/if}</span>{/if}
        </div>
        <div class="filmstrip">
          {#each photos as photo (photo.id)}
            {@const job = batch?.items.find((item) => item.photoId === photo.id)}
            <button class:selected={photo.id === selectedId} class="thumbnail" on:click={() => selectedId = photo.id} aria-label={$t("预览 {0}", {0:photo.name})} aria-pressed={photo.id === selectedId} title={photo.name}>
              {#if photo.thumbnail}<img src={photo.thumbnail} alt="" />{:else}<FileImage size={22}/>{/if}
              {#if job?.status === 'completed'}<span class="thumbnail-status success"><Check size={10} strokeWidth={3} /></span>{:else if job?.status === 'failed'}<span class="thumbnail-status failed"><AlertCircle size={11} /></span>{:else if job?.status === 'running'}<span class="thumbnail-status"><LoaderCircle size={11} class="spin" /></span>{/if}
            </button>
          {/each}
          <button class="add-tile" on:click={choosePhotos} disabled={!desktop || importing || loading || !initialized} aria-label={$t("添加更多照片")}><Plus size={20} strokeWidth={1.5} /></button>
        </div>
      </section>
    </main>

    <aside class="settings-panel" aria-label={$t("边框和水印设置")}>

      <section class="setting-section">
        <div class="section-heading"><h2>{$t("边框")}</h2><span>{$t("应用到全部照片")}</span></div>
        <div class="frame-entry-row">
          <button class="button secondary compact" disabled={!desktop||!initialized||loading||importing||isRunning} on:click={()=>openTemplates()}><Bookmark size={15}/>{$t('边框模板')}</button>
          <button class="button secondary compact" disabled={!desktop||!selectedPhoto||importing||isRunning} on:click={()=>{modelMode='image';modelTarget='frame';modelOpen=true;}}><WandSparkles size={15}/>{$t('AI 设计')}</button>
        </div>
        <label class="field-label" for="ratio">{$t("最终比例")}</label>
        <div class="ratio-row">
          <select id="ratio" value={ratioMode} on:change={(event) => changeRatio(event.currentTarget.value)}><option value="auto">{$t("自动 · 四边等宽")}</option>{#each ratioPresets as ratio}<option value={ratio}>{ratio.replace(':', ' : ')}</option>{/each}<option value="custom">{$t("自定义比例")}</option></select>
          <button class="icon-button bordered" disabled={!settings.ratio} on:click={swapRatio} title={$t("交换宽高")} aria-label={$t("交换宽高")}><ArrowLeftRight size={16} /></button>
        </div>
        {#if ratioMode === 'custom'}<div class="custom-ratio"><label><span>{$t("宽")}</span><input type="number" min="0.01" step="0.1" value={customWidth} on:input={(event) => changeCustomRatio('width', event.currentTarget.valueAsNumber)} aria-label={$t("自定义比例宽")} /></label><span>:</span><label><span>{$t("高")}</span><input type="number" min="0.01" step="0.1" value={customHeight} on:input={(event) => changeCustomRatio('height', event.currentTarget.valueAsNumber)} aria-label={$t("自定义比例高")} /></label></div>{/if}
        <div class="field-title"><label class="field-label" for="border-range">{$t("边框比例")}</label><div class="number-with-unit"><input aria-label={$t("边框百分比")} type="number" min="0" max="30" step="0.5" bind:value={settings.borderPercent} /><span>%</span></div></div>
        <input id="border-range" type="range" min="0" max="30" step="0.5" bind:value={settings.borderPercent} />
        <p class="field-help">{$t("按照片短边计算最小留白；指定比例时，部分方向会增加留白")}</p>
        <label class="field-label color-label" for="frame-hex">{$t("边框颜色")}</label>
        <div class="color-row">
          <input class="color-picker" type="color" aria-label={$t("选择边框颜色")} value={settings.frameColor} on:input={(event) => setFrameColor(event.currentTarget.value)} />
          <input id="frame-hex" class="hex-input" value={frameHex} maxlength="7" on:input={(event) => setFrameColor(event.currentTarget.value)} spellcheck="false" />
          <button bind:this={pickerButton} class="icon-button bordered" class:color-active={pickingColor} aria-label={$t("从照片取色")} aria-pressed={pickingColor} title={$t("从照片取色（带原像素放大镜）")} disabled={!preview || previewLoading || isRunning || importing} on:click={() => pickingColor = !pickingColor}><Pipette size={16} /></button>
        </div>
        <div class="color-palette" role="group" aria-label={$t("边框色盘")}>{#each palette as [color,name]}<button style:background={color} class:active={settings.frameColor.toUpperCase() === color} class="palette-swatch" aria-label={`${$t(name)} ${color}`} aria-pressed={settings.frameColor.toUpperCase() === color} title={$t(name)} on:click={() => setFrameColor(color)}></button>{/each}</div>
        <p class="field-help">{$t("选色盘，或点滴管从照片中取色")}</p>
        {#if frameHexError}<p class="field-error">{$t(frameHexError)}</p>{/if}
        <details class="advanced-watermark motif-details"><summary><ChevronDown size={14} />{$t("简约花纹")}</summary>
          <div class="motif-grid" role="group" aria-label={$t("边框花纹")}>{#each motifs as motif}<button class:chosen={settings.decoration.kind===motif.kind} aria-pressed={settings.decoration.kind===motif.kind} on:click={()=>settings.decoration.kind=motif.kind}><PatternSwatch decoration={{...settings.decoration,kind:motif.kind}} background={settings.frameColor} /><span>{$t(motif.name)}</span></button>{/each}</div>
          {#if settings.decoration.kind!=='none'}
            <div class="field-title"><label class="field-label" for="motif-opacity">{$t("花纹强度")}</label><output>{Math.round(settings.decoration.opacity*100)}%</output></div><input id="motif-opacity" type="range" min="0" max="0.3" step="0.01" bind:value={settings.decoration.opacity} />
            <div class="field-title"><label class="field-label" for="motif-scale">{$t("花纹大小")}</label><output>{settings.decoration.scale.toFixed(1)}×</output></div><input id="motif-scale" type="range" min="0.5" max="3" step="0.1" bind:value={settings.decoration.scale} />
            <div class="font-row"><label class="field-label" for="motif-color">{$t("花纹颜色")}</label><input id="motif-color" class="color-picker" type="color" bind:value={settings.decoration.color} /></div>
          {/if}
          <p class="field-help">{$t("只装饰留白，轻一点更耐看；0% 留白时可能没有可绘制区域")}</p>
        </details>
        {#if settings.artwork}
         <details class="advanced-watermark"><summary><ChevronDown size={14}/>{$t('艺术边框调整')}</summary>
          {#if settings.artwork.design}
           {#if settings.artwork.design.film}
            {#if settings.artwork.design.id!=='scan-instant'}<label class="field-help"><input type="checkbox" bind:checked={settings.artwork.design.film.labels}/>{$t('片边标记')}</label>{/if}
            <div class="field-title"><label for="applied-film-texture" class="field-label">{$t('扫描颗粒')}</label><output>{Math.round(settings.artwork.design.film.texture*100)}%</output></div><input id="applied-film-texture" type="range" min="0" max="1" step="0.05" bind:value={settings.artwork.design.film.texture}/>
           {/if}
           <div class="field-title"><label for="art-strength" class="field-label">{$t('装饰强度')}</label><output>{Math.round(settings.artwork.design.intensity*100)}%</output></div><input id="art-strength" type="range" min="0.2" max="1.5" step="0.05" bind:value={settings.artwork.design.intensity}/>
           <div class="field-title"><label for="art-size" class="field-label">{$t('装饰大小')}</label><output>{Math.round((settings.artwork.design.scale??1)*100)}%</output></div><input id="art-size" type="range" min="0.2" max="1" step="0.05" value={settings.artwork.design.scale??1} on:input={e=>{if(settings.artwork?.design)settings.artwork.design.scale=Number(e.currentTarget.value);}}/>
           <div class="color-row">{#each settings.artwork.design.colors as color,i}<input class="color-picker" type="color" aria-label={$t('艺术配色 {0}',{0:i+1})} value={color} on:input={e=>{if(settings.artwork?.design)settings.artwork.design.colors[i]=e.currentTarget.value;}}/>{/each}</div>
          {/if}
          {#if settings.artwork.frameImage}<p class="field-help">{$t('插画随四边与四角等比铺满，边缘可能裁切；照片始终完整保留。')}</p><div class="field-title"><label class="field-label" for="frame-art-opacity">{$t('插画透明度')}</label><output>{Math.round(settings.artwork.frameImage.opacity*100)}%</output></div><input id="frame-art-opacity" type="range" min="0" max="1" step="0.01" bind:value={settings.artwork.frameImage.opacity}/>{/if}
          {#each settings.artwork.stickers||[] as sticker,i}
           <label class="field-label" for={'sticker-position-'+i}>{$t('手绘素材位置')}</label><select id={'sticker-position-'+i} bind:value={sticker.anchor}>{#each positions.filter(p=>p.value.startsWith('top')||p.value.startsWith('bottom')) as p}<option value={p.value}>{$t(p.label)}</option>{/each}</select>
           <div class="field-title"><label class="field-label" for={'sticker-size-'+i}>{$t('素材大小')}</label><output>{Math.round(sticker.scale*100)}%</output></div><input id={'sticker-size-'+i} type="range" min="0.2" max="1" step="0.01" bind:value={sticker.scale}/>
           <div class="field-title"><label class="field-label" for={'sticker-opacity-'+i}>{$t('素材透明度')}</label><output>{Math.round(sticker.opacity*100)}%</output></div><input id={'sticker-opacity-'+i} type="range" min="0" max="1" step="0.01" bind:value={sticker.opacity}/>
           <label class="field-help"><input type="checkbox" bind:checked={sticker.flipX}/>{$t('水平翻转素材')}</label>
          {/each}
          <p class="field-help">{$t('图案保持等比缩放，仅绘制在边框内；窄边框会缩小装饰。')}</p>
          <button type="button" class="text-button" on:click={()=>settings.artwork=null}>{$t('移除艺术图层')}</button>
         </details>
         {#if !settings.artwork.design}<button class="text-button" disabled={!selectedPhoto||isRunning||importing} on:click={()=>{modelMode='refine';modelOpen=true;}}><WandSparkles size={12}/>{$t('精修一次')}</button>{/if}
        {/if}
        <details class="advanced-watermark saved-settings"><summary><ChevronDown size={14}/>{$t('保存与预设')}</summary>
          <button class="text-button save-current-frame" disabled={!desktop||!initialized||loading||!!localError} on:click={()=>openTemplates(settings)}><Bookmark size={12}/>{$t('保存当前边框为模板')}</button>
      <div class="preset-row">
        <Bookmark size={15} />
        <select aria-label={$t("个人预设")} value={selectedPreset} on:change={(event) => applyPreset(event.currentTarget.value)}><option value="">{presets.length ? $t("选择个人预设") : $t("我的预设")}</option>{#each presets as preset}<option value={preset.id}>{preset.name}</option>{/each}</select>
        {#if selectedPreset}<button class="icon-button" on:click={deletePreset} disabled={!desktop} title={$t("删除此预设")} aria-label={$t("删除此预设")}><Trash2 size={14} /></button>{/if}
        <button class="icon-button" on:click={() => { namingPreset = !namingPreset; presetName = $t("我的方案 {0}", {0:presets.length + 1}); }} disabled={!desktop || !!localError} title={$t("保存当前设置为预设")} aria-label={$t("保存当前设置为预设")}><Plus size={16} /></button>
      </div>
      {#if namingPreset}<form class="preset-form" on:submit|preventDefault={savePreset}><input aria-label={$t("预设名称")} bind:value={presetName} maxlength="40" placeholder={$t("预设名称")} /><button class="button secondary compact" type="submit" disabled={savingPreset || !presetName.trim()}>{savingPreset ? $t("保存中") : $t("保存")}</button></form>{/if}
        </details>
        {#if previousArt}<button class="text-button" on:click={undoArt}><RotateCcw size={12} />{$t("撤销上次智能配框")}</button>{/if}
      </section>

      <section class="setting-section watermark-section">
        <div class="section-heading"><h2>{$t("水印")}</h2></div>
        <div class="segmented" role="group" aria-label={$t("水印类型")}>
          <button class:active={settings.watermark.kind === 'none'} aria-pressed={settings.watermark.kind === 'none'} on:click={() => changeWatermark('none')}>{$t("关闭")}</button>
          <button class:active={settings.watermark.kind === 'text'} aria-pressed={settings.watermark.kind === 'text'} on:click={() => changeWatermark('text')}><Type size={13} />{$t("文字")}</button>
          <button class:active={settings.watermark.kind === 'image'} aria-pressed={settings.watermark.kind === 'image'} on:click={() => changeWatermark('image')}><ImageIcon size={13} />PNG</button>
        </div>
        {#if settings.watermark.kind === 'none'}<p class="watermark-off">{$t("给作品加上你的签名或 Logo")}</p>
        {:else}
          {#if settings.watermark.kind === 'text'}
            <label class="field-label" for="watermark-text">{$t("署名文字")}</label>
            <input id="watermark-text" class="full-width" bind:value={settings.watermark.text} maxlength="160" placeholder={$t("输入你的署名")} />
            <p class="field-help">{settings.watermark.fontFamily || (settings.watermark.fontPath ? basename(settings.watermark.fontPath) : $t("内置思源黑体 · 支持中文"))}</p>
          {:else}
            <button class="watermark-file" disabled={!desktop} on:click={chooseWatermarkImage}><ImagePlus size={16} /><span>{settings.watermark.imagePath ? basename(settings.watermark.imagePath) : $t("选择 PNG 水印")}</span><ChevronDown size={13} /></button>
            <p class="field-help">{$t("支持透明背景，保留 Logo 原始比例")}</p>
          {/if}
          <details class="advanced-watermark" on:toggle={(event) => { if (event.currentTarget.open) { void loadFonts(); if(!watermarkPreviewCollapsed)void ensureWatermarkWorkspace(); } }}>
            <summary><ChevronDown size={14} />{$t("字体、大小与精确位置")}</summary>
            {#if settings.watermark.kind === 'text'}
              <FontPicker {fonts} selected={settings.watermark.fontFamily} onchange={(font)=>{settings.watermark.fontPath=font?.path||null;settings.watermark.fontFamily=font?.family??null;}}/>
              {#if fontsLoading}<p class="field-help">{$t("正在读取本机字体…")}</p>{/if}
              {#if fontsError}<p class="field-error">{$t(fontsError)}</p><button class="text-button" on:click={loadFonts}>{$t("重新读取字体")}</button>{/if}
              <div class="font-row"><button class="file-button" disabled={!desktop} on:click={chooseFont}><span>{$t("导入 TTF / OTF / TTC 字体")}</span><Plus size={13} /></button><input class="color-picker" type="color" aria-label={$t("文字水印颜色")} bind:value={settings.watermark.color} /></div>
              <p class="field-help">{$t("缺失字形会由内置字体补齐；字体随预设在本机保留")}</p>
            {/if}
          <div class="field-title"><label class="field-label" for="watermark-size">{settings.watermark.kind === 'text' ? $t("文字大小") : $t("Logo 大小")}</label><div class="number-with-unit"><input aria-label={$t("水印大小百分比")} type="number" min={settings.watermark.kind === 'text' ? 0.5 : 2} max={settings.watermark.kind === 'text' ? 15 : 60} step="0.5" bind:value={settings.watermark.sizePercent}/><span>%</span></div></div>
          <input id="watermark-size" type="range" min={settings.watermark.kind === 'text' ? 0.5 : 2} max={settings.watermark.kind === 'text' ? 15 : 60} step="0.5" bind:value={settings.watermark.sizePercent} />
          <div class="field-title"><label class="field-label" for="watermark-opacity">{$t("透明度")}</label><output>{Math.round(settings.watermark.opacity * 100)}%</output></div>
          <input id="watermark-opacity" type="range" min="0" max="1" step="0.05" bind:value={settings.watermark.opacity} />
          <div class="two-fields"><label><span class="field-label">{$t("放置区域")}</span><select value={settings.watermark.region} on:change={(event) => changeRegion(event.currentTarget.value as WatermarkRegion)}><option value="photo">{$t("照片内")}</option><option value="border">{$t("下边框")}</option></select></label><label><span class="field-label">{$t("位置")}</span><select value={settings.watermark.position} on:change={(event) => { settings.watermark.position = event.currentTarget.value as WatermarkPosition; settings.watermark.offsetX = 0; settings.watermark.offsetY = 0; }}>{#each positions.filter((position) => settings.watermark.region === 'photo' || position.value.startsWith('bottom-')) as position}<option value={position.value}>{$t(position.label)}</option>{/each}</select></label></div>
          <div class="field-title"><label class="field-label" for="watermark-margin">{$t("安全边距")}</label><output>{settings.watermark.marginPercent}%</output></div><input id="watermark-margin" type="range" min="0" max="10" step="0.5" bind:value={settings.watermark.marginPercent} />
            <div class="offset-control"><div class="field-title"><label class="field-label" for="offset-x">{$t("水平偏移")}</label><div class="number-with-unit"><input type="number" min="-32768" max="32768" step="1" bind:value={settings.watermark.offsetX} aria-label={$t("水印水平偏移")}/><span>px</span></div></div>
            {#key `${offsetLimits?.minX}|${offsetLimits?.maxX}`}<input id="offset-x" type="range" aria-label={$t("水印水平偏移滑块")} min={Math.ceil(offsetLimits?.minX??-(selectedPhoto?.width??1000))} max={Math.floor(offsetLimits?.maxX??(selectedPhoto?.width??1000))} step="1" bind:value={settings.watermark.offsetX}/>{/key}</div>
            <div class="offset-control"><div class="field-title"><label class="field-label" for="offset-y">{$t("垂直偏移")}</label><div class="number-with-unit"><input type="number" min="-32768" max="32768" step="1" bind:value={settings.watermark.offsetY} aria-label={$t("水印垂直偏移")}/><span>px</span></div></div>
            {#key `${offsetLimits?.minY}|${offsetLimits?.maxY}`}<input id="offset-y" type="range" aria-label={$t("水印垂直偏移滑块")} min={Math.ceil(offsetLimits?.minY??-(selectedPhoto?.height??1000))} max={Math.floor(offsetLimits?.maxY??(selectedPhoto?.height??1000))} step="1" bind:value={settings.watermark.offsetY}/>{/key}</div>
            {#if offsetLimits}<button class="text-button" on:click={()=>settings.watermark.offsetY=Math.floor(offsetLimits!.maxY)}>{$t("贴近下沿")}</button>{/if}
            <p class="field-help">{$t("相对所选位置，正值向右 / 向下；按原图像素计算，缩图时同步缩放")}</p>
            <button class="text-button" on:click={() => { settings.watermark.offsetX = 0; settings.watermark.offsetY = 0; }}><RotateCcw size={12} />{$t("重置偏移")}</button>
          </details>
        {/if}
      </section>

      <section class="setting-section export-section">
        <button class="section-disclosure" on:click={() => advancedExport = !advancedExport} aria-expanded={advancedExport}><h2>{$t("导出设置")}</h2><span>{settings.export.format.toUpperCase()}<ChevronDown size={14} class={advancedExport ? 'turned' : ''} /></span></button>
        {#if advancedExport}
          <label class="field-label" for="export-format">{$t("文件格式")}</label><select id="export-format" bind:value={settings.export.format}><option value="jpg">JPG</option><option value="png">PNG</option></select>
          {#if settings.export.format === 'jpg'}<div class="field-title"><label class="field-label" for="jpg-quality">{$t("JPG 质量")}</label><output>{settings.export.jpegQuality}</output></div><input id="jpg-quality" type="range" min="60" max="100" step="1" bind:value={settings.export.jpegQuality} />{/if}
          <label class="field-label" for="long-edge">{$t("输出长边")}</label><select id="long-edge" value={longEdgeMode} on:change={(event) => setLongEdge(event.currentTarget.value)}><option value="original">{$t("保留原始像素")}</option><option value="2560">{$t("最多 2560 px")}</option><option value="3840">{$t("最多 3840 px")}</option><option value="custom">{$t("自定义")}</option></select>
          {#if longEdgeMode === 'custom'}<div class="custom-edge"><input aria-label={$t("自定义输出长边")} type="number" min="64" max="32768" step="1" bind:value={customLongEdge} on:input={(event) => settings.export.longEdge = event.currentTarget.valueAsNumber} /><span>px</span></div>{/if}
          <p class="field-help">{$t("按最终画布缩小，不放大照片；统一导出为 sRGB")}</p>
        {:else}<p class="field-help">{settings.export.longEdge ? $t("长边最多 {0} px", {0:settings.export.longEdge}) : $t("保留原始像素")} · {settings.export.format === 'jpg' ? $t("质量 {0}", {0:settings.export.jpegQuality}) : $t("无损 PNG")}</p>{/if}
      </section>
      <div class="privacy-note"><span class="privacy-dot"></span>{$t("加框在本机 · 智能配框按需联网")}</div>
    </aside>
  </div>

  <footer class="app-footer">
    {#if batch && totalCount > 0}<div class="batch-track" role="progressbar" aria-label={$t("批量导出进度")} aria-valuemin="0" aria-valuemax="100" aria-valuenow={Math.round(progress)}><div style:width={`${progress}%`}></div></div>{/if}
    <div class="footer-destination"><button class="folder-button" disabled={!desktop || isRunning} on:click={chooseOutputDirectory} title={outputDirectory || $t("选择导出文件夹")}><FolderOpen size={17} /><span>{outputDirectory ? basename(outputDirectory) : $t("选择导出文件夹")}</span><ChevronDown size={12} /></button><span class="originals-note">{$t("另存副本，保留原图")}</span></div>
    <div class="footer-actions">
      {#if batch && totalCount > 0}
        <button class="batch-summary text-button" on:click={() => jobDialog?.showModal()} title={$t("查看处理记录")}><List size={14} />{#if isRunning}<span>{completedCount + failedCount} / {totalCount}</span>{:else}<span>{completedCount} {$t("张已导出")}</span>{/if}{#if failedCount}<span class="failure-count">{failedCount} {$t("张失败")}</span>{/if}</button>
        {#if failedCount && !isRunning}<button class="button secondary compact" disabled={starting} title={$t("沿用本批次开始时的设置")} on:click={() => continueBatch('retry_failed')}>{$t("重试失败项")}</button>{/if}
      {/if}
      {#if isRunning}<button class="button secondary" disabled={cancelling} on:click={cancelBatch}><X size={15} />{cancelling ? $t("正在停止") : $t("取消导出")}</button>
      {:else}<button class="button primary export-button" on:click={startBatch} disabled={!desktop || !initialized || !photos.length || !!localError || !!previewError || starting || importing}>{#if starting}<LoaderCircle size={16} class="spin" />{:else}<Download size={17} />{/if}{starting ? $t("准备导出") : $t("导出{0}", {0:photos.length ? $t(" {0} 张", {0:photos.length}) : $t(" 照片")})}</button>{/if}
    </div>
  </footer>
  {#if dragActive}<div class="drop-overlay"><div><ImagePlus size={30} strokeWidth={1.5} /><strong>{$t(dragCount?'松开以添加 {0} 张照片':'松开以添加一组照片',{0:dragCount})}</strong><span>JPG · PNG · HEIC · HEIF</span><span>{$t(importing||loading?'会自动排队，随后逐张导入':'支持多选拖入，原照片保留在原位置')}</span></div></div>{/if}
</div>

<dialog bind:this={jobDialog} class="job-dialog">
  <header><div><h2>{$t("处理记录")}</h2><p>{completedCount} {$t("张已导出")}{failedCount ? $t("，{0} 张失败", {0:failedCount}) : ''}{isRunning ? $t(" · 正在处理") : ''} {$t("· 沿用本批次开始时的设置")}</p></div><button class="icon-button" aria-label={$t("关闭处理记录")} on:click={() => jobDialog.close()}><X size={18} /></button></header>
  <div class="job-list">{#each batch?.items ?? [] as item}<div class="job-row"><span class:failed={item.status === 'failed'} class:done={item.status === 'completed'} class="job-icon">{#if item.status === 'completed'}<CheckCircle2 size={16} />{:else if item.status === 'failed'}<AlertCircle size={16} />{:else if item.status === 'running'}<LoaderCircle size={16} class="spin" />{:else}<FileImage size={16} />{/if}</span><div><strong>{item.name}</strong><p>{$t(item.error??'') || (item.width && item.height ? `${item.width} × ${item.height} px` : $t(jobLabels[item.status]))}</p></div>{#if item.outputPath && item.status === 'completed'}<button class="text-button" on:click={() => item.outputPath && revealPath(item.outputPath)}>{$t("打开")}</button>{:else}<span class="job-state">{$t(jobLabels[item.status])}</span>{/if}</div>{/each}</div>
  <footer><span>{$t("每张照片独立处理，失败不影响其他照片")}</span>{#if batch?.outputDirectory}<button class="button secondary compact" on:click={() => batch && revealPath(batch.outputDirectory)}><FolderOpen size={14} />{$t("打开文件夹")}</button>{/if}</footer>
</dialog>

{#if modelOpen && selectedPhoto}<ModelPanel bind:initialMode={modelMode} bind:imageTarget={modelTarget} initialConfig={stagedModelConfig} onconfigure={configureModels} photo={selectedPhoto} count={photos.length} baseSettings={settings} onsavetemplate={saveSuggestedFrame} bind:apiKey={sessionApiKey} bind:apiOrigin={sessionApiOrigin} bind:imageApiKey={sessionImageApiKey} bind:imageApiOrigin={sessionImageApiOrigin} onapply={applyArt} onclose={()=>{modelOpen=false;stagedModelConfig=null;}} />{/if}
{#if modelsConfigOpen}<ModelsConfig initialConfig={stagedModelConfig} bind:apiKey={sessionApiKey} bind:apiOrigin={sessionApiOrigin} bind:imageApiKey={sessionImageApiKey} bind:imageApiOrigin={sessionImageApiOrigin} onsaved={value=>stagedModelConfig=clone(value)} onclose={closeModelConfig}/>{/if}

{#if templatesOpen}<FrameTemplates photo={selectedPhoto} currentPreview={preview?.dataUrl||''} currentSettings={settings} templates={frameTemplates} draft={templateDraft} initialName={templateName} onsave={saveFrameTemplate} ondelete={deleteFrameTemplate} onapply={applyFrameTemplate} onclose={()=>templatesOpen=false}/>{/if}
