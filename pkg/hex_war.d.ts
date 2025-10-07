/* tslint:disable */
/* eslint-disable */
/**
 * Initialize with *per-team* count (0..=5) and speed
 */
export function init_app(canvas_id: string, css_w: number, css_h: number, balls_per_team: number, speed: number): void;
export function start(): void;
export function stop(): void;
export function reset_grid(): void;
export function set_speed(multiplier: number): void;
/**
 * Set balls *per team* (0..=5)
 */
export function set_balls_per_team(n: number): void;
/**
 * Back-compat alias (treat input as per-team)
 */
export function set_num_balls(n: number): void;
export function resize(css_w: number, css_h: number): void;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
  readonly memory: WebAssembly.Memory;
  readonly init_app: (a: number, b: number, c: number, d: number, e: number, f: number) => [number, number];
  readonly start: () => [number, number];
  readonly stop: () => void;
  readonly reset_grid: () => void;
  readonly set_speed: (a: number) => void;
  readonly set_balls_per_team: (a: number) => void;
  readonly resize: (a: number, b: number) => void;
  readonly set_num_balls: (a: number) => void;
  readonly __wbindgen_exn_store: (a: number) => void;
  readonly __externref_table_alloc: () => number;
  readonly __wbindgen_export_2: WebAssembly.Table;
  readonly __wbindgen_malloc: (a: number, b: number) => number;
  readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
  readonly __wbindgen_export_5: WebAssembly.Table;
  readonly __externref_table_dealloc: (a: number) => void;
  readonly wasm_bindgen__convert__closures_____invoke__h063f14237101987b: (a: number, b: number, c: number) => void;
  readonly __wbindgen_start: () => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;
/**
* Instantiates the given `module`, which can either be bytes or
* a precompiled `WebAssembly.Module`.
*
* @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
*
* @returns {InitOutput}
*/
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
* If `module_or_path` is {RequestInfo} or {URL}, makes a request and
* for everything else, calls `WebAssembly.instantiate` directly.
*
* @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
*
* @returns {Promise<InitOutput>}
*/
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
