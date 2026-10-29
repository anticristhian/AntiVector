import '@testing-library/jest-dom/vitest';
import { afterEach, vi } from 'vitest';
import { cleanup } from '@testing-library/react';
afterEach(() => cleanup());
class ResizeObserverStub { observe() {} unobserve() {} disconnect() {} }
vi.stubGlobal('ResizeObserver', ResizeObserverStub);
class PathStub { constructor(public data?: string) {} }
vi.stubGlobal('Path2D', PathStub);
vi.stubGlobal('createImageBitmap', vi.fn(async () => ({ width: 128, height: 128, close: vi.fn() })));
Object.defineProperty(HTMLImageElement.prototype, 'decode', { value: vi.fn(async () => {}) });
vi.stubGlobal('PointerEvent', MouseEvent);
const context = { setTransform: vi.fn(), clearRect: vi.fn(), save: vi.fn(), restore: vi.fn(), translate: vi.fn(), scale: vi.fn(), drawImage: vi.fn(), beginPath: vi.fn(), rect: vi.fn(), clip: vi.fn(), stroke: vi.fn(), fill: vi.fn(), fillRect: vi.fn(), arc: vi.fn(), isPointInPath: vi.fn(() => false), fillStyle: '', strokeStyle: '', lineWidth: 1, imageSmoothingEnabled: true };
Object.defineProperty(HTMLCanvasElement.prototype, 'getContext', { value: () => context });
