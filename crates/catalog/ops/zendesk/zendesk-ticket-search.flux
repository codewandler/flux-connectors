op zendesk-ticket-search(query: String) -> Any
  description "List Search Results"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://{subdomain}.zendesk.com"
  url = fmt("{base}/api/v2/search?query={query}")
  response = http.request(method: "GET", url)
  return response
