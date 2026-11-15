<script lang="ts">
 import type { FrameDecoration } from './types';
 export let decoration: FrameDecoration;
 export let background = '#F3ECDD';
 export let photo = '';
 const points=Array.from({length:80},(_,i)=>({x:(i*37)%100,y:(i*23)%70}));
</script>
<svg viewBox="0 0 100 70" aria-hidden="true">
 <rect width="100" height="70" rx="3" fill={background} />
 <g stroke={decoration.color} fill={decoration.color} opacity={Math.max(0.1,decoration.opacity)}>
 {#if decoration.kind === 'dots'}{#each points.slice(0,20) as _,i}<circle cx={(i%5)*22+6} cy={Math.floor(i/5)*20+5} r="0.8" stroke="none" />{/each}
 {:else if decoration.kind === 'lines'}{#each Array(16) as _,i}<path d={`M${i*10-65} 70 l70 -70`} fill="none" stroke-width=".7" />{/each}
 {:else if decoration.kind === 'waves'}{#each Array(8) as _,i}<path d={`M0 ${i*12} q12 -8 25 0 t25 0 t25 0 t25 0`} fill="none" stroke-width=".7" />{/each}
 {:else if decoration.kind === 'paper'}{#each points as p}<circle cx={p.x} cy={p.y} r=".65" stroke="none" />{/each}
 {:else if decoration.kind === 'linen'}{#each Array(30) as _,i}<path d={`M${i*4} 0 v70 M0 ${i*4} h100`} stroke-width=".45" fill="none" />{/each}
 {:else if decoration.kind === 'corners'}<path d="M4 15 V4 H15 M85 4 H96 V15 M96 55 V66 H85 M15 66 H4 V55" fill="none" stroke-width="1" />{/if}
 </g>
 {#if photo}<image href={photo} x="18" y="12" width="64" height="46" preserveAspectRatio="xMidYMid meet" />{:else}<rect x="18" y="12" width="64" height="46" fill="#D4DACF" />{/if}
</svg>
