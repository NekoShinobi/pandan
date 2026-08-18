import {
  ApiError,
  fetchAuthenticationConfig,
  fetchDashboard,
  fetchSetupStatus,
} from "$lib/api";
import type { PageLoad } from "./$types";

export const load: PageLoad = async ({ fetch }) => {
  const [setup, auth] = await Promise.all([
    fetchSetupStatus(fetch),
    fetchAuthenticationConfig(fetch).catch(() => ({
      password_login_enabled: true,
      password_registration_enabled: true,
      oidc_enabled: false,
      oidc_registration_enabled: false,
      oidc_provider_name: null,
    })),
  ]);
  if (setup.required) {
    return { dashboard: null, error: null, auth, setup };
  }
  try {
    return {
      dashboard: await fetchDashboard(fetch),
      error: null,
      auth,
      setup,
    };
  } catch (reason: unknown) {
    if (reason instanceof ApiError && reason.status === 401) {
      return { dashboard: null, error: null, auth, setup };
    }
    return {
      dashboard: null,
      auth,
      setup,
      error:
        reason instanceof Error ? reason.message : "Unable to reach the API",
    };
  }
};
