op zendesk-view-ticket-list(view_id: String) -> Any
  description "List tickets from one numeric or built-in Zendesk view"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://{subdomain}.zendesk.com"
  url = fmt("{base}/api/v2/views/{view_id}/tickets")
  response = http.request(method: "GET", url)
  return response
