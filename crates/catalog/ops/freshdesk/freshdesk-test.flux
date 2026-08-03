op freshdesk-test(per_page: Number) -> Any
  description "Verify credentials with a bounded contact read"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://{domain}/api/v2"
  url = fmt("{base}/contacts")
  response = http.request(method: "GET", query: { per_page }, url)
  return response
