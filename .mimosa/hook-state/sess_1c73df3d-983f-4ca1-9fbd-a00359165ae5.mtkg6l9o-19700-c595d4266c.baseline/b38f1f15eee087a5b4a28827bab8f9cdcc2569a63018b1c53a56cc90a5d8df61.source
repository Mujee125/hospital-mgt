import "@testing-library/jest-dom";

// Radix UI (used by shadcn/ui Select, Dialog, etc.) requires these browser
// APIs that jsdom doesn't implement. Stub them so component tests don't crash.
global.matchMedia =
  global.matchMedia ||
  ((query: string) => ({
    matches: false,
    media: query,
    onchange: null,
    addListener: () => {},
    removeListener: () => {},
    addEventListener: () => {},
    removeEventListener: () => {},
    dispatchEvent: () => false,
  }));

class ResizeObserverStub {
  observe() {}
  unobserve() {}
  disconnect() {}
}
global.ResizeObserver = ResizeObserverStub as unknown as typeof ResizeObserver;

// jsdom doesn't implement scrollIntoView
Element.prototype.scrollIntoView = Element.prototype.scrollIntoView || (() => {});

// Pointer capture API (used by Radix Select)
Element.prototype.hasPointerCapture = Element.prototype.hasPointerCapture || (() => false);
Element.prototype.setPointerCapture = Element.prototype.setPointerCapture || (() => {});
Element.prototype.releasePointerCapture =
  Element.prototype.releasePointerCapture || (() => {});

// PointerEvent (used by Radix)
global.PointerEvent =
  global.PointerEvent ||
  (class PointerEvent extends Event {} as unknown as typeof PointerEvent);

// elementFromPoint (used by Radix for overlay positioning)
document.elementFromPoint = document.elementFromPoint || (() => null);
