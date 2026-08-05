op anthropic-organization-members-list -> Any
  description "List the people in this organization, with each member's name, email address, organization role and the date they joined. Returns personal data about real individuals. Unpaginated — this connector cannot request a further page, so on an organization larger than one page this is a sample and not a roster. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/error/message`, its error code at `/error/type` in the response body."
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://api.anthropic.com"
  url = fmt("{base}/v1/organizations/users")
  anthropic_version = "2023-06-01"
  response = http.request(headers: { "anthropic-version": anthropic_version }, method: "GET", url)
  return response
