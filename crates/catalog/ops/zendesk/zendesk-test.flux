op zendesk-test -> Any
  description "Show Self"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://{subdomain}.zendesk.com"
  url = fmt("{base}/api/v2/users/me")
  response = http.request(method: "GET", url)
  return response
