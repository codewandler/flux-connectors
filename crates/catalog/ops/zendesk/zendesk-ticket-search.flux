op zendesk-ticket-search(query: String) -> Any
  description "List Search Results"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://{subdomain}.zendesk.com"
  url = fmt("{base}/api/v2/search")
  response = http.request(method: "GET", query: { query }, url)
  return response
