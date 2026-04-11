import {
  canRestoreGoogleSession,
  isAccessTokenExpired,
  TOKEN_REFRESH_SKEW_MS,
} from "@/services/google-calendar/session";

describe("google calendar session helpers", () => {
  it("treats tokens near expiry as expired to leave refresh headroom", () => {
    expect(
      isAccessTokenExpired(
        "2026-04-10T10:01:00.000Z",
        Date.parse("2026-04-10T10:00:30.000Z"),
        TOKEN_REFRESH_SKEW_MS,
      ),
    ).toBe(true);
  });

  it("accepts active sessions only when both refresh token and client id exist", () => {
    expect(
      canRestoreGoogleSession({
        refreshToken: "refresh-token",
        clientId: "client-id",
      }),
    ).toBe(true);

    expect(
      canRestoreGoogleSession({
        refreshToken: "refresh-token",
        clientId: "",
      }),
    ).toBe(false);
  });
});
