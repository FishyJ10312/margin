<script lang="ts">
  import {t} from './lib/i18n';
  import { onMount, onDestroy, tick } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow';
  import { open } from '@tauri-apps/plugin-dialog';
  import {
    ImagePlus, Plus, Download, FolderOpen, ArrowLeftRight, X, Check,
    LoaderCircle, AlertCircle, CheckCircle2, Bookmark, Trash2, RotateCcw, RotateCw, Plug,
    Image as ImageIcon, Type, ChevronDown, FileImage, List, Pipette, WandSparkles, SlidersHorizontal, Languages,
  } from '@lucide/svelte';
  import ModelPanel from './lib/ModelPanel.svelte';
  import ModelsConfig from './lib/ModelsConfig.svelte';
  import PatternSwatch from './lib/PatternSwatch.svelte';
  import PhotoViewport from './lib/PhotoViewport.svelte';
  import FontPicker from './lib/FontPicker.svelte';
  import WatermarkDetail from './lib/WatermarkDetail.svelte';
  import FrameTemplates from './lib/FrameTemplates.svelte';
  import PhotoLibraryActions from './lib/PhotoLibraryActions.svelte';
  import AgentConnection from './lib/AgentConnection.svelte';
  import {patchAgentSettings,absoluteLocalPath,catalogIds,readOnlyAgentTools,type AgentStatus,type AgentCommand} from './lib/agent-settings';
  import {singlePhotoSelection,allPhotoSelection,clearPhotoSelection,restoredPhotoSelection,selectCatalogPhoto,contextCatalogPhoto,selectionAfterRemoval,type PhotoSelection} from './lib/photo-selection';
  import {language} from './lib/i18n';
  import {brand} from './lib/brand';
  import {startupDeadline} from './lib/startup';
  import type {OffsetLimits,AiConfig} from './lib/types';
  import {
    defaultSettings,
    type AppSnapshot, type BatchSnapshot, type ImportResult, type Photo,
    type Preset, type PreviewResult, type RenderSettings, type WatermarkKind,
    type WatermarkPosition, type WatermarkRegion, type FontOption, type FrameSuggestion, type FrameDecoration, type ArtScene, type FrameTemplate, type BuiltinFrame,
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
  let frameTemplates:FrameTemplate[]=[],templatesOpen=false,templateDraft:RenderSettings|null=null,templateName='',templateCategory='all';
  function openTemplates(draft:RenderSettings|null=null,name='',category='all'){templateDraft=draft?clone(draft):null;templateName=name;templateCategory=category;templatesOpen=true;}
  async function saveFrameTemplate(name:string,style:RenderSettings){const saved=await invoke<FrameTemplate>('save_frame_template',{name,settings:style});frameTemplates=[...frameTemplates,saved];}
  async function deleteFrameTemplate(id:string){await invoke('delete_frame_template',{id});frameTemplates=frameTemplates.filter(t=>t.id!==id);}
  function applyFrameTemplate(style:RenderSettings){previousArt={ratio:clone(settings.ratio),frameColor:settings.frameColor,borderPercent:settings.borderPercent,decoration:clone(settings.decoration),artwork:settings.artwork?clone(settings.artwork):null};settings={...settings,ratio:clone(style.ratio),borderPercent:style.borderPercent,frameColor:style.frameColor,decoration:clone(style.decoration),artwork:style.artwork?clone(style.artwork):null};templatesOpen=false;modelOpen=false;syncControls();}
  function saveSuggestedFrame(suggestion:FrameSuggestion){openTemplates({...settings,frameColor:suggestion.frameColor,borderPercent:suggestion.borderPercent,decoration:clone(suggestion.decoration),artwork:clone(suggestion.artwork)},suggestion.title);}

  let photoSelection = singlePhotoSelection();
  let agentOpen=false,agentConnected=false,agentBusy=false,agentRegistered=false;
  type RemovalEntry={id:string;count:number;selection:PhotoSelection};
  type RemovalReceipt={id:string;count:number;photoIds:string[];catalogRevision?:number;photoSelection?:PhotoSelection};
  let removalUndo:RemovalEntry[]=[],removalRedo:RemovalEntry[]=[];
  let catalogRevision=0;
  let savedSelectionKey='',selectionSaving=false;
  let pendingSelectionSave:{selection:PhotoSelection;catalogRevision:number}|null=null;
  let undoing=false,undoError='';
  let historyAction:'undo'|'redo'|null=null;
  let removalNotice:{kind:'removed'|'undone';count:number}|null=null;
  $: selectedId = photoSelection.activeId;
  $: selectedIds = photoSelection.ids;
  $: selectedPhotoIds = new Set(selectedIds);
  function selectPhoto(id:string,event?:MouseEvent) {
    if(undoing||agentBusy)return;
    photoSelection=selectCatalogPhoto(photos.map(photo=>photo.id),photoSelection,id,{additive:event?.metaKey||event?.ctrlKey,range:event?.shiftKey});
  }
  function selectContextPhoto(id:string) {
    photoSelection=contextCatalogPhoto(photos.map(photo=>photo.id),photoSelection,id);
  }
  function selectAllPhotos(){photoSelection=allPhotoSelection(photos.map(photo=>photo.id),photoSelection);}
  function clearSelectedPhotos(){photoSelection=clearPhotoSelection(photoSelection);}
  let photoActions:PhotoLibraryActions;
  let batch: BatchSnapshot | null = null;
  let initialized = false;
  let loading = true;
  let importing = false;
  let choosingPhotos=false;
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
  let exportDialog: HTMLDialogElement;
  let exportScope:'selected'|'all'='selected';
  let exportError='';
  $: exportPhotoIds=photos.filter(photo=>exportScope==='all'||selectedPhotoIds.has(photo.id)).map(photo=>photo.id);
  function openExportDialog() {
    exportScope='selected';
    exportError='';
    exportDialog?.showModal();
  }
  let inspectorTab: 'frame' | 'watermark' = 'frame';
  function selectInspector(tab:'frame'|'watermark') {
    inspectorTab=tab;
    if(tab==='watermark')void loadFonts();
  }
  function inspectorKey(event:KeyboardEvent) {
    if(!['ArrowLeft','ArrowRight','Home','End'].includes(event.key))return;
    event.preventDefault();
    selectInspector(event.key==='Home'?'frame':event.key==='End'?'watermark':inspectorTab==='frame'?'watermark':'frame');
    document.getElementById(`inspector-${inspectorTab}`)?.focus();
  }
  function applyPlainFrame(color:string) {
    previousArt={ratio:clone(settings.ratio),frameColor:settings.frameColor,borderPercent:settings.borderPercent,decoration:clone(settings.decoration),artwork:settings.artwork?clone(settings.artwork):null};
    settings={...settings,frameColor:color,decoration:{...settings.decoration,kind:'none'},artwork:null};
    syncControls();
  }
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
  $: galleryRatios = ['1:1','4:5','3:2','16:9'].map(value=>{
    const [width,height]=value.split(':');
    const reverse=`${height}:${width}`;
    return ratioMode===reverse?reverse:value;
  });
  $: isRunning = batch?.status === 'running';
  $: completedCount = batch?.items.filter((item) => item.status === 'completed').length ?? 0;
  $: failedCount = batch?.items.filter((item) => item.status === 'failed').length ?? 0;
  $: pendingCount = batch?.items.filter((item) => item.status === 'pending' || item.status === 'cancelled').length ?? 0;
  $: totalCount = batch?.items.length ?? 0;
  $: progress = totalCount ? ((completedCount + failedCount) / totalCount) * 100 : 0;
  $: localError = $t(validateLocal(settings)) || $t(frameHexError);
  $: if (initialized && desktop && !agentBusy) scheduleSettingsSave(JSON.stringify(settings));
  $: if (initialized && desktop && !loading && !importing && !undoing && !agentBusy) queueSelectionSave(photoSelection,catalogRevision);
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
      catalogRevision=state.catalogRevision??0;
      photoSelection = state.photoSelection?restoredPhotoSelection(photos.map(p=>p.id),state.photoSelection):singlePhotoSelection(photos[0]?.id);
      removalUndo=state.catalogHistory?.undo??[];
      removalRedo=state.catalogHistory?.redo??[];
      const recent=removalRedo[removalRedo.length-1]??removalUndo[removalUndo.length-1];
      removalNotice=recent?{kind:removalRedo.length?'undone':'removed',count:recent.count}:null;
      savedSelectionKey=JSON.stringify({selection:photoSelection,catalogRevision});
      if (state.batch) acceptBatch(state.batch);
      outputDirectory = batch?.outputDirectory ?? '';
      notices = [...new Set([...notices, ...(state.warnings ?? [])])];
      syncControls();
      initialized = true;
      if(agentRegistered)void markAgentReady();
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
    const registerAgent = async () => {
      const off = await listen<AgentCommand>('agent-command',({payload})=>void handleAgentCommand(payload));
      if(disposed){off();return;}unlisteners.push(off);agentRegistered=true;
      const offError = await listen<string>('agent-status-error',({payload})=>notices=[...notices,payload]);
      if(disposed)offError();else unlisteners.push(offError);
      if(initialized)await markAgentReady();
    };
    // Accept drops before loading the workspace; importPaths queues early groups.
    // A progress-listener failure must not disable file import.
    void registerProgress().catch(error=>{if(!disposed)notices=[...notices,errorMessage(error)];});
    void registerAgent().catch(error=>{if(!disposed)notices=[...notices,errorMessage(error)];});
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
          if (request.revision === previewRevision && request.photoId === previewPhotoId && !validateLocal(settings)) {
            if(result.sourceThumbnail)photos=photos.map(p=>p.id===request.photoId?{...p,thumbnail:result.sourceThumbnail!}:p);
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
    if (!desktop || importing || choosingPhotos || agentBusy || !initialized || loading || undoing) return;
    choosingPhotos=true;
    try {
      const paths = await open({ multiple: true, directory: false, title: $t("选择照片"), filters: [{ name: $t("照片"), extensions: ['jpg', 'jpeg', 'png', 'heic', 'heif'] }] });
      if (paths) await importPaths(Array.isArray(paths) ? paths : [paths]);
    } catch (error) { notices = [...notices, errorMessage(error)]; }
    finally{choosingPhotos=false;}
  }

  function queueSelectionSave(selection:PhotoSelection,revision:number){
    const request={selection:clone(selection),catalogRevision:revision};
    const key=JSON.stringify(request);
    if(key===savedSelectionKey)return;
    savedSelectionKey=key;
    pendingSelectionSave=request;
    void drainSelectionSaves();
  }

  async function drainSelectionSaves(){
    if(selectionSaving)return;
    selectionSaving=true;
    try{
      while(pendingSelectionSave){
        const request=pendingSelectionSave;pendingSelectionSave=null;
        try{await invoke('save_photo_selection',request);}
        catch(error){notices=[...new Set([...notices,$t('无法保存照片选择：{0}',{0:errorMessage(error)})])];}
      }
    }finally{selectionSaving=false;}
  }

  async function importPaths(paths: string[], forAgent=false) {
    if (!desktop || !paths.length) return;
    if (!initialized || loading || importing || undoing || agentBusy&&!forAgent) { pendingImports = [...new Set([...pendingImports, ...paths])]; return; }
    importing = true;
    importingCount = paths.length;
    importReport = null;
    try {
      const result = await invoke<ImportResult>('import_photos', { paths });
      const existing = new Set(photos.map((photo) => photo.id));
      const added = result.photos.filter((photo) => !existing.has(photo.id));
      photos = [...photos, ...added];
      catalogRevision=result.catalogRevision??catalogRevision;
      if(added.length){removalRedo=[];undoError='';}
      if (!selectedId && photos.length) photoSelection = singlePhotoSelection(photos[0].id);
      if(result.photoSelection)photoSelection=restoredPhotoSelection(photos.map(p=>p.id),result.photoSelection);
      importReport = { added: added.length, skipped: Math.max(0, paths.length - added.length) };
      if (result.errors.length) notices = [...notices, ...result.errors];
      // Reselecting an existing source can restore OS file access without adding a duplicate.
      const current = photos.find((photo) => photo.id === selectedId);
      if (previewError && current && !result.errors.length && paths.includes(current.path)) {
        schedulePreview(selectedId, JSON.stringify(settings));
      }
      return result;
    } catch (error) { if(forAgent)throw error;notices = [...notices, errorMessage(error)]; }
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
    if (isRunning || starting || importing || undoing || !desktop) throw new Error($t('请等待当前操作完成后再移除照片'));
    const selection=clone(photoSelection);
    const receipt=await invoke<RemovalReceipt|null>('remove_photos', { ids,selection });
    if(!receipt)throw new Error($t('当前照片列表已变化，请重试'));
    removalUndo=[...removalUndo.slice(-19),{id:receipt.id,count:receipt.count,selection}];
    removalRedo=[];
    removalNotice={kind:'removed',count:receipt.count};
    undoError='';
    photoSelection=selectionAfterRemoval(photos.map(photo=>photo.id),photoSelection,ids);
    const removed=new Set(ids);
    photos = photos.filter((photo) => !removed.has(photo.id));
    catalogRevision=receipt.catalogRevision??catalogRevision;
    if(receipt.photoSelection)photoSelection=restoredPhotoSelection(photos.map(p=>p.id),receipt.photoSelection);
  }

  async function undoPhotoRemoval(){
    if(!desktop||!initialized||loading||isRunning||starting||importing||undoing||!removalUndo.length)return;
    const entry=removalUndo[removalUndo.length-1];
    undoing=true;historyAction='undo';undoError='';
    try{
      const result=await invoke<{photos:Photo[];idMap:Record<string,string>;catalogRevision?:number;photoSelection?:PhotoSelection}>('undo_photo_removal',{undoId:entry.id});
      photos=result.photos;
      photoSelection=result.photoSelection?restoredPhotoSelection(photos.map(photo=>photo.id),result.photoSelection):restoredPhotoSelection(photos.map(photo=>photo.id),entry.selection,result.idMap);
      catalogRevision=result.catalogRevision??catalogRevision;
      removalUndo=removalUndo.slice(0,-1);
      removalRedo=[...removalRedo,entry];
      removalNotice={kind:'undone',count:entry.count};
      await tick();
      document.querySelector<HTMLElement>('.filmstrip .thumbnail.active')?.focus({preventScroll:true});
    }catch(error){undoError=errorMessage(error);}
    finally{
      undoing=false;historyAction=null;
      if(pendingImports.length){const paths=pendingImports;pendingImports=[];void importPaths(paths);}
    }
  }

  async function redoPhotoRemoval(){
    if(!desktop||!initialized||loading||isRunning||starting||importing||undoing||!removalRedo.length)return;
    const entry=removalRedo[removalRedo.length-1],selection=clone(photoSelection);
    undoing=true;historyAction='redo';undoError='';
    try{
      const receipt=await invoke<RemovalReceipt>('redo_photo_removal',{redoId:entry.id,selection});
      photoSelection=selectionAfterRemoval(photos.map(photo=>photo.id),selection,receipt.photoIds);
      const removed=new Set(receipt.photoIds);
      photos=photos.filter(photo=>!removed.has(photo.id));
      catalogRevision=receipt.catalogRevision??catalogRevision;
      if(receipt.photoSelection)photoSelection=restoredPhotoSelection(photos.map(p=>p.id),receipt.photoSelection);
      removalRedo=removalRedo.slice(0,-1);
      removalUndo=[...removalUndo,{id:receipt.id,count:receipt.count,selection}];
      removalNotice={kind:'removed',count:receipt.count};
      await tick();
      document.querySelector<HTMLElement>('.filmstrip .thumbnail.active, .empty-state button')?.focus({preventScroll:true});
    }catch(error){undoError=errorMessage(error);}
    finally{
      undoing=false;historyAction=null;
      if(pendingImports.length){const paths=pendingImports;pendingImports=[];void importPaths(paths);}
    }
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
    if (!desktop || !initialized || loading || isRunning || starting || importing || undoing || !exportPhotoIds.length || localError || previewError) return;
    // Fix the range and settings before the asynchronous directory chooser.
    const photoIds=[...exportPhotoIds],batchSettings=clone(settings);
    starting = true;
    exportError='';
    try {
      if (!outputDirectory) await chooseOutputDirectory();
      if (!outputDirectory) return;
      const result = await invoke<BatchSnapshot>('start_batch', { outputDirectory, settings: batchSettings, photoIds });
      acceptBatch(result);
      exportDialog?.close();
    } catch (error) { exportError=errorMessage(error); }
    finally { starting = false; }
  }

  async function cancelBatch() {
    if (cancelling) return;
    cancelling = true;
    try { await invoke('cancel_batch'); }
    catch (error) { cancelling = false; notices = [...notices, errorMessage(error)]; }
  }

  async function continueBatch(command: 'resume_batch' | 'retry_failed') {
    if (isRunning || starting || undoing) return;
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

  async function markAgentReady(){
    await invoke('agent_window_ready',{ready:true});
    const state=await invoke<AgentStatus>('get_agent_status');agentConnected=state.enabled;
  }

  function agentWorkspace(args:Record<string,unknown>={}){
    const offset=typeof args.offset==='number'?args.offset:0,limit=typeof args.limit==='number'?args.limit:200;
    return {photoCount:photos.length,photos:photos.slice(offset,offset+limit).map(({id,name,path,width,height})=>({id,name,path,width,height})),
      nextOffset:offset+limit<photos.length?offset+limit:null,selection:clone(photoSelection),settings:clone(settings),
      history:{undoCount:removalUndo.length,redoCount:removalRedo.length},importing,importingCount,pendingImportCount:pendingImports.length,
      export:batch?{id:batch.id,status:batch.status,total:batch.items.length,completed:completedCount,failed:failedCount}:null};
  }

  async function agentWait(ready:()=>boolean,milliseconds=15000){
    const end=Date.now()+milliseconds;
    while(!ready()){if(Date.now()>end)throw new Error($t('操作尚未完成，请查看主界面进度后重试'));await new Promise(resolve=>setTimeout(resolve,35));}
  }

  async function handleAgentCommand(command:AgentCommand){
    let result:unknown=null,problem:string|null=null,ownsMutation=false;
    try{
      if(!initialized||loading)throw new Error($t('照片窗口尚未就绪，请稍后重试'));
      const mutating=!readOnlyAgentTools.has(command.name);
      if(mutating){
        if(agentBusy||importing||choosingPhotos||starting||undoing||pendingImports.length)throw new Error($t('请等待当前操作完成后再使用 Agent'));
        if(pickingColor||document.querySelector('dialog[open]:not(.agent-connection)'))throw new Error($t('请先完成或关闭当前对话框'));
        agentBusy=true;ownsMutation=true;await tick();
        clearTimeout(saveTimer);if(!validateLocal(settings))queuedSave=clone(settings);
        void drainSave();await agentWait(()=>!saveInFlight&&!queuedSave&&!selectionSaving&&!pendingSelectionSave);
      }
      const args=command.arguments,order=photos.map(photo=>photo.id);
      switch(command.name){
        case 'margin_get_workspace':result=agentWorkspace(args);break;
        case 'margin_get_export_status':result={batch:clone(batch),cancelling};break;
        case 'margin_list_fonts':result={fonts:await invoke<FontOption[]>('list_fonts')};break;
        case 'margin_list_templates':{
          const builtins=await invoke<BuiltinFrame[]>('list_builtin_frames');
          result={templates:[...builtins.map(({id,name,englishName,category,description})=>({id:'builtin:'+id,name,englishName,category,description})),...frameTemplates.map(({id,name})=>({id:'saved:'+id,name,category:'saved'}))]};break;
        }
        case 'margin_apply_template':{
          const id=String(args.id),builtin=id.startsWith('builtin:')?(await invoke<BuiltinFrame[]>('list_builtin_frames')).find(t=>'builtin:'+t.id===id):null;
          const style=builtin?.settings??frameTemplates.find(t=>'saved:'+t.id===id)?.settings;
          if(!style)throw new Error($t('模板已变化，请重新读取模板列表'));
          const next={...clone(settings),ratio:clone(style.ratio),borderPercent:style.borderPercent,frameColor:style.frameColor,decoration:clone(style.decoration),artwork:style.artwork?clone(style.artwork):null};
          const invalid=validateLocal(next);if(invalid)throw new Error(invalid);
          await invoke('save_settings',{settings:next});applyFrameTemplate(next);await tick();result=agentWorkspace();break;
        }
        case 'margin_import_photos':{
          if(isRunning)throw new Error($t('请等待当前导出完成'));
          const paths=(args.paths as unknown[]).map(absoluteLocalPath);
          const imported=await importPaths(paths,true);await tick();
          if(!imported)throw new Error($t('导入正在排队，请稍后查看照片列表'));
          result={added:imported.photos.length,errors:imported.errors,workspace:agentWorkspace()};break;
        }
        case 'margin_select_photos':{
          const mode=args.mode??'ids';let next:PhotoSelection;
          if(mode==='all')next=allPhotoSelection(order,photoSelection);
          else if(mode==='none')next=clearPhotoSelection(photoSelection);
          else{
            const ids=catalogIds(args.ids,order),activeId=typeof args.activeId==='string'?args.activeId:ids[0]??photoSelection.activeId;
            const anchorId=typeof args.anchorId==='string'?args.anchorId:activeId;
            if(activeId&&!order.includes(activeId)||anchorId&&!order.includes(anchorId)||ids.length&&!ids.includes(activeId))throw new Error($t('当前照片列表已变化，请重试'));
            next={ids,activeId,anchorId};
          }
          const saved=await invoke<boolean>('save_photo_selection',{selection:next,catalogRevision});
          if(!saved)throw new Error($t('当前照片列表已变化，请重试'));
          photoSelection=next;savedSelectionKey=JSON.stringify({selection:next,catalogRevision});await tick();result=agentWorkspace();break;
        }
        case 'margin_update_settings':{
          const fonts=await invoke<FontOption[]>('list_fonts');
          const next=patchAgentSettings(settings,args.patch,fonts),invalid=validateLocal(next);
          if(invalid)throw new Error(invalid);
          await invoke('save_settings',{settings:next});settings=next;syncControls();await tick();result=agentWorkspace();break;
        }
        case 'margin_get_preview':{
          if(agentBusy||importing)throw new Error($t('请等待当前操作完成后再使用 Agent'));
          if(!selectedPhoto)throw new Error($t('请先选择一张照片。'));
          await tick();await agentWait(()=>!!previewError||!previewLoading&&!previewInFlight&&!queuedPreview&&!!preview,45000);
          if(previewError||!preview)throw new Error(previewError||$t('预览暂不可用'));
          result={dataUrl:preview.dataUrl,width:preview.width,height:preview.height,photoId:selectedId,photoBounds:preview.photoBounds,warning:preview.warning??null};break;
        }
        case 'margin_export_photos':{
          if(isRunning)throw new Error($t('请等待当前导出完成'));
          const directory=absoluteLocalPath(args.outputDirectory),scope=args.scope??'selected';
          const ids=scope==='all'?order:scope==='ids'?catalogIds(args.ids,order):[...selectedIds];
          if(!ids.length)throw new Error($t('请先导入并选择要导出的照片。'));
          const started=await invoke<BatchSnapshot>('start_batch',{outputDirectory:directory,settings:clone(settings),photoIds:ids});
          outputDirectory=directory;acceptBatch(started);result={batch:clone(started),photoCount:ids.length};break;
        }
        case 'margin_cancel_export':await invoke('cancel_batch');cancelling=isRunning;result={requested:true,batch:clone(batch)};break;
        case 'margin_remove_photos':{
          const ids=args.ids===undefined?[...selectedIds]:catalogIds(args.ids,order);
          if(!ids.length)throw new Error($t('请先选择要移除的照片'));
          await removePhotos(ids);await tick();result=agentWorkspace();break;
        }
        case 'margin_undo_removal':
          if(isRunning)throw new Error($t('请等待当前导出完成'));
          if(!removalUndo.length)throw new Error($t('没有可撤销的移除记录'));
          await undoPhotoRemoval();if(undoError)throw new Error(undoError);await tick();result=agentWorkspace();break;
        case 'margin_redo_removal':
          if(isRunning)throw new Error($t('请等待当前导出完成'));
          if(!removalRedo.length)throw new Error($t('没有可重做的移除记录'));
          await redoPhotoRemoval();if(undoError)throw new Error(undoError);await tick();result=agentWorkspace();break;
        default:throw new Error('Unknown Margin agent tool');
      }
    }catch(error){problem=errorMessage(error);}
    finally{
      if(ownsMutation)agentBusy=false;
      await tick();
      // A timed-out/closed client can no longer receive the result; the visible
      // app still owns any work already started and its progress/undo history.
      await invoke('complete_agent_command',{requestId:command.requestId,result,error:problem}).catch(()=>{});
      if(ownsMutation&&pendingImports.length&&!importing){const paths=pendingImports;pendingImports=[];void importPaths(paths);}
    }
  }
</script>

<svelte:window on:keydown={(event) => {
  if (!agentBusy && (event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 'o' && !document.querySelector('dialog[open]')) {
    event.preventDefault();
    void choosePhotos();
  }
}} />

<div class:drop-active={dragActive} class="app-shell gallery-shell">
  <header class="app-header">
    <div class="header-leading"><div class="brand"><h1>{brand.name}</h1></div>
      <button class="button quiet compact import-button" on:click={choosePhotos} disabled={!desktop || importing || choosingPhotos || agentBusy || loading || !initialized}>
        {#if importing}<LoaderCircle size={15} class="spin" />{:else}<ImagePlus size={16} />{/if}
        {importing ? $t("正在导入") : $t("导入照片")}
      </button>
    </div>
    <div class="header-actions">
      <button type="button" class:agent-connected={agentConnected} class="icon-button agent-connect-trigger" aria-label={$t('Agent 连接')} title={$t('Agent 连接')} aria-expanded={agentOpen} disabled={!desktop} on:click={()=>agentOpen=true}><Plug size={16}/></button>
      <button type="button" class="icon-button model-config-trigger" aria-label={$t('模型配置')} title={$t('模型配置')} disabled={!desktop} on:click={()=>configureModels()}><SlidersHorizontal size={16}/></button>
      <div class="language-picker">
        <Languages size={16} aria-hidden="true" />
        <select id="interface-language" class="language-select" aria-label={$t("界面语言 / Interface language")} bind:value={$language}>
          <option value="en" lang="en" label="EN">English</option>
          <option value="zh-CN" lang="zh-CN" label="中">简体中文</option>
        </select>
        <ChevronDown class="language-chevron" size={12} aria-hidden="true" />
      </div>
      {#if isRunning}<button class="button secondary compact" disabled={cancelling} on:click={cancelBatch}><X size={15}/>{cancelling?$t('正在停止'):$t('取消导出')}</button>
      {:else}<button class="button primary compact export-button" on:click={openExportDialog} disabled={!desktop || !initialized || !selectedIds.length || !!localError || !!previewError || starting || importing || undoing || agentBusy}><Download size={16}/>{$t('导出选中 {0} 张',{0:selectedIds.length})}</button>{/if}
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

  {#if agentBusy}<div class="agent-activity" role="status"><LoaderCircle size={12} class="spin"/>{$t('Agent 正在更新照片…')}</div>{/if}
  <div class="workspace" inert={agentBusy}>
    <main class="content-area">
      <div class="viewer">
        {#if loading}
          <div class="empty-state"><LoaderCircle size={25} class="spin" /><p>{$t("正在载入工作区")}</p></div>
        {:else if selectedPhoto}
          {#key selectedPhoto.id}
            <PhotoViewport photo={selectedPhoto} {preview} picking={pickingColor} loading={previewLoading} oncolor={setFrameColor} onclose={closePicker}>
              <svelte:fragment slot="heading"><div class="file-heading"><span title={selectedPhoto.path}>{selectedPhoto.name}</span><span class="source-size">{selectedPhoto.width} × {selectedPhoto.height}</span><button type="button" class="remove-photo" title={$t('从目录中移除照片（Backspace / Delete）')} aria-label={$t(selectedIds.length>1?'移除选中的 {0} 张照片':'移除当前照片',{0:selectedIds.length})} aria-keyshortcuts="Backspace Delete" disabled={isRunning||starting||importing||!selectedIds.length} on:click={()=>photoActions.requestRemoval(selectedIds)}><Trash2 size={15}/><span>{$t(selectedIds.length>1?'移除 {0} 张':'移除照片',{0:selectedIds.length})}</span></button></div></svelte:fragment>
              {#if watermarkPreviewVisible}
                <WatermarkDetail photo={selectedPhoto} {settings} overview={preview} collapsed={watermarkPreviewCollapsed} oncollapse={collapseWatermarkPreview} onlimits={(value)=>offsetLimits=value}/>
              {/if}
            </PhotoViewport>
          {/key}
          <div class="viewer-bottomline" aria-live="polite">
            {#if previewError}<span class="preview-error"><AlertCircle size={14} />{$t(previewError)}</span>
            {:else if preview}<span><strong>{preview.width.toLocaleString()} × {preview.height.toLocaleString()}</strong> px</span><span>{$t(preview.warning || "本机处理")}</span>
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

    </main>

    <aside class="settings-panel" aria-label={$t("边框和水印设置")}>
      <div class="inspector-tabs" role="tablist" aria-label={$t('边框与水印')}>
        <button type="button" id="inspector-frame" role="tab" aria-controls="frame-panel" aria-selected={inspectorTab==='frame'} tabindex={inspectorTab==='frame'?0:-1} on:click={()=>selectInspector('frame')} on:keydown={inspectorKey}>{$t('边框')}</button>
        <button type="button" id="inspector-watermark" role="tab" aria-controls="watermark-panel" aria-selected={inspectorTab==='watermark'} tabindex={inspectorTab==='watermark'?0:-1} on:click={()=>selectInspector('watermark')} on:keydown={inspectorKey}>{$t('水印')}</button>
      </div>
      <div id="frame-panel" class="setting-section inspector-panel" role="tabpanel" tabindex="0" aria-labelledby="inspector-frame" hidden={inspectorTab!=='frame'}>
        <div class="field-title ratio-title"><span class="field-label">{$t('最终比例')}</span><button class="icon-button" disabled={!settings.ratio} on:click={swapRatio} title={$t('交换宽高')} aria-label={$t('交换宽高')}><ArrowLeftRight size={15}/></button></div>
        <div class="ratio-grid" role="group" aria-label={$t('最终比例')}>
          <button type="button" class:active={ratioMode==='auto'} aria-pressed={ratioMode==='auto'} on:click={()=>changeRatio('auto')}>{$t('自动')}</button>
          {#each galleryRatios as ratio}<button type="button" class:active={ratioMode===ratio} aria-pressed={ratioMode===ratio} on:click={()=>changeRatio(ratio)}>{ratio}</button>{/each}
          <button type="button" class:active={ratioMode==='custom'} aria-pressed={ratioMode==='custom'} on:click={()=>changeRatio('custom')}>{$t('自定义')}</button>
        </div>
        {#if ratioMode === 'custom'}<div class="custom-ratio"><label><span>{$t("宽")}</span><input type="number" min="0.01" step="0.1" value={customWidth} on:input={(event) => changeCustomRatio('width', event.currentTarget.valueAsNumber)} aria-label={$t("自定义比例宽")} /></label><span>:</span><label><span>{$t("高")}</span><input type="number" min="0.01" step="0.1" value={customHeight} on:input={(event) => changeCustomRatio('height', event.currentTarget.valueAsNumber)} aria-label={$t("自定义比例高")} /></label></div>{/if}
        <div class="field-title"><label class="field-label" for="border-range">{$t("边框比例")}</label><div class="number-with-unit"><input aria-label={$t("边框百分比")} type="number" min="0" max="30" step="0.5" bind:value={settings.borderPercent} /><span>%</span></div></div>
        <input id="border-range" type="range" min="0" max="30" step="0.5" bind:value={settings.borderPercent} />
        <p class="field-help">{$t("按照片短边计算最小留白；指定比例时，部分方向会增加留白")}</p>
        <label class="field-label color-label" for="frame-hex">{$t("边框颜色")}</label>
        <div class="color-palette" role="group" aria-label={$t('边框色盘')}>{#each palette.slice(0,8) as [color,name]}<button style:background={color} class:active={settings.frameColor.toUpperCase()===color} class="palette-swatch" aria-label={`${$t(name)} ${color}`} aria-pressed={settings.frameColor.toUpperCase()===color} title={$t(name)} on:click={()=>setFrameColor(color)}></button>{/each}</div>
        <div class="color-row">
          <input class="color-picker" type="color" aria-label={$t("选择边框颜色")} value={settings.frameColor} on:input={(event) => setFrameColor(event.currentTarget.value)} />
          <input id="frame-hex" class="hex-input" value={frameHex} maxlength="7" on:input={(event) => setFrameColor(event.currentTarget.value)} spellcheck="false" />
          <button bind:this={pickerButton} class="icon-button bordered" class:color-active={pickingColor} aria-label={$t("从照片取色")} aria-pressed={pickingColor} title={$t("从照片取色（带原像素放大镜）")} disabled={!preview || previewLoading || isRunning || importing} on:click={() => pickingColor = !pickingColor}><Pipette size={16} /></button>
        </div>
        <details class="extra-colors"><summary>{$t('更多颜色')}</summary><div class="color-palette" role="group" aria-label={$t('更多颜色')}>{#each palette.slice(8) as [color,name]}<button style:background={color} class:active={settings.frameColor.toUpperCase()===color} class="palette-swatch" aria-label={`${$t(name)} ${color}`} aria-pressed={settings.frameColor.toUpperCase()===color} title={$t(name)} on:click={()=>setFrameColor(color)}></button>{/each}</div></details>
        {#if frameHexError}<p class="field-error">{$t(frameHexError)}</p>{/if}
        <section class="quick-templates" aria-label={$t('常用模板')}>
          <div class="quick-templates-heading"><h2>{$t('常用模板')}</h2><button type="button" class="text-button" disabled={!desktop||!initialized||loading||importing||isRunning} on:click={()=>openTemplates()}>{$t('全部模板')}</button></div>
          <div class="quick-template-grid">
            <button type="button" aria-pressed={!settings.artwork&&settings.decoration.kind==='none'&&settings.frameColor==='#FFFFFF'} on:click={()=>applyPlainFrame('#FFFFFF')}><span class="template-sample plain">{#if selectedPhoto?.thumbnail}<img src={selectedPhoto.thumbnail} alt=""/>{:else}<ImageIcon size={20}/>{/if}</span><span>{$t('纯白边框')}</span></button>
            <button type="button" aria-pressed={!settings.artwork&&settings.decoration.kind==='none'&&settings.frameColor==='#F3ECDD'} on:click={()=>applyPlainFrame('#F3ECDD')}><span class="template-sample cream">{#if selectedPhoto?.thumbnail}<img src={selectedPhoto.thumbnail} alt=""/>{:else}<ImageIcon size={20}/>{/if}</span><span>{$t('奶油边框')}</span></button>
            <button type="button" disabled={!desktop||!initialized||loading||importing||isRunning} on:click={()=>openTemplates(null,'','film')}><span class="template-sample film">{#if selectedPhoto?.thumbnail}<img src={selectedPhoto.thumbnail} alt=""/>{:else}<ImageIcon size={20}/>{/if}</span><span>{$t('胶片外框')}</span></button>
          </div>
        </section>
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
        {#if previousArt}<button class="text-button" on:click={undoArt}><RotateCcw size={12} />{$t('撤销上次配框')}</button>{/if}
      </div>

      <div id="watermark-panel" class="setting-section watermark-section inspector-panel" role="tabpanel" tabindex="0" aria-labelledby="inspector-watermark" hidden={inspectorTab!=='watermark'}>
        <div class="segmented" role="group" aria-label={$t("水印类型")}>
          <button class:active={settings.watermark.kind === 'none'} aria-pressed={settings.watermark.kind === 'none'} on:click={() => changeWatermark('none')}>{$t("关闭")}</button>
          <button class:active={settings.watermark.kind === 'text'} aria-pressed={settings.watermark.kind === 'text'} on:click={() => changeWatermark('text')}><Type size={13} />{$t("文字")}</button>
          <button class:active={settings.watermark.kind === 'image'} aria-pressed={settings.watermark.kind === 'image'} on:click={() => changeWatermark('image')}><ImageIcon size={13} />PNG</button>
        </div>
        {#if settings.watermark.kind === 'none'}<p class="watermark-off">{$t("给作品加上你的签名或 Logo")}</p>
        {:else}
          {#if settings.watermark.kind === 'text'}
            <label class="field-label" for="watermark-text">{$t("署名文字")}</label>
            <div class="watermark-text-row"><input id="watermark-text" class="full-width" bind:value={settings.watermark.text} maxlength="160" placeholder={$t("输入你的署名")} /><input class="color-picker" type="color" aria-label={$t('文字水印颜色')} bind:value={settings.watermark.color}/></div>
          {:else}
            <button class="watermark-file" disabled={!desktop} on:click={chooseWatermarkImage}><ImagePlus size={16} /><span>{settings.watermark.imagePath ? basename(settings.watermark.imagePath) : $t("选择 PNG 水印")}</span><ChevronDown size={13} /></button>
            <p class="field-help">{$t("支持透明背景，保留 Logo 原始比例")}</p>
          {/if}
          <div class="watermark-size-group">
            <div class="field-title"><label class="field-label" for="watermark-size">{settings.watermark.kind === 'text' ? $t('文字大小') : $t('Logo 大小')}</label><div class="number-with-unit"><input aria-label={$t('水印大小百分比')} type="number" min={settings.watermark.kind === 'text' ? 0.5 : 2} max={settings.watermark.kind === 'text' ? 15 : 60} step="0.5" bind:value={settings.watermark.sizePercent}/><span>%</span></div></div>
            <input id="watermark-size" type="range" min={settings.watermark.kind === 'text' ? 0.5 : 2} max={settings.watermark.kind === 'text' ? 15 : 60} step="0.5" bind:value={settings.watermark.sizePercent}/>
          </div>
          <div class="font-settings">
            {#if settings.watermark.kind === 'text'}
              <FontPicker {fonts} selected={settings.watermark.fontFamily} onchange={(font)=>{settings.watermark.fontPath=font?.path||null;settings.watermark.fontFamily=font?.family??null;}}/>
              {#if fontsLoading}<p class="field-help">{$t("正在读取本机字体…")}</p>{/if}
              {#if fontsError}<p class="field-error">{$t(fontsError)}</p><button class="text-button" on:click={loadFonts}>{$t("重新读取字体")}</button>{/if}
              <div class="font-row"><button class="file-button" disabled={!desktop} on:click={chooseFont}><span>{$t("导入 TTF / OTF / TTC 字体")}</span><Plus size={13} /></button></div>
            {/if}
          </div>
          <details class="advanced-watermark position-details">
            <summary><ChevronDown size={14}/>{$t('位置与透明度')}</summary>
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
      </div>

      <div class="inspector-bottom"><button class="button secondary compact art-trigger" disabled={!desktop||!selectedPhoto||importing||isRunning} on:click={()=>{modelMode='image';modelTarget='frame';modelOpen=true;}}><WandSparkles size={15}/>{$t('艺术边框')}</button></div>
    </aside>
  </div>

  <footer class="library-footer">
    {#if isRunning && totalCount > 0}<div class="batch-track" role="progressbar" aria-label={$t("批量导出进度")} aria-valuemin="0" aria-valuemax="100" aria-valuenow={Math.round(progress)}><div style:width={`${progress}%`}></div></div>{/if}
    <section class="filmstrip-section" inert={agentBusy} aria-label={$t('已导入的照片')}>
      <div class="filmstrip" role="group" aria-label={$t('照片选择')} aria-describedby="photo-selection-hint" aria-keyshortcuts="Meta+A Control+A Escape Meta+Z Control+Z Meta+Shift+Z Control+Shift+Z">
        {#each photos as photo (photo.id)}
          {@const job = batch?.items.find((item) => item.photoId === photo.id)}
          <button class:selected={selectedPhotoIds.has(photo.id)} class:active={photo.id===selectedId} class="thumbnail" data-photo-id={photo.id} on:click={event=>selectPhoto(photo.id,event)} on:contextmenu={event=>photoActions.openPhotoMenu(event,photo.id)} aria-label={$t('预览 {0}',{0:photo.name})} aria-pressed={selectedPhotoIds.has(photo.id)} aria-current={photo.id===selectedId?'true':undefined} aria-haspopup="menu" title={photo.name}>
            {#if photo.thumbnail}<img src={photo.thumbnail} alt="" draggable="false"/>{:else}<FileImage size={22}/>{/if}
            {#if job?.status==='completed'}<span class="thumbnail-status success"><Check size={10} strokeWidth={3}/></span>{:else if job?.status==='failed'}<span class="thumbnail-status failed"><AlertCircle size={11}/></span>{:else if job?.status==='running'}<span class="thumbnail-status"><LoaderCircle size={11} class="spin"/></span>{/if}
          </button>
        {/each}
        <button class="add-tile" on:click={choosePhotos} disabled={!desktop||importing||loading||!initialized} aria-label={$t('添加更多照片')}><Plus size={18}/></button>
      </div>
      <div class="library-summary"><strong>{photos.length} {$t('张照片',{count:photos.length})}{#if photos.length}<span class="selection-count" role="status"> · {$t('已选 {0} 张',{0:selectedIds.length})}</span>{/if}</strong><span id="photo-selection-hint" title={$t('先单击起点，再按住 Shift 点击终点；⌘ / Ctrl+A 全选，Esc 取消选择')}>{$t(photos.length>1?'Shift 连选 · ⌘ / Ctrl+A 全选 · Esc 取消':'全部照片共用当前设置')}</span></div>
      <div class="library-actions">
      {#if batch && totalCount > 0}
        <button class="batch-summary text-button" on:click={() => jobDialog?.showModal()} title={$t("查看处理记录")}><List size={14} />{#if isRunning}<span>{completedCount + failedCount} / {totalCount}</span>{:else}<span>{completedCount} {$t("张已导出")}</span>{/if}{#if failedCount}<span class="failure-count">{failedCount} {$t("张失败")}</span>{/if}</button>
        {#if failedCount && !isRunning}<button class="button secondary compact" disabled={starting} title={$t("沿用本批次开始时的设置")} on:click={() => continueBatch('retry_failed')}>{$t("重试失败项")}</button>{/if}
      {/if}
        <button type="button" class="export-options" on:click={openExportDialog} disabled={!desktop||!initialized||!photos.length||isRunning||starting||importing||undoing} aria-label={$t('导出选项')}>{settings.export.format.toUpperCase()} · {settings.export.format==='jpg'?settings.export.jpegQuality:$t('无损 PNG')} · {settings.export.longEdge?`${settings.export.longEdge} px`:$t('保留原始像素')}<ChevronDown size={12}/></button>
        {#if photos.length}<button type="button" class="icon-button clear-photos" aria-label={$t('清空列表')} title={$t('清空列表')} disabled={isRunning||starting||importing||undoing} on:click={()=>photoActions.requestRemoval(photos.map(photo=>photo.id))}><Trash2 size={16}/></button>{/if}
      </div>
    </section>
    {#if removalUndo.length||removalRedo.length||undoError}<div class="catalog-undo"><span role={undoError?'alert':'status'}>{undoError?$t(undoError):removalNotice?$t(removalNotice.kind==='undone'?'已恢复 {0} 张照片，原图保留':'已从目录移除 {0} 张照片，原图保留',{0:removalNotice.count}):''}</span><div class="catalog-history-actions">
      {#if removalUndo.length}<button type="button" class="text-button undo-photo-removal" disabled={undoing||loading||isRunning||starting||importing||agentBusy} aria-keyshortcuts="Meta+Z Control+Z" title={$t('撤销最近一次移除，重启后仍可用（⌘ / Ctrl+Z）')} on:click={undoPhotoRemoval}><RotateCcw size={14}/>{historyAction==='undo'?$t('正在恢复照片…'):$t('撤销移除')}</button>{/if}
      {#if removalRedo.length}<button type="button" class="text-button redo-photo-removal" disabled={undoing||loading||isRunning||starting||importing||agentBusy} aria-keyshortcuts="Meta+Shift+Z Control+Shift+Z" title={$t('重新执行刚撤销的移除（⌘ / Ctrl+Shift+Z）')} on:click={redoPhotoRemoval}><RotateCw size={14}/>{historyAction==='redo'?$t('正在重做移除…'):$t('重做移除')}</button>{/if}
    </div></div>{/if}
    <div class="import-feedback" role="status" aria-live="polite" aria-atomic="true">
      {#if importing}<LoaderCircle size={12} class="spin"/><span>{$t('正在导入 {0} 张照片…',{0:importingCount})}{#if pendingImports.length}{' · ' }{$t('另有 {0} 张排队',{0:pendingImports.length})}{/if}</span>
      {:else if pendingImports.length}<span>{$t('已接收 {0} 张照片，载入后导入',{0:pendingImports.length})}</span>
      {:else if importReport}<CheckCircle2 size={12}/><span>{$t('已添加 {0} 张照片',{0:importReport.added})}{#if importReport.skipped}{' · ' }{$t('跳过 {0} 个重复、不支持或无法读取的文件',{0:importReport.skipped})}{/if}</span>{/if}
    </div>
  </footer>
  {#if dragActive}<div class="drop-overlay"><div><ImagePlus size={30} strokeWidth={1.5} /><strong>{$t(dragCount?'松开以添加 {0} 张照片':'松开以添加一组照片',{0:dragCount})}</strong><span>JPG · PNG · HEIC · HEIF</span><span>{$t(importing||loading?'会自动排队，随后逐张导入':'支持多选拖入，原照片保留在原位置')}</span></div></div>{/if}
</div>

<PhotoLibraryActions bind:this={photoActions} {photos} {selectedIds} canUndo={!!removalUndo.length} onundo={()=>void undoPhotoRemoval()} canRedo={!!removalRedo.length} onredo={()=>void redoPhotoRemoval()} disabled={!desktop||!initialized||loading||isRunning||starting||importing||undoing||agentBusy} selectionDisabled={!desktop||!initialized||loading||starting||importing||pickingColor||undoing||agentBusy} onselectall={selectAllPhotos} onclearselection={clearSelectedPhotos} oncontextselect={selectContextPhoto} onremove={removePhotos} onerror={message=>notices=[...notices,message]}/>

{#if agentOpen}<AgentConnection onclose={()=>agentOpen=false} onstatus={status=>agentConnected=status.enabled}/>{/if}

<dialog bind:this={exportDialog} class="export-review" aria-labelledby="export-title" on:cancel={event=>{if(starting)event.preventDefault();}}>
  <header><div><h2 id="export-title">{$t('导出照片')}</h2><p class="export-count" aria-live="polite">{$t(exportScope==='selected'?'将导出选中的 {0} 张照片':'将导出全部 {0} 张照片',{0:exportPhotoIds.length})} · {$t('全部照片共用当前设置')}</p></div><button class="icon-button" aria-label={$t('返回编辑')} disabled={starting} on:click={()=>exportDialog.close()}><X size={18}/></button></header>
  <div class="export-review-body">
    <fieldset class="export-scope" disabled={starting||isRunning}>
      <legend class="field-label">{$t('导出范围')}</legend>
      <label><input type="radio" name="export-scope" value="selected" bind:group={exportScope} disabled={!selectedIds.length}/><span>{$t('选中的照片（{0} 张）',{0:selectedIds.length})}</span></label>
      <label><input type="radio" name="export-scope" value="all" bind:group={exportScope}/><span>{$t('全部照片（{0} 张）',{0:photos.length})}</span></label>
    </fieldset>
    <label class="field-label" for="export-directory">{$t('保存到')}</label>
    <button type="button" id="export-directory" class="export-directory" on:click={chooseOutputDirectory} disabled={!desktop||isRunning||starting} title={outputDirectory||$t('尚未选择文件夹')}><FolderOpen size={17}/><span>{outputDirectory||$t('尚未选择文件夹')}</span><ChevronDown size={14}/></button>
    <div class="export-format-row"><label><span class="field-label">{$t('文件格式')}</span><select id="export-format" bind:value={settings.export.format}><option value="jpg">JPG</option><option value="png">PNG</option></select></label>{#if settings.export.format==='jpg'}<div><div class="field-title"><label class="field-label" for="jpg-quality">{$t('JPG 质量')}</label><output>{settings.export.jpegQuality}</output></div><input id="jpg-quality" type="range" min="60" max="100" step="1" bind:value={settings.export.jpegQuality}/></div>{/if}</div>
    <details class="advanced-watermark export-size-options" bind:open={advancedExport}><summary><ChevronDown size={14}/>{$t('尺寸选项')}</summary>
      <label class="field-label" for="long-edge">{$t('输出长边')}</label><select id="long-edge" value={longEdgeMode} on:change={event=>setLongEdge(event.currentTarget.value)}><option value="original">{$t('保留原始像素')}</option><option value="2560">{$t('最多 2560 px')}</option><option value="3840">{$t('最多 3840 px')}</option><option value="custom">{$t('自定义')}</option></select>
      {#if longEdgeMode==='custom'}<div class="custom-edge"><input aria-label={$t('自定义输出长边')} type="number" min="64" max="32768" step="1" bind:value={customLongEdge} on:input={event=>settings.export.longEdge=event.currentTarget.valueAsNumber}/><span>px</span></div>{/if}<p class="field-help">{$t('按最终画布缩小，不放大照片；统一导出为 sRGB')}</p>
    </details>
    {#if preview}<div class="export-dimensions"><span>{$t('当前照片预计尺寸')}</span><span>{preview.width.toLocaleString()} × {preview.height.toLocaleString()} px</span></div>{/if}
    {#if localError||previewError}<p class="field-error" role="alert">{localError||previewError}</p>{/if}
    {#if exportError}<p class="field-error export-error" role="alert">{$t(exportError)}</p>{/if}
    <p class="field-help">{$t('另存副本，保留原图')}</p>
  </div>
  <footer><button class="button secondary" disabled={starting} on:click={()=>exportDialog.close()}>{$t('返回编辑')}</button><button type="button" class="button primary start-export" on:click={startBatch} disabled={!desktop||!initialized||!exportPhotoIds.length||!!localError||!!previewError||starting||importing||isRunning}>{#if starting}<LoaderCircle size={16} class="spin"/>{:else}<Download size={16}/>{/if}{starting?$t('准备导出'):$t(exportPhotoIds.length===1?'开始导出 1 张':'开始导出 {0} 张',{0:exportPhotoIds.length})}</button></footer>
</dialog>

<dialog bind:this={jobDialog} class="job-dialog">
  <header><div><h2>{$t("处理记录")}</h2><p>{completedCount} {$t("张已导出")}{failedCount ? $t("，{0} 张失败", {0:failedCount}) : ''}{isRunning ? $t(" · 正在处理") : ''} {$t("· 沿用本批次开始时的设置")}</p></div><button class="icon-button" aria-label={$t("关闭处理记录")} on:click={() => jobDialog.close()}><X size={18} /></button></header>
  <div class="job-list">{#each batch?.items ?? [] as item}<div class="job-row"><span class:failed={item.status === 'failed'} class:done={item.status === 'completed'} class="job-icon">{#if item.status === 'completed'}<CheckCircle2 size={16} />{:else if item.status === 'failed'}<AlertCircle size={16} />{:else if item.status === 'running'}<LoaderCircle size={16} class="spin" />{:else}<FileImage size={16} />{/if}</span><div><strong>{item.name}</strong><p>{$t(item.error??'') || (item.width && item.height ? `${item.width} × ${item.height} px` : $t(jobLabels[item.status]))}</p></div>{#if item.outputPath && item.status === 'completed'}<button class="text-button" on:click={() => item.outputPath && revealPath(item.outputPath)}>{$t("打开")}</button>{:else}<span class="job-state">{$t(jobLabels[item.status])}</span>{/if}</div>{/each}</div>
  <footer><span>{$t("每张照片独立处理，失败不影响其他照片")}</span>{#if batch?.outputDirectory}<button class="button secondary compact" on:click={() => batch && revealPath(batch.outputDirectory)}><FolderOpen size={14} />{$t("打开文件夹")}</button>{/if}</footer>
</dialog>

{#if modelOpen && selectedPhoto}<ModelPanel bind:initialMode={modelMode} bind:imageTarget={modelTarget} initialConfig={stagedModelConfig} onconfigure={configureModels} photo={selectedPhoto} count={photos.length} baseSettings={settings} onsavetemplate={saveSuggestedFrame} bind:apiKey={sessionApiKey} bind:apiOrigin={sessionApiOrigin} bind:imageApiKey={sessionImageApiKey} bind:imageApiOrigin={sessionImageApiOrigin} onapply={applyArt} onclose={()=>{modelOpen=false;stagedModelConfig=null;}} />{/if}
{#if modelsConfigOpen}<ModelsConfig initialConfig={stagedModelConfig} bind:apiKey={sessionApiKey} bind:apiOrigin={sessionApiOrigin} bind:imageApiKey={sessionImageApiKey} bind:imageApiOrigin={sessionImageApiOrigin} onsaved={value=>stagedModelConfig=clone(value)} onclose={closeModelConfig}/>{/if}

{#if templatesOpen}<FrameTemplates photo={selectedPhoto} currentPreview={preview?.dataUrl||''} currentSettings={settings} templates={frameTemplates} draft={templateDraft} initialName={templateName} initialCategory={templateCategory} onsave={saveFrameTemplate} ondelete={deleteFrameTemplate} onapply={applyFrameTemplate} onclose={()=>templatesOpen=false}/>{/if}
