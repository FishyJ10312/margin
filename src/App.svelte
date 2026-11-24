<script lang="ts">
  import {t} from './lib/i18n';
  import {actualPhotoSettings,adoptOwnedAssets,frameStyleEdit,photoEdit,type ExplicitSettingsEdit,type PhotoEditPolicy} from './lib/photo-settings';
  import { onMount, onDestroy, tick } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow';
  import { open } from '@tauri-apps/plugin-dialog';
  import {
    ImagePlus, Plus, Download, Keyboard, FolderOpen, ArrowLeftRight, X, Check,
    LoaderCircle, AlertCircle, CheckCircle2, Bookmark, Trash2, RotateCcw, RotateCw, Plug,
    Image as ImageIcon, Type, ChevronDown, FileImage, List, Pipette, WandSparkles, SlidersHorizontal, Languages, Aperture, SunMoon,
    Undo2, Redo2, LayoutGrid, Square, Copy, ClipboardPaste, Columns2, ChevronRight, Sparkles } from '@lucide/svelte';
  import OverviewGrid from './lib/OverviewGrid.svelte';
  import ShortcutSheet from './lib/ShortcutSheet.svelte';
  import {photoPalette} from './lib/photo-palette';
  import ModelPanel from './lib/ModelPanel.svelte';
  import ModelsConfig from './lib/ModelsConfig.svelte';
  import PatternSwatch from './lib/PatternSwatch.svelte';
  import PhotoViewport from './lib/PhotoViewport.svelte';
  import FontPicker from './lib/FontPicker.svelte';
  import WatermarkDetail from './lib/WatermarkDetail.svelte';
  import FrameTemplates from './lib/FrameTemplates.svelte';
  import PhotoLibraryActions from './lib/PhotoLibraryActions.svelte';
  import AgentConnection from './lib/AgentConnection.svelte';
  import {patchAgentSettings,explicitAgentEdit,absoluteLocalPath,catalogIds,readOnlyAgentTools,type AgentStatus,type AgentCommand} from './lib/agent-settings';
  import {singlePhotoSelection,allPhotoSelection,clearPhotoSelection,restoredPhotoSelection,selectCatalogPhoto,contextCatalogPhoto,selectionAfterRemoval,type PhotoSelection} from './lib/photo-selection';
  import {language} from './lib/i18n';
  import {brand} from './lib/brand';
  import {theme} from './lib/theme';
  import {startupDeadline} from './lib/startup';
  import type {OffsetLimits,AiConfig} from './lib/types';
  import {
    defaultSettings,
    type AppSnapshot, type BatchSnapshot, type ImportResult, type Photo,
    type Preset, type PreviewResult, type RenderSettings, type SettingsCommit, type WatermarkKind,
    type WatermarkPosition, type WatermarkRegion, type FrameBackground, defaultBackground, type FontOption, type FrameSuggestion, type FrameDecoration, type FrameTemplate, type BuiltinFrame, defaultCameraTemplate, type CameraInfo } from './lib/types';

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
  function applyFrameTemplate(style:RenderSettings, keepLayout=false, minimumPadding=0){
    const next={...settings,ratio:clone(style.ratio),borderPercent:style.borderPercent,frameColor:style.frameColor,decoration:clone(style.decoration),background:clone(style.background??defaultBackground),artwork:style.artwork?clone(style.artwork):null};
    performFrameEdit(next,frameStyleEdit(style,keepLayout),{keepLayout,minimumPadding});
    templatesOpen=false;modelOpen=false;
  }
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
    if(undoing||agentBusy||scopeBusy)return;
    stageSettings(settings);
    photoSelection=selectCatalogPhoto(photos.map(photo=>photo.id),photoSelection,id,{additive:event?.metaKey||event?.ctrlKey,range:event?.shiftKey});
  }
  function selectContextPhoto(id:string) {
    if(scopeBusy||undoing||agentBusy)return;
    stageSettings(settings);
    photoSelection=contextCatalogPhoto(photos.map(photo=>photo.id),photoSelection,id);
  }
  function stepPhoto(delta:number,extend:boolean){
    if(undoing||agentBusy||scopeBusy)return;
    const ids=photos.map(photo=>photo.id);
    if(!ids.length)return;
    const current=ids.indexOf(selectedId??'');
    const next=ids[Math.max(0,Math.min(ids.length-1,current<0?0:current+delta))];
    if(!next||(next===selectedId&&!extend))return;
    stageSettings(settings);
    photoSelection=selectCatalogPhoto(ids,photoSelection,next,{additive:false,range:extend});
    requestAnimationFrame(()=>{
      const thumb=document.querySelector<HTMLElement>(`.thumbnail[data-photo-id="${CSS.escape(next)}"]`);
      thumb?.scrollIntoView({block:'nearest',inline:'nearest'});
      if(document.activeElement?.closest('.thumbnail'))thumb?.focus({preventScroll:true});
    });
  }
  let sharedSettings = clone(defaultSettings);
  let editSelected = false;
  let saveBaseline = clone(settings);
  type PhotoUpdates = Record<string, RenderSettings | null>;
  let pendingPhotoSettings: PhotoUpdates = {};
  let pendingSharedSettings: RenderSettings | null = null;
  let savingSettings: Promise<boolean> | null = null;
  let scopeBusy = false;
  let inspectorSelectionKey = '';
  $: if (initialized && JSON.stringify(photoSelection) !== inspectorSelectionKey) loadInspector(photoSelection);
  $: scopedIds = selectedIds.length ? selectedIds : selectedId ? [selectedId] : [];
  $: hasScopedOverrides = photos.some(photo => photo.settings && scopedIds.includes(photo.id));
  $: sharedOverridePreview = !editSelected && !!selectedPhoto?.settings;
  $: frameAffectedCount = editSelected ? scopedIds.length : photos.filter(photo=>!photo.settings).length;

  function loadInspector(selection: PhotoSelection, scope?: boolean) {
    commitSettingsStep();
    inspectorSelectionKey = JSON.stringify(selection);
    const photo = photos.find(item => item.id === selection.activeId);
    editSelected = !!photo && (scope ?? (selection.ids.length > 1 || !!photo.settings));
    refreshInspector();
    cancelFrameTrial();
    previousArt = null;
    markBox = null;
    markDragStart = null;
    offsetLimits = null;
  }
  function refreshInspector() {
    const photo=photos.find(item=>item.id===photoSelection.activeId);
    settings=clone(editSelected&&photo?.settings ? {...photo.settings,export:sharedSettings.export} : sharedSettings);
    saveBaseline=clone(settings);
    syncControls();
  }

  function setSettingsScope(own: boolean) {
    if (scopeBusy || own === editSelected) return;
    stageSettings(settings);
    loadInspector(photoSelection, own);
  }

  async function restoreSharedSettings() {
    if (scopeBusy) return;
    const ids = new Set(scopedIds);
    scopeBusy = true;
    try {
      if (!(await flushSettingsSave())) return;
      const updates = Object.fromEntries(photos.filter(photo => photo.settings && ids.has(photo.id)).map(({id}) => [id, null]));
      if (!Object.keys(updates).length) return;
      commitSettingsStep();
      const previous = captureSettings(new Set(Object.keys(updates)));
      await invoke('set_photo_settings', { updates, shared: null });
      photos = photos.map(photo => ids.has(photo.id) ? { ...photo, settings: null } : photo);
      settings = clone(sharedSettings);
      saveBaseline = clone(settings);
      cancelFrameTrial();
      syncControls();
      settingsUndo = [...settingsUndo.slice(-49), previous];
      settingsRedo = [];
      previousArt=null;
      lastSettingsEditAt = Date.now();
    } catch (error) { notices = [...notices, errorMessage(error)]; }
    finally { scopeBusy = false; }
  }

  function selectAllPhotos(){stageSettings(settings);photoSelection=allPhotoSelection(photos.map(photo=>photo.id),photoSelection);}
  function clearSelectedPhotos(){stageSettings(settings);photoSelection=clearPhotoSelection(photoSelection);}

  // Workspace view: one photo at a time, or every framed photo at a glance.
  let workspaceView:'single'|'overview'='single';
  function openSingle(id:string){if(id!==selectedId)selectPhoto(id);workspaceView='single';}

  // Copy the active photo's frame and watermark, then paste them onto the
  // selection as independent settings. Export options stay shared.
  let copiedSettings:RenderSettings|null=null,copiedFrom='';
  function copySettings(){
    if(!selectedPhoto||scopeBusy)return;
    stageSettings(settings);
    copiedSettings=clone(actualPhotoSettings(settings,sharedSettings,selectedPhoto.settings,editSelected));
    copiedFrom=selectedPhoto.name;
  }
  async function pasteSettings(){
    if(!copiedSettings||scopeBusy||isRunning||!desktop)return;
    const ids=new Set(scopedIds);
    if(!ids.size)return;
    stageSettings(settings);
    scopeBusy=true;
    try{
      if(!(await flushSettingsSave()))return;
      const source={...clone(copiedSettings),export:clone(sharedSettings.export)};
      const invalid=validateLocal(source);
      if(invalid)throw new Error(invalid);
      const updates:PhotoUpdates=Object.fromEntries(photos.filter(photo=>ids.has(photo.id)&&!sameSettings(photo.settings??null,source)).map(photo=>[photo.id,clone(source)]));
      if(!Object.keys(updates).length)return;
      commitSettingsStep();
      const previous=captureSettings(new Set(Object.keys(updates)));
      const receipt=await invoke<SettingsCommit>('set_photo_settings',{updates,shared:null});
      photos=photos.map(photo=>Object.hasOwn(updates,photo.id)?{...photo,settings:receipt?.updates?.[photo.id]??updates[photo.id]}:photo);
      settingsUndo=[...settingsUndo.slice(-49),previous];
      settingsRedo=[];
      previousArt=null;
      lastSettingsEditAt=Date.now();
      loadInspector(photoSelection,true);
    }catch(error){notices=[...notices,errorMessage(error)];}
    finally{scopeBusy=false;}
  }

  // Frame colour suggestions from the active photo's catalogue thumbnail.
  let photoColors:string[]=[],paletteSource='';
  $: if((selectedPhoto?.thumbnail??'')!==paletteSource){
    const source=paletteSource=selectedPhoto?.thumbnail??'';
    photoColors=[];
    if(source)photoPalette(source).then(colors=>{if(paletteSource===source)photoColors=colors;}).catch(()=>{});
  }
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
  let lastPreviewKey = '';
  let saveTimer: ReturnType<typeof setTimeout> | undefined;
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
  let exportTargetId = '';
  let uniformExport = false;
  $: exportTargetRatio = exportTargets.find(target => target.id === exportTargetId)?.ratio ?? null;
  $: exportOwnCount = photos.filter(photo => photo.settings && (exportScope === 'all' || selectedPhotoIds.has(photo.id))).length;
  $: exportPhotoIds=photos.filter(photo=>exportScope==='all'||selectedPhotoIds.has(photo.id)).map(photo=>photo.id);
  function openExportDialog() {
    cancelFrameTrial();
    exportScope='selected';
    exportTargetId='';
    uniformExport=false;
    exportError='';
    exportDialog?.showModal();
  }
  let inspectorTab: 'frame' | 'watermark' = 'frame';
  function selectInspector(tab:'frame'|'watermark') {
    inspectorTab=tab;
    if(tab!=='frame')cancelFrameTrial();
    if(tab==='watermark')void loadFonts();
  }
  function inspectorKey(event:KeyboardEvent) {
    if(!['ArrowLeft','ArrowRight','Home','End'].includes(event.key))return;
    event.preventDefault();
    selectInspector(event.key==='Home'?'frame':event.key==='End'?'watermark':inspectorTab==='frame'?'watermark':'frame');
    document.getElementById(`inspector-${inspectorTab}`)?.focus();
  }
  const quickFrames = [
    { label: '纯白边框', style: 'plain' },
    { label: '细线卡纸', style: 'keyline' },
    { label: '胶片扫描', style: 'scan' },
  ];
  let quickStyles: RenderSettings[] = [];
  let loadingQuickStyles = false;
  let quickGrid: HTMLDivElement;
  let trialIndex = -1;
  let comparingOriginal = false;
  $: trialSettings = trialIndex >= 0 && quickStyles[trialIndex] ? {
    ...settings,
    frameColor: quickStyles[trialIndex].frameColor,
    borderPercent: trialIndex === 0 ? settings.borderPercent : Math.max(settings.borderPercent, quickStyles[trialIndex].borderPercent),
    decoration: quickStyles[trialIndex].decoration,
    background: clone(quickStyles[trialIndex].background ?? defaultBackground),
    artwork: quickStyles[trialIndex].artwork,
  } : null;
  $: effectiveSettings = actualPhotoSettings(settings,sharedSettings,selectedPhoto?.settings,editSelected);
  $: displayedSettings = trialSettings ?? effectiveSettings;

  async function loadQuickStyles() {
    if (loadingQuickStyles || quickStyles.length) return;
    loadingQuickStyles = true;
    try { quickStyles = await invoke<RenderSettings[]>('quick_frame_styles'); }
    catch { /* The full template library is still available. */ }
    finally { loadingQuickStyles = false; }
  }

  function cancelFrameTrial() {
    trialIndex = -1;
    comparingOriginal = false;
  }

  function applyTrial() {
    if (!trialSettings || previewLoading || previewError) return;
    const candidate = trialSettings;
    const floor=trialIndex===0?0:quickStyles[trialIndex]?.borderPercent??0;
    cancelFrameTrial();
    applyFrameTemplate(candidate,true,floor);
  }

  function quickFrameKey(event: KeyboardEvent, index: number) {
    if (event.key === 'Escape') { event.preventDefault(); event.stopPropagation(); cancelFrameTrial(); return; }
    const direction = event.key === 'ArrowLeft' || event.key === 'ArrowUp' ? -1
      : event.key === 'ArrowRight' || event.key === 'ArrowDown' ? 1 : 0;
    if (!direction || event.altKey || event.metaKey || event.ctrlKey || event.shiftKey) return;
    event.preventDefault();
    event.stopPropagation();
    trialIndex = (index + direction + quickFrames.length) % quickFrames.length;
    quickGrid.querySelectorAll('button')[trialIndex]?.focus();
  }

  function compareKey(event: KeyboardEvent, show: boolean) {
    if (event.key !== ' ' && event.key !== 'Enter') return;
    event.preventDefault();
    comparingOriginal = show;
  }

  let errorsExpanded = false;
  let pendingImports: string[] = [];
  let modelOpen = false;
  let modelsConfigOpen=false,returnToModel=false;
  let stagedModelConfig:AiConfig|null=null;
  let modelConfigTarget: 'analysis' | 'image' = 'analysis';
  function configureModels(current: AiConfig | null = null, target: 'analysis' | 'image' = 'analysis') {
    stagedModelConfig = current ? clone(current) : null;
    modelConfigTarget = target;
    returnToModel = modelOpen;
    modelOpen = false;
    modelsConfigOpen = true;
  }
  function closeModelConfig(){modelsConfigOpen=false;if(returnToModel&&selectedPhoto)modelOpen=true;else stagedModelConfig=null;returnToModel=false;}
  let modelMode:'design'|'refine'|'image'='design';
  let modelTarget:'frame'|'sticker'='frame';
  let sessionApiKey = '',sessionApiOrigin='';
  let sessionImageApiKey='',sessionImageApiOrigin='';
  let previousArt: SettingsEdit | null = null;
  const motifs: {kind:FrameDecoration['kind'];name:string}[]=[{kind:'none',name:'纯色'},{kind:'paper',name:'纸感'},{kind:'linen',name:'亚麻'},{kind:'dots',name:'点阵'},{kind:'lines',name:'细条纹'},{kind:'waves',name:'柔波纹'},{kind:'corners',name:'角标'}];
  function applyArt(suggestion:FrameSuggestion) {
    const next={...settings,frameColor:suggestion.frameColor,borderPercent:suggestion.borderPercent,decoration:clone(suggestion.decoration),artwork:clone(suggestion.artwork)};
    performFrameEdit(next,{frameColor:next.frameColor,borderPercent:next.borderPercent,decoration:next.decoration,artwork:next.artwork});
    modelOpen=false;stagedModelConfig=null;
  }
  function performFrameEdit(next:RenderSettings,explicit:ExplicitSettingsEdit,policy:PhotoEditPolicy={}){
    commitSettingsStep();
    settings=clone(next);
    if(stageSettings(settings,explicit,policy)){
      commitSettingsStep();
      previousArt=settingsUndo.at(-1)??null;
    }
    syncControls();
  }
  function undoArt() {
    if(previousArt&&!pendingUndo&&settingsUndo.at(-1)===previousArt)undoSettings();
    previousArt=null;
  }
  let pickingColor = false;
  let pickerButton: HTMLButtonElement;
  function closePicker() { pickingColor = false; pickerButton?.focus({ preventScroll: true }); }
  let offsetLimits:OffsetLimits|null=null;
  type MarkBox = {x:number; y:number; width:number; height:number};
  let markBox:MarkBox|null=null;
  let markDragStart: {x:number; y:number; box: MarkBox|null; scale: number; limits: OffsetLimits|null} | null = null;
  $: if(!watermarkPreviewVisible)markBox=null;
  function dragMark(dx: number, dy: number, phase: 'start'|'move'|'end') {
    if(sharedOverridePreview||trialSettings||undoing||agentBusy)return;
    if (phase === 'start') {
      const scale = preview && selectedPhoto?.width ? preview.photoBounds.width / selectedPhoto.width : 1;
      markDragStart = { x:settings.watermark.offsetX, y:settings.watermark.offsetY, box:markBox, scale, limits:offsetLimits };
      return;
    }
    const start = markDragStart;
    if (!start) return;
    const clamp = (value:number, min?:number, max?:number) => Math.round(Math.min(max ?? Infinity, Math.max(min ?? -Infinity, value)));
    settings.watermark.offsetX = clamp(start.x + dx / start.scale, start.limits?.minX, start.limits?.maxX);
    settings.watermark.offsetY = clamp(start.y + dy / start.scale, start.limits?.minY, start.limits?.maxY);
    if (phase === 'end') {
      if (start.box) markBox = { ...start.box,
        x: start.box.x + (settings.watermark.offsetX - start.x) * start.scale,
        y: start.box.y + (settings.watermark.offsetY - start.y) * start.scale };
      markDragStart = null;
    }
  }
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
  $: watermarkPreviewVisible = desktop && !!selectedPhoto && displayedSettings.watermark.kind !== 'none';
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
  $: if (initialized && desktop && !agentBusy && !undoing) stageSettings(settings);
  $: if (initialized && desktop && !loading && !importing && !undoing && !agentBusy) queueSelectionSave(photoSelection,catalogRevision);
  $: if (initialized) schedulePreview(selectedId, JSON.stringify(displayedSettings));
  $: if (settings.frameColor !== lastFrameColor) {
    lastFrameColor = settings.frameColor;
    frameHex = settings.frameColor;
    frameHexError = '';
  }

  type SettingsEdit = { inspector: RenderSettings; shared: RenderSettings; photos: PhotoUpdates };
  let settingsUndo: SettingsEdit[] = [], settingsRedo: SettingsEdit[] = [];
  let pendingUndo: SettingsEdit | null = null;
  let settingsStepTimer: ReturnType<typeof setTimeout> | undefined;
  let lastSettingsEditAt = 0, lastRemovalAt = 0, lastOrderAt = 0, lastUndoKind: 'settings'|'removal'|'order'|null = null;
  let orderUndo:{before:string[];after:string[]}[]=[], orderRedo:{before:string[];after:string[]}[]=[];
  let shortcutSheet:ShortcutSheet;
  let photoDrag:{ids:string[];x:number;y:number;pointer:number;active:boolean}|null=null, dropIndex=-1, suppressThumbClick=false;
  $: removalUndo,removalRedo,lastRemovalAt=Date.now();

  function captureSettings(ids: Set<string> = new Set()): SettingsEdit {
    return clone({
      inspector: saveBaseline, shared: sharedSettings,
      photos: Object.fromEntries(photos.filter(photo => ids.has(photo.id)).map(photo => [photo.id, photo.settings ?? null])),
    });
  }

  function trackSettings(ids: Set<string>) {
    if (!pendingUndo) pendingUndo = captureSettings(ids);
    else {
      const missing= new Set([...ids].filter(id=>!Object.hasOwn(pendingUndo!.photos,id)));
      if(missing.size)pendingUndo={...pendingUndo,photos:{...pendingUndo.photos,...captureSettings(missing).photos}};
    }
    previousArt=null;
    lastSettingsEditAt = Date.now();
    clearTimeout(settingsStepTimer);
    settingsStepTimer = setTimeout(commitSettingsStep, 400);
  }

  function commitSettingsStep() {
    clearTimeout(settingsStepTimer);
    settingsStepTimer = undefined;
    if (!pendingUndo) return;
    settingsUndo = [...settingsUndo.slice(-49), pendingUndo];
    settingsRedo = [];
    pendingUndo = null;
  }

  function resetSettingsHistory() {
    clearTimeout(settingsStepTimer);
    settingsStepTimer = undefined;
    pendingUndo = null;
    settingsUndo = [];
    settingsRedo = [];
    previousArt=null;
  }

  function restoreSettings(entry: SettingsEdit) {
    const liveIds=new Set(photos.map(photo=>photo.id));
    const updates:PhotoUpdates=Object.fromEntries(Object.entries(entry.photos).filter(([id])=>liveIds.has(id)));
    const inverse = captureSettings(new Set(Object.keys(updates)));
    if (!sameSettings(sharedSettings,entry.shared)) {
      sharedSettings = clone(entry.shared);
      pendingSharedSettings = sharedSettings;
    }
    pendingPhotoSettings = { ...pendingPhotoSettings, ...clone(updates) };
    if (Object.keys(updates).length) {
      photos = photos.map(photo => Object.hasOwn(updates, photo.id) ? { ...photo, settings: clone(updates[photo.id]) } : photo);
    }
    refreshInspector();
    cancelFrameTrial();
    previousArt=null;
    lastSettingsEditAt = Date.now();
    void flushSettingsSave();
    return inverse;
  }

  function undoSettings() {
    commitSettingsStep();
    const previous = settingsUndo.at(-1);
    if (!previous) return;
    settingsUndo = settingsUndo.slice(0, -1);
    settingsRedo = [...settingsRedo, restoreSettings(previous)];
  }
  function redoSettings() {
    const next = settingsRedo.at(-1);
    if (!next) return;
    settingsRedo = settingsRedo.slice(0, -1);
    settingsUndo = [...settingsUndo, restoreSettings(next)];
  }
  // ⌘Z follows focus: in the photo strip it undoes catalog changes (removals);
  // elsewhere it undoes whichever happened last, a settings edit or a removal.
  const inLibrary=()=>!!document.activeElement?.closest('.library-footer');
  function undoAny(){
    if(settingsStepTimer)commitSettingsStep();
    if(orderUndo.length&&lastOrderAt>=lastSettingsEditAt&&lastOrderAt>=lastRemovalAt){lastUndoKind='order';void undoOrder();return;}
    const preferRemoval=inLibrary()&&removalUndo.length>0;
    if(!preferRemoval&&settingsUndo.length&&(!removalUndo.length||lastSettingsEditAt>=lastRemovalAt)){lastUndoKind='settings';undoSettings();}
    else if(removalUndo.length){lastUndoKind='removal';void undoPhotoRemoval();}
  }
  function redoAny(){
    if(orderRedo.length&&(lastUndoKind==='order'||!settingsRedo.length&&!removalRedo.length)){void redoOrder();return;}
    const preferRemoval=inLibrary()&&removalRedo.length>0;
    if(!preferRemoval&&settingsRedo.length&&(lastUndoKind==='settings'||!removalRedo.length))redoSettings();
    else if(removalRedo.length)void redoPhotoRemoval();
  }

  /** Keeps every current photo exactly once, in `wanted` order where possible. */
  function fitOrder(wanted:string[]){
    const current=photos.map(photo=>photo.id),present=new Set(current),seen=new Set<string>();
    const kept=wanted.filter(id=>present.has(id)&&!seen.has(id)&&!!seen.add(id));
    return [...kept,...current.filter(id=>!seen.has(id))];
  }
  async function applyOrder(order:string[]):Promise<boolean>{
    if(!desktop||!initialized||loading||isRunning||starting||importing||undoing||agentBusy||scopeBusy)return false;
    order=fitOrder(order);
    if(order.join('\n')===photos.map(photo=>photo.id).join('\n'))return false;
    undoing=true;undoError='';
    try{
      const result=await invoke<{order:string[];catalogRevision:number;photoSelection:PhotoSelection}>('reorder_photos',{order,catalogRevision});
      const byId=new Map(photos.map(photo=>[photo.id,photo]));
      photos=result.order.map(id=>byId.get(id)).filter((photo):photo is Photo=>!!photo);
      catalogRevision=result.catalogRevision;
      photoSelection=restoredPhotoSelection(result.order,photoSelection);
      return true;
    }catch(error){undoError=errorMessage(error);return false;}
    finally{undoing=false;}
  }
  /** Moves `ids` (kept in their current relative order) so they start before index `to` of the current list. */
  async function movePhotos(ids:string[],to:number){
    const before=photos.map(photo=>photo.id),moving=new Set(ids);
    const rest=before.filter(id=>!moving.has(id));
    const insertAt=before.slice(0,to).filter(id=>!moving.has(id)).length;
    const after=[...rest.slice(0,insertAt),...before.filter(id=>moving.has(id)),...rest.slice(insertAt)];
    if(await applyOrder(after)){orderUndo=[...orderUndo.slice(-19),{before,after}];orderRedo=[];lastOrderAt=Date.now();lastUndoKind=null;}
  }
  function nudgeSelection(delta:number){
    const ids=photos.map(photo=>photo.id),chosen=ids.filter(id=>selectedPhotoIds.has(id));
    if(!chosen.length)return;
    const first=ids.indexOf(chosen[0]),last=ids.indexOf(chosen[chosen.length-1]);
    if(delta<0&&first>0)void movePhotos(chosen,first-1);
    else if(delta>0&&last<ids.length-1)void movePhotos(chosen,last+2);
  }
  async function undoOrder(){
    const entry=orderUndo.at(-1);if(!entry)return;
    if(await applyOrder(entry.before)){orderUndo=orderUndo.slice(0,-1);orderRedo=[...orderRedo,entry];}
  }
  async function redoOrder(){
    const entry=orderRedo.at(-1);if(!entry)return;
    if(await applyOrder(entry.after)){orderRedo=orderRedo.slice(0,-1);orderUndo=[...orderUndo,entry];lastOrderAt=Date.now();}
  }
  function thumbPointerDown(event:PointerEvent,id:string){
    if(event.button!==0||event.metaKey||event.ctrlKey||event.shiftKey||photos.length<2)return;
    const ids=selectedPhotoIds.has(id)?photos.map(photo=>photo.id).filter(other=>selectedPhotoIds.has(other)):[id];
    photoDrag={ids,x:event.clientX,y:event.clientY,pointer:event.pointerId,active:false};
  }
  function thumbPointerMove(event:PointerEvent){
    if(!photoDrag||event.pointerId!==photoDrag.pointer)return;
    if(!photoDrag.active){
      if(Math.hypot(event.clientX-photoDrag.x,event.clientY-photoDrag.y)<6)return;
      if(undoing||isRunning||agentBusy||scopeBusy){photoDrag=null;return;}
      photoDrag.active=true;
    }
    const tiles=[...document.querySelectorAll<HTMLElement>('.filmstrip .thumbnail[data-photo-id]')];
    let index=tiles.length;
    for(let i=0;i<tiles.length;i++){const box=tiles[i].getBoundingClientRect();if(event.clientX<box.left+box.width/2){index=i;break;}}
    dropIndex=index;
  }
  function thumbPointerUp(event:PointerEvent){
    if(!photoDrag||event.pointerId!==photoDrag.pointer)return;
    const drag=photoDrag;photoDrag=null;
    if(drag.active&&dropIndex>=0){suppressThumbClick=true;setTimeout(()=>suppressThumbClick=false,0);void movePhotos(drag.ids,dropIndex);}
    dropIndex=-1;
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
      sharedSettings = clone(state.settings);
      resetSettingsHistory();
      settings = clone(state.settings);
      saveBaseline = clone(settings);
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
      void loadQuickStyles();
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
    clearTimeout(settingsStepTimer);
    clearTimeout(exportDoneTimer);
    unlisteners.forEach((unlisten) => unlisten());
  });

  function validateLocal(value: RenderSettings): string {
    if (!/^#[0-9a-f]{6}$/i.test(value.frameColor)) return $t("请输入六位 HEX 边框颜色");
    if (!Number.isFinite(value.borderPercent) || value.borderPercent < 0 || value.borderPercent > 30) return $t("边框比例应在 0% 到 30% 之间");
    if (value.ratio && (!(value.ratio.width > 0) || !(value.ratio.height > 0))) return $t("自定义比例的宽和高必须大于 0");
    const backdrop = value.background ?? defaultBackground;
    if (!['color','blur'].includes(backdrop.kind) || !(backdrop.blur >= 0 && backdrop.blur <= 1) || !(backdrop.dim >= 0 && backdrop.dim <= 0.8)) return $t("请检查边框底色设置");
    if (value.watermark.kind !== 'none' && (!Number.isFinite(value.watermark.offsetX) || !Number.isFinite(value.watermark.offsetY))) return $t("请输入有效的水印偏移");
    if (value.watermark.kind === 'text' && !value.watermark.text.trim()) return $t("请输入水印文字");
    if (value.watermark.kind === 'camera' && !value.watermark.cameraTemplate?.trim()) return $t("请填写参数水印的内容");
    if (value.watermark.kind === 'image' && !value.watermark.imagePath) return $t("请选择一张 PNG 水印");
    if ((value.watermark.kind === 'text' || value.watermark.kind === 'camera') && !/^#[0-9a-f]{6}$/i.test(value.watermark.color)) return $t("请选择有效的水印颜色");
    if (value.export.longEdge != null && (!Number.isFinite(value.export.longEdge) || value.export.longEdge < 64)) return $t("输出长边至少为 64 像素");
    if (value.export.namePattern != null && !value.export.namePattern.trim()) return $t("请填写导出文件名");
    return '';
  }

  const sameSettings=(a:unknown,b:unknown)=>JSON.stringify(a)===JSON.stringify(b);
  type SettingsPlan={next:RenderSettings;updates:PhotoUpdates;shared:RenderSettings|null};

  // Build a complete, validated transaction before touching either the catalog
  // or the save queue. Explicit actions are intentionally not reduced to the
  // active photo's delta. Omitted fields still stay independent on each photo.
  function planSettings(value:RenderSettings,explicit:ExplicitSettingsEdit={},policy:PhotoEditPolicy={}):SettingsPlan|null {
    const next=clone(value),updates:PhotoUpdates={};
    let shared:RenderSettings|null=null;
    if(!editSelected){
      if(!sameSettings(sharedSettings,next))shared=next;
    }else{
      const renderChanged=!sameSettings({...saveBaseline,export:next.export},next)||Object.keys(explicit).length>0;
      if(renderChanged){
        const ids=new Set(photoSelection.ids.length?photoSelection.ids:[photoSelection.activeId]);
        const apply=photoEdit(saveBaseline,next,explicit,policy);
        for(const photo of photos){
          if(!ids.has(photo.id))continue;
          const changed=apply(photo.settings??sharedSettings);
          const invalid=validateLocal(changed);
          if(invalid)throw new Error(photo.name+': '+invalid);
          if(!sameSettings(photo.settings,changed))updates[photo.id]=changed;
        }
      }
      if(!sameSettings(sharedSettings.export,next.export))shared={...sharedSettings,export:next.export};
    }
    return shared||Object.keys(updates).length?{next,updates,shared}:null;
  }

  function applySettingsPlan(plan:SettingsPlan,queue:boolean){
    saveBaseline=clone(plan.next);
    if(plan.shared){
      sharedSettings=plan.shared;
      if(queue)pendingSharedSettings=plan.shared;
    }
    if(Object.keys(plan.updates).length){
      photos=photos.map(photo=>Object.hasOwn(plan.updates,photo.id)?{...photo,settings:plan.updates[photo.id]}:photo);
      if(queue)pendingPhotoSettings={...pendingPhotoSettings,...plan.updates};
    }
  }

  function stageSettings(value:RenderSettings,explicit:ExplicitSettingsEdit={},policy:PhotoEditPolicy={}):boolean {
    if(!initialized||!desktop||validateLocal(value)||(sameSettings(value,saveBaseline)&&!Object.keys(explicit).length))return false;
    let plan:SettingsPlan|null;
    try{plan=planSettings(value,explicit,{matchWatermarkKind:true,...policy});}
    catch(error){
      const message=errorMessage(error);
      if(!notices.includes(message))notices=[...notices,message];
      settings=clone(saveBaseline);syncControls();return false;
    }
    if(!plan){saveBaseline=clone(value);return false;}
    trackSettings(new Set(Object.keys(plan.updates)));
    applySettingsPlan(plan,true);
    clearTimeout(saveTimer);
    saveTimer = setTimeout(() => void flushSettingsSave(), 500);
    return true;
  }

  async function persistSettings(updates:PhotoUpdates,shared:RenderSettings|null):Promise<SettingsCommit>{
    if(Object.keys(updates).length)return invoke<SettingsCommit>('set_photo_settings',{updates,shared});
    if(shared)return {updates:{},shared:await invoke<RenderSettings>('save_settings',{settings:shared})};
    return {updates:{},shared:null};
  }

  function adoptSettingsReceipt(sentUpdates:PhotoUpdates,sentShared:RenderSettings|null,receipt:SettingsCommit){
    if(sentShared&&receipt.shared){
      sharedSettings=adoptOwnedAssets(sharedSettings,sentShared,receipt.shared);
      if(pendingSharedSettings)pendingSharedSettings=adoptOwnedAssets(pendingSharedSettings,sentShared,receipt.shared);
    }
    const activeId=photoSelection.activeId;
    const activeHasOwn=editSelected&&!!photos.find(photo=>photo.id===activeId)?.settings;
    const sentForInspector=activeHasOwn?sentUpdates[activeId]:sentShared;
    const ownedForInspector=activeHasOwn?receipt.updates[activeId]:receipt.shared;
    const nextPhotos=photos.map(photo=>{
      const sent=sentUpdates[photo.id],owned=receipt.updates[photo.id];
      if(!sent||!owned||!photo.settings)return photo;
      const next=adoptOwnedAssets(photo.settings,sent,owned);
      return next===photo.settings?photo:{...photo,settings:next};
    });
    if(nextPhotos.some((photo,index)=>photo!==photos[index]))photos=nextPhotos;
    for(const [id,pending] of Object.entries(pendingPhotoSettings)){
      const sent=sentUpdates[id],owned=receipt.updates[id];
      if(pending&&sent&&owned)pendingPhotoSettings[id]=adoptOwnedAssets(pending,sent,owned);
    }
    if(sentForInspector&&ownedForInspector){
      saveBaseline=adoptOwnedAssets(saveBaseline,sentForInspector,ownedForInspector);
      settings=adoptOwnedAssets(settings,sentForInspector,ownedForInspector);
    }
  }

  async function commitAgentSettings(value:RenderSettings,explicit:ExplicitSettingsEdit,frameChange=false){
    const invalid=validateLocal(value);if(invalid)throw new Error(invalid);
    const plan=planSettings(value,explicit,{matchWatermarkKind:false});
    if(!plan)return;
    // Native commit is the transaction boundary: only acknowledge and mutate
    // visible state on success. A failed Agent mutation is never queued for retry.
    const previous=captureSettings(new Set(Object.keys(plan.updates)));
    const receipt=await persistSettings(plan.updates,plan.shared);
    commitSettingsStep();
    applySettingsPlan({...plan,...receipt},false);
    refreshInspector();
    cancelFrameTrial();
    settingsUndo=[...settingsUndo.slice(-49),previous];settingsRedo=[];
    previousArt=frameChange?previous:null;
    lastSettingsEditAt=Date.now();
  }

  function flushSettingsSave(): Promise<boolean> {
    clearTimeout(saveTimer);
    return savingSettings ??= drainSettingsSaves().finally(() => { savingSettings = null; });
  }

  async function drainSettingsSaves(): Promise<boolean> {
    while (pendingSharedSettings || Object.keys(pendingPhotoSettings).length) {
      const shared = pendingSharedSettings;
      const updates = pendingPhotoSettings;
      pendingSharedSettings = null;
      pendingPhotoSettings = {};
      try {
        const receipt=await persistSettings(updates,shared);
        adoptSettingsReceipt(updates,shared,receipt);
      } catch (error) {
        pendingPhotoSettings = { ...updates, ...pendingPhotoSettings };
        pendingSharedSettings ??= shared;
        const message = $t("设置未保存：{0}", {0:errorMessage(error)});
        if (!notices.includes(message)) notices = [...notices, message];
        return false;
      }
    }
    return true;
  }

  function schedulePreview(photoId: string, serialized: string, force=false) {
    const key=JSON.stringify([photoId,serialized]);
    if(!force&&key===lastPreviewKey)return;
    lastPreviewKey=key;
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
          if (request.revision === previewRevision && request.photoId === previewPhotoId) {
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
        schedulePreview(selectedId, JSON.stringify(displayedSettings),true);
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
    stageSettings(settings);
    if (!(await flushSettingsSave())) throw new Error($t('请先保存设置'));
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
    stageSettings(settings);
    undoing=true;historyAction='undo';undoError='';
    try{
      if (!(await flushSettingsSave())) throw new Error($t('请先保存设置'));
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
    stageSettings(settings);
    undoing=true;historyAction='redo';undoError='';
    try{
      if (!(await flushSettingsSave())) throw new Error($t('请先保存设置'));
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
    cancelFrameTrial();
    ratioMode = mode;
    if (mode === 'auto') settings.ratio = null;
    else if (mode === 'custom') settings.ratio = { width: customWidth, height: customHeight };
    else {
      const [width, height] = mode.split(':').map(Number);
      settings.ratio = { width, height };
      customWidth = width;
      customHeight = height;
    }
    if (editSelected && selectedIds.length > 1) stageSettings(settings, {ratio:settings.ratio});
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
      cancelFrameTrial();
      settings.frameColor = normalized.toUpperCase();
      frameHex = normalized.toUpperCase();
      frameHexError = '';
      if (editSelected && selectedIds.length > 1) stageSettings(settings, {frameColor:settings.frameColor});
    } else frameHexError = $t("颜色格式如 #FFFFFF");
  }

  function setBackground(patch:Partial<FrameBackground>) {
    const next={...defaultBackground,...(settings.background??{}),...patch};
    cancelFrameTrial();
    settings.background=next;
    if(editSelected&&selectedIds.length>1)stageSettings(settings,{background:clone(next)});
  }

  function setBorderPercent(value:number) {
    settings.borderPercent=value;
    if(editSelected&&photoSelection.ids.length>1)stageSettings(settings,{borderPercent:value});
  }

  function changeWatermark(kind: WatermarkKind) {
    if (settings.watermark.kind !== kind && kind !== 'none') settings.watermark.sizePercent = kind === 'image' ? 15 : kind === 'camera' ? 2.2 : 3;
    if (kind === 'camera' && settings.watermark.kind !== 'camera') {
      if (!settings.watermark.cameraTemplate?.trim()) settings.watermark.cameraTemplate = defaultCameraTemplate;
      // Camera details read best as a caption in the bottom border.
      if (settings.borderPercent > 0) { settings.watermark.region = 'border'; settings.watermark.position = 'bottom-left'; settings.watermark.offsetX = 0; settings.watermark.offsetY = 0; }
    }
    settings.watermark.kind = kind;
    if(editSelected&&photoSelection.ids.length>1)stageSettings(settings,{watermark:{kind}});
  }

  const cameraPresets = [
    { name: '单行参数', template: defaultCameraTemplate },
    { name: '两行', template: '{camera}\n{focal} {aperture} {shutter} {iso}' },
    { name: '镜头与日期', template: '{camera} · {lens}\n{date}' },
    { name: 'Shot on', template: 'Shot on {camera}' },
  ];
  const cameraTokens = [
    ['机身', 'camera'], ['镜头', 'lens'], ['焦距', 'focal'], ['等效焦距', 'focal35'],
    ['光圈', 'aperture'], ['快门', 'shutter'], ['ISO', 'iso'], ['日期', 'date'],
  ] as const;
  let cameraTemplateInput: HTMLTextAreaElement;
  let cameraInfos: Record<string, CameraInfo | 'error'> = {};
  $: if (desktop && settings.watermark.kind === 'camera' && selectedPhoto && !(selectedPhoto.id in cameraInfos)) void loadCameraInfo(selectedPhoto.id);
  $: currentCamera = selectedPhoto ? cameraInfos[selectedPhoto.id] : undefined;
  $: cameraSummary = currentCamera && currentCamera !== 'error'
    ? [currentCamera.camera, currentCamera.lens, currentCamera.focal, currentCamera.aperture, currentCamera.shutter, currentCamera.iso, currentCamera.date].filter(Boolean).join(' · ')
    : '';
  async function loadCameraInfo(photoId: string) {
    cameraInfos = { ...cameraInfos, [photoId]: 'error' };
    try { cameraInfos = { ...cameraInfos, [photoId]: await invoke<CameraInfo>('get_camera_info', { photoId }) }; }
    catch { cameraInfos = { ...cameraInfos, [photoId]: 'error' }; }
  }
  function insertCameraToken(token: string) {
    const value = settings.watermark.cameraTemplate ?? '';
    const start = cameraTemplateInput?.selectionStart ?? value.length;
    const end = cameraTemplateInput?.selectionEnd ?? value.length;
    const before = value.slice(0, start), after = value.slice(end);
    const glue = before && !/[\s·|]$/.test(before) ? ' · ' : '';
    const inserted = `${glue}{${token}}`;
    settings.watermark.cameraTemplate = (before + inserted + after).slice(0, 200);
    const caret = Math.min(200, start + inserted.length);
    requestAnimationFrame(() => { cameraTemplateInput?.focus(); cameraTemplateInput?.setSelectionRange(caret, caret); });
  }

  function changeRegion(region: WatermarkRegion) {
    settings.watermark.region = region;
    settings.watermark.offsetX = 0; settings.watermark.offsetY = 0;
    if (region === 'border') {
      settings.watermark.position = settings.watermark.position.endsWith('left') ? 'bottom-left' : settings.watermark.position.endsWith('right') ? 'bottom-right' : 'bottom-center';
    }
    if(editSelected&&photoSelection.ids.length>1)stageSettings(settings,{watermark:{region,offsetX:0,offsetY:0,...(region==='border'?{position:settings.watermark.position}:{})}});
  }

  async function chooseWatermarkImage() {
    try {
      const path = await open({ multiple: false, title: $t("选择透明 PNG 水印"), filters: [{ name: $t("PNG 水印"), extensions: ['png'] }] });
      if (typeof path === 'string') {
        settings.watermark.imagePath = path;
        if(editSelected&&photoSelection.ids.length>1)stageSettings(settings,{watermark:{kind:'image',imagePath:path}});
      }
    } catch (error) { notices = [...notices, errorMessage(error)]; }
  }

  async function chooseFont() {
    try {
      const path = await open({ multiple: false, title: $t("选择水印字体"), filters: [{ name: $t("字体"), extensions: ['ttf', 'otf', 'ttc', 'otc'] }] });
      if (typeof path === 'string') {
        settings.watermark.fontPath = path; settings.watermark.fontFamily = null;
        if(editSelected&&photoSelection.ids.length>1)stageSettings(settings,{watermark:{fontPath:path,fontFamily:null}});
      }
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
    if (preset) {
      commitSettingsStep();
      settings=clone(preset.settings);
      stageSettings(settings,{...frameStyleEdit(settings),watermark:settings.watermark});
      commitSettingsStep();
      syncControls();
    }
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

  // One-click export targets: canvas ratio, long edge and JPG quality together.
  const exportTargets: {id:string;name:string;ratio?:{width:number;height:number}|null;longEdge:number|null;quality:number}[] = [
    { id: 'original', name: '原图', longEdge: null, quality: 95 },
    { id: 'xiaohongshu', name: '小红书 3:4', ratio: { width: 3, height: 4 }, longEdge: 2560, quality: 92 },
    { id: 'instagram', name: 'Instagram 4:5', ratio: { width: 4, height: 5 }, longEdge: 1350, quality: 90 },
    { id: 'moments', name: '朋友圈', longEdge: 2160, quality: 90 },
  ];
  function applyExportTarget(id: string) {
    const target = exportTargets.find(item => item.id === id);
    if (!target) return;
    exportTargetId = id;
    uniformExport = !!target.ratio;
    settings.export = { ...settings.export, format: 'jpg', longEdge: target.longEdge, jpegQuality: target.quality };
    syncControls();
  }
  $: exportNameExample = (() => {
    const name = (selectedPhoto?.name ?? 'IMG_0001.jpg').replace(/\.[^.]+$/, '');
    const stem = (settings.export.namePattern || '{name}-framed').replaceAll('{name}', name).replaceAll('{index}', '001').replace(/[<>:"/\\|?*]/g, '_').trim();
    return `${stem || name + '-framed'}.${settings.export.format === 'png' ? 'png' : 'jpg'}`;
  })();
  let exportDone: { count: number; failed: number } | null = null;
  let exportDoneTimer: ReturnType<typeof setTimeout> | undefined;

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

  function dispatchExport(directory: string, photoIds: string[], composition: RenderSettings, uniformRatio: RenderSettings['ratio']) {
    return invoke<BatchSnapshot>('start_batch', { outputDirectory: directory, settings: composition, photoIds, uniformRatio });
  }

  async function startBatch() {
    if (!desktop || !initialized || loading || isRunning || starting || importing || undoing || scopeBusy || !exportPhotoIds.length || localError) return;
    starting = true;
    exportError = '';
    try {
      stageSettings(settings);
      const ids = [...exportPhotoIds];
      const composition = clone(sharedSettings);
      const ratio = uniformExport && exportTargetRatio ? { ...exportTargetRatio } : null;
      if (!(await flushSettingsSave())) throw new Error($t('请先保存设置'));
      if (!outputDirectory) await chooseOutputDirectory();
      if (!outputDirectory) return;
      acceptBatch(await dispatchExport(outputDirectory, ids, composition, ratio));
      exportDialog?.close();
    } catch (error) { exportError = errorMessage(error); }
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
    const finished = batch?.id === incoming.id && batch.status === 'running' && incoming.status === 'completed';
    batch = incoming;
    if (finished) {
      exportDone = { count: incoming.items.filter(item => item.status === 'completed').length, failed: incoming.items.filter(item => item.status === 'failed').length };
      clearTimeout(exportDoneTimer);
      exportDoneTimer = setTimeout(() => exportDone = null, 15000);
    }
    if (incoming.status !== 'running') cancelling = false;
  }

  async function markAgentReady(){
    await invoke('agent_window_ready',{ready:true});
    const state=await invoke<AgentStatus>('get_agent_status');agentConnected=state.enabled;
  }

  function agentWorkspace(args:Record<string,unknown>={}){
    const offset=typeof args.offset==='number'?args.offset:0,limit=typeof args.limit==='number'?args.limit:200;
    return {photoCount:photos.length,photos:photos.slice(offset,offset+limit).map(({id,name,path,width,height})=>({id,name,path,width,height})),
      nextOffset:offset+limit<photos.length?offset+limit:null,selection:clone(photoSelection),settings:clone(settings),editScope:editSelected?'selected':'shared',
      previewSettings:clone(effectiveSettings),previewUsesOwnSettings:sharedOverridePreview,
      history:{undoCount:removalUndo.length,redoCount:removalRedo.length},importing,importingCount,pendingImportCount:pendingImports.length,
      export:batch?{id:batch.id,status:batch.status,total:batch.items.length,completed:completedCount,failed:failedCount}:null};
  }

  async function agentWait(ready:()=>boolean,milliseconds=15000){
    const end=Date.now()+milliseconds;
    while(!ready()){if(Date.now()>end)throw new Error($t('操作尚未完成，请查看主界面进度后重试'));await new Promise(resolve=>setTimeout(resolve,35));}
  }

  function requireAgentWindow(){
    if(pickingColor||choosingPhotos||document.querySelector('dialog[open]:not(.agent-connection)'))throw new Error($t('请先完成或关闭当前对话框'));
  }

  async function handleAgentCommand(command:AgentCommand){
    let result:unknown=null,problem:string|null=null,ownsMutation=false;
    try{
      if(!initialized||loading)throw new Error($t('照片窗口尚未就绪，请稍后重试'));
      const mutating=!readOnlyAgentTools.has(command.name);
      if(mutating){
        if(agentBusy||importing||choosingPhotos||starting||undoing||scopeBusy||pendingImports.length)throw new Error($t('请等待当前操作完成后再使用 Agent'));
        requireAgentWindow();
        stageSettings(settings);
        agentBusy=true;ownsMutation=true;await tick();
        if (!(await flushSettingsSave())) throw new Error($t("请先保存设置"));
        await agentWait(()=>!selectionSaving&&!pendingSelectionSave);
        requireAgentWindow();
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
          requireAgentWindow();
          await commitAgentSettings(next,frameStyleEdit(style),true);
          await tick();result=agentWorkspace();break;
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
          requireAgentWindow();
          await commitAgentSettings(next,explicitAgentEdit(args.patch,next));
          await tick();result=agentWorkspace();break;
        }
        case 'margin_get_preview':{
          if(agentBusy||importing)throw new Error($t('请等待当前操作完成后再使用 Agent'));
          if(!selectedPhoto)throw new Error($t('请先选择一张照片。'));
          if(trialSettings||comparingOriginal)throw new Error($t('请先应用或取消正在试用的边框'));
          await tick();await agentWait(()=>!!previewError||!previewLoading&&!previewInFlight&&!queuedPreview&&!!preview,45000);
          if(previewError||!preview)throw new Error(previewError||$t('预览暂不可用'));
          result={dataUrl:preview.dataUrl,width:preview.width,height:preview.height,photoId:selectedId,photoBounds:preview.photoBounds,warning:preview.warning??null};break;
        }
        case 'margin_export_photos':{
          if(isRunning)throw new Error($t('请等待当前导出完成'));
          if(trialSettings||comparingOriginal)throw new Error($t('请先应用或取消正在试用的边框'));
          const directory=absoluteLocalPath(args.outputDirectory),scope=args.scope??'selected';
          const ids=scope==='all'?order:scope==='ids'?catalogIds(args.ids,order):[...selectedIds];
          if(!ids.length)throw new Error($t('请先导入并选择要导出的照片。'));
          const ratio = (args.uniformRatio ?? null) as RenderSettings['ratio'];
          const started = await dispatchExport(directory, ids, clone(sharedSettings), ratio);
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

<svelte:window on:pointermove={thumbPointerMove} on:pointerup={thumbPointerUp} on:pointercancel={()=>{photoDrag=null;dropIndex=-1;}} on:blur={() => comparingOriginal = false} on:keydown={(event) => {
  if (event.key === 'Escape' && (trialIndex >= 0 || comparingOriginal) && !document.querySelector('dialog[open]')) {
    event.preventDefault();
    cancelFrameTrial();
  }
  if (!agentBusy && (event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 'o' && !document.querySelector('dialog[open]')) {
    event.preventDefault();
    void choosePhotos();
  }
  const typing=event.target instanceof HTMLElement&&!!event.target.closest('input,textarea,select,[contenteditable=true]');
  if (!agentBusy && !typing && (event.metaKey || event.ctrlKey) && event.shiftKey && !document.querySelector('dialog[open]')) {
    const key=event.key.toLowerCase();
    if(key==='c'&&selectedPhoto){event.preventDefault();copySettings();}
    else if(key==='v'&&copiedSettings){event.preventDefault();void pasteSettings();}
  }
  if (event.key === '?' && !typing && !event.metaKey && !event.ctrlKey && !event.altKey && !document.querySelector('dialog[open]')) {
    event.preventDefault();
    shortcutSheet.open();
  }
}} />

<div class:drop-active={dragActive} class="app-shell gallery-shell ws">
  <header class="app-header">
    <div class="header-leading">
      <div class="brand"><h1>{brand.name}</h1></div>
      {#if photos.length}
        <div class="view-switch" role="group" aria-label={$t('工作区视图')}>
          <button type="button" aria-pressed={workspaceView==='single'} on:click={()=>workspaceView='single'}><Square size={13}/>{$t('单张')}</button>
          <button type="button" class="overview-toggle" aria-pressed={workspaceView==='overview'} on:click={()=>{stageSettings(settings);workspaceView='overview';}}><LayoutGrid size={13}/>{$t('总览')}</button>
        </div>
      {/if}
      {#if workspaceView==='overview'&&photos.length}
        <div class="file-heading"><span>{$t('全部照片')}</span><span class="source-size">{$t('已选 {0} 张',{0:selectedIds.length})} / {photos.length}</span></div>
      {:else if selectedPhoto}
        <div class="file-heading"><span title={selectedPhoto.path}>{selectedPhoto.name}</span><span class="source-size">{selectedPhoto.width} × {selectedPhoto.height}</span></div>
      {/if}
    </div>
    <div class="header-actions">
      <button type="button" class="icon-button header-undo" aria-label={$t('撤销')} title={$t('撤销（⌘ / Ctrl+Z）')} disabled={!desktop||!(removalUndo.length||settingsUndo.length||orderUndo.length||pendingUndo)||undoing||agentBusy||isRunning} on:click={undoAny}><Undo2 size={16}/></button>
      <button type="button" class="icon-button header-redo" aria-label={$t('重做')} title={$t('重做（⌘ / Ctrl+Shift+Z）')} disabled={!desktop||!(removalRedo.length||settingsRedo.length||orderRedo.length)||undoing||agentBusy||isRunning} on:click={redoAny}><Redo2 size={16}/></button>
      <span class="header-divider" aria-hidden="true"></span>
      <button type="button" class="icon-button shortcuts-button" aria-label={$t('键盘快捷键')} title={$t('键盘快捷键（?）')} aria-keyshortcuts="Shift+Slash" on:click={()=>shortcutSheet.open()}><Keyboard size={16}/></button>
      <button class="button quiet compact import-button" on:click={choosePhotos} disabled={!desktop || importing || choosingPhotos || agentBusy || loading || !initialized}>
        {#if importing}<LoaderCircle size={15} class="spin" />{:else}<ImagePlus size={16} />{/if}
        {importing ? $t("正在导入") : $t("导入")}
      </button>
      <button type="button" class:agent-connected={agentConnected} class="icon-button agent-connect-trigger" aria-label={$t('Agent 连接')} title={$t('Agent 连接')} aria-expanded={agentOpen} disabled={!desktop} on:click={()=>agentOpen=true}><Plug size={16}/></button>
      <button type="button" class="text-button header-labelled model-config-trigger" title={$t('连接 ChatGPT 或自己的模型，用于 AI 艺术边框')} disabled={!desktop} on:click={()=>configureModels()}><Sparkles size={15}/><span>{$t('模型')}</span></button>
      <div class="language-picker theme-picker" title={$t('界面主题')}>
        <SunMoon size={16} aria-hidden="true" />
        <select class="theme-select" aria-label={$t('界面主题')} bind:value={$theme}>
          <option value="dark" label={$t('深色')}>{$t('深色')}</option>
          <option value="light" label={$t('浅色')}>{$t('浅色')}</option>
          <option value="system" label={$t('自动')}>{$t('跟随系统')}</option>
        </select>
      </div>
      <div class="language-picker" title={$t("界面语言 / Interface language")}>
        <Languages size={16} aria-hidden="true" />
        <select id="interface-language" class="language-select" aria-label={$t("界面语言 / Interface language")} bind:value={$language}>
          <option value="en" lang="en" label="EN">English</option>
          <option value="zh-CN" lang="zh-CN" label="中">简体中文</option>
        </select>
      </div>
      {#if isRunning}<button class="button secondary compact export-primary" disabled={cancelling} on:click={cancelBatch}><X size={15}/>{cancelling?$t('正在停止'):$t('取消导出')}</button>
      {:else}<button class="button primary compact export-primary export-button" on:click={openExportDialog} disabled={!desktop || !initialized || !selectedIds.length || !!localError || starting || importing || undoing || agentBusy}><Download size={15}/>{$t('导出选中 {0} 张',{0:selectedIds.length})}</button>{/if}
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
  <div class="workspace" inert={agentBusy||undoing}>
    <main class="content-area">
      <div class="viewer">
        {#if loading}
          <div class="empty-state"><LoaderCircle size={25} class="spin" /><p>{$t("正在载入工作区")}</p></div>
        {:else if workspaceView==='overview'&&photos.length}
          <OverviewGrid {photos} shared={sharedSettings} selected={selectedPhotoIds} activeId={selectedId??''} onselect={selectPhoto} onopen={openSingle}/>
        {:else if selectedPhoto}
          {#key selectedPhoto.id}
            <PhotoViewport photo={selectedPhoto} preview={comparingOriginal ? null : preview} picking={pickingColor} loading={previewLoading} oncolor={setFrameColor} onclose={closePicker} markBox={watermarkPreviewVisible&&!sharedOverridePreview&&!trialSettings&&!comparingOriginal?markBox:null} onmarkdrag={dragMark}>
              <svelte:fragment slot="tools">
                <button type="button" class="text-button compare-original" aria-pressed={comparingOriginal} title={$t('按住查看未加框的原片')}
                  on:pointerdown={event => { if (event.button === 0) { event.currentTarget.setPointerCapture(event.pointerId); comparingOriginal = true; } }}
                  on:pointerup={() => comparingOriginal = false} on:lostpointercapture={() => comparingOriginal = false}
                  on:keydown={event => compareKey(event, true)} on:keyup={event => compareKey(event, false)} on:blur={() => comparingOriginal = false}><Columns2 size={13}/>{$t('对比原片')}</button>
              </svelte:fragment>
              {#if watermarkPreviewVisible}
                <WatermarkDetail photo={selectedPhoto} settings={displayedSettings} overview={preview} hidden={comparingOriginal} collapsed={watermarkPreviewCollapsed} oncollapse={collapseWatermarkPreview} onlimits={(value)=>offsetLimits=sharedOverridePreview||trialSettings?null:value} onbox={(box)=>{if(!markDragStart)markBox=box;}}/>
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
      <div class="scope-row"><span class="scope-label">{$t('应用到')}</span><div class="settings-scope" role="group" aria-label={$t('设置应用于')}>
        <button type="button" class:active={!editSelected} aria-pressed={!editSelected} disabled={!selectedPhoto||scopeBusy||isRunning} on:click={() => setSettingsScope(false)}>{$t('共享设置')}</button>
        <button type="button" class:active={editSelected} aria-pressed={editSelected} disabled={!selectedPhoto||scopeBusy||isRunning} on:click={() => setSettingsScope(true)}>{$t(selectedIds.length>1?'编辑选中 {0} 张':'仅这张',{0:selectedIds.length})}</button>
      </div></div>
      {#if sharedOverridePreview}<p class="scope-note" role="status">{$t('当前照片的预览和导出均使用单独设置；共享调整只影响没有单独设置的照片。')}</p>{/if}
      {#if hasScopedOverrides}<button type="button" class="text-button restore-shared" disabled={scopeBusy||isRunning} on:click={restoreSharedSettings}>{$t('恢复共享设置')}</button>{/if}
      <div class="inspector-tabs" role="tablist" aria-label={$t('边框与水印')}>
        <button type="button" id="inspector-frame" role="tab" aria-controls="frame-panel" aria-selected={inspectorTab==='frame'} tabindex={inspectorTab==='frame'?0:-1} on:click={()=>selectInspector('frame')} on:keydown={inspectorKey}><span class="step-number" aria-hidden="true">01</span>{$t('边框')}</button>
        <button type="button" id="inspector-watermark" role="tab" aria-controls="watermark-panel" aria-selected={inspectorTab==='watermark'} tabindex={inspectorTab==='watermark'?0:-1} on:click={()=>selectInspector('watermark')} on:keydown={inspectorKey}><span class="step-number" aria-hidden="true">02</span>{$t('水印')}</button>
      </div>
      <div id="frame-panel" inert={scopeBusy} class="setting-section inspector-panel" role="tabpanel" tabindex="0" aria-labelledby="inspector-frame" hidden={inspectorTab!=='frame'}>
        <section class="quick-templates" aria-label={$t('快速试框')}>
          <div class="quick-templates-heading"><h2>{$t('样式')}</h2><button type="button" class="text-button" disabled={!desktop||!initialized||loading||importing||isRunning} on:click={() => { cancelFrameTrial(); openTemplates(); }}>{$t('全部模板')}</button></div>
          <div class="quick-template-grid" role="group" aria-label={$t('快速试框')} bind:this={quickGrid}>
            {#each quickFrames as frame, index}
              <button type="button" aria-pressed={trialIndex === index} tabindex={trialIndex < 0 ? (index === 0 ? 0 : -1) : (trialIndex === index ? 0 : -1)}
                disabled={!selectedPhoto||!quickStyles[index]||scopeBusy||isRunning} on:keydown={event => quickFrameKey(event, index)} on:click={() => trialIndex = index}>
                <span class={'template-sample ' + frame.style}>{#if selectedPhoto?.thumbnail}<img src={selectedPhoto.thumbnail} alt=""/>{:else}<ImageIcon size={20}/>{/if}</span>
                <span>{$t(frame.label)}</span>
              </button>
            {/each}
          </div>
          {#if trialSettings}
            <div class="trial-actions">
              <button type="button" class="text-button" aria-pressed={comparingOriginal}
                on:pointerdown={event => { if (event.button === 0) { event.currentTarget.setPointerCapture(event.pointerId); comparingOriginal = true; } }}
                on:pointerup={() => comparingOriginal = false} on:lostpointercapture={() => comparingOriginal = false}
                on:keydown={event => compareKey(event, true)} on:keyup={event => compareKey(event, false)} on:blur={() => comparingOriginal = false}>{$t('按住看原片')}</button>
              <button type="button" class="text-button" on:click={cancelFrameTrial}>{$t('取消')}</button>
              <button type="button" class="button secondary compact apply-trial" disabled={previewLoading||!!previewError} on:click={applyTrial}>{$t('应用边框')}</button>
            </div>
          {/if}
          <button type="button" class="art-trigger ai-card" disabled={!desktop||!selectedPhoto||importing||isRunning} on:click={()=>{modelMode='design';modelTarget='frame';modelOpen=true;}}>
            <span class="ai-card-icon" aria-hidden="true"><Sparkles size={15}/></span>
            <span class="ai-card-text"><strong>{$t('AI 艺术边框')}</strong><small>{$t('按照片生成独一份设计 · 需连接模型')}</small></span>
            <ChevronRight size={15} aria-hidden="true"/>
          </button>
        </section>
        <div class="inspector-group-start" aria-hidden="true"></div>
        <div class="field-title ratio-title"><span class="field-label">{$t('最终比例')}</span><button class="icon-button" disabled={!settings.ratio} on:click={swapRatio} title={$t('交换宽高')} aria-label={$t('交换宽高')}><ArrowLeftRight size={15}/></button></div>
        <div class="ratio-grid" role="group" aria-label={$t('最终比例')}>
          <button type="button" class:active={ratioMode==='auto'} aria-pressed={ratioMode==='auto'} on:click={()=>changeRatio('auto')}>{$t('自动')}</button>
          {#each galleryRatios as ratio}<button type="button" class:active={ratioMode===ratio} aria-pressed={ratioMode===ratio} on:click={()=>changeRatio(ratio)}>{ratio}</button>{/each}
          <button type="button" class:active={ratioMode==='custom'} aria-pressed={ratioMode==='custom'} on:click={()=>changeRatio('custom')}>{$t('自定义')}</button>
        </div>
        {#if ratioMode === 'custom'}<div class="custom-ratio"><label><span>{$t("宽")}</span><input type="number" min="0.01" step="0.1" value={customWidth} on:input={(event) => changeCustomRatio('width', event.currentTarget.valueAsNumber)} aria-label={$t("自定义比例宽")} /></label><span>:</span><label><span>{$t("高")}</span><input type="number" min="0.01" step="0.1" value={customHeight} on:input={(event) => changeCustomRatio('height', event.currentTarget.valueAsNumber)} aria-label={$t("自定义比例高")} /></label></div>{/if}
        <div class="field-title"><label class="field-label" for="border-range">{$t("边框比例")}</label><div class="number-with-unit"><input aria-label={$t("边框百分比")} type="number" min="0" max="30" step="0.5" value={settings.borderPercent} on:input={(event)=>setBorderPercent(event.currentTarget.valueAsNumber)} /><span>%</span></div></div>
        <input id="border-range" type="range" min="0" max="30" step="0.5" value={settings.borderPercent} on:input={(event)=>setBorderPercent(event.currentTarget.valueAsNumber)} />
        <p class="field-help">{$t("按照片短边计算最小留白；指定比例时，部分方向会增加留白")}</p>
        <div class="inspector-group-start" aria-hidden="true"></div>
        <label class="field-label color-label" for="frame-hex">{$t("边框颜色")}</label>
        <div class="segmented background-kind" role="group" aria-label={$t('边框底色')}>
          <button type="button" class:active={(settings.background?.kind??'color')==='color'} aria-pressed={(settings.background?.kind??'color')==='color'} on:click={()=>setBackground({kind:'color'})}>{$t('纯色')}</button>
          <button type="button" class:active={settings.background?.kind==='blur'} aria-pressed={settings.background?.kind==='blur'} on:click={()=>setBackground({kind:'blur'})}>{$t('照片模糊')}</button>
        </div>
        {#if settings.background?.kind==='blur'}
          <div class="field-title"><label class="field-label" for="background-blur">{$t('模糊程度')}</label><output>{Math.round(settings.background.blur*100)}%</output></div>
          <input id="background-blur" type="range" min="0" max="1" step="0.05" value={settings.background.blur} on:input={e=>setBackground({blur:e.currentTarget.valueAsNumber})}/>
          <div class="field-title"><label class="field-label" for="background-dim">{$t('叠加边框颜色')}</label><output>{Math.round(settings.background.dim*100)}%</output></div>
          <input id="background-dim" type="range" min="0" max="0.8" step="0.05" value={settings.background.dim} on:input={e=>setBackground({dim:e.currentTarget.valueAsNumber})}/>
          <p class="field-help">{$t('用照片自身的模糊版本铺满留白，照片仍完整保留；下面的颜色会按比例叠加在模糊底上。')}</p>
        {/if}
        <div class="color-palette" role="group" aria-label={$t('边框色盘')}>{#each palette.slice(0,8) as [color,name]}<button style:background={color} class:active={settings.frameColor.toUpperCase()===color} class="palette-swatch" aria-label={`${$t(name)} ${color}`} aria-pressed={settings.frameColor.toUpperCase()===color} title={$t(name)} on:click={()=>setFrameColor(color)}></button>{/each}</div>
        {#if photoColors.length}
          <span class="photo-colors-label">{$t('来自这张照片')}</span>
          <div class="color-palette photo-colors" role="group" aria-label={$t('照片配色建议')}>{#each photoColors as color}<button style:background={color} class:active={settings.frameColor.toUpperCase()===color} class="photo-swatch" aria-label={$t('照片配色 {0}',{0:color})} aria-pressed={settings.frameColor.toUpperCase()===color} title={color} on:click={()=>setFrameColor(color)}></button>{/each}</div>
        {/if}
        <div class="color-row">
          <input class="color-picker" type="color" aria-label={$t("选择边框颜色")} value={settings.frameColor} on:input={(event) => setFrameColor(event.currentTarget.value)} />
          <input id="frame-hex" class="hex-input" value={frameHex} maxlength="7" on:input={(event) => setFrameColor(event.currentTarget.value)} spellcheck="false" />
          <button bind:this={pickerButton} class="icon-button bordered" class:color-active={pickingColor} aria-label={$t("从照片取色")} aria-pressed={pickingColor} title={$t("从照片取色（带原像素放大镜）")} disabled={!preview || previewLoading || isRunning || importing} on:click={() => pickingColor = !pickingColor}><Pipette size={16} /></button>
        </div>
        <details class="extra-colors"><summary>{$t('更多颜色')}</summary><div class="color-palette" role="group" aria-label={$t('更多颜色')}>{#each palette.slice(8) as [color,name]}<button style:background={color} class:active={settings.frameColor.toUpperCase()===color} class="palette-swatch" aria-label={`${$t(name)} ${color}`} aria-pressed={settings.frameColor.toUpperCase()===color} title={$t(name)} on:click={()=>setFrameColor(color)}></button>{/each}</div></details>
        {#if frameHexError}<p class="field-error">{$t(frameHexError)}</p>{/if}
        <details class="advanced-watermark motif-details"><summary><ChevronDown size={14} />{$t("纸张纹理")}</summary>
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
        <details class="advanced-watermark saved-settings"><summary><ChevronDown size={14}/>{$t('我的样式')}</summary>
          <button class="text-button save-current-frame" disabled={!desktop||!initialized||loading||!!localError} on:click={()=>openTemplates(settings)}><Bookmark size={12}/>{$t('保存当前边框为模板')}</button>
      <div class="preset-row">
        <Bookmark size={15} />
        <select aria-label={$t("个人预设")} value={selectedPreset} on:change={(event) => applyPreset(event.currentTarget.value)}><option value="">{presets.length ? $t("选择个人预设") : $t("我的预设")}</option>{#each presets as preset}<option value={preset.id}>{preset.name}</option>{/each}</select>
        {#if selectedPreset}<button class="icon-button" on:click={deletePreset} disabled={!desktop} title={$t("删除此预设")} aria-label={$t("删除此预设")}><Trash2 size={14} /></button>{/if}
        <button class="icon-button" on:click={() => { namingPreset = !namingPreset; presetName = $t("我的方案 {0}", {0:presets.length + 1}); }} disabled={!desktop || !!localError} title={$t("保存当前设置为预设")} aria-label={$t("保存当前设置为预设")}><Plus size={16} /></button>
      </div>
      {#if namingPreset}<form class="preset-form" on:submit|preventDefault={savePreset}><input aria-label={$t("预设名称")} bind:value={presetName} maxlength="40" placeholder={$t("预设名称")} /><button class="button secondary compact" type="submit" disabled={savingPreset || !presetName.trim()}>{savingPreset ? $t("保存中") : $t("保存")}</button></form>{/if}
        </details>
        {#if previousArt&&!pendingUndo&&settingsUndo.at(-1)===previousArt}<button class="text-button" on:click={undoArt}><RotateCcw size={12} />{$t('撤销上次配框')}</button>{/if}
      </div>

      <div id="watermark-panel" inert={scopeBusy} class="setting-section watermark-section inspector-panel" role="tabpanel" tabindex="0" aria-labelledby="inspector-watermark" hidden={inspectorTab!=='watermark'}>
        <div class="segmented" role="group" aria-label={$t("水印类型")}>
          <button class:active={settings.watermark.kind === 'none'} aria-pressed={settings.watermark.kind === 'none'} on:click={() => changeWatermark('none')}>{$t("关闭")}</button>
          <button class:active={settings.watermark.kind === 'text'} aria-pressed={settings.watermark.kind === 'text'} on:click={() => changeWatermark('text')}><Type size={13} />{$t("文字")}</button>
          <button class:active={settings.watermark.kind === 'camera'} aria-pressed={settings.watermark.kind === 'camera'} on:click={() => changeWatermark('camera')}><Aperture size={13} />{$t("参数")}</button>
          <button class:active={settings.watermark.kind === 'image'} aria-pressed={settings.watermark.kind === 'image'} on:click={() => changeWatermark('image')}><ImageIcon size={13} />PNG</button>
        </div>
        {#if settings.watermark.kind === 'none'}<p class="watermark-off">{$t("给作品加上你的签名或 Logo")}</p>
        {:else}
          {#if settings.watermark.kind === 'text'}
            <label class="field-label" for="watermark-text">{$t("署名文字")}</label>
            <div class="watermark-text-row"><input id="watermark-text" class="full-width" bind:value={settings.watermark.text} maxlength="160" placeholder={$t("输入你的署名")} /><input class="color-picker" type="color" aria-label={$t('文字水印颜色')} bind:value={settings.watermark.color}/></div>
          {:else if settings.watermark.kind === 'camera'}
            <div class="camera-presets" role="group" aria-label={$t('参数水印样式')}>{#each cameraPresets as preset}<button type="button" class:chosen={settings.watermark.cameraTemplate === preset.template} aria-pressed={settings.watermark.cameraTemplate === preset.template} on:click={() => settings.watermark.cameraTemplate = preset.template}>{$t(preset.name)}</button>{/each}</div>
            <label class="field-label" for="camera-template">{$t('显示内容')}</label>
            <div class="watermark-text-row"><textarea id="camera-template" class="camera-template" rows="2" maxlength="200" spellcheck="false" bind:this={cameraTemplateInput} bind:value={settings.watermark.cameraTemplate}></textarea><input class="color-picker" type="color" aria-label={$t('文字水印颜色')} bind:value={settings.watermark.color}/></div>
            <div class="camera-tokens" role="group" aria-label={$t('插入参数')}>{#each cameraTokens as [label, token]}<button type="button" title={`{${token}}`} on:click={() => insertCameraToken(token)}>{$t(label)}</button>{/each}</div>
            {#if currentCamera === 'error' || (currentCamera && !cameraSummary)}<p class="field-help camera-missing" role="status">{$t('这张照片没有拍摄参数，参数水印不会显示')}</p>
            {:else if cameraSummary}<p class="field-help camera-summary" title={cameraSummary}>{$t('本张：')}{cameraSummary}</p>{/if}
            <p class="field-help">{$t('每张照片自动读取自己的拍摄参数；缺少的项目会连同分隔符一起省略')}</p>
          {:else}
            <button class="watermark-file" disabled={!desktop} on:click={chooseWatermarkImage}><ImagePlus size={16} /><span>{settings.watermark.imagePath ? basename(settings.watermark.imagePath) : $t("选择 PNG 水印")}</span><ChevronDown size={13} /></button>
            <p class="field-help">{$t("支持透明背景，保留 Logo 原始比例")}</p>
          {/if}
          <div class="watermark-size-group">
            <div class="field-title"><label class="field-label" for="watermark-size">{settings.watermark.kind !== 'image' ? $t('文字大小') : $t('Logo 大小')}</label><div class="number-with-unit"><input aria-label={$t('水印大小百分比')} type="number" min={settings.watermark.kind !== 'image' ? 0.5 : 2} max={settings.watermark.kind !== 'image' ? 15 : 60} step="0.5" bind:value={settings.watermark.sizePercent}/><span>%</span></div></div>
            <input id="watermark-size" type="range" min={settings.watermark.kind !== 'image' ? 0.5 : 2} max={settings.watermark.kind !== 'image' ? 15 : 60} step="0.5" bind:value={settings.watermark.sizePercent}/>
          </div>
          <div class="font-settings">
            {#if settings.watermark.kind === 'text' || settings.watermark.kind === 'camera'}
              <FontPicker {fonts} selected={settings.watermark.fontFamily} onchange={(font)=>{settings.watermark.fontPath=font?.path||null;settings.watermark.fontFamily=font?.family??null;if(editSelected&&photoSelection.ids.length>1)stageSettings(settings,{watermark:{fontPath:settings.watermark.fontPath,fontFamily:settings.watermark.fontFamily}});}}/>
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

      <div class="inspector-bottom">
        <span class="inspector-bottom-status">{editSelected?$t('单独设置 · {0} 张',{0:frameAffectedCount}):$t('共享设置 · {0} 张',{0:frameAffectedCount})}</span>
        <button type="button" class="export-options" on:click={openExportDialog} disabled={!desktop||!initialized||!photos.length||isRunning||starting||importing||undoing} aria-label={$t('导出选项')}>{settings.export.format.toUpperCase()} · {settings.export.format==='jpg'?settings.export.jpegQuality:$t('无损 PNG')} · {settings.export.longEdge?`${settings.export.longEdge} px`:$t('保留原始像素')}<ChevronDown size={12}/></button>
      </div>
    </aside>
  </div>

  <footer class="library-footer">
    {#if isRunning && totalCount > 0}<div class="batch-track" role="progressbar" aria-label={$t("批量导出进度")} aria-valuemin="0" aria-valuemax="100" aria-valuenow={Math.round(progress)}><div style:width={`${progress}%`}></div></div>{/if}
    <section class="filmstrip-section" inert={agentBusy} aria-label={$t('已导入的照片')}>
      <div class="library-bar">
        <div class="library-summary"><strong>{photos.length} {$t('张照片',{count:photos.length})}{#if photos.length}<span class="selection-count" role="status">{$t('已选 {0} 张',{0:selectedIds.length})}</span>{/if}</strong><span id="photo-selection-hint" title={$t('先单击起点，再按住 Shift 点击终点；⌘ / Ctrl+A 全选，Esc 取消选择')}>{$t(photos.length>1?'Shift 连选 · 拖动排序 · ⌘ / Ctrl+A 全选':'全部照片共用当前设置')}</span></div>
        {#if photos.length}<div class="library-tools" role="toolbar" aria-label={$t('照片操作')}>
          <button type="button" class="text-button copy-settings" title={$t('复制当前照片的边框和水印（⌘ / Ctrl+Shift+C）')} disabled={!selectedPhoto||scopeBusy||isRunning||agentBusy} on:click={copySettings}><Copy size={13}/>{$t('复制设置')}</button>
          <button type="button" class="text-button paste-settings" title={copiedSettings?$t('把「{0}」的设置粘贴到选中的照片（⌘ / Ctrl+Shift+V）',{0:copiedFrom}):$t('先复制一张照片的设置')} disabled={!copiedSettings||!selectedIds.length||scopeBusy||isRunning||agentBusy} on:click={pasteSettings}><ClipboardPaste size={13}/>{$t(selectedIds.length>1?'粘贴到 {0} 张':'粘贴设置',{0:selectedIds.length})}</button>
          <button type="button" class="text-button remove-photo" title={$t('从目录中移除照片（Backspace / Delete）')} aria-label={$t(selectedIds.length>1?'移除选中的 {0} 张照片':'移除当前照片',{0:selectedIds.length})} aria-keyshortcuts="Backspace Delete" disabled={isRunning||starting||importing||!selectedIds.length} on:click={()=>photoActions.requestRemoval(selectedIds)}><Trash2 size={13}/><span>{$t(selectedIds.length>1?'移除 {0} 张':'移除照片',{0:selectedIds.length})}</span></button>
          <button type="button" class="text-button clear-photos" aria-label={$t('清空列表')} title={$t('清空列表（原图保留）')} disabled={isRunning||starting||importing||undoing} on:click={()=>photoActions.requestRemoval(photos.map(photo=>photo.id))}>{$t('清空')}</button>
        </div>{/if}
      </div>
      <div class="filmstrip" role="group" aria-label={$t('照片选择')} aria-describedby="photo-selection-hint" aria-keyshortcuts="Meta+A Control+A Escape Meta+Z Control+Z Meta+Shift+Z Control+Shift+Z">
        {#each photos as photo,index (photo.id)}
          {@const job = batch?.items.find((item) => item.photoId === photo.id)}
          <button class:selected={selectedPhotoIds.has(photo.id)} class:active={photo.id===selectedId} class:drop-before={dropIndex===index} class:drop-after={dropIndex===photos.length&&index===photos.length-1} class:dragging={photoDrag?.active&&photoDrag.ids.includes(photo.id)} class="thumbnail" data-photo-id={photo.id} on:pointerdown={event=>thumbPointerDown(event,photo.id)} on:click={event=>{if(!suppressThumbClick)selectPhoto(photo.id,event);}} on:contextmenu={event=>photoActions.openPhotoMenu(event,photo.id)} aria-label={$t('预览 {0}',{0:photo.name}) + (photo.settings ? ' · ' + $t('单独设置') : '')} aria-pressed={selectedPhotoIds.has(photo.id)} aria-current={photo.id===selectedId?'true':undefined} aria-haspopup="menu" title={photo.name}>
            {#if photo.thumbnail}<img src={photo.thumbnail} alt="" draggable="false"/>{:else}<FileImage size={22}/>{/if}{#if photo.settings}<span class="own-badge" title={$t('单独设置')} aria-hidden="true"><SlidersHorizontal size={11}/></span>{/if}
            {#if job?.status==='completed'}<span class="thumbnail-status success"><Check size={10} strokeWidth={3}/></span>{:else if job?.status==='failed'}<span class="thumbnail-status failed"><AlertCircle size={11}/></span>{:else if job?.status==='running'}<span class="thumbnail-status"><LoaderCircle size={11} class="spin"/></span>{/if}
          </button>
        {/each}
        <button class="add-tile" on:click={choosePhotos} disabled={!desktop||importing||loading||!initialized} aria-label={$t('添加更多照片')}><Plus size={18}/></button>
      </div>
      <div class="library-actions">
      {#if batch && totalCount > 0}
        <button class="batch-summary text-button" on:click={() => jobDialog?.showModal()} title={$t("查看处理记录")}><List size={14} />{#if isRunning}<span>{completedCount + failedCount} / {totalCount}</span>{:else}<span>{completedCount} {$t("张已导出")}</span>{/if}{#if failedCount}<span class="failure-count">{failedCount} {$t("张失败")}</span>{/if}</button>
        {#if failedCount && !isRunning}<button class="button secondary compact" disabled={starting} title={$t("沿用本批次开始时的设置")} on:click={() => continueBatch('retry_failed')}>{$t("重试失败项")}</button>{/if}
      {/if}
      </div>
    </section>
    {#if removalUndo.length||removalRedo.length||undoError}<div class="catalog-undo"><span role={undoError?'alert':'status'}>{undoError?$t(undoError):removalNotice?$t(removalNotice.kind==='undone'?'已恢复 {0} 张照片，原图保留':'已从目录移除 {0} 张照片，原图保留',{0:removalNotice.count}):''}</span><div class="catalog-history-actions">
      {#if removalUndo.length}<button type="button" class="text-button undo-photo-removal" disabled={undoing||loading||isRunning||starting||importing||agentBusy} aria-keyshortcuts="Meta+Z Control+Z" title={$t('撤销最近一次移除，重启后仍可用（⌘ / Ctrl+Z）')} on:click={undoPhotoRemoval}><RotateCcw size={14}/>{historyAction==='undo'?$t('正在恢复照片…'):$t('撤销移除')}</button>{/if}
      {#if removalRedo.length}<button type="button" class="text-button redo-photo-removal" disabled={undoing||loading||isRunning||starting||importing||agentBusy} aria-keyshortcuts="Meta+Shift+Z Control+Shift+Z" title={$t('重新执行刚撤销的移除（⌘ / Ctrl+Shift+Z）')} on:click={redoPhotoRemoval}><RotateCw size={14}/>{historyAction==='redo'?$t('正在重做移除…'):$t('重做移除')}</button>{/if}
    </div></div>{/if}
    {#if exportDone}<div class="export-done" role="status"><CheckCircle2 size={15}/><span>{$t('已导出 {0} 张照片',{0:exportDone.count})}{#if exportDone.failed} · {$t('{0} 张失败',{0:exportDone.failed})}{/if}</span><button type="button" class="text-button" on:click={() => batch && revealPath(batch.outputDirectory)}><FolderOpen size={13}/>{$t('在 Finder 中显示')}</button><button type="button" class="icon-button" aria-label={$t('关闭提示')} on:click={() => exportDone = null}><X size={14}/></button></div>{/if}
    <div class="import-feedback" role="status" aria-live="polite" aria-atomic="true">
      {#if importing}<LoaderCircle size={12} class="spin"/><span>{$t('正在导入 {0} 张照片…',{0:importingCount})}{#if pendingImports.length}{' · ' }{$t('另有 {0} 张排队',{0:pendingImports.length})}{/if}</span>
      {:else if pendingImports.length}<span>{$t('已接收 {0} 张照片，载入后导入',{0:pendingImports.length})}</span>
      {:else if importReport}<CheckCircle2 size={12}/><span>{$t('已添加 {0} 张照片',{0:importReport.added})}{#if importReport.skipped}{' · ' }{$t('跳过 {0} 个重复、不支持或无法读取的文件',{0:importReport.skipped})}{/if}</span>{/if}
    </div>
  </footer>
  {#if dragActive}<div class="drop-overlay"><div><ImagePlus size={30} strokeWidth={1.5} /><strong>{$t(dragCount?'松开以添加 {0} 张照片':'松开以添加一组照片',{0:dragCount})}</strong><span>JPG · PNG · HEIC · HEIF</span><span>{$t(importing||loading?'会自动排队，随后逐张导入':'支持多选拖入，原照片保留在原位置')}</span></div></div>{/if}
</div>

<PhotoLibraryActions bind:this={photoActions} {photos} {selectedIds} onstep={stepPhoto} canUndo={!!removalUndo.length||!!settingsUndo.length||!!orderUndo.length||!!pendingUndo} onundo={undoAny} canRedo={!!removalRedo.length||!!settingsRedo.length||!!orderRedo.length} onredo={redoAny} onmove={nudgeSelection} disabled={!desktop||!initialized||loading||isRunning||starting||importing||undoing||agentBusy||scopeBusy} selectionDisabled={!desktop||!initialized||loading||starting||importing||pickingColor||undoing||agentBusy||scopeBusy} onselectall={selectAllPhotos} onclearselection={clearSelectedPhotos} oncontextselect={selectContextPhoto} onremove={removePhotos} onerror={message=>notices=[...notices,message]}/>

{#if agentOpen}<AgentConnection onclose={()=>agentOpen=false} onstatus={status=>agentConnected=status.enabled}/>{/if}

<dialog bind:this={exportDialog} class="export-review" aria-labelledby="export-title" on:cancel={event=>{if(starting)event.preventDefault();}}>
  <header><div><h2 id="export-title">{$t('导出照片')}</h2><p class="export-count" aria-live="polite">{$t(exportScope==='selected'?'将导出选中的 {0} 张照片':'将导出全部 {0} 张照片',{0:exportPhotoIds.length})} · {exportOwnCount?$t('{0} 张使用单独设置',{0:exportOwnCount}):$t('全部照片共用当前设置')}</p></div><button class="icon-button" aria-label={$t('返回编辑')} disabled={starting} on:click={()=>exportDialog.close()}><X size={18}/></button></header>
  <div class="export-review-body">
    <div class="export-targets" role="group" aria-label={$t('导出用途')}>
      <span class="field-label">{$t('导出用途')}</span>
      <div>{#each exportTargets as target}<button type="button" class:chosen={exportTargetId === target.id} aria-pressed={exportTargetId === target.id} disabled={starting||isRunning} on:click={() => applyExportTarget(target.id)}>{$t(target.name)}</button>{/each}</div>
      {#if exportTargetRatio}
        <fieldset class="export-scope export-composition" disabled={starting||isRunning}>
          <legend class="field-label">{$t('导出构图')}</legend>
          <label><input type="radio" name="export-composition" bind:group={uniformExport} value={false}/><span>{$t('保留每张构图')}</span></label>
          <label><input type="radio" name="export-composition" bind:group={uniformExport} value={true}/><span>{$t('本次统一为 {0}', {0:exportTargetRatio.width + ':' + exportTargetRatio.height})}</span></label>
        </fieldset>
      {/if}
    </div>
    <fieldset class="export-scope" disabled={starting||isRunning}>
      <legend class="field-label">{$t('导出范围')}</legend>
      <label><input type="radio" name="export-scope" value="selected" bind:group={exportScope} disabled={!selectedIds.length}/><span>{$t('选中的照片（{0} 张）',{0:selectedIds.length})}</span></label>
      <label><input type="radio" name="export-scope" value="all" bind:group={exportScope}/><span>{$t('全部照片（{0} 张）',{0:photos.length})}</span></label>
    </fieldset>
    <label class="field-label" for="export-directory">{$t('保存到')}</label>
    <button type="button" id="export-directory" class="export-directory" on:click={chooseOutputDirectory} disabled={!desktop||isRunning||starting} title={outputDirectory||$t('尚未选择文件夹')}><FolderOpen size={17}/><span>{outputDirectory||$t('尚未选择文件夹')}</span><ChevronDown size={14}/></button>
    <div class="export-format-row"><label><span class="field-label">{$t('文件格式')}</span><select id="export-format" bind:value={settings.export.format}><option value="jpg">JPG</option><option value="png">PNG</option></select></label>{#if settings.export.format==='jpg'}<div><div class="field-title"><label class="field-label" for="jpg-quality">{$t('JPG 质量')}</label><output>{settings.export.jpegQuality}</output></div><input id="jpg-quality" type="range" min="60" max="100" step="1" bind:value={settings.export.jpegQuality}/></div>{/if}</div>
    <label class="field-label export-name-label" for="export-name">{$t('文件名')}</label>
    <input id="export-name" class="full-width" maxlength="120" spellcheck="false" bind:value={settings.export.namePattern} disabled={starting||isRunning}/>
    <p class="field-help export-name-example">{$t('{name} 为原文件名，{index} 为序号')} · {exportNameExample}</p>
    <label class="export-keep-exif"><input type="checkbox" bind:checked={settings.export.keepCameraInfo} disabled={settings.export.format !== 'jpg'||starting||isRunning}/><span>{$t('保留拍摄参数（相机、镜头、光圈等，不含位置）')}{#if settings.export.format !== 'jpg'}<small> · {$t('仅 JPG')}</small>{/if}</span></label>
    <details class="advanced-watermark export-size-options" bind:open={advancedExport}><summary><ChevronDown size={14}/>{$t('尺寸选项')}</summary>
      <label class="field-label" for="long-edge">{$t('输出长边')}</label><select id="long-edge" value={longEdgeMode} on:change={event=>setLongEdge(event.currentTarget.value)}><option value="original">{$t('保留原始像素')}</option><option value="2560">{$t('最多 2560 px')}</option><option value="3840">{$t('最多 3840 px')}</option><option value="custom">{$t('自定义')}</option></select>
      {#if longEdgeMode==='custom'}<div class="custom-edge"><input aria-label={$t('自定义输出长边')} type="number" min="64" max="32768" step="1" bind:value={customLongEdge} on:input={event=>settings.export.longEdge=event.currentTarget.valueAsNumber}/><span>px</span></div>{/if}<p class="field-help">{$t('按最终画布缩小，不放大照片；统一导出为 sRGB')}</p>
    </details>
    {#if preview && !previewLoading && !trialSettings && !uniformExport && exportPhotoIds.includes(selectedId)}<div class="export-dimensions"><span>{$t('当前照片预计尺寸')}</span><span>{preview.width.toLocaleString()} × {preview.height.toLocaleString()} px</span></div>{/if}
    {#if localError}<p class="field-error" role="alert">{localError}</p>{/if}
    {#if exportError}<p class="field-error export-error" role="alert">{$t(exportError)}</p>{/if}
    <p class="field-help">{$t('另存副本，保留原图')}</p>
  </div>
  <footer><button class="button secondary" disabled={starting} on:click={()=>exportDialog.close()}>{$t('返回编辑')}</button><button type="button" class="button primary start-export" on:click={startBatch} disabled={!desktop||!initialized||!exportPhotoIds.length||!!localError||starting||importing||isRunning}>{#if starting}<LoaderCircle size={16} class="spin"/>{:else}<Download size={16}/>{/if}{starting?$t('准备导出'):$t(exportPhotoIds.length===1?'开始导出 1 张':'开始导出 {0} 张',{0:exportPhotoIds.length})}</button></footer>
</dialog>

<ShortcutSheet bind:this={shortcutSheet}/>

<dialog bind:this={jobDialog} class="job-dialog">
  <header><div><h2>{$t("处理记录")}</h2><p>{completedCount} {$t("张已导出")}{failedCount ? $t("，{0} 张失败", {0:failedCount}) : ''}{isRunning ? $t(" · 正在处理") : ''} {$t("· 沿用本批次开始时的设置")}</p></div><button class="icon-button" aria-label={$t("关闭处理记录")} on:click={() => jobDialog.close()}><X size={18} /></button></header>
  <div class="job-list">{#each batch?.items ?? [] as item}<div class="job-row"><span class:failed={item.status === 'failed'} class:done={item.status === 'completed'} class="job-icon">{#if item.status === 'completed'}<CheckCircle2 size={16} />{:else if item.status === 'failed'}<AlertCircle size={16} />{:else if item.status === 'running'}<LoaderCircle size={16} class="spin" />{:else}<FileImage size={16} />{/if}</span><div><strong>{item.name}</strong><p>{$t(item.error??'') || (item.width && item.height ? `${item.width} × ${item.height} px` : $t(jobLabels[item.status]))}</p></div>{#if item.outputPath && item.status === 'completed'}<button class="text-button" on:click={() => item.outputPath && revealPath(item.outputPath)}>{$t("打开")}</button>{:else}<span class="job-state">{$t(jobLabels[item.status])}</span>{/if}</div>{/each}</div>
  <footer><span>{$t("每张照片独立处理，失败不影响其他照片")}</span>{#if batch?.outputDirectory}<button class="button secondary compact" on:click={() => batch && revealPath(batch.outputDirectory)}><FolderOpen size={14} />{$t("打开文件夹")}</button>{/if}</footer>
</dialog>

{#if modelOpen && selectedPhoto}<ModelPanel bind:initialMode={modelMode} bind:imageTarget={modelTarget} initialConfig={stagedModelConfig} onconfigure={configureModels} photo={selectedPhoto} count={frameAffectedCount} baseSettings={settings} onsavetemplate={saveSuggestedFrame} bind:apiKey={sessionApiKey} bind:apiOrigin={sessionApiOrigin} bind:imageApiKey={sessionImageApiKey} bind:imageApiOrigin={sessionImageApiOrigin} onapply={applyArt} onclose={()=>{modelOpen=false;stagedModelConfig=null;}} />{/if}
{#if modelsConfigOpen}<ModelsConfig initialConfig={stagedModelConfig} target={modelConfigTarget} bind:apiKey={sessionApiKey} bind:apiOrigin={sessionApiOrigin} bind:imageApiKey={sessionImageApiKey} bind:imageApiOrigin={sessionImageApiOrigin} onsaved={value=>stagedModelConfig=clone(value)} onclose={closeModelConfig}/>{/if}

{#if templatesOpen}<FrameTemplates photo={selectedPhoto} currentPreview={preview?.dataUrl||''} currentSettings={settings} templates={frameTemplates} draft={templateDraft} initialName={templateName} initialCategory={templateCategory} onsave={saveFrameTemplate} ondelete={deleteFrameTemplate} onapply={applyFrameTemplate} onclose={()=>templatesOpen=false}/>{/if}
