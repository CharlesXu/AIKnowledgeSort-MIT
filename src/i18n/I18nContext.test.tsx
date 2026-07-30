import { fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, test } from "vitest";
import { I18nProvider, useI18n } from "./I18nContext";

function Harness() {
  const { language, setLanguage, t } = useI18n();
  return (
    <>
      <span>{language}</span>
      <strong>{t("settings.title")}</strong>
      <button onClick={() => setLanguage("zh-CN")} type="button">
        中文
      </button>
    </>
  );
}

describe("I18nProvider", () => {
  afterEach(() => {
    window.localStorage.clear();
    document.documentElement.lang = "";
  });

  test("switches language, persists the selection, and updates the document language", () => {
    const first = render(
      <I18nProvider initialLanguage="en">
        <Harness />
      </I18nProvider>,
    );
    expect(screen.getByText("Settings")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "中文" }));

    expect(screen.getByText("设置")).toBeInTheDocument();
    expect(window.localStorage.getItem("aiks.ui.language.v1")).toBe("zh-CN");
    expect(document.documentElement.lang).toBe("zh-CN");

    first.unmount();
    render(
      <I18nProvider>
        <Harness />
      </I18nProvider>,
    );
    expect(screen.getByText("设置")).toBeInTheDocument();
  });
});
