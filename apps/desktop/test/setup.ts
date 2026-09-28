import { cleanup } from "@testing-library/react";
import { afterEach } from "vitest";

// jsdom does not implement these browser APIs.
Element.prototype.scrollIntoView = function scrollIntoView() {
  return undefined;
};
window.matchMedia = (query: string) =>
  ({
    matches: false,
    media: query,
    onchange: null,
    addEventListener: () => undefined,
    removeEventListener: () => undefined,
    addListener: () => undefined,
    removeListener: () => undefined,
    dispatchEvent: () => false,
  }) as MediaQueryList;

afterEach(cleanup);
