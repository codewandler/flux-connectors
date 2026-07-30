op sentry-issue-update(organization_id_or_slug: String, issue_id: String, status: String) -> Any
  description "Change an issue's triage state: resolve it, ignore it, or return it to unresolved. This is the state the whole organization triages against — a resolved issue leaves the unresolved queue, and an ignored one stops alerting until it recurs on Sentry's terms. Recorded in the issue's activity feed under the token's owner. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/detail` in the response body."
  risk "high"
  idempotency "non_idempotent"
  effects ["network"]
  expose true

  $base = "https://sentry.io"
  $url = fmt("{base}/api/0/organizations/{organization_id_or_slug}/issues/{issue_id}/")
  $content_type = "application/json"
  $payload = { status: $status }
  $response = http.request({ body: $payload, headers: { "content-type": $content_type }, method: "PUT", url: $url })
  return $response
