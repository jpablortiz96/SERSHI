import { act, fireEvent, render, screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";

import { TitleBar } from "../src/components/shell/TitleBar";
import { useLocaleStore } from "../src/i18n";
import { connectLocale } from "../src/i18n/store";
import { SettingsView } from "../src/surfaces/command-center/SettingsView";

describe("switching language at runtime", () => {
  beforeEach(() => {
    localStorage.clear();
    useLocaleStore.setState({ preference: "auto", systemLocale: "en-US", locale: "en-US" });
  });

  it("updates every visible surface immediately, without a restart", () => {
    const cleanup = connectLocale();
    render(
      <>
        <TitleBar view="settings" onNavigate={() => undefined} />
        <SettingsView />
      </>,
    );
    expect(screen.getByRole("button", { name: "Home" })).toBeTruthy();
    expect(document.documentElement.lang).toBe("en-US");

    const picker = screen.getByRole("radiogroup", { name: "Language" });
    fireEvent.click(within(picker).getByLabelText("Español"));

    expect(screen.getByRole("button", { name: "Inicio" })).toBeTruthy();
    expect(screen.getByRole("heading", { level: 1, name: "Configuración" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Minimizar" })).toBeTruthy();
    expect(document.documentElement.lang).toBe("es-419");

    fireEvent.click(within(screen.getByRole("radiogroup")).getByLabelText("Português"));
    expect(screen.getByRole("button", { name: "Início" })).toBeTruthy();
    expect(screen.getByRole("radiogroup", { name: "Idioma" })).toBeTruthy();
    cleanup();
  });

  it("Automatic shows and follows the detected system language", () => {
    useLocaleStore.setState({ preference: "es-419", systemLocale: "pt-BR", locale: "es-419" });
    render(<SettingsView />);
    const auto = screen.getByRole("radio", { name: /Automático/ });
    expect(auto.closest("label")?.textContent).toContain("Português");
    fireEvent.click(auto);
    expect(useLocaleStore.getState().locale).toBe("pt-BR");
    expect(screen.getByRole("heading", { level: 1, name: "Configurações" })).toBeTruthy();
  });

  it("follows a change made in the other SERSHI window", () => {
    const cleanup = connectLocale();
    render(<TitleBar view="home" onNavigate={() => undefined} />);
    act(() => {
      window.dispatchEvent(
        new StorageEvent("storage", {
          key: "sershi.preferences.v1",
          newValue: JSON.stringify({ uiLocale: "pt-BR", conversationLanguage: "automatic" }),
        }),
      );
    });
    expect(screen.getByRole("button", { name: "Configurações" })).toBeTruthy();
    cleanup();
  });
});
