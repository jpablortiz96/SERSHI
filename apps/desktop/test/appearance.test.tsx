import { act, fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it } from "vitest";

import { useLocaleStore } from "../src/i18n";
import { DEFAULT_PREFERENCES, parsePreferences, PREFERENCES_KEY } from "../src/i18n/preferences";
import { SettingsView } from "../src/surfaces/command-center/SettingsView";
import { connectAppearance, motionReduced, useAppearance } from "../src/visual/appearance";

describe("appearance preferences", () => {
  let disconnect: () => void = () => undefined;

  beforeEach(() => {
    localStorage.clear();
    useLocaleStore.setState({ preference: "auto", systemLocale: "en-US", locale: "en-US" });
    useAppearance.setState({ motion: "system", companionSize: "medium", systemReduced: false });
    disconnect = connectAppearance();
  });
  afterEach(() => {
    disconnect();
  });

  it("parse safely, falling back to defaults", () => {
    expect(parsePreferences(null)).toEqual(DEFAULT_PREFERENCES);
    expect(parsePreferences('{"motion":"wild","companionSize":"huge"}')).toEqual(
      DEFAULT_PREFERENCES,
    );
    expect(parsePreferences('{"motion":"reduced","companionSize":"large"}')).toMatchObject({
      motion: "reduced",
      companionSize: "large",
    });
  });

  it("drive a single <html data-motion> switch from the user or the system", () => {
    expect(document.documentElement.dataset.motion).toBe("full");
    act(() => {
      useAppearance.getState().setMotion("reduced");
    });
    expect(document.documentElement.dataset.motion).toBe("reduced");
    act(() => {
      useAppearance.setState({ motion: "system", systemReduced: true });
    });
    expect(motionReduced()).toBe(true);
    expect(document.documentElement.dataset.motion).toBe("reduced");
  });

  it("persist without losing the language choice", () => {
    useLocaleStore.getState().setPreference("pt-BR");
    act(() => {
      useAppearance.getState().setCompanionSize("large");
    });
    const stored = parsePreferences(localStorage.getItem(PREFERENCES_KEY));
    expect(stored).toMatchObject({ uiLocale: "pt-BR", companionSize: "large" });
    expect(document.documentElement.dataset.companionSize).toBe("large");
  });

  it("follow changes made in another SERSHI window", () => {
    act(() => {
      window.dispatchEvent(
        new StorageEvent("storage", {
          key: PREFERENCES_KEY,
          newValue: JSON.stringify({ motion: "reduced", companionSize: "small" }),
        }),
      );
    });
    expect(useAppearance.getState()).toMatchObject({ motion: "reduced", companionSize: "small" });
  });

  it("are real, localized settings in the Appearance section", () => {
    render(<SettingsView />);
    const motion = screen.getByRole("radiogroup", { name: "Motion" });
    fireEvent.click(within(motion).getByLabelText("Reduced"));
    expect(useAppearance.getState().motion).toBe("reduced");
    const size = screen.getByRole("radiogroup", { name: "Companion size" });
    fireEvent.click(within(size).getByLabelText("Small"));
    expect(useAppearance.getState().companionSize).toBe("small");

    act(() => {
      useLocaleStore.setState({ locale: "es-419" });
    });
    expect(screen.getByRole("radiogroup", { name: "Movimiento" })).toBeTruthy();
    expect(screen.getByRole("radiogroup", { name: "Tamaño del compañero" })).toBeTruthy();
  });
});
