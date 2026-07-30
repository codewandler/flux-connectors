op sentry-project-get(organization_id_or_slug: String, project_id_or_slug: String) -> Any
  description "Get one project — its name, slug, platform, teams, DSN-bearing client keys' status and whether it is currently accepting events. This is the project an issue belongs to, so it is how a triage flow finds out which service is broken. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/detail` in the response body."
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  $base = "https://sentry.io"
  $url = fmt("{base}/api/0/projects/{organization_id_or_slug}/{project_id_or_slug}/")
  $response = http.request({ method: "GET", url: $url })
  return $response
