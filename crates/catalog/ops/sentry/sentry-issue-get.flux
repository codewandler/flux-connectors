op sentry-issue-get(organization_id_or_slug: String, issue_id: String) -> Any
  description "Get one issue — its title, culprit, level, status, assignee, first and last seen timestamps and event counts. An issue is Sentry's group of like events, not a single occurrence; use `sentry-issue-event-latest` for the stack trace. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/detail` in the response body."
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://sentry.io"
  url = fmt("{base}/api/0/organizations/{organization_id_or_slug}/issues/{issue_id}/")
  response = http.request(method: "GET", url)
  return response
