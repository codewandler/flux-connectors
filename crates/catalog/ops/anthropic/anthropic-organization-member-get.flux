op anthropic-organization-member-get(user_id: String) -> Any
  description "Retrieve one organization member by user id, returning their name, email address, organization role and join date. Returns personal data about a real individual. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/error/message`, its error code at `/error/type` in the response body."
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://api.anthropic.com"
  url = fmt("{base}/v1/organizations/users/{user_id}")
  anthropic_version = "2023-06-01"
  response = http.request(headers: { "anthropic-version": anthropic_version }, method: "GET", url)
  return response
