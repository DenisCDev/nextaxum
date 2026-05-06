import { describe, expect, test } from "vitest";
import { render, screen } from "@testing-library/react";
import Page from "@/app/page";

describe("home page", () => {
  test("renders the heading", () => {
    render(<Page />);
    expect(
      screen.getByRole("heading", { level: 1, name: "NextAxum" }),
    ).toBeInTheDocument();
  });

  test("links to login and dashboard", () => {
    render(<Page />);
    expect(screen.getByRole("link", { name: "Login" })).toHaveAttribute(
      "href",
      "/login",
    );
    expect(screen.getByRole("link", { name: "Dashboard" })).toHaveAttribute(
      "href",
      "/dashboard",
    );
  });
});
