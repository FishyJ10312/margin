export interface Ratio {
  width: number;
  height: number;
}

export type WatermarkKind = 'none' | 'text' | 'image';
export type WatermarkRegion = 'photo' | 'border';
export type WatermarkPosition =
  | 'top-left' | 'top-center' | 'top-right'
  | 'center-left' | 'center' | 'center-right'
  | 'bottom-left' | 'bottom-center' | 'bottom-right';

export interface WatermarkSettings {
  kind: WatermarkKind;
  text: string;
  imagePath: string | null;
  fontPath: string | null;
  fontFamily: string | null;
  offsetX: number;
  offsetY: number;
  color: string;
  opacity: number;
  sizePercent: number;
  region: WatermarkRegion;
  position: WatermarkPosition;
  marginPercent: number;
}

export interface ExportSettings {
  format: 'jpg' | 'png';
  jpegQuality: number;
  longEdge: number | null;
}

export interface RenderSettings {
  ratio: Ratio | null;
  borderPercent: number;
  frameColor: string;
  decoration: FrameDecoration;
  artwork?:ArtScene|null;
  watermark: WatermarkSettings;
  export: ExportSettings;
}

export interface Photo {
  id: string;
  path: string;
  name: string;
  width: number;
  height: number;
  thumbnail: string;
}

export interface PreviewResult {
  dataUrl: string;
  sourceThumbnail?: string | null;
  width: number;
  height: number;
  warning?: string | null;
  photoBounds: { x: number; y: number; width: number; height: number };
}

export interface Preset {
  id: string;
  name: string;
  settings: RenderSettings;
}

export type JobStatus = 'pending' | 'running' | 'completed' | 'failed' | 'cancelled';

export interface JobItem {
  id: string;
  photoId: string;
  name: string;
  sourcePath: string;
  status: JobStatus;
  outputPath?: string | null;
  error?: string | null;
  width?: number;
  height?: number;
}

export interface BatchSnapshot {
  id: string;
  revision: number;
  status: 'idle' | 'running' | 'cancelled' | 'completed' | 'interrupted';
  outputDirectory: string;
  settings: RenderSettings;
  items: JobItem[];
  error?: string | null;
}

export interface AppSnapshot {
  settings: RenderSettings;
  presets: Preset[];
  frameTemplates:FrameTemplate[];
  photos: Photo[];
  batch: BatchSnapshot | null;
  warnings?: string[];
}

export interface ImportResult {
  photos: Photo[];
  errors: string[];
}

export const defaultSettings: RenderSettings = {
  ratio: null,
  borderPercent: 5,
  frameColor: '#FFFFFF',
  decoration: {kind:'none',color:'#75816F',opacity:0.12,scale:1},
  watermark: {
    kind: 'none',
    text: '',
    imagePath: null,
    fontPath: null,
    fontFamily: null,
    offsetX: 0,
    offsetY: 0,
    color: '#FFFFFF',
    opacity: 0.8,
    sizePercent: 3,
    region: 'photo',
    position: 'bottom-right',
    marginPercent: 2,
  },
  export: { format: 'jpg', jpegQuality: 95, longEdge: null },
};

export interface FontOption { family: string; path: string;chinese?:boolean;bundled?:boolean }
export interface ColorSample { x: number; y: number; hex: string; patchDataUrl: string }

export interface FrameDecoration {kind:'none'|'paper'|'linen'|'dots'|'lines'|'waves'|'corners';color:string;opacity:number;scale:number}
export interface FrameSuggestion {title:string;rationale:string;frameColor:string;borderPercent:number;decoration:FrameDecoration;artwork:ArtScene}
export interface ImageConfig {provider:'codex'|'openai';baseUrl:string;model:string;quality:'low'|'medium'|'high';size:'1024x1024'|'1536x1024'|'1024x1536';codexModel:string}
export const defaultImageConfig:ImageConfig={provider:'codex',baseUrl:'https://api.openai.com/v1',model:'gpt-image-2.5-flare',quality:'medium',size:'1024x1024',codexModel:''};
export interface FrameDesign {style:'auto'|'graphic'|'editorial'|'film'|'linework'|'collage'|'illustration'|'custom';placement:'auto'|'corners'|'sides'|'bottom';palette:'photo'|'mono'|'custom';colors:string[];texture:boolean;notes:string}
export const defaultFrameDesign:FrameDesign={style:'auto',placement:'auto',palette:'photo',colors:['#315D69','#C27B5E','#D6C3A0'],texture:false,notes:''};
export interface AiConfig {frameDesign:FrameDesign;image:ImageConfig;provider:'pi'|'chatgpt'|'codex'|'api';piProvider?:string;piPath?:string|null;chatgptAccountId?:string|null;baseUrl:string;model:string;codexPath:string|null;styleDirection:string;designDensity:'auto'|'simple'|'balanced'|'rich';language?:'zh-CN'|'en'}
export const defaultAiConfig:AiConfig={frameDesign:{...defaultFrameDesign,colors:[...defaultFrameDesign.colors]},image:{...defaultImageConfig},provider:'codex',piProvider:'openai',piPath:null,chatgptAccountId:null,baseUrl:'',model:'',codexPath:null,styleDirection:'',designDensity:'auto'};
export const piProviders=[{id:'openai',name:'OpenAI / ChatGPT',billing:'ChatGPT 订阅用量'},{id:'anthropic',name:'Claude Pro / Max',billing:'Claude 订阅用量'},{id:'github-copilot',name:'GitHub Copilot',billing:'Copilot 套餐用量'},{id:'openrouter',name:'OpenRouter',billing:'OpenRouter 账户余额'}];
export interface PiRuntimeInfo {available:boolean;path:string|null;version:string|null}
export interface PiModel {id:string;name:string;input:string[];contextWindow:number;reasoning:boolean}
export interface PiCatalog {provider:string;connected:boolean;authType:'oauth'|'api_key'|null;models:PiModel[]}
export interface PiPrompt {id:string;kind:'text'|'secret'|'select'|'manual_code';message:string;placeholder:string;options:{id:string;label:string;description?:string}[]}
export interface PiAuthEvent {sessionId:string;provider:string;phase:string;host:string|null;deviceCode:string|null;prompt:PiPrompt|null}
export interface ChatGptAccount {id:string;label:string;connected:boolean;planEnabled:boolean}
export interface ChatGptDiagnostic {status:number;code:string|null;param:string|null;requestId:string|null;bodyShape:string}
export interface ChatGptState {accounts:ChatGptAccount[];activeId:string|null;diagnostic?:ChatGptDiagnostic|null}
export interface ChatGptModel {slug:string;displayName:string}
export interface AiState {imageModels?:string[];config:AiConfig;codexPath:string|null;codexLoggedIn:boolean}
export interface AiPhoto {token:string;dataUrl:string;width:number;height:number;before?:string|null;geometry?:{canvasWidth:number;canvasHeight:number;photoBounds:{x:number;y:number;width:number;height:number};borderPercent:number}|null}

export interface SampleImage {dataUrl:string;x:number;y:number;width:number;height:number}
export interface OffsetLimits {minX:number;maxX:number;minY:number;maxY:number}
export interface WatermarkDetail {dataUrl:string;viewWidth:number;viewHeight:number;watermarkBounds:{x:number;y:number;width:number;height:number};position:{x:number;y:number;width:number;height:number};width:number;height:number;limits:OffsetLimits}

export interface ArtElement {kind:string;x:number;y:number;width:number;height:number;rotation:number;color:string;opacity:number;strokeWidth:number;filled:boolean;points:[number,number][]}
export interface DesignRecipe {id:string;colors:string[];intensity:number;scale?:number;film?:{labels:boolean;texture:number}|null}
export interface Sticker {imagePath:string;anchor:string;scale:number;opacity:number;flipX?:boolean}
export interface FrameImage {imagePath:string;reference:{width:number;height:number;x:number;y:number;photoWidth:number;photoHeight:number};opacity:number}
export interface ArtScene {elements:ArtElement[];design?:DesignRecipe|null;reference?:unknown;stickers?:Sticker[];frameImage?:FrameImage|null}
export interface BuiltinFrame {id:string;name:string;englishName:string;category:string;description:string;settings:RenderSettings;thumbnail:string}
export interface GeneratedArt {imagePath:string;thumbnail:string;width:number;height:number;summary:string}

export interface FrameTemplate {id:string;name:string;settings:RenderSettings;thumbnail:string}
