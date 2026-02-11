import { readFile } from 'fs/promises';
import { dirname, join } from 'path';
import { fileURLToPath } from 'url';
import init, { add } from './dist/wasm_demo.js';

const __dirname = dirname(fileURLToPath(import.meta.url));
const wasmPath = join(__dirname, 'target/wasm32-unknown-unknown/release/wasm_demo.wasm');

const wasmBytes = await readFile(wasmPath);
await init(wasmBytes);

console.log('Module initialized via GENERATED bindings');

const result = add(2, 3);
console.log(`add(2, 3) = ${result}`);
if (result !== 5) throw new Error(`Expected 5, got ${result}`);

console.log('e2e test passed!');
