op zendesk-ticket-search(query: String, page: Number, per_page: Number) -> Any
  description "Search Zendesk tickets with Zendesk search syntax."
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://example.zendesk.com"
  url = fmt("{base}/api/v2/search.json")
  response = http.request(method: "GET", query: { page, per_page, query }, url)
  return response
