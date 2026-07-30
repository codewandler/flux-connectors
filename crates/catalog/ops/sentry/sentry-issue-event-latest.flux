op sentry-issue-event-latest(organization_id_or_slug: String, issue_id: String) -> Any
  description "Get the most recent event of an issue — the actual occurrence, with its stack trace, breadcrumbs, request context and tags. This is what makes an issue diagnosable rather than merely countable; the event list itself is excluded because it pages with an opaque cursor. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/detail` in the response body."
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://sentry.io"
  url = fmt("{base}/api/0/organizations/{organization_id_or_slug}/issues/{issue_id}/events/latest/")
  response = http.request(method: "GET", url)
  return response
