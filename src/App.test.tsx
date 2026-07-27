import { render, screen } from "@testing-library/react";
import { expect, test } from "vitest";
import App from "./App";

test("renders the source workbench landmark", () => {
  render(<App />);

  expect(
    screen.getByRole("main", { name: "Source workbench" }),
  ).toBeInTheDocument();
});
