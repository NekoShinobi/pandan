import {
  ApiError,
  fetchDashboard,
  fetchOidcConfig,
  fetchSetupStatus,
} from "$lib/api";
import type { PageLoad } from "./$types";

export const load: PageLoad = async ({ fetch }) => {
  const [setup, oidc] = await Promise.all([
    fetchSetupStatus(fetch),
    fetchOidcConfig(fetch).catch(() => ({
      enabled: false,
      provider_name: null,
    })),
  ]);
  if (setup.required) {
    return { dashboard: null, error: null, oidc, setup };
  }
  try {
    return {
      dashboard: await fetchDashboard(fetch),
      error: null,
      oidc,
      setup,
    };
  } catch (reason: unknown) {
    if (reason instanceof ApiError && reason.status === 401) {
      return { dashboard: null, error: null, oidc, setup };
    }
    return {
      dashboard: null,
      oidc,
      setup,
      error:
        reason instanceof Error ? reason.message : "Unable to reach the API",
    };
  }
};
