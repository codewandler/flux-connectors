op anthropic-invites-list -> Any
  description "List the organization's invites, with the email address invited, the role offered, and whether the invite is still pending. Returns personal data — an email address for a real person. Unpaginated and unfiltered by status, so a `pending` invite must be selected from the returned entries rather than asked for. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/error/message`, its error code at `/error/type` in the response body."
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://api.anthropic.com"
  url = fmt("{base}/v1/organizations/invites")
  anthropic_version = "2023-06-01"
  response = http.request(headers: { "anthropic-version": anthropic_version }, method: "GET", url)
  return response
