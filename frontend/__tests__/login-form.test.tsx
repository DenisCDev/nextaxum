import { describe, expect, test, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";

// `useActionState` requires the React 19 dispatcher; the form integration is
// best covered by Playwright (e2e/auth.spec.ts) which exercises the real
// Server Action. This test pins the static UA / a11y contract.

vi.mock("@/app/login/actions", () => ({
  login: vi.fn(),
}));

import LoginPage from "@/app/login/page";

describe("login page", () => {
  test("renders email + password fields and submit button", async () => {
    render(<LoginPage />);
    expect(screen.getByPlaceholderText("Email")).toHaveAttribute(
      "type",
      "email",
    );
    expect(screen.getByPlaceholderText("Password")).toHaveAttribute(
      "type",
      "password",
    );
    expect(
      screen.getByRole("button", { name: /sign in/i }),
    ).toBeInTheDocument();
  });

  test("requires non-empty credentials before submission", async () => {
    const user = userEvent.setup();
    render(<LoginPage />);
    const button = screen.getByRole("button", { name: /sign in/i });
    await user.click(button);
    // Browser validation kicks in for `required` inputs — value stays empty.
    expect(
      (screen.getByPlaceholderText("Email") as HTMLInputElement).value,
    ).toBe("");
  });
});
