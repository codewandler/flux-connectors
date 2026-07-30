op zendesk-test -> Any
  description "Verify credentials by fetching the authenticated Zendesk user"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://{subdomain}.zendesk.com"
  url = fmt("{base}/api/v2/users/me.json")
  response = http.request(method: "GET", url)
  return response
