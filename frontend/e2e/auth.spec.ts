import { expect, test } from "@playwright/test";

const TEST_USER = {
  email: process.env.E2E_TEST_USER_EMAIL,
  password: process.env.E2E_TEST_USER_PASSWORD,
};

test.skip(
  !TEST_USER.email || !TEST_USER.password,
  "E2E_TEST_USER_EMAIL and E2E_TEST_USER_PASSWORD must be set",
);

test.describe("auth + items happy path", () => {
  test("home → login → dashboard → add item → logout", async ({ page }) => {
    await page.goto("/");
    await expect(
      page.getByRole("heading", { level: 1, name: "NextAxum" }),
    ).toBeVisible();

    await page.getByRole("link", { name: "Login" }).click();
    await expect(page).toHaveURL(/\/login$/);

    await page.getByPlaceholder("Email").fill(TEST_USER.email!);
    await page.getByPlaceholder("Password").fill(TEST_USER.password!);
    await page.getByRole("button", { name: /sign in/i }).click();

    await expect(page).toHaveURL(/\/dashboard$/);
    await expect(
      page.getByRole("heading", { level: 1, name: "Dashboard" }),
    ).toBeVisible();

    const title = `e2e-${Date.now()}`;
    await page.getByPlaceholder("New item title").fill(title);
    await page.getByRole("button", { name: /^add$/i }).click();
    await expect(page.getByText(title)).toBeVisible();

    await page.getByRole("button", { name: /^logout$/i }).click();
    await expect(page).toHaveURL(/\/login$/);
  });
});
